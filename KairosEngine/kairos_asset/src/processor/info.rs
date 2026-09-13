//! The in-memory processed-asset graph.
//!
//! [`ProcessorAssetInfos`] keeps one [`ProcessorAssetInfo`] per asset it has
//! seen: the [`ProcessedInfo`] it was last processed to, the set of assets that
//! depend on it (its `dependents`), and its [`ProcessStatus`]. Forward edges
//! live in [`ProcessedInfo::process_dependencies`]; the reverse `dependents`
//! edges live here, kept in sync as assets are processed, removed, and re-pointed
//! at new dependencies.
//!
//! An asset absent from the graph is treated as **non-existent**; a dependency
//! that does not exist yet is parked in `non_existent_dependents` so that when
//! it appears its dependents are still linked to it. This mirrors `bevy_asset`'s
//! structure, minus the processor's I/O, scheduling, and gated-reader state,
//! which arrive with those slices.
// Nothing outside the tests consumes this yet; the `AssetProcessor` (S5) and the
// gated reader (S7) do.
#![allow(dead_code)]

use std::collections::{HashMap, HashSet};

use crate::meta::{AssetHash, ProcessedInfo};
use crate::path::AssetPath;

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
/// relevant) by the rename handling the processor adds later.
#[derive(Debug, Default)]
pub(crate) struct ProcessorAssetInfo {
    processed_info: Option<ProcessedInfo>,
    /// Paths of assets that depend on this asset when they are being processed.
    dependents: HashSet<AssetPath<'static>>,
    status: Option<ProcessStatus>,
}

impl ProcessorAssetInfo {
    /// The info this asset was last processed to, if it has been processed.
    pub(crate) fn processed_info(&self) -> Option<&ProcessedInfo> {
        self.processed_info.as_ref()
    }

    /// The paths that depend on this asset when they are processed.
    pub(crate) fn dependents(&self) -> &HashSet<AssetPath<'static>> {
        &self.dependents
    }

    /// The asset's current [`ProcessStatus`], if it has one yet.
    pub(crate) fn status(&self) -> Option<ProcessStatus> {
        self.status
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
    fn get_or_insert(&mut self, asset_path: AssetPath<'static>) -> &mut ProcessorAssetInfo {
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

    fn get_mut(&mut self, asset_path: &AssetPath<'static>) -> Option<&mut ProcessorAssetInfo> {
        self.infos.get_mut(asset_path)
    }

    fn add_dependent(&mut self, asset_path: &AssetPath<'static>, dependent: AssetPath<'static>) {
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
    pub(crate) fn insert_processed(
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
        info.status = Some(ProcessStatus::Processed);
        info.dependents.iter().cloned().collect()
    }

    /// Marks `asset_path` as [`Failed`](ProcessStatus::Failed) to process.
    pub(crate) fn mark_failed(&mut self, asset_path: AssetPath<'static>) {
        self.get_or_insert(asset_path).status = Some(ProcessStatus::Failed);
    }

    /// Removes `asset_path` from the graph. A removed asset reads as
    /// non-existent, and its dependents are parked until it reappears.
    pub(crate) fn remove(&mut self, asset_path: &AssetPath<'static>) {
        let Some(info) = self.infos.remove(asset_path) else {
            return;
        };
        if let Some(processed_info) = info.processed_info {
            self.clear_dependencies(asset_path, processed_info);
        }
        if !info.dependents.is_empty() {
            self.non_existent_dependents
                .insert(asset_path.clone(), info.dependents);
        }
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
            } else if let Some(dependents) = self
                .non_existent_dependents
                .get_mut(&old_dependency.path)
            {
                dependents.remove(asset_path);
            }
        }
    }
}
