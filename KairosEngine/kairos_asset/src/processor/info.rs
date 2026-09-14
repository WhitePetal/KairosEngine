//! The in-memory processed-asset graph.
//!
//! [`ProcessorAssetInfos`] keeps one [`ProcessorAssetInfo`] per asset it has
//! seen: the [`ProcessedInfo`] it was last processed to, the set of assets that
//! depend on it (its `dependents`), and its [`ProcessStatus`]. Forward edges
//! live in [`ProcessedInfo::process_dependencies`]; the reverse `dependents`
//! edges live here, kept in sync as assets are processed, removed, and re-pointed
//! at new dependencies. Each asset also carries a `file_transaction_lock` that
//! the processor holds while writing its processed output, so a concurrent reader
//! never sees a half-written asset (the gated reader holds the read side; it
//! lands with the gating slice).
//!
//! An asset absent from the graph is treated as **non-existent**; a dependency
//! that does not exist yet is parked in `non_existent_dependents` so that when
//! it appears its dependents are still linked to it. This mirrors `bevy_asset`'s
//! structure, minus the processor's I/O, scheduling, and gated-reader state.
//!
//! [`ProcessorAssetInfos::finish_processing`] folds a completed (or skipped, or
//! failed) pass back into the graph and hands the caller the dependents that must
//! be re-checked.
#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::Arc;

use kairos_collections::{FixedHashMap as HashMap, FixedHashSet as HashSet};

use crate::io::AssetSourceId;
use crate::meta::{AssetHash, ProcessedInfo};
use crate::path::AssetPath;
use crate::server::AssetLoadError;

use super::asset_processor::ProcessResult;
use super::process::ProcessError;

/// The final status of processing one asset.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub(crate) enum ProcessStatus {
    /// The asset was processed, or its existing output was found up to date.
    Processed,
    /// Processing the asset failed.
    Failed,
    /// The asset does not exist in the source.
    NonExistent,
}

/// The graph's record for one asset.
///
/// Note: if a new field is added here, make sure it is propagated (where
/// relevant) by [`ProcessorAssetInfos::rename`].
#[derive(Debug)]
pub(crate) struct ProcessorAssetInfo {
    processed_info: Option<ProcessedInfo>,
    /// Paths of assets that depend on this asset when they are being processed.
    dependents: HashSet<AssetPath<'static>>,
    status: Option<ProcessStatus>,
    /// A lock that controls read/write access to the processed asset's bytes and
    /// its `.meta` sidecar.
    ///
    /// The processor holds the write side for the whole of a processing pass, so
    /// a reader that acquires the read side can never observe payload bytes and
    /// metadata from different versions of the asset.
    file_transaction_lock: Arc<async_lock::RwLock<()>>,
    /// Broadcasts every status change to the gated reader waiting on this asset.
    status_sender: async_broadcast::Sender<ProcessStatus>,
    /// The receiving end of `status_sender`. Kept alive (and never drained here)
    /// so the channel is never closed and a late wait can clone it.
    status_receiver: async_broadcast::Receiver<ProcessStatus>,
}

impl Default for ProcessorAssetInfo {
    fn default() -> Self {
        let (mut status_sender, status_receiver) = async_broadcast::broadcast(1);
        // Overflow lets a late receiver read the latest status instead of the
        // broadcast blocking on an unread slot.
        status_sender.set_overflow(true);
        Self {
            processed_info: None,
            dependents: HashSet::default(),
            status: None,
            file_transaction_lock: Arc::new(async_lock::RwLock::new(())),
            status_sender,
            status_receiver,
        }
    }
}

impl ProcessorAssetInfo {
    /// The info this asset was last processed to, if it has been processed.
    pub(crate) fn processed_info(&self) -> Option<&ProcessedInfo> {
        self.processed_info.as_ref()
    }

    /// Replaces this asset's recorded [`ProcessedInfo`].
    pub(crate) fn set_processed_info(&mut self, processed_info: Option<ProcessedInfo>) {
        self.processed_info = processed_info;
    }

