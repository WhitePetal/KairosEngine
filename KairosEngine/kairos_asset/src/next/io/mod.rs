//! Byte IO for asset sources: the reader/writer interface.
//!
//! Asset data is addressed by a [`Path`](std::path::Path) inside an
//! [`AssetSource`](crate::next::io::AssetSource). A source hands out an
//! [`AssetReader`] for bytes and an optional [`AssetWriter`] for writing them
//! back; the loader ticket builds on top of this, and the file and embedded
//! backends live beside it ([`file`], [`embedded`]).
//!
//! The read abstraction is a [`Reader`]: anything that is
//! [`AsyncRead`](futures_io::AsyncRead) [`AsyncSeek`](futures_io::AsyncSeek)
//! and can cross threads. It is deliberately executor-agnostic — kairos drives
//! loading on [`IoTaskPool`](kairos_tasks::IoTaskPool), not on a tokio runtime
//! (ADR 0001). Paths that a loader needs to stream are read through
//! [`Reader`], and returned futures are `ConditionalSendFuture`s so they can be
//! spawned on the task pool.

use core::{
    pin::Pin,
    task::{Context, Poll},
};
use std::{
    io::SeekFrom,
    path::{Path, PathBuf},
};

use futures_io::{AsyncRead, AsyncSeek, AsyncWrite};
use futures_lite::Stream;
use kairos_tasks::ConditionalSendFuture;

pub mod embedded;
pub mod file;
mod source;

pub use source::{
    AssetSource, AssetSourceBuilder, AssetSourceBuilders, AssetSourceId, AssetSources,
    MissingAssetSourceError, MissingAssetWriterError, MissingProcessedAssetReaderError,
};

/// A boxed future that may cross threads. The box is what makes the erased
/// (object-safe) reader and writer traits possible.
pub type BoxedFuture<'a, T> = Pin<Box<dyn ConditionalSendFuture<Output = T> + 'a>>;

/// A stream of directory entry paths, as returned by
/// [`AssetReader::read_directory`].
pub type PathStream = dyn Stream<Item = PathBuf> + Unpin + Send;

/// A writer's byte sink. This is the type alias `AssetWriter` hands out.
pub type Writer = dyn AsyncWrite + Unpin + Send + Sync;

/// An empty [`PathStream`], for readers with no directories.
pub fn empty_path_stream() -> Box<PathStream> {
    Box::new(futures_lite::stream::empty())
}

/// Errors that can occur while reading an asset.
#[derive(Debug, Clone)]
pub enum AssetReaderError {
    /// The path does not exist in this source.
    NotFound(PathBuf),
    /// An OS-level IO error occurred.
    Io(std::sync::Arc<std::io::Error>),
    /// An HTTP source returned an unexpected status code.
    HttpError(u16),
}

impl PartialEq for AssetReaderError {
    /// Equality for [`AssetReaderError::Io`] compares only
    /// [`std::io::ErrorKind`]; the OS error itself has no `PartialEq`.
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::NotFound(path), Self::NotFound(other_path)) => path == other_path,
            (Self::Io(error), Self::Io(other_error)) => error.kind() == other_error.kind(),
            (Self::HttpError(code), Self::HttpError(other_code)) => code == other_code,
            _ => false,
        }
    }
}

impl Eq for AssetReaderError {}

impl From<std::io::Error> for AssetReaderError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(std::sync::Arc::new(value))
    }
}

impl core::fmt::Display for AssetReaderError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotFound(path) => write!(f, "Path not found: {}", path.display()),
            Self::Io(error) => write!(f, "Encountered an I/O error while loading asset: {error}"),
            Self::HttpError(code) => {
                write!(f, "Encountered HTTP status {code:?} when loading asset")
            }
        }
    }
}

impl std::error::Error for AssetReaderError {}

/// Errors that can occur while writing an asset.
#[derive(Debug)]
#[non_exhaustive]
pub enum AssetWriterError {
    /// An OS-level IO error occurred.
    Io(std::io::Error),
}

impl From<std::io::Error> for AssetWriterError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl core::fmt::Display for AssetWriterError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "Encountered an I/O error while writing asset: {error}"),
        }
    }
}

