//! The filesystem-backed asset io: [`FileAssetReader`] and [`FileAssetWriter`].
//!
//! Paths are resolved against a root that defaults to the process working
//! directory, so today's cwd-relative asset literals (`res/models/Suzanne.mesh`
//! and friends) keep resolving to the same files (ADR 0004). Reads and writes go
//! through [`async_fs`] so they run on whatever executor drives them, without a
//! tokio runtime (ADR 0001).
//!
//! With the `file_watcher` feature the module also carries [`FileWatcher`], the
//! filesystem hot-reload backend (see [`file_watcher`]).

#[cfg(feature = "file_watcher")]
pub(crate) mod file_watcher;

#[cfg(feature = "file_watcher")]
pub use file_watcher::FileWatcher;

use std::path::{Path, PathBuf};

use futures_lite::StreamExt;
use tracing::error;

use crate::io::{
    AssetReader, AssetReaderError, AssetWriter, AssetWriterError, PathStream, Reader, Writer,
    get_meta_path,
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

/// Writes assets to a directory on disk.
///
/// The write-side counterpart of [`FileAssetReader`]: paths are resolved against
/// the same root under [`get_base_path`], parents are created on demand, and
/// `.meta` sidecars are addressed through [`get_meta_path`].
///
/// Unlike the reader, the root can be created eagerly: [`AssetSourceBuilder`]s
/// build the processed writer with `create_root = true`, so the output directory
/// exists before the processor attempts its first write.
///
/// [`AssetSourceBuilder`]: crate::io::AssetSourceBuilder
pub struct FileAssetWriter {
    root_path: PathBuf,
}

impl FileAssetWriter {
    /// Creates a writer rooted at `path` under [`get_base_path`]. An empty `path`
    /// means the base path itself. When `create_root` is true the root directory
    /// is created (with its parents) if it does not already exist; a failure to
    /// create it is logged and left for the first write to surface.
    pub fn new<P: AsRef<Path>>(path: P, create_root: bool) -> Self {
        let path = path.as_ref();
        let root_path = if path.as_os_str().is_empty() {
            get_base_path()
        } else {
            get_base_path().join(path)
        };
        if create_root
            && let Err(error) = std::fs::create_dir_all(&root_path)
        {
            error!(
                "failed to create the asset writer root {}: {error}",
                root_path.display()
            );
        }
        Self { root_path }
    }

    /// The resolved root this writer writes to.
    pub fn root_path(&self) -> &Path {
        &self.root_path
    }
}

impl AssetWriter for FileAssetWriter {
    async fn write<'a>(&'a self, path: &'a Path) -> Result<Box<Writer>, AssetWriterError> {
        let full_path = self.root_path.join(path);
        create_parent_dirs(&full_path).await?;
        let file = async_fs::File::create(&full_path).await?;
        Ok(Box::new(file))
    }

    async fn write_meta<'a>(&'a self, path: &'a Path) -> Result<Box<Writer>, AssetWriterError> {
        let full_path = self.root_path.join(get_meta_path(path));
        create_parent_dirs(&full_path).await?;
        let file = async_fs::File::create(&full_path).await?;
        Ok(Box::new(file))
    }

    async fn remove<'a>(&'a self, path: &'a Path) -> Result<(), AssetWriterError> {
        async_fs::remove_file(self.root_path.join(path)).await?;
        Ok(())
    }

    async fn remove_meta<'a>(&'a self, path: &'a Path) -> Result<(), AssetWriterError> {
        async_fs::remove_file(self.root_path.join(get_meta_path(path))).await?;
        Ok(())
    }

    async fn rename<'a>(
        &'a self,
        old_path: &'a Path,
        new_path: &'a Path,
    ) -> Result<(), AssetWriterError> {
        let full_old_path = self.root_path.join(old_path);
        let full_new_path = self.root_path.join(new_path);
        create_parent_dirs(&full_new_path).await?;
        async_fs::rename(full_old_path, full_new_path).await?;
        Ok(())
    }

    async fn rename_meta<'a>(
        &'a self,
        old_path: &'a Path,
        new_path: &'a Path,
    ) -> Result<(), AssetWriterError> {
        let full_old_path = self.root_path.join(get_meta_path(old_path));
        let full_new_path = self.root_path.join(get_meta_path(new_path));
        create_parent_dirs(&full_new_path).await?;
        async_fs::rename(full_old_path, full_new_path).await?;
        Ok(())
    }

    async fn create_directory<'a>(&'a self, path: &'a Path) -> Result<(), AssetWriterError> {
        async_fs::create_dir_all(self.root_path.join(path)).await?;
        Ok(())
    }

    async fn remove_directory<'a>(&'a self, path: &'a Path) -> Result<(), AssetWriterError> {
        async_fs::remove_dir_all(self.root_path.join(path)).await?;
        Ok(())
    }

    async fn remove_empty_directory<'a>(&'a self, path: &'a Path) -> Result<(), AssetWriterError> {
        async_fs::remove_dir(self.root_path.join(path)).await?;
        Ok(())
    }

    async fn remove_assets_in_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<(), AssetWriterError> {
        let full_path = self.root_path.join(path);
        async_fs::remove_dir_all(&full_path).await?;
        async_fs::create_dir_all(&full_path).await?;
        Ok(())
    }

    async fn write_bytes<'a>(
        &'a self,
        path: &'a Path,
        bytes: &'a [u8],
    ) -> Result<(), AssetWriterError> {
        let full_path = self.root_path.join(path);
        create_parent_dirs(&full_path).await?;
        async_fs::write(full_path, bytes).await?;
        Ok(())
    }

    async fn write_meta_bytes<'a>(
        &'a self,
        path: &'a Path,
        bytes: &'a [u8],
    ) -> Result<(), AssetWriterError> {
        let full_path = self.root_path.join(get_meta_path(path));
        create_parent_dirs(&full_path).await?;
        async_fs::write(full_path, bytes).await?;
        Ok(())
    }
}

/// Creates the parent directories of `full_path`, if it has any.
async fn create_parent_dirs(full_path: &Path) -> Result<(), AssetWriterError> {
    if let Some(parent) = full_path.parent() {
        async_fs::create_dir_all(parent).await?;
    }
    Ok(())
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
