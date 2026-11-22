use std::{io, process::Command};
use url::Url;

pub struct BuildSource {
    pub repository: String,
    pub release: String,
    pub profile: Option<String>,
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn dated(id: &str) -> bool {
    let Some(stamp) = id.strip_prefix("tizen-10.1-unified_") else {
        return false;
    };
    let Some((day, time)) = stamp.split_once('.') else {
        return false;
    };
    day.len() == 8 && time.len() == 6 && day.bytes().chain(time.bytes()).all(|b| b.is_ascii_digit())
}

impl BuildSource {
    pub fn resolve(input: &str) -> io::Result<Self> {
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
                    return Err(invalid("Expected an emulator image profile directory."));
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
        let output = Command::new(curl)
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
    let upper = name.to_ascii_uppercase();
    if name.is_empty()
        || name.len() > 64
        || !name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        || matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (upper.len() == 4
            && (upper.starts_with("COM") || upper.starts_with("LPT"))
            && upper.as_bytes()[3].is_ascii_digit())
    {
        return Err(invalid(
            "Use 1–64 letters, digits, hyphens or underscores for the image name; Windows device names are reserved.",
        ));
    }
    Ok(())
}
