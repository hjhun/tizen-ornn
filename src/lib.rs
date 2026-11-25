use serde::{Deserialize, Serialize};
use std::{env, io, path::PathBuf};

pub mod proxy;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Deserialize, Serialize)]
pub struct Image {
    pub arch: String,
    pub path: PathBuf,
}

pub fn arm_arch(path: &std::path::Path) -> Option<&'static str> {
    if !["rootfs.img", "system-data.img", "user.img", "hal.img"]
        .iter()
        .all(|part| path.join("disk").join(part).is_file())
    {
        return None;
    }
    [("armv7l", "zImage"), ("aarch64", "Image")]
        .into_iter()
        .find(|(_, kernel)| path.join(kernel).is_file())
        .map(|(arch, _)| arch)
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

    fn read_setting<T: serde::de::DeserializeOwned>(name: &str) -> io::Result<Option<T>> {
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
        let default = default_dir()?;
        #[cfg(target_os = "linux")]
        if !has_images(&default) {
            let data = env::var_os("XDG_DATA_HOME")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| default.parent().unwrap().join("share"));
            let legacy = data.join("tizen-ornn");
            if has_images(&legacy) {
                return Ok(legacy);
            }
        }
        Ok(default)
    }

    #[cfg(target_os = "linux")]
    fn has_images(store: &std::path::Path) -> bool {
        let is_x86_image = |dir: PathBuf| {
            dir.join("bzImage").is_file()
                && ["tizen-rootfs.ext4", "tizen-rootfs.qcow2"]
                    .iter()
                    .any(|disk| dir.join(disk).is_file())
        };
        if ["current", ".tizen-current", "dev", ""]
            .iter()
            .any(|entry| is_x86_image(store.join(entry)))
        {
            return true;
        }
        fs::read_dir(store.join("images")).is_ok_and(|mut images| {
            images.any(|item| {
                item.is_ok_and(|item| {
                    !item.file_name().to_string_lossy().starts_with('.')
                        && is_x86_image(item.path())
                })
            })
        })
    }

    pub fn save(value: &str) -> io::Result<()> {
        save_setting("workdir.json", value.trim())
    }

    pub fn ca_setting() -> io::Result<Option<PathBuf>> {
        read_setting("build-ca.json")
    }

    pub fn save_ca(value: &str) -> io::Result<()> {
        let path = std::path::Path::new(value);
        if !value.is_empty() && (!path.is_absolute() || !path.is_file()) {
            return Err(io::Error::other("Choose an existing certificate file"));
        }
        save_setting("build-ca.json", value)
    }

    #[derive(Clone, Copy, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
    #[serde(rename_all = "lowercase")]
    pub enum Language {
        #[default]
        En,
        Ko,
    }

    impl Language {
        pub fn text<'a>(self, en: &'a str, ko: &'a str) -> &'a str {
            match self {
                Self::En => en,
                Self::Ko => ko,
            }
        }
    }

    pub fn language() -> io::Result<Language> {
        Ok(read_setting("language.json")?.unwrap_or_default())
    }

    pub fn save_language(language: Language) -> io::Result<()> {
        write_json("language.json", &language)
    }

    fn write_json(name: &str, value: &impl serde::Serialize) -> io::Result<()> {
        let base = settings_dir()?;
        fs::create_dir_all(&base)?;
        let mut temp = tempfile::NamedTempFile::new_in(&base)?;
        serde_json::to_writer(&mut temp, value)?;
        temp.persist(base.join(name))?;
        Ok(())
    }

    pub fn proxy_setting() -> io::Result<Option<crate::proxy::Manual>> {
        read_setting("build-proxy.json")
    }

    pub fn save_proxy(proxy: Option<&crate::proxy::Manual>) -> io::Result<()> {
        let base = settings_dir()?;
        let path = base.join("build-proxy.json");
        if let Some(proxy) = proxy {
            proxy.validate()?;
            write_json("build-proxy.json", proxy)?;
        } else if let Err(e) = fs::remove_file(path)
            && e.kind() != io::ErrorKind::NotFound
        {
            return Err(e);
        }
        Ok(())
    }

    pub fn shared_directory() -> io::Result<Option<PathBuf>> {
        read_setting("share.json")
    }

    pub fn default_share() -> io::Result<PathBuf> {
        let home = env::var_os("TIZEN_ORNN_HOME");
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
