//! Proxy settings for host downloads and disposable image builders.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, env, io, process::Command};
use url::Url;

const KEYS: [&str; 4] = ["http_proxy", "https_proxy", "all_proxy", "no_proxy"];

#[derive(Default, Debug, Deserialize, Serialize)]
pub struct Manual {
    pub url: String,
    pub bypass: String,
}

impl Manual {
    pub fn validate(&self) -> io::Result<()> {
        if self.url.trim().is_empty() || self.url.chars().any(char::is_whitespace) {
            return Err(io::Error::other("Enter an HTTP or HTTPS proxy address"));
        }
        let uri = parse_url(&self.url)?;
        if !matches!(uri.scheme(), "http" | "https")
            || !uri.username().is_empty()
            || uri.password().is_some()
            || uri.path() != "/"
            || uri.query().is_some()
            || uri.fragment().is_some()
            || self.bypass.chars().any(char::is_control)
        {
            return Err(io::Error::other(
                "Use an HTTP(S) proxy address without credentials or a path",
            ));
        }
        Ok(())
    }
}

fn parse_url(value: &str) -> io::Result<Url> {
    let value = if value.contains("://") {
        value.to_owned()
    } else {
        format!("http://{value}")
    };
    Url::parse(&value)
        .ok()
        .filter(Url::has_host)
        .ok_or_else(|| io::Error::other("Invalid proxy address"))
}

#[derive(Default, Debug)]
pub struct BuildProxy(BTreeMap<String, String>);

impl BuildProxy {
    pub fn load() -> io::Result<Self> {
        if let Some(manual) = crate::workdir::proxy_setting()? {
            manual.validate()?;
            let url = parse_url(&manual.url)?.to_string();
            return Ok(Self(BTreeMap::from([
                ("http_proxy".into(), url.clone()),
                ("https_proxy".into(), url),
                ("no_proxy".into(), manual.bypass),
            ])));
        }
        let mut values = BTreeMap::new();
        for key in KEYS {
            if let Some(value) = env::var_os(key).or_else(|| env::var_os(key.to_uppercase())) {
                values.insert(
                    key.to_owned(),
                    value
                        .into_string()
                        .map_err(|_| io::Error::other("Invalid proxy environment encoding"))?,
                );
            }
        }
        if KEYS[..3].iter().all(|key| !values.contains_key(*key)) {
            let mut defaults = system_proxy()?;
            defaults.extend(values);
            values = defaults;
        }
        Ok(Self(values))
    }

    pub fn apply(&self, cmd: &mut Command) {
        for key in KEYS {
            cmd.env_remove(key).env_remove(key.to_uppercase());
            if let Some(value) = self.0.get(key) {
                cmd.env(key, value).env(key.to_uppercase(), value);
            }
        }
    }

    #[cfg(any(windows, test))]
    pub fn builder_values(&self) -> io::Result<BTreeMap<String, String>> {
        let mut values = self.0.clone();
        for key in &KEYS[..3] {
            if let Some(value) = values.get_mut(*key)
                && !value.is_empty()
            {
                let mut uri = parse_url(value)?;
                if let Some(host) = uri.host()
                    && match host {
                        url::Host::Domain(name) => {
                            name.trim_end_matches('.').eq_ignore_ascii_case("localhost")
                        }
                        url::Host::Ipv4(ip) => ip.is_loopback(),
                        url::Host::Ipv6(ip) => ip.is_loopback(),
                    }
                {
                    uri.set_host(Some("10.0.2.2")).map_err(io::Error::other)?;
                    *value = uri.to_string();
                }
            }
        }
        Ok(values)
    }
}

fn system_proxy() -> io::Result<BTreeMap<String, String>> {
    #[cfg(target_os = "linux")]
    {
        let output = Command::new("/usr/bin/python3")
            .args(["-c", include_str!("../scripts/desktop_proxy.py")])
            .output()?;
        if !output.status.success() {
            return Err(io::Error::other(
                "Could not read the desktop proxy configuration",
            ));
        }
        let settings: BTreeMap<String, String> = serde_json::from_slice(&output.stdout)?;
        if !settings.is_empty() || env::var_os("WSL_DISTRO_NAME").is_none() {
            return Ok(settings);
        }
    }
    let mut cmd = Command::new("powershell.exe");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let output = match cmd
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            include_str!("windows/system-proxy.ps1"),
        ])
        .output()
    {
        Ok(output) => output,
        #[cfg(target_os = "linux")]
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(e) => return Err(e),
    };
    if !output.status.success() {
        return Err(io::Error::other(
            "Could not read the Windows system proxy configuration",
        ));
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_routes_loopback_to_host_and_preserves_bypass() {
        let proxy = BuildProxy(BTreeMap::from([
            ("http_proxy".into(), "http://localhost:3128".into()),
            ("https_proxy".into(), "http://[::1]:8080".into()),
            ("all_proxy".into(), "socks5h://office.example:1080".into()),
            ("no_proxy".into(), ".internal.example,localhost".into()),
        ]));
        let builder = proxy.builder_values().unwrap();
        assert_eq!(builder["http_proxy"], "http://10.0.2.2:3128/");
        assert_eq!(builder["https_proxy"], "http://10.0.2.2:8080/");
        assert_eq!(builder["all_proxy"], proxy.0["all_proxy"]);
        assert_eq!(builder["no_proxy"], proxy.0["no_proxy"]);
        assert_eq!(proxy.0["http_proxy"], "http://localhost:3128");
    }

    #[test]
    fn manual_settings_accept_static_endpoints_and_reject_credentials() {
        for (url, valid) in [
            ("proxy.example:8080", true),
            ("https://proxy.example:443", true),
            ("http://user:password@proxy.example:8080", false),
            ("ftp://proxy.example", false),
            ("http://proxy.example/path", false),
        ] {
            assert_eq!(
                Manual {
                    url: url.into(),
                    bypass: String::new()
                }
                .validate()
                .is_ok(),
                valid
            );
        }
    }
}
