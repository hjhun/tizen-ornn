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
        if let Some(dir) = env::var_os("TIZEN_ORNN_PORTABLE_DIR") {
            return Ok(dir.into());
        }
        // A terminal can invoke the CLI directly from an unpacked portable runtime.
        let exe = env::current_exe()?;
        if let Some(runtime) = exe.parent()
            && runtime
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("runtime-"))
            && let Some(portable) = runtime.parent()
            && portable.ends_with("Tizen Ornn Portable")
        {
            return Ok(portable.to_owned());
        }
        default_dir()
    }

    pub fn setting() -> io::Result<Option<PathBuf>> {
        read_setting("workdir.json")
    }

    fn read_setting(name: &str) -> io::Result<Option<PathBuf>> {
        let path = settings_dir()?.join(name);
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
        save_setting("workdir.json", value.trim())
    }

    pub fn shared_directory() -> io::Result<Option<PathBuf>> {
        read_setting("share.json")
    }

    pub fn default_share() -> io::Result<PathBuf> {
        let home = env::var_os("TIZEN_ORNN_HOME");
        #[cfg(target_os = "linux")]
        let home = home.or_else(|| env::var_os("TIZEN_VM_HOME"));
        Ok(match home {
            Some(dir) => std::path::absolute(dir)?,
            None => current()?,
        }
        .join("share"))
    }

    pub fn save_share(value: &str) -> io::Result<()> {
        let path = std::path::Path::new(value);
        if !value.is_empty() && (!path.is_absolute() || !path.is_dir()) {
            return Err(io::Error::other(
                "Enter the absolute path of an existing directory",
            ));
        }
        save_setting("share.json", value)
    }

    fn save_setting(name: &str, value: &str) -> io::Result<()> {
        let base = settings_dir()?;
        let file = base.join(name);
        if value.is_empty() {
            return match fs::remove_file(file) {
                Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
                result => result,
            };
        }
        let path = PathBuf::from(value);
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
        if name == "workdir.json" {
            fs::create_dir_all(&destination)?;
            tempfile::NamedTempFile::new_in(destination)?;
        }
        fs::create_dir_all(&base)?;
        let mut temp = tempfile::NamedTempFile::new_in(base)?;
        temp.write_all(&serde_json::to_vec(&path)?)?;
        temp.persist(file)?;
        Ok(())
    }
}