    /// The paths that depend on this asset when they are processed.
    pub(crate) fn dependents(&self) -> &HashSet<AssetPath<'static>> {
        &self.dependents
    }

    /// The asset's current [`ProcessStatus`], if it has one yet.
    pub(crate) fn status(&self) -> Option<ProcessStatus> {
        self.status
    }

    /// A receiver that fires on this asset's next status change.
    ///
    /// Callers must read [`ProcessorAssetInfo::status`] under the same lock as
    /// this clone: if the status flips in between, the broadcast is missed.
    pub(crate) fn status_receiver(&self) -> async_broadcast::Receiver<ProcessStatus> {
        self.status_receiver.clone()
    }

    /// The lock guarding writes to this asset's processed output.
    pub(crate) fn file_transaction_lock(&self) -> Arc<async_lock::RwLock<()>> {
        self.file_transaction_lock.clone()
    }

    /// Records `status` and wakes anything waiting on this asset.
    ///
    /// A repeat of the current status is not re-broadcast.
    async fn update_status(&mut self, status: ProcessStatus) {
        if self.status != Some(status) {
            self.status = Some(status);
            let _ = self.status_sender.broadcast(status).await;
        }
    }
}

/// The "current" in-memory view of the processed asset space.
///
/// This is eventually consistent: it does not directly represent what is on
/// disk, only a historical view that converges as processing completes.
#[derive(Default, Debug)]
pub(crate) struct ProcessorAssetInfos {
    /// The in-memory view. A path absent from here should be considered
    /// non-existent.
    ///
    /// Add items through [`ProcessorAssetInfos::get_or_insert`] so any parked
    /// `non_existent_dependents` are consumed.
    infos: HashMap<AssetPath<'static>, ProcessorAssetInfo>,
    /// Dependents of assets that do not exist yet. When a dependent asset is
    /// added, it resolves these references and inherits them. Kept consistent
    /// with `infos`: adding an asset consumes its entry here; removing one
    /// (re)inserts its dependents.
    non_existent_dependents: HashMap<AssetPath<'static>, HashSet<AssetPath<'static>>>,
}

impl ProcessorAssetInfos {
    pub(crate) fn get_or_insert(
        &mut self,
        asset_path: AssetPath<'static>,
    ) -> &mut ProcessorAssetInfo {
        self.infos.entry(asset_path.clone()).or_insert_with(|| {
            let mut info = ProcessorAssetInfo::default();
            // Resolve any dependents that were waiting for this asset.
            if let Some(dependents) = self.non_existent_dependents.remove(&asset_path) {
                info.dependents = dependents;
            }
            info
        })
    }

    /// The info recorded for `asset_path`, if any.
    ///
    /// [`None`] means the asset is (or is treated as) non-existent.
    pub(crate) fn get(&self, asset_path: &AssetPath<'static>) -> Option<&ProcessorAssetInfo> {
        self.infos.get(asset_path)
    }

    pub(crate) fn get_mut(
        &mut self,
        asset_path: &AssetPath<'static>,
    ) -> Option<&mut ProcessorAssetInfo> {
        self.infos.get_mut(asset_path)
    }

    pub(crate) fn add_dependent(
        &mut self,
        asset_path: &AssetPath<'static>,
        dependent: AssetPath<'static>,
    ) {
        if let Some(info) = self.get_mut(asset_path) {
            info.dependents.insert(dependent);
        } else {
            self.non_existent_dependents
                .entry(asset_path.clone())
                .or_default()
                .insert(dependent);
        }
    }

    /// Records the successful result of processing `asset_path`.
    ///
    /// The asset's previous dependency edges are cleared, its new
    /// `process_dependencies` become reverse `dependents` edges, its
    /// [`ProcessedInfo`] is replaced, and it is marked
    /// [`Processed`](ProcessStatus::Processed). Returns the paths that depend on
    /// it, so the caller can queue them for a (possibly skipped) reprocessing
    /// pass.
    pub(crate) async fn insert_processed(
        &mut self,
        asset_path: AssetPath<'static>,
        processed_info: ProcessedInfo,
    ) -> Vec<AssetPath<'static>> {
        let old_processed_info = self
            .infos
            .get_mut(&asset_path)
            .and_then(|info| info.processed_info.take());
        if let Some(old_processed_info) = old_processed_info {
            self.clear_dependencies(&asset_path, old_processed_info);
        }

        for dependency in &processed_info.process_dependencies {
            self.add_dependent(&dependency.path, asset_path.clone());
        }

