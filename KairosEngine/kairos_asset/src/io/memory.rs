//! An in-memory asset backend, ported from `bevy_asset` 0.19.1's `io/memory.rs`.
//!
//! [`Dir`] is a clone-able, thread-safe virtual filesystem: cloning one shares
//! the same underlying tree, so a [`MemoryAssetReader`] handed to an
//! [`AssetSource`](crate::io::AssetSource) sees every later mutation. The
//! [`embedded`](crate::io::embedded) source reads through it, and tests use it
//! to stand in for the filesystem.
//!
//! Locks are poisoned-tolerant: a panic while a lock is held must not make the
//! whole virtual filesystem unusable, so poisoning is recovered from rather than
//! propagated.

use core::{pin::Pin, task::Poll};
use std::{
    io::{Error, ErrorKind, SeekFrom},
    path::{Path, PathBuf},
    sync::{Arc, PoisonError, RwLock},
};

use futures_io::{AsyncRead, AsyncSeek, AsyncWrite};
use futures_lite::Stream;

use kairos_collections::FixedHashMap as HashMap;

use super::{
    AssetReader, AssetReaderError, AssetWriter, AssetWriterError, PathStream, Reader, Writer,
    slice_read, slice_seek,
};

/// The internal tree behind a [`Dir`].
#[derive(Default, Debug)]
struct DirInternal {
    assets: HashMap<Box<str>, Data>,
    metadata: HashMap<Box<str>, Data>,
    dirs: HashMap<Box<str>, Dir>,
    path: PathBuf,
}

/// A clone-able (internally `Arc`-ed) and thread-safe "in-memory" filesystem.
///
/// Built for [`MemoryAssetReader`] and primarily intended for the embedded
/// source and for unit tests. Cloning a `Dir` shares the same tree.
#[derive(Default, Clone, Debug)]
pub struct Dir(Arc<RwLock<DirInternal>>);

impl Dir {
    /// Creates a new [`Dir`] for the given `path`.
    pub fn new(path: PathBuf) -> Self {
        Self(Arc::new(RwLock::new(DirInternal {
            path,
            ..Default::default()
        })))
    }

    /// Stores `asset` (which may be a `Vec<u8>` or a `&'static [u8]`) at
    /// `path`.
    pub fn insert_asset(&self, path: &Path, value: impl Into<Value>) {
        self.insert_asset_internal(path, value.into());
    }

    // Implements `insert_asset`, but with a non-generic `value` parameter. This
    // stops the function from being duplicated many times by monomorphization.
    fn insert_asset_internal(&self, path: &Path, value: Value) {
        let mut dir = self.clone();
        if let Some(parent) = path.parent() {
            dir = self.get_or_insert_dir(parent);
        }
        dir.0
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .assets
            .insert(
                path.file_name().unwrap().to_string_lossy().into(),
                Data {
                    value,
                    path: path.to_owned(),
                },
            );
    }

    /// Removes the stored asset at `path`.
    ///
    /// Returns the [`Data`] stored if found, [`None`] otherwise.
    pub fn remove_asset(&self, path: &Path) -> Option<Data> {
        let mut dir = self.clone();
        if let Some(parent) = path.parent() {
            dir = self.get_or_insert_dir(parent);
        }
        let key: Box<str> = path.file_name().unwrap().to_string_lossy().into();
        dir.0
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .assets
            .remove(&key)
    }

    /// Stores `value` as the meta sidecar for `path`.
    pub fn insert_meta(&self, path: &Path, value: impl Into<Value>) {
        self.insert_meta_internal(path, value.into());
    }

    // Implements `insert_meta` — see `insert_asset_internal` for rationale.
    fn insert_meta_internal(&self, path: &Path, value: Value) {
        let mut dir = self.clone();
        if let Some(parent) = path.parent() {
            dir = self.get_or_insert_dir(parent);
        }
        dir.0
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .metadata
            .insert(
                path.file_name().unwrap().to_string_lossy().into(),
                Data {
                    value,
                    path: path.to_owned(),
                },
            );
    }

