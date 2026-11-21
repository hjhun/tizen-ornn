use serde::{Deserialize, Serialize};
use std::{env, io, path::PathBuf};

pub const VERSION: &str = include_str!("../config/version").trim_ascii_end();

#[derive(Debug, Deserialize, Serialize)]
pub struct Image {
    pub arch: String,
    pub path: PathBuf,
}

pub fn sibling(name: &str) -> io::Result<PathBuf> {
    Ok(env::current_exe()?.with_file_name(format!("{name}{}", env::consts::EXE_SUFFIX)))
}

#[cfg(feature = "desktop")]
pub mod workdir {
    use std::{env, fs, io, io::Write, path::PathBuf};

    pub fn default_dir() -> io::Result<PathBuf> {
        #[cfg(windows)]
        let path = env::var_os("LOCALAPPDATA").map(|dir| PathBuf::from(dir).join("Tizen Ornn"));
        #[cfg(target_os = "linux")]
        let path = env::var_os("HOME").map(|dir| PathBuf::from(dir).join(".local/tizen-ornn"));
        path.ok_or_else(|| io::Error::other("Cannot locate the user data directory"))
    }

    fn settings_dir() -> io::Result<PathBuf> {
        env::var_os("TIZEN_ORNN_PORTABLE_DIR")
            .map(PathBuf::from)
            .map_or_else(default_dir, Ok)
    }

    pub fn setting() -> io::Result<Option<PathBuf>> {
        let path = settings_dir()?.join("workdir.json");
        match fs::read(path) {
            Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn current() -> io::Result<PathBuf> {
        if let Some(path) = setting()? {
            return Ok(settings_dir()?.join(path));
        }
        default_dir()
    }

    pub fn save(value: &str) -> io::Result<()> {
        let base = settings_dir()?;
        let file = base.join("workdir.json");
        if value.trim().is_empty() {
            return match fs::remove_file(file) {
                Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
                result => result,
            };
        }
        let path = PathBuf::from(value.trim());
        if !path.is_absolute()
            && (path.has_root()
                || path
                    .components()
                    .any(|part| matches!(part, std::path::Component::Prefix(_))))
        {
            return Err(io::Error::other(
                "Enter a full path or a relative folder name",
            ));
        }
        let destination = base.join(&path);
        fs::create_dir_all(&destination)?;
        tempfile::NamedTempFile::new_in(destination)?;
        fs::create_dir_all(&base)?;
        let mut temp = tempfile::NamedTempFile::new_in(base)?;
        temp.write_all(&serde_json::to_vec(&path)?)?;
        temp.persist(file)?;
        Ok(())
    }
}
