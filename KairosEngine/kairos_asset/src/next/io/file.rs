//! The filesystem-backed [`AssetReader`].
//!
//! Paths are resolved against a root that defaults to the process working
//! directory, so today's cwd-relative asset literals (`res/models/Suzanne.mesh`
//! and friends) keep resolving to the same files (ADR 0004). reads go through
//! [`async_fs`] so they run on whatever executor drives them, without a tokio
//! runtime (ADR 0001).

use std::path::{Path, PathBuf};

use futures_lite::StreamExt;

use crate::next::io::{
    AssetReader, AssetReaderError, PathStream, Reader, get_meta_path,
};

/// The root the default file source resolves against: the process working
/// directory.
///
/// This deliberately diverges from bevy's executable-relative `assets` folder;
/// see ADR 0004.
pub fn get_base_path() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Reads assets from a directory on disk.
pub struct FileAssetReader {
    root_path: PathBuf,
}

impl FileAssetReader {
    /// Creates a reader rooted at `path` under [`get_base_path`]. An empty
    /// `path` means the base path itself.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        let path = path.as_ref();
        let root_path = if path.as_os_str().is_empty() {
            get_base_path()
        } else {
            get_base_path().join(path)
        };
        Self { root_path }
    }

    /// The resolved root this reader reads from.
    pub fn root_path(&self) -> &Path {
        &self.root_path
    }
}

impl Reader for async_fs::File {}

impl AssetReader for FileAssetReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        let full_path = self.root_path.join(path);
        let file = async_fs::File::open(&full_path)
            .await
            .map_err(|error| open_error(error, full_path))?;
        Ok(file)
    }

    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        let full_path = self.root_path.join(get_meta_path(path));
        let file = async_fs::File::open(&full_path)
            .await
            .map_err(|error| open_error(error, full_path))?;
        Ok(file)
    }

    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        let full_path = self.root_path.join(path);
        let mut read_dir = async_fs::read_dir(&full_path)
            .await
            .map_err(|error| open_error(error, full_path))?;

        let mut paths = Vec::new();
        while let Some(entry) = read_dir.next().await {
            let entry = entry?;
            let path = entry.path();

            // Meta sidecars and hidden files are addressable but are not
            // listed as assets.
            let is_meta = path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("meta"));
            let is_hidden = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with('.'));
            if is_meta || is_hidden {
                continue;
            }

            // Entries are source-relative: consumers address them the same way
            // they address an `AssetPath`.
            let Ok(relative) = path.strip_prefix(&self.root_path) else {
                continue;
            };
            paths.push(relative.to_path_buf());
        }

        Ok(Box::new(futures_lite::stream::iter(paths)))
    }

    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
        let full_path = self.root_path.join(path);
        match async_fs::metadata(&full_path).await {
            Ok(metadata) => Ok(metadata.is_dir()),
            Err(_) => Err(AssetReaderError::NotFound(path.to_path_buf())),
        }
    }
}

/// Maps a failed open onto [`AssetReaderError`], preserving the resolved path
/// for [`AssetReaderError::NotFound`].
fn open_error(error: std::io::Error, full_path: PathBuf) -> AssetReaderError {
    if error.kind() == std::io::ErrorKind::NotFound {
        AssetReaderError::NotFound(full_path)
    } else {
        error.into()
    }
}