    /// Removes the stored metadata at `path`.
    ///
    /// Returns the [`Data`] stored if found, [`None`] otherwise.
    pub fn remove_metadata(&self, path: &Path) -> Option<Data> {
        let mut dir = self.clone();
        if let Some(parent) = path.parent() {
            dir = self.get_or_insert_dir(parent);
        }
        let key: Box<str> = path.file_name().unwrap().to_string_lossy().into();
        dir.0
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .metadata
            .remove(&key)
    }

    /// Returns the [`Dir`] at `path`, creating it (and its parents) if absent.
    pub fn get_or_insert_dir(&self, path: &Path) -> Dir {
        let mut dir = self.clone();
        let mut full_path = PathBuf::new();
        for c in path.components() {
            full_path.push(c);
            let name: Box<str> = c.as_os_str().to_string_lossy().into();
            dir = {
                let dirs = &mut dir.0.write().unwrap_or_else(PoisonError::into_inner).dirs;
                dirs.entry(name)
                    .or_insert_with(|| Dir::new(full_path.clone()))
                    .clone()
            };
        }

        dir
    }

    /// Removes the dir at `path`.
    ///
    /// Returns the [`Dir`] stored if found, [`None`] otherwise.
    pub fn remove_dir(&self, path: &Path) -> Option<Dir> {
        let mut dir = self.clone();
        if let Some(parent) = path.parent() {
            dir = self.get_or_insert_dir(parent);
        }
        let key: Box<str> = path.file_name().unwrap().to_string_lossy().into();
        dir.0
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .dirs
            .remove(&key)
    }

    /// Returns the [`Dir`] at `path`, if it exists.
    pub fn get_dir(&self, path: &Path) -> Option<Dir> {
        let mut dir = self.clone();
        for p in path.components() {
            let component = p.as_os_str().to_str().unwrap();
            let next_dir = dir
                .0
                .read()
                .unwrap_or_else(PoisonError::into_inner)
                .dirs
                .get(component)?
                .clone();
            dir = next_dir;
        }
        Some(dir)
    }

    /// Returns the asset stored at `path`, if any.
    pub fn get_asset(&self, path: &Path) -> Option<Data> {
        let mut dir = self.clone();
        if let Some(parent) = path.parent() {
            dir = dir.get_dir(parent)?;
        }

        path.file_name().and_then(|f| {
            dir.0
                .read()
                .unwrap_or_else(PoisonError::into_inner)
                .assets
                .get(f.to_str().unwrap())
                .cloned()
        })
    }

    /// Returns the meta sidecar stored at `path`, if any.
    pub fn get_metadata(&self, path: &Path) -> Option<Data> {
        let mut dir = self.clone();
        if let Some(parent) = path.parent() {
            dir = dir.get_dir(parent)?;
        }

        path.file_name().and_then(|f| {
            dir.0
                .read()
                .unwrap_or_else(PoisonError::into_inner)
                .metadata
                .get(f.to_str().unwrap())
                .cloned()
        })
    }

    /// The path this [`Dir`] was created for.
    pub fn path(&self) -> PathBuf {
        self.0
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .path
            .to_owned()
    }
}

/// A [`Stream`] over the entries of a [`Dir`]: its subdirectories first, then its
/// assets.
pub struct DirStream {
    dir: Dir,
    index: usize,
    dir_index: usize,
}

impl DirStream {
    fn new(dir: Dir) -> Self {
        Self {
            dir,
            index: 0,
            dir_index: 0,
        }
    }
}

impl Stream for DirStream {
    type Item = PathBuf;

    fn poll_next(
        self: Pin<&mut Self>,
        _cx: &mut core::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        let dir = this.dir.0.read().unwrap_or_else(PoisonError::into_inner);

        let dir_index = this.dir_index;
        if let Some(dir_path) = dir
            .dirs
            .keys()
            .nth(dir_index)
            .map(|d| dir.path.join(d.as_ref()))
        {
            this.dir_index += 1;
            Poll::Ready(Some(dir_path))
        } else {
            let index = this.index;
            this.index += 1;
            Poll::Ready(dir.assets.values().nth(index).map(|d| d.path().to_owned()))
        }
    }
}

/// In-memory [`AssetReader`] implementation.
///
/// Primarily intended for the embedded source and for unit tests.
#[derive(Default, Clone)]
pub struct MemoryAssetReader {
    /// The root of the virtual filesystem this reader reads from.
    pub root: Dir,
}

