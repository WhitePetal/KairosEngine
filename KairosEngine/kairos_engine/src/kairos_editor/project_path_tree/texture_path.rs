use std::{ffi::OsString, path::PathBuf};

pub struct TexturePath {
    pub path: PathBuf,
    pub source: PathBuf,
    pub file_name: OsString,
}
impl TexturePath {
    pub fn new(path: PathBuf, source: PathBuf, file_name: OsString) -> Self {
        Self {
            path,
            source,
            file_name,
        }
    }
}
