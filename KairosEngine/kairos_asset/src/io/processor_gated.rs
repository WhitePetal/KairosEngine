//! The gated processed-asset reader.
//!
//! [`ProcessorGatedReader`] wraps a source's [`processed_reader`] so that reads
//! wait for the [`AssetProcessor`](crate::processor::AssetProcessor) to finish
//! the requested asset first. It is installed by
//! [`AssetSource::gate_on_processor`](super::AssetSource::gate_on_processor) and
//! is what makes layout ② (a runtime processor plus a server reading its output)
//! safe to load from on a first run.
//!
//! Two guarantees come out of the gate:
//!
//! - **Readiness.** A read of `path` resolves the asset's [`ProcessStatus`]:
//!   [`Processed`](ProcessStatus::Processed) lets the read through, while
//!   [`Failed`](ProcessStatus::Failed) and [`NonExistent`](ProcessStatus::NonExistent)
//!   surface as [`AssetReaderError::NotFound`]. Directory reads wait for the
//!   whole current pass instead, because a directory is not one asset.
//! - **No half-written files.** Once the asset is processed, the read takes the
//!   asset's *file transaction lock* for as long as the reader lives. The
//!   processor holds the write side of that lock for the whole of a processing
//!   pass, so a reader can never observe bytes and `.meta` from different
//!   versions.
//!
//! The processor's own reads do **not** go through here; they use the
//! [`ungated_processed_reader`](super::AssetSource::ungated_processed_reader) so
//! it never waits on its own output.

use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures_io::{AsyncRead, AsyncSeek};
use kairos_tasks::BoxedFuture;

use crate::path::AssetPath;
use crate::processor::{ProcessStatus, ProcessingState};

use super::{AssetReader, AssetReaderError, AssetSourceId, ErasedAssetReader, PathStream, Reader};

/// An [`AssetReader`] that will not let a read (or a meta read) of a processed
/// asset finish until the processor has processed that asset.
pub(crate) struct ProcessorGatedReader {
    reader: Arc<dyn ErasedAssetReader>,
    source: AssetSourceId<'static>,
    processing_state: Arc<ProcessingState>,
}

impl ProcessorGatedReader {
    /// Wraps `reader` so reads wait on `processing_state`.
    pub(crate) fn new(
        source: AssetSourceId<'static>,
        reader: Arc<dyn ErasedAssetReader>,
        processing_state: Arc<ProcessingState>,
    ) -> Self {
        Self {
            reader,
            source,
            processing_state,
        }
    }

    /// Waits for `path` to be processed, returning its absolute asset path.
    ///
    /// A failed or non-existent asset is reported the same way a missing file
    /// is: the gate has nothing for the caller.
    async fn wait_until_ready(&self, path: &Path) -> Result<AssetPath<'static>, AssetReaderError> {
        let asset_path = AssetPath::from(path.to_path_buf()).with_source(self.source.clone());
        match self
            .processing_state
            .wait_until_processed(asset_path.clone())
            .await
        {
            ProcessStatus::Processed => Ok(asset_path),
            ProcessStatus::Failed | ProcessStatus::NonExistent => {
                Err(AssetReaderError::NotFound(path.to_path_buf()))
            }
        }
    }
}

impl AssetReader for ProcessorGatedReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        let asset_path = self.wait_until_ready(path).await?;
        let lock = self
            .processing_state
            .get_transaction_lock(&asset_path)
            .await?;
        let reader = self.reader.read(path).await?;
        Ok(TransactionLockedReader::new(reader, lock))
    }

    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        let asset_path = self.wait_until_ready(path).await?;
        let lock = self
            .processing_state
            .get_transaction_lock(&asset_path)
            .await?;
        let reader = self.reader.read_meta(path).await?;
        Ok(TransactionLockedReader::new(reader, lock))
    }

    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        self.processing_state.wait_until_finished().await;
        self.reader.read_directory(path).await
    }

    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
        self.processing_state.wait_until_finished().await;
        self.reader.is_directory(path).await
    }
}

/// A [`Reader`] that holds an asset's file transaction lock until it is dropped.
///
/// This is the read half of the pair the processor writes under: while it lives,
/// the processor cannot take the write lock to overwrite the asset, so the bytes
/// a loader streams are internally consistent.
pub(crate) struct TransactionLockedReader<'a> {
    reader: Box<dyn Reader + 'a>,
    _file_transaction_lock: async_lock::RwLockReadGuardArc<()>,
}

impl<'a> TransactionLockedReader<'a> {
    fn new(
        reader: Box<dyn Reader + 'a>,
        file_transaction_lock: async_lock::RwLockReadGuardArc<()>,
    ) -> Self {
        Self {
            reader,
            _file_transaction_lock: file_transaction_lock,
        }
    }
}

impl AsyncRead for TransactionLockedReader<'_> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut *self.reader).poll_read(cx, buf)
    }
}

impl AsyncSeek for TransactionLockedReader<'_> {
    fn poll_seek(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        pos: std::io::SeekFrom,
    ) -> Poll<std::io::Result<u64>> {
        Pin::new(&mut *self.reader).poll_seek(cx, pos)
    }
}

impl Reader for TransactionLockedReader<'_> {
    fn read_to_end<'a>(
        &'a mut self,
        buf: &'a mut Vec<u8>,
    ) -> BoxedFuture<'a, std::io::Result<usize>> {
        self.reader.read_to_end(buf)
    }
}