/// In-memory [`AssetWriter`] implementation.
///
/// Primarily intended for unit tests.
#[derive(Default, Clone)]
pub struct MemoryAssetWriter {
    /// The root of the virtual filesystem this writer writes to.
    pub root: Dir,
}

/// Asset data stored in a [`Dir`].
#[derive(Clone, Debug)]
pub struct Data {
    path: PathBuf,
    value: Value,
}

/// Stores either an allocated vec of bytes or a static array of bytes.
#[derive(Clone, Debug)]
pub enum Value {
    /// Bytes owned by the virtual filesystem.
    Vec(Arc<Vec<u8>>),
    /// Bytes embedded in the binary, shared for the process lifetime.
    Static(&'static [u8]),
}

impl Data {
    /// The path that this data was written to.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The value in bytes that was written here.
    pub fn value(&self) -> &[u8] {
        match &self.value {
            Value::Vec(vec) => vec,
            Value::Static(value) => value,
        }
    }
}

impl From<Vec<u8>> for Value {
    fn from(value: Vec<u8>) -> Self {
        Self::Vec(Arc::new(value))
    }
}

impl From<&'static [u8]> for Value {
    fn from(value: &'static [u8]) -> Self {
        Self::Static(value)
    }
}

impl<const N: usize> From<&'static [u8; N]> for Value {
    fn from(value: &'static [u8; N]) -> Self {
        Self::Static(value)
    }
}

/// A [`Reader`] over one [`Data`] value.
struct DataReader {
    data: Data,
    bytes_read: usize,
}

impl AsyncRead for DataReader {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut core::task::Context<'_>,
        buf: &mut [u8],
    ) -> Poll<futures_io::Result<usize>> {
        // Get the mut borrow to avoid trying to borrow the pin itself multiple
        // times.
        let this = self.get_mut();
        Poll::Ready(Ok(slice_read(this.data.value(), &mut this.bytes_read, buf)))
    }
}

impl AsyncSeek for DataReader {
    fn poll_seek(
        self: Pin<&mut Self>,
        _cx: &mut core::task::Context<'_>,
        pos: SeekFrom,
    ) -> Poll<std::io::Result<u64>> {
        // Get the mut borrow to avoid trying to borrow the pin itself multiple
        // times.
        let this = self.get_mut();
        Poll::Ready(slice_seek(this.data.value(), &mut this.bytes_read, pos))
    }
}

impl Reader for DataReader {}

impl AssetReader for MemoryAssetReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        self.root
            .get_asset(path)
            .map(|data| DataReader {
                data,
                bytes_read: 0,
            })
            .ok_or_else(|| AssetReaderError::NotFound(path.to_path_buf()))
    }

    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        self.root
            .get_metadata(path)
            .map(|data| DataReader {
                data,
                bytes_read: 0,
            })
            .ok_or_else(|| AssetReaderError::NotFound(path.to_path_buf()))
    }

    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        self.root
            .get_dir(path)
            .map(|dir| Box::new(DirStream::new(dir)) as Box<PathStream>)
            .ok_or_else(|| AssetReaderError::NotFound(path.to_path_buf()))
    }

    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
        Ok(self.root.get_dir(path).is_some())
    }
}

/// A writer that writes into a [`Dir`], buffering internally until flushed.
struct DataWriter {
    /// The dir to write to.
    dir: Dir,
    /// The path to write to.
    path: PathBuf,
    /// The current buffer of data.
    ///
    /// This includes data that has been flushed already.
    current_data: Vec<u8>,
    /// Whether to write to the data or to the meta.
    is_meta_writer: bool,
}

impl AsyncWrite for DataWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        _: &mut core::task::Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        self.get_mut().current_data.extend_from_slice(buf);
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        _: &mut core::task::Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        // Write the data to our fake disk. This means we will repeatedly
        // reinsert the asset.
        if self.is_meta_writer {
            self.dir.insert_meta(&self.path, self.current_data.clone());
        } else {
            self.dir.insert_asset(&self.path, self.current_data.clone());
        }
        Poll::Ready(Ok(()))
    }

    fn poll_close(
        self: Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        // A flush writes the data into the `Dir`, which is all close needs to do.
        self.poll_flush(cx)
    }
}

