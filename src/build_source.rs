use std::{io, process::Command};
use url::Url;

#[derive(serde::Deserialize)]
pub struct BuildSource {
    pub repository: String,
    #[serde(rename = "snapshot")]
    pub release: String,
    pub profile: Option<String>,
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

pub(crate) fn dated(id: &str) -> bool {
    let Some(stamp) = id.strip_prefix("tizen-10.1-unified_") else {
        return false;
    };
    let Some((day, time)) = stamp.split_once('.') else {
        return false;
    };
    day.len() == 8 && time.len() == 6 && day.bytes().chain(time.bytes()).all(|b| b.is_ascii_digit())
}

impl BuildSource {
    pub fn resolve(input: &str, proxy: &tizen_ornn::proxy::BuildProxy) -> io::Result<Self> {
        let url =
            Url::parse(input).map_err(|_| invalid("Enter a full HTTPS Tizen snapshot URL."))?;
        if url.scheme() != "https"
            || url.host().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || input.chars().any(|c| c.is_whitespace() || c.is_control())
        {
            return Err(invalid(
                "Use an HTTPS snapshot URL without credentials, a query, or a fragment.",
            ));
        }
        let path = url.path().trim_end_matches('/');
        let mut root = url.clone();
        let mut release = "latest";
        let mut profile = None;
        if let Some((base, tail)) = path.split_once("/Tizen-10.1-Unified/") {
            root.set_path(&format!("{base}/"));
            let (id, suffix) = tail.split_once('/').unwrap_or((tail, ""));
            if !(dated(id) || id == "latest") {
                return Err(invalid("Expected a dated Unified snapshot or latest."));
            }
            if !suffix.is_empty() && suffix != "build.xml" {
                let mut parts = suffix.split('/');
                let (Some("images"), Some("emulator"), Some(name)) =
                    (parts.next(), parts.next(), parts.next())
                else {
                    return Err(invalid(
                        "For x86_64, choose an emulator image profile. For a pinned ARM headed image, select armv7l or aarch64 in Build image.",
                    ));
                };
                if name.is_empty()
                    || name.len() > 100
                    || !name
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
                {
                    return Err(invalid("Invalid emulator profile name."));
                }
                if let Some(file) = parts.next() {
                    let stem = format!("{id}_{name}.");
                    if !matches!(
                        file.strip_prefix(&stem),
                        Some("ks" | "packages" | "xml" | "tar.gz")
                    ) || parts.next().is_some()
                    {
                        return Err(invalid(
                            "Expected the selected profile's image or recipe URL.",
                        ));
                    }
                }
                profile = Some(name.to_owned());
            }
            release = id;
        } else {
            root.set_path(&format!(
                "{}/",
                path.strip_suffix("/Tizen-10.1-Unified").unwrap_or(path)
            ));
        }
        let metadata = root
            .join(&format!("Tizen-10.1-Unified/{release}/build.xml"))
            .map_err(io::Error::other)?;
        let curl = if cfg!(windows) { "curl.exe" } else { "curl" };
        let mut command = Command::new(curl);
        proxy.apply(&mut command);
        let output = command
            .args([
                "--disable",
                "-fsSL",
                "--proto",
                "=https",
                "--proto-redir",
                "=https",
                "--max-time",
                "60",
                "--max-filesize",
                "4194304",
                "--",
                metadata.as_str(),
            ])
            .output()?;
        if !output.status.success() {
            return Err(io::Error::other(format!(
                "Could not read {metadata}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        let xml = std::str::from_utf8(&output.stdout).map_err(io::Error::other)?;
        let doc = roxmltree::Document::parse(xml).map_err(io::Error::other)?;
        let id = doc
            .root_element()
            .children()
            .find(|n| n.has_tag_name("id"))
            .and_then(|n| n.text())
            .unwrap_or("")
            .trim();
        if !dated(id) || (release != "latest" && id != release) {
            return Err(invalid(
                "build.xml does not identify the requested Tizen 10.1 Unified snapshot.",
            ));
        }
        Ok(Self {
            repository: root.to_string(),
            release: id.to_owned(),
            profile,
        })
    }

    pub fn config(&self) -> String {
        let mut config = format!(
            "repository = {}\nsnapshot = {}\n",
            serde_json::json!(self.repository),
            serde_json::json!(self.release)
        );
        if let Some(profile) = &self.profile {
            config.push_str(&format!("profile = {}\n", serde_json::json!(profile)));
        }
        config
    }
}

pub fn validate_name(name: &str) -> io::Result<()> {
    // Windows reserves device names even when followed by an extension.
    let upper = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    if name.is_empty()
        || name.len() > 64
        || name.starts_with('.')
        || name.ends_with('.')
        || !name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.'))
        || matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (upper.len() == 4
            && (upper.starts_with("COM") || upper.starts_with("LPT"))
            && upper.as_bytes()[3].is_ascii_digit())
    {
        return Err(invalid(
            "Use 1–64 letters, digits, hyphens, underscores or interior dots. Windows device names are reserved.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod name_tests {
    use super::validate_name;

    #[test]
    fn accepts_snapshot_names_and_rejects_reserved_paths() {
        for name in ["tizen-10.1-unified_20261119.053314", "tizen-10-1-dev"] {
            assert!(validate_name(name).is_ok(), "{name}");
        }
        for name in [
            ".",
            "..",
            ".hidden",
            "trailing.",
            "con.img",
            "LPT1.dev",
            "../other",
        ] {
            assert!(validate_name(name).is_err(), "{name}");
        }
    }
}
