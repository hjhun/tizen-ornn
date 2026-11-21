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