        let info = self.get_or_insert(asset_path);
        info.processed_info = Some(processed_info);
        info.update_status(ProcessStatus::Processed).await;
        info.dependents.iter().cloned().collect()
    }

    /// Marks `asset_path` as [`Processed`](ProcessStatus::Processed) without
    /// changing its recorded [`ProcessedInfo`].
    ///
    /// This is the `SkippedNotChanged` outcome: the existing output is still
    /// valid, but the asset should be considered ready.
    pub(crate) async fn mark_processed(&mut self, asset_path: &AssetPath<'static>) {
        let info = self.get_or_insert(asset_path.clone());
        info.update_status(ProcessStatus::Processed).await;
    }

    /// Marks `asset_path` as [`Failed`](ProcessStatus::Failed) to process.
    pub(crate) async fn mark_failed(&mut self, asset_path: AssetPath<'static>) {
        let info = self.get_or_insert(asset_path);
        info.update_status(ProcessStatus::Failed).await;
    }

    /// Marks `asset_path` as [`NonExistent`](ProcessStatus::NonExistent), so a
    /// gated reader waiting on it resolves instead of waiting forever.
    ///
    /// This is for assets that produce no processed output at all — ignored,
    /// extension-less, or whose source vanished mid-pass. Upstream leaves their
    /// status unset, which would hang a `ProcessorGatedReader`; kairos marks
    /// them non-existent so the gate reports `NotFound` (ADR 0005 deviation).
    pub(crate) async fn mark_non_existent(&mut self, asset_path: &AssetPath<'static>) {
        let info = self.get_or_insert(asset_path.clone());
        info.update_status(ProcessStatus::NonExistent).await;
    }

    /// Marks `asset_path` as failed and records `dependency` as something that
    /// must be reprocessed before this asset can be retried.
    pub(crate) async fn mark_failed_with_dependency(
        &mut self,
        asset_path: AssetPath<'static>,
        dependency: AssetPath<'static>,
    ) {
        let info = self.get_or_insert(asset_path.clone());
        info.processed_info = Some(ProcessedInfo {
            hash: AssetHash::default(),
            full_hash: AssetHash::default(),
            process_dependencies: Vec::new(),
        });
        info.update_status(ProcessStatus::Failed).await;
        self.add_dependent(&dependency, asset_path);
    }

    /// Folds one finished pass into the graph.
    ///
    /// A successful pass records the new [`ProcessedInfo`] and queues its
    /// dependents through `reprocess_sender`; a skipped pass just marks the asset
    /// ready; ignored, extension-less, and vanished assets are marked
    /// non-existent so the gate resolves; a failure is recorded (and, when it
    /// stems from a loader dependency, links that dependency so the asset is
    /// retried after it changes).
    pub(crate) async fn finish_processing(
        &mut self,
        asset_path: AssetPath<'static>,
        result: Result<ProcessResult, ProcessError>,
        reprocess_sender: &async_channel::Sender<(AssetSourceId<'static>, PathBuf)>,
    ) {
        match result {
            Ok(ProcessResult::Processed(processed_info)) => {
                let dependents = self.insert_processed(asset_path, processed_info).await;
                for dependent in dependents {
                    let _ = reprocess_sender
                        .send((
                            dependent.source().clone_owned(),
                            dependent.path().to_owned(),
                        ))
                        .await;
                }
            }
            Ok(ProcessResult::SkippedNotChanged) => {
                self.mark_processed(&asset_path).await;
            }
            Ok(ProcessResult::Ignored) => {
                self.mark_non_existent(&asset_path).await;
            }
            Err(ProcessError::ExtensionRequired) => {
                self.mark_non_existent(&asset_path).await;
            }
            Err(ProcessError::AssetReaderError {
                err: crate::io::AssetReaderError::NotFound(_),
                ..
            }) => {
                self.mark_non_existent(&asset_path).await;
            }
            Err(err) => {
                if let ProcessError::AssetLoadError(AssetLoadError::AssetLoaderError(
                    loader_error,
                )) = &err
                {
                    let dependency = loader_error.path().clone();
                    self.mark_failed_with_dependency(asset_path, dependency)
                        .await;
                } else {
                    self.mark_failed(asset_path).await;
                }
            }
        }
    }

    /// Removes `asset_path` from the graph. A removed asset reads as
    /// non-existent, and its dependents are parked until it reappears.
    ///
    /// Returns the asset's transaction lock so the caller can wait for
    /// in-flight reads and writes to finish before deleting the processed files.
    /// Anything waiting on the asset is told it is now
    /// [`NonExistent`](ProcessStatus::NonExistent).
    pub(crate) async fn remove(
        &mut self,
        asset_path: &AssetPath<'static>,
    ) -> Option<Arc<async_lock::RwLock<()>>> {
        let mut info = self.infos.remove(asset_path)?;
        if let Some(processed_info) = info.processed_info.take() {
            self.clear_dependencies(asset_path, processed_info);
        }
        // Tell anything waiting on this asset that it is gone.
        let _ = info
            .status_sender
            .broadcast(ProcessStatus::NonExistent)
            .await;
        if !info.dependents.is_empty() {
            self.non_existent_dependents
                .insert(asset_path.clone(), info.dependents);
        }
        Some(info.file_transaction_lock)
    }

    /// Moves the graph's record for `old` to `new`, re-pointing the dependents
    /// edges of both.
    ///
    /// Returns the transaction locks for the old and new paths so the caller can
    /// wait for in-flight access before moving the processed files.
    pub(crate) async fn rename(
        &mut self,
        old: &AssetPath<'static>,
        new: &AssetPath<'static>,
        new_task_sender: &async_channel::Sender<(AssetSourceId<'static>, PathBuf)>,
    ) -> Option<(Arc<async_lock::RwLock<()>>, Arc<async_lock::RwLock<()>>)> {
        let mut info = self.infos.remove(old)?;

        if !info.dependents.is_empty() {
            // Folder renames with relative paths cannot be rewritten yet, so the
            // old path's dependents are parked until the path or they change.
            // (See `bevy_asset`'s equivalent TODO: AssetPath erases
            // relativeness, so a renamed folder's dependents cannot be matched.)
            self.non_existent_dependents
                .insert(old.clone(), core::mem::take(&mut info.dependents));
        }

        if let Some(processed_info) = &info.processed_info {
            // Re-point the dependents of this asset's process dependencies.
            for dependency in &processed_info.process_dependencies {
                if let Some(dependency_info) = self.infos.get_mut(&dependency.path) {
                    dependency_info.dependents.remove(old);
                    dependency_info.dependents.insert(new.clone());
                } else if let Some(dependents) =
                    self.non_existent_dependents.get_mut(&dependency.path)
                {
                    dependents.remove(old);
                    dependents.insert(new.clone());
                }
            }
        }

        // Anything waiting on the old path must be told it no longer exists.
        let _ = info
            .status_sender
            .broadcast(ProcessStatus::NonExistent)
            .await;

        let new_info = self.get_or_insert(new.clone());
        new_info.processed_info = info.processed_info;
        new_info.status = info.status;
        // Carry the status over to the new path so anything waiting on it learns
        // the outcome without a fresh processing pass.
        if let Some(status) = info.status {
            let _ = new_info.status_sender.broadcast(status).await;
        }
        let dependents: Vec<AssetPath<'static>> = new_info.dependents.iter().cloned().collect();

        // The renamed asset may need new meta, and its dependents may have been
        // waiting for it, so both are queued for a reprocess check.
        let _ = new_task_sender
            .send((new.source().clone_owned(), new.path().to_owned()))
            .await;
        for dependent in dependents {
            let _ = new_task_sender
                .send((
                    dependent.source().clone_owned(),
                    dependent.path().to_owned(),
                ))
                .await;
        }

        Some((
            info.file_transaction_lock,
            new_info.file_transaction_lock.clone(),
        ))
    }

    /// Whether reprocessing `asset_path` can be skipped: its own content hash is
    /// unchanged **and** every recorded dependency's live `full_hash` still
    /// matches.
    ///
    /// This is the only thing [`ProcessedInfo`] is used for; it is never a
    /// readiness signal.
    pub(crate) fn is_up_to_date(
        &self,
        asset_path: &AssetPath<'static>,
        new_hash: AssetHash,
    ) -> bool {
        let Some(current) = self
            .get(asset_path)
            .and_then(ProcessorAssetInfo::processed_info)
        else {
            return false;
        };
        if current.hash != new_hash {
            return false;
        }
        current.process_dependencies.iter().all(|dependency| {
            self.get(&dependency.path)
                .and_then(ProcessorAssetInfo::processed_info)
                .map(|info| info.full_hash)
                == Some(dependency.full_hash)
        })
    }

    /// Drops `asset_path` from the `dependents` sets of its old dependencies, so
    /// a stale edge does not re-queue it after they change.
    fn clear_dependencies(&mut self, asset_path: &AssetPath<'static>, removed_info: ProcessedInfo) {
        for old_dependency in removed_info.process_dependencies {
            if let Some(info) = self.infos.get_mut(&old_dependency.path) {
                info.dependents.remove(asset_path);
            } else if let Some(dependents) =
                self.non_existent_dependents.get_mut(&old_dependency.path)
            {
                dependents.remove(asset_path);
            }
        }
    }
}