impl std::error::Error for AssetWriterError {}

/// The bytes of one asset (or one asset's meta sidecar).
///
/// `Reader` is an executor-agnostic handle over a byte stream that can also
/// seek backwards — GLTF and similar formats need to re-read headers — so it
/// is bound to [`AsyncRead`] + [`AsyncSeek`] and nothing else. The provided
/// [`read_to_end`](Reader::read_to_end) collects the rest into a `Vec`.
///
/// Unlike bevy's `Reader`, seeking is mandatory rather than an optional
/// capability: every backend kairos ships (`async-fs`, in-memory) can seek, so
/// the separate `SeekableReader` split has no buyer yet.
pub trait Reader: AsyncRead + AsyncSeek + Unpin + Send + Sync {
    /// Reads the remaining bytes and appends them to `buf`.
    ///
    /// Implementors may override this to fill the buffer more efficiently than
    /// the default poll loop.
    fn read_to_end<'a>(&'a mut self, buf: &'a mut Vec<u8>) -> BoxedFuture<'a, std::io::Result<usize>> {
        Box::pin(async move { futures_lite::AsyncReadExt::read_to_end(self, buf).await })
    }
}

impl Reader for Box<dyn Reader + '_> {
    fn read_to_end<'a>(&'a mut self, buf: &'a mut Vec<u8>) -> BoxedFuture<'a, std::io::Result<usize>> {
        (**self).read_to_end(buf)
    }
}

/// A future that resolves to a value or an [`AssetReaderError`].
///
/// This exists only so [`AssetReader`] can express
/// `impl AssetReaderFuture<Value: Reader>` in its return position: the
/// associated `Value` is the successful type, without naming the concrete
/// future.
pub trait AssetReaderFuture:
    ConditionalSendFuture<Output = Result<Self::Value, AssetReaderError>>
{
    /// The value the future resolves to on success.
    type Value;
}

impl<F, T> AssetReaderFuture for F
where
    F: ConditionalSendFuture<Output = Result<T, AssetReaderError>>,
{
    type Value = T;
}

/// Reads asset bytes and asset meta bytes from a storage backend.
///
/// This is the non-object-safe (RPITIT) trait; use [`ErasedAssetReader`] when
/// a `dyn` reader is needed. The methods are named after the storage concept,
/// not the asset: `read` returns the asset's bytes, `read_meta` the sidecar's.
pub trait AssetReader: Send + Sync + 'static {
    /// Opens the asset's bytes at `path`.
    fn read<'a>(&'a self, path: &'a Path) -> impl AssetReaderFuture<Value: Reader + 'a>;

    /// Opens the asset meta sidecar's bytes at `path` (without the `.meta`
    /// suffix; backends append it).
    fn read_meta<'a>(&'a self, path: &'a Path) -> impl AssetReaderFuture<Value: Reader + 'a>;

    /// Lists the entry paths in the directory at `path`, relative to the
    /// source root.
    fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> impl ConditionalSendFuture<Output = Result<Box<PathStream>, AssetReaderError>>;

    /// Whether `path` points at a directory.
    fn is_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> impl ConditionalSendFuture<Output = Result<bool, AssetReaderError>>;

    /// Reads the asset meta sidecar at `path` into a `Vec<u8>`.
    fn read_meta_bytes<'a>(
        &'a self,
        path: &'a Path,
    ) -> impl ConditionalSendFuture<Output = Result<Vec<u8>, AssetReaderError>> {
        async move {
            let mut meta_reader = <Self as AssetReader>::read_meta(self, path).await?;
            let mut meta_bytes = Vec::new();
            meta_reader.read_to_end(&mut meta_bytes).await?;
            Ok(meta_bytes)
        }
    }
}