impl AssetWriter for MemoryAssetWriter {
    async fn write<'a>(&'a self, path: &'a Path) -> Result<Box<Writer>, AssetWriterError> {
        Ok(Box::new(DataWriter {
            dir: self.root.clone(),
            path: path.to_owned(),
            current_data: vec![],
            is_meta_writer: false,
        }))
    }

    async fn write_meta<'a>(&'a self, path: &'a Path) -> Result<Box<Writer>, AssetWriterError> {
        Ok(Box::new(DataWriter {
            dir: self.root.clone(),
            path: path.to_owned(),
            current_data: vec![],
            is_meta_writer: true,
        }))
    }

    async fn remove<'a>(&'a self, path: &'a Path) -> Result<(), AssetWriterError> {
        if self.root.remove_asset(path).is_none() {
            return Err(AssetWriterError::Io(Error::new(
                ErrorKind::NotFound,
                "no such file",
            )));
        }
        Ok(())
    }

    async fn remove_meta<'a>(&'a self, path: &'a Path) -> Result<(), AssetWriterError> {
        self.root.remove_metadata(path);
        Ok(())
    }

    async fn rename<'a>(
        &'a self,
        old_path: &'a Path,
        new_path: &'a Path,
    ) -> Result<(), AssetWriterError> {
        let Some(old_asset) = self.root.get_asset(old_path) else {
            return Err(AssetWriterError::Io(Error::new(
                ErrorKind::NotFound,
                "no such file",
            )));
        };
        self.root.insert_asset(new_path, old_asset.value);
        // Remove the asset after instead of before: otherwise there'd be a
        // moment where the `Dir` holds neither the old nor the new path.
        self.root.remove_asset(old_path);
        Ok(())
    }

    async fn rename_meta<'a>(
        &'a self,
        old_path: &'a Path,
        new_path: &'a Path,
    ) -> Result<(), AssetWriterError> {
        let Some(old_meta) = self.root.get_metadata(old_path) else {
            return Err(AssetWriterError::Io(Error::new(
                ErrorKind::NotFound,
                "no such file",
            )));
        };
        self.root.insert_meta(new_path, old_meta.value);
        // Remove the meta after instead of before; see `rename`.
        self.root.remove_metadata(old_path);
        Ok(())
    }

    async fn create_directory<'a>(&'a self, path: &'a Path) -> Result<(), AssetWriterError> {
        // Pretend we're on a filesystem that does not consider re-creation a
        // failure.
        self.root.get_or_insert_dir(path);
        Ok(())
    }

    async fn remove_directory<'a>(&'a self, path: &'a Path) -> Result<(), AssetWriterError> {
        if self.root.remove_dir(path).is_none() {
            return Err(AssetWriterError::Io(Error::new(
                ErrorKind::NotFound,
                "no such dir",
            )));
        }
        Ok(())
    }

    async fn remove_empty_directory<'a>(&'a self, path: &'a Path) -> Result<(), AssetWriterError> {
        let Some(dir) = self.root.get_dir(path) else {
            return Err(AssetWriterError::Io(Error::new(
                ErrorKind::NotFound,
                "no such dir",
            )));
        };

        let dir = dir.0.read().unwrap_or_else(PoisonError::into_inner);
        if !dir.assets.is_empty() || !dir.metadata.is_empty() || !dir.dirs.is_empty() {
            return Err(AssetWriterError::Io(Error::new(
                ErrorKind::DirectoryNotEmpty,
                "not empty",
            )));
        }

        self.root.remove_dir(path);
        Ok(())
    }

    async fn remove_assets_in_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<(), AssetWriterError> {
        let Some(dir) = self.root.get_dir(path) else {
            return Err(AssetWriterError::Io(Error::new(
                ErrorKind::NotFound,
                "no such dir",
            )));
        };

        let mut dir = dir.0.write().unwrap_or_else(PoisonError::into_inner);
        dir.assets.clear();
        dir.dirs.clear();
        dir.metadata.clear();
        Ok(())
    }
}