/// The object-safe counterpart of [`AssetReader`], used behind a `dyn`.
pub trait ErasedAssetReader: Send + Sync + 'static {
    /// Opens the asset's bytes at `path`.
    fn read<'a>(&'a self, path: &'a Path)
    -> BoxedFuture<'a, Result<Box<dyn Reader + 'a>, AssetReaderError>>;

    /// Opens the asset meta sidecar's bytes at `path`.
    fn read_meta<'a>(
        &'a self,
        path: &'a Path,
    ) -> BoxedFuture<'a, Result<Box<dyn Reader + 'a>, AssetReaderError>>;

    /// Lists the entry paths in the directory at `path`, relative to the
    /// source root.
    fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> BoxedFuture<'a, Result<Box<PathStream>, AssetReaderError>>;

    /// Whether `path` points at a directory.
    fn is_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> BoxedFuture<'a, Result<bool, AssetReaderError>>;

    /// Reads the asset meta sidecar at `path` into a `Vec<u8>`.
    fn read_meta_bytes<'a>(
        &'a self,
        path: &'a Path,
    ) -> BoxedFuture<'a, Result<Vec<u8>, AssetReaderError>>;
}

impl<T: AssetReader> ErasedAssetReader for T {
    fn read<'a>(
        &'a self,
        path: &'a Path,
    ) -> BoxedFuture<'a, Result<Box<dyn Reader + 'a>, AssetReaderError>> {
        Box::pin(async move {
            let reader = <T as AssetReader>::read(self, path).await?;
            Ok(Box::new(reader) as Box<dyn Reader>)
        })
    }

    fn read_meta<'a>(
        &'a self,
        path: &'a Path,
    ) -> BoxedFuture<'a, Result<Box<dyn Reader + 'a>, AssetReaderError>> {
        Box::pin(async move {
            let reader = <T as AssetReader>::read_meta(self, path).await?;
            Ok(Box::new(reader) as Box<dyn Reader>)
        })
    }

    fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> BoxedFuture<'a, Result<Box<PathStream>, AssetReaderError>> {
        Box::pin(<T as AssetReader>::read_directory(self, path))
    }

    fn is_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> BoxedFuture<'a, Result<bool, AssetReaderError>> {
        Box::pin(<T as AssetReader>::is_directory(self, path))
    }

    fn read_meta_bytes<'a>(
        &'a self,
        path: &'a Path,
    ) -> BoxedFuture<'a, Result<Vec<u8>, AssetReaderError>> {
        Box::pin(<T as AssetReader>::read_meta_bytes(self, path))
    }
}

/// Writes asset bytes and asset meta bytes to a storage backend.
///
/// This is the non-object-safe (RPITIT) trait; use [`ErasedAssetWriter`] when a
/// `dyn` writer is needed. No backend implements it yet — the processed-asset
/// pipeline that writes bytes back is deferred — but the surface exists so
/// sources can carry a writer slot.
pub trait AssetWriter: Send + Sync + 'static {
    /// Opens a byte sink for the asset at `path`.
    fn write<'a>(
        &'a self,
        path: &'a Path,
    ) -> impl ConditionalSendFuture<Output = Result<Box<Writer>, AssetWriterError>>;

    /// Opens a byte sink for the asset meta sidecar at `path`. The `.meta`
    /// suffix is not included; backends append it.
    fn write_meta<'a>(
        &'a self,
        path: &'a Path,
    ) -> impl ConditionalSendFuture<Output = Result<Box<Writer>, AssetWriterError>>;

    /// Removes the asset at `path`.
    fn remove<'a>(
        &'a self,
        path: &'a Path,
    ) -> impl ConditionalSendFuture<Output = Result<(), AssetWriterError>>;

    /// Removes the asset meta sidecar at `path`.
    fn remove_meta<'a>(
        &'a self,
        path: &'a Path,
    ) -> impl ConditionalSendFuture<Output = Result<(), AssetWriterError>>;

    /// Renames the asset at `old_path` to `new_path`.
    fn rename<'a>(
        &'a self,
        old_path: &'a Path,
        new_path: &'a Path,
    ) -> impl ConditionalSendFuture<Output = Result<(), AssetWriterError>>;

    /// Renames the asset meta sidecar for `old_path` to `new_path`.
    fn rename_meta<'a>(
        &'a self,
        old_path: &'a Path,
        new_path: &'a Path,
    ) -> impl ConditionalSendFuture<Output = Result<(), AssetWriterError>>;

    /// Creates the directory at `path`, including parents.
    fn create_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> impl ConditionalSendFuture<Output = Result<(), AssetWriterError>>;

    /// Removes the directory at `path`, including all contents.
    fn remove_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> impl ConditionalSendFuture<Output = Result<(), AssetWriterError>>;

    /// Removes the directory at `path`, failing if it is not empty.
    fn remove_empty_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> impl ConditionalSendFuture<Output = Result<(), AssetWriterError>>;

    /// Removes every asset and directory inside `path`, leaving it empty.
    fn remove_assets_in_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> impl ConditionalSendFuture<Output = Result<(), AssetWriterError>>;

    /// Writes `bytes` to the asset at `path`.
    fn write_bytes<'a>(
        &'a self,
        path: &'a Path,
        bytes: &'a [u8],
    ) -> impl ConditionalSendFuture<Output = Result<(), AssetWriterError>> {
        async move {
            let mut writer = self.write(path).await?;
            futures_lite::AsyncWriteExt::write_all(&mut writer, bytes).await?;
            futures_lite::AsyncWriteExt::flush(&mut writer).await?;
            Ok(())
        }
    }

    /// Writes `bytes` to the asset meta sidecar at `path`.
    fn write_meta_bytes<'a>(
        &'a self,
        path: &'a Path,
        bytes: &'a [u8],
    ) -> impl ConditionalSendFuture<Output = Result<(), AssetWriterError>> {
        async move {
            let mut meta_writer = self.write_meta(path).await?;
            futures_lite::AsyncWriteExt::write_all(&mut meta_writer, bytes).await?;
            futures_lite::AsyncWriteExt::flush(&mut meta_writer).await?;
            Ok(())
        }
    }
}

/// The object-safe counterpart of [`AssetWriter`], used behind a `dyn`.
pub trait ErasedAssetWriter: Send + Sync + 'static {
    /// Opens a byte sink for the asset at `path`.
    fn write<'a>(
        &'a self,
        path: &'a Path,
    ) -> BoxedFuture<'a, Result<Box<Writer>, AssetWriterError>>;

    /// Opens a byte sink for the asset meta sidecar at `path`.
    fn write_meta<'a>(
        &'a self,
        path: &'a Path,
    ) -> BoxedFuture<'a, Result<Box<Writer>, AssetWriterError>>;

    /// Removes the asset at `path`.
    fn remove<'a>(&'a self, path: &'a Path) -> BoxedFuture<'a, Result<(), AssetWriterError>>;

    /// Removes the asset meta sidecar at `path`.
    fn remove_meta<'a>(&'a self, path: &'a Path) -> BoxedFuture<'a, Result<(), AssetWriterError>>;

    /// Renames the asset at `old_path` to `new_path`.
    fn rename<'a>(
        &'a self,
        old_path: &'a Path,
        new_path: &'a Path,
    ) -> BoxedFuture<'a, Result<(), AssetWriterError>>;

    /// Renames the asset meta sidecar for `old_path` to `new_path`.
    fn rename_meta<'a>(
        &'a self,
        old_path: &'a Path,
        new_path: &'a Path,
    ) -> BoxedFuture<'a, Result<(), AssetWriterError>>;

    /// Creates the directory at `path`, including parents.
    fn create_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> BoxedFuture<'a, Result<(), AssetWriterError>>;

    /// Removes the directory at `path`, including all contents.
    fn remove_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> BoxedFuture<'a, Result<(), AssetWriterError>>;

    /// Removes the directory at `path`, failing if it is not empty.
    fn remove_empty_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> BoxedFuture<'a, Result<(), AssetWriterError>>;

    /// Removes every asset and directory inside `path`, leaving it empty.
    fn remove_assets_in_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> BoxedFuture<'a, Result<(), AssetWriterError>>;

    /// Writes `bytes` to the asset at `path`.
    fn write_bytes<'a>(
        &'a self,
        path: &'a Path,
        bytes: &'a [u8],
    ) -> BoxedFuture<'a, Result<(), AssetWriterError>>;

    /// Writes `bytes` to the asset meta sidecar at `path`.
    fn write_meta_bytes<'a>(
        &'a self,
        path: &'a Path,
        bytes: &'a [u8],
    ) -> BoxedFuture<'a, Result<(), AssetWriterError>>;
}

impl<T: AssetWriter> ErasedAssetWriter for T {
    fn write<'a>(
        &'a self,
        path: &'a Path,
    ) -> BoxedFuture<'a, Result<Box<Writer>, AssetWriterError>> {
        Box::pin(<T as AssetWriter>::write(self, path))
    }

    fn write_meta<'a>(
        &'a self,
        path: &'a Path,
    ) -> BoxedFuture<'a, Result<Box<Writer>, AssetWriterError>> {
        Box::pin(<T as AssetWriter>::write_meta(self, path))
    }

    fn remove<'a>(&'a self, path: &'a Path) -> BoxedFuture<'a, Result<(), AssetWriterError>> {
        Box::pin(<T as AssetWriter>::remove(self, path))
    }

    fn remove_meta<'a>(&'a self, path: &'a Path) -> BoxedFuture<'a, Result<(), AssetWriterError>> {
        Box::pin(<T as AssetWriter>::remove_meta(self, path))
    }

    fn rename<'a>(
        &'a self,
        old_path: &'a Path,
        new_path: &'a Path,
    ) -> BoxedFuture<'a, Result<(), AssetWriterError>> {
        Box::pin(<T as AssetWriter>::rename(self, old_path, new_path))
    }

    fn rename_meta<'a>(
        &'a self,
        old_path: &'a Path,
        new_path: &'a Path,
    ) -> BoxedFuture<'a, Result<(), AssetWriterError>> {
        Box::pin(<T as AssetWriter>::rename_meta(self, old_path, new_path))
    }

    fn create_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> BoxedFuture<'a, Result<(), AssetWriterError>> {
        Box::pin(<T as AssetWriter>::create_directory(self, path))
    }

    fn remove_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> BoxedFuture<'a, Result<(), AssetWriterError>> {
        Box::pin(<T as AssetWriter>::remove_directory(self, path))
    }

    fn remove_empty_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> BoxedFuture<'a, Result<(), AssetWriterError>> {
        Box::pin(<T as AssetWriter>::remove_empty_directory(self, path))
    }

    fn remove_assets_in_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> BoxedFuture<'a, Result<(), AssetWriterError>> {
        Box::pin(<T as AssetWriter>::remove_assets_in_directory(self, path))
    }

    fn write_bytes<'a>(
        &'a self,
        path: &'a Path,
        bytes: &'a [u8],
    ) -> BoxedFuture<'a, Result<(), AssetWriterError>> {
        Box::pin(<T as AssetWriter>::write_bytes(self, path, bytes))
    }

    fn write_meta_bytes<'a>(
        &'a self,
        path: &'a Path,
        bytes: &'a [u8],
    ) -> BoxedFuture<'a, Result<(), AssetWriterError>> {
        Box::pin(<T as AssetWriter>::write_meta_bytes(self, path, bytes))
    }
}

/// An asset source change event.
///
/// Nothing emits these yet — the file watcher that would is deferred — but the
/// type is the contract a watcher backend will produce and the asset server
/// will consume.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssetSourceEvent {
    /// An asset at this path was added.
    AddedAsset(PathBuf),
    /// An asset at this path was modified.
    ModifiedAsset(PathBuf),
    /// An asset at this path was removed.
    RemovedAsset(PathBuf),
    /// An asset was renamed.
    RenamedAsset { old: PathBuf, new: PathBuf },
    /// Asset metadata at this path was added.
    AddedMeta(PathBuf),
    /// Asset metadata at this path was modified.
    ModifiedMeta(PathBuf),
    /// Asset metadata at this path was removed.
    RemovedMeta(PathBuf),
    /// Asset metadata was renamed.
    RenamedMeta { old: PathBuf, new: PathBuf },
    /// A folder at this path was added.
    AddedFolder(PathBuf),
    /// A folder at this path was removed.
    RemovedFolder(PathBuf),
    /// A folder was renamed.
    RenamedFolder { old: PathBuf, new: PathBuf },
    /// Something of unknown type was removed. Determining whether it was an
    /// asset, a meta file, or a folder is the handler's job.
    RemovedUnknown {
        /// The path of the removed asset or folder.
        path: PathBuf,
        /// Whether `path` names a meta file rather than an asset.
        is_meta: bool,
    },
}

/// A handle to a process watching an asset source for [`AssetSourceEvent`]s.
///
/// The handle must be kept alive for as long as watching should continue. No
/// backend implements it yet.
pub trait AssetWatcher: Send + Sync + 'static {}

/// How the asset server reacts to load requests for paths outside the approved
/// asset source roots.
///
/// Approved roots are the process working directory and each source's folder;
/// subfolders count as approved. The default is
/// [`Forbid`](UnapprovedPathMode::Forbid), so a `..` that climbs out of a
/// source is rejected (ADR 0004).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UnapprovedPathMode {
    /// Unapproved paths are loaded anyway. Strongly discouraged: it allows
    /// arbitrary file access for modding or scripted content.
    Allow,
    /// Unapproved paths fail unless the load explicitly overrides the mode.
    Deny,
    /// Unapproved paths always fail.
    #[default]
    Forbid,
}

/// Appends `.meta` to a path: `foo` becomes `foo.meta`, `foo.bar` becomes
/// `foo.bar.meta`.
pub fn get_meta_path(path: &Path) -> PathBuf {
    let mut meta_path = path.to_path_buf();
    let mut extension = path.extension().unwrap_or_default().to_os_string();
    if !extension.is_empty() {
        extension.push(".");
    }
    extension.push("meta");
    meta_path.set_extension(extension);
    meta_path
}

/// An [`AsyncRead`] + [`AsyncSeek`] implementation over an owned `Vec<u8>`.
///
/// Handy for tests and for loaders that need a seekable reader over bytes they
/// already hold.
pub struct VecReader {
    /// The bytes being read. Kept public because loaders often need the backing
    /// buffer after seeking.
    pub bytes: Vec<u8>,
    bytes_read: usize,
}

impl VecReader {
    /// Creates a reader over `bytes`, positioned at the start.
    pub fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes_read: 0,
            bytes,
        }
    }
}

impl AsyncRead for VecReader {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<futures_io::Result<usize>> {
        let this = self.get_mut();
        Poll::Ready(Ok(slice_read(&this.bytes, &mut this.bytes_read, buf)))
    }
}

impl AsyncSeek for VecReader {
    fn poll_seek(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        pos: SeekFrom,
    ) -> Poll<std::io::Result<u64>> {
        let this = self.get_mut();
        Poll::Ready(slice_seek(&this.bytes, &mut this.bytes_read, pos))
    }
}

impl Reader for VecReader {}

/// Reads from `slice` into `buf`, tracking the cursor in `bytes_read`.
pub(crate) fn slice_read(slice: &[u8], bytes_read: &mut usize, buf: &mut [u8]) -> usize {
    if *bytes_read >= slice.len() {
        0
    } else {
        let n = std::io::Read::read(&mut &slice[(*bytes_read)..], buf).unwrap();
        *bytes_read += n;
        n
    }
}

/// Seeks `bytes_read` within `slice`, returning the new byte position.
pub(crate) fn slice_seek(
    slice: &[u8],
    bytes_read: &mut usize,
    pos: SeekFrom,
) -> std::io::Result<u64> {
    let make_error = || {
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "seek position is out of range",
        ))
    };
    let (origin, offset) = match pos {
        SeekFrom::Current(offset) => (*bytes_read, Ok(offset)),
        SeekFrom::Start(offset) => (0, offset.try_into()),
        SeekFrom::End(offset) => (slice.len(), Ok(offset)),
    };
    let Ok(offset) = offset else {
        return make_error();
    };
    let Ok(origin): Result<i64, _> = origin.try_into() else {
        return make_error();
    };
    let Ok(new_pos) = (origin + offset).try_into() else {
        return make_error();
    };
    *bytes_read = new_pos;
    Ok(new_pos as _)
}
