//! The [`AssetProcessor`] body: the background task that turns source assets
//! into processed ones.
//!
//! This is the port of `bevy_asset` 0.19.1's `AssetProcessor` that kairos's
//! rewrite deferred (ADR 0005). It is **not** an ECS system: its only mount
//! point is a one-shot [`AssetProcessor::start`], which the caller schedules in
//! its `Startup` stage. Everything after that runs on
//! [`IoTaskPool`](kairos_tasks::IoTaskPool) as an async loop.
//!
//! [`AssetProcessor::start`] runs three phases:
//!
//! 1. [`initialize`](AssetProcessor::initialize) scans every processed source's
//!    unprocessed and processed folders to build the in-memory picture, then
//!    [`queue_initial_processing_tasks`](AssetProcessor::queue_initial_processing_tasks)
//!    queues every source asset.
//! 2. [`execute_processing_tasks`](AssetProcessor::execute_processing_tasks)
//!    drains the queue, spawning one `process_asset` task per entry and
//!    re-queueing the dependents of anything it (re)processes.
//! 3. [`spawn_source_change_event_listeners`](AssetProcessor::spawn_source_change_event_listeners)
//!    watches the unprocessed side of every processed source: added/modified
//!    assets are queued for processing, removed ones have their processed output
//!    deleted, and renamed ones have it moved.
//!
//! The processor runs its own [`AssetServer`] (mode
//! [`Processed`](AssetServerMode::Processed), `.meta` always checked) so its id
//! space is independent of the app's server. The two share the same
//! [`AssetSources`] and loaders; the decision's layout ② wires them together in
//! a later slice.
//!
//! Two later slices extend this body and are deliberately absent here:
//!
//! - the write-ahead log and its startup recovery (`ProcessorTransactionLog`,
//!   `validate_transaction_log_and_recover`), and
//! - the gated reader (`gate_on_processor` / `ProcessorGatedReader`), which is
//!   why the processor reads processed files through the plain
//!   [`processed_reader`](AssetSource::processed_reader) rather than an ungated
//!   copy of it. With gating in place those self-reads move to
//!   [`ungated_processed_reader`](AssetSource::ungated_processed_reader) so the
//!   processor does not wait on its own output.

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use futures_lite::{AsyncWriteExt, StreamExt};
use futures_util::{FutureExt, select_biased};
use kairos_ecs::resource::Resource;
use kairos_ecs::world::World;
use kairos_tasks::{IoTaskPool, TaskPool};

use crate::io::{
    AssetReaderError, AssetSource, AssetSourceBuilders, AssetSourceEvent, AssetSourceId,
    AssetSources, AssetWriterError, ErasedAssetReader, MissingAssetSourceError,
    UnapprovedPathMode,
};
use crate::meta::{
    AssetAction, AssetActionMinimal, AssetMeta, AssetMetaCheck, AssetMetaDyn, AssetMetaMinimal,
    ProcessedInfo, ProcessedInfoMinimal, get_asset_hash, get_full_asset_hash,
};
use crate::path::AssetPath;
use crate::server::{AssetServer, AssetServerMode};

use super::info::ProcessorAssetInfos;
use super::process::{
    ErasedProcessor, MetaTypePathKind, Process, ProcessContext, ProcessError,
};
use super::registry::{GetProcessorError, Processors};

/// A "background" asset processor: it reads source assets from each processed
/// [`AssetSource`], transforms them, and writes the results to the same source's
/// processed root.
///
/// It is a [`Clone`]able [`Resource`] backed by an [`Arc`], so clones share
/// state and may be used in parallel. Mount it once with
/// [`AssetProcessor::start`] in the host's `Startup` stage.
#[derive(Resource, Clone)]
pub struct AssetProcessor {
    server: AssetServer,
    pub(crate) data: Arc<AssetProcessorData>,
}

/// The shared state behind every [`AssetProcessor`] clone.
pub struct AssetProcessorData {
    /// The overall processing state and the per-asset graph.
    pub(crate) processing_state: Arc<ProcessingState>,
    /// The registered processors, behind a plain lock: registration and lookup
    /// are synchronous, and no guard is ever held across an `await`.
    processors: RwLock<Processors>,
    /// The sources the processor reads from and writes to.
    sources: Arc<AssetSources>,
}

/// The processor's overall state plus the per-asset graph.
pub(crate) struct ProcessingState {
    /// Where the processor is in its lifecycle.
    state: async_lock::RwLock<ProcessorState>,
    /// Broadcast when initialization completes (consumed by the gated reader).
    initialized_sender: async_broadcast::Sender<()>,
    /// Reserved for the gated reader; nothing reads it until that slice lands.
    #[allow(dead_code)]
    initialized_receiver: async_broadcast::Receiver<()>,
    /// Broadcast when a processing pass finishes.
    finished_sender: async_broadcast::Sender<()>,
    finished_receiver: async_broadcast::Receiver<()>,
    /// The "current" in-memory view of the processed asset space.
    pub(crate) asset_infos: async_lock::RwLock<ProcessorAssetInfos>,
}

/// The current state of the [`AssetProcessor`].
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ProcessorState {
    /// Scanning the current asset folders, building the in-memory view, and
    /// cleaning up stale processed files.
    Initializing,
    /// Processing assets.
    Processing,
    /// All valid assets have been processed (or reported as invalid).
    Finished,
}

/// The (successful) result of processing one asset.
#[derive(Debug, Clone)]
pub enum ProcessResult {
    /// The asset was processed; its new info is recorded.
    Processed(ProcessedInfo),
    /// The asset was left alone because neither it nor its dependencies changed.
    SkippedNotChanged,
    /// The asset's `.meta` says [`Ignore`](AssetAction::Ignore).
    Ignored,
}

/// An error from [`AssetProcessor::initialize`].
#[derive(Debug)]
pub enum InitializeError {
    /// Reading the unprocessed folder failed.
    FailedToReadSourcePaths(AssetReaderError),
    /// Reading the processed folder failed.
    FailedToReadDestinationPaths(AssetReaderError),
}

impl core::fmt::Display for InitializeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::FailedToReadSourcePaths(error) => {
                write!(f, "failed to read the source asset folder: {error}")
            }
            Self::FailedToReadDestinationPaths(error) => {
                write!(f, "failed to read the processed asset folder: {error}")
            }
        }
    }
}

impl std::error::Error for InitializeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::FailedToReadSourcePaths(error) | Self::FailedToReadDestinationPaths(error) => {
                Some(error)
            }
        }
    }
}

impl AssetProcessor {
    /// Creates a processor over `sources`, binding the processor and the
    /// app's server to the same frozen [`AssetSources`].
    ///
    /// The returned `Arc<AssetSources>` is what the app's [`AssetServer`] should
    /// read in layout ② (a later slice wires that up), so both servers share the
    /// same readers, writers, watchers, and loaders while keeping independent id
    /// spaces.
    ///
    /// `watch_processed` controls whether the processed side is watched; the
    /// unprocessed side is always watched, because that is where the processor's
    /// change events come from.
    pub fn new(
        sources: &mut AssetSourceBuilders,
        watch_processed: bool,
    ) -> (Self, Arc<AssetSources>) {
        let state = Arc::new(ProcessingState::new());
        let sources = sources.build_sources(true, watch_processed);
        // The gating slice wraps each source's processed reader here, so the app
        // server waits for the processor's output instead of reading a partial
        // file. Until then the processor reads the processed side directly.
        let sources = Arc::new(sources);

        let data = Arc::new(AssetProcessorData::new(sources.clone(), state));
        // The processor's own server has a different id space from the app's.
        let server = AssetServer::new_with_meta_check(
            sources.clone(),
            AssetServerMode::Processed,
            AssetMetaCheck::Always,
            false,
            UnapprovedPathMode::default(),
        );
        (Self { server, data }, sources)
    }

    /// The shared processor data.
    pub fn data(&self) -> &Arc<AssetProcessorData> {
        &self.data
    }

    /// The processor's internal [`AssetServer`].
    ///
    /// This is *separate* from the app's server: it carries the
    /// processor-specific configuration and its own id space.
    pub fn server(&self) -> &AssetServer {
        &self.server
    }

    /// The processor's current [`ProcessorState`].
    pub async fn get_state(&self) -> ProcessorState {
        self.data.processing_state.get_state().await
    }

    /// The [`AssetSource`] named by `id`.
    pub fn get_source<'a>(
        &self,
        id: impl Into<AssetSourceId<'a>>,
    ) -> Result<&AssetSource, MissingAssetSourceError> {
        self.data.sources.get(id)
    }

    /// Every source the processor reads from and writes to.
    pub fn sources(&self) -> &AssetSources {
        &self.data.sources
    }

    /// Registers a new processor.
    pub fn register_processor<P: Process>(&self, processor: P) {
        self.data
            .processors
            .write()
            .expect("processor registry lock poisoned")
            .register_processor(processor);
    }

    /// Makes `P` the default processor for `extension`. `P` must have been
    /// registered with [`AssetProcessor::register_processor`].
    pub fn set_default_processor<P: Process>(&self, extension: &str) {
        self.data
            .processors
            .write()
            .expect("processor registry lock poisoned")
            .set_default_processor::<P>(extension);
    }

    /// The default processor for `extension`, if one is registered.
    pub fn get_default_processor(&self, extension: &str) -> Option<Arc<dyn ErasedProcessor>> {
        self.data
            .processors
            .read()
            .expect("processor registry lock poisoned")
            .get_default_processor(extension)
    }

    /// The processor named `processor_type_name` (full name or unambiguous short
    /// name).
    pub fn get_processor(
        &self,
        processor_type_name: &str,
    ) -> Result<Arc<dyn ErasedProcessor>, GetProcessorError> {
        self.data
            .processors
            .read()
            .expect("processor registry lock poisoned")
            .get_processor(processor_type_name)
    }

    /// Starts the processor in the background.
    ///
    /// This is the processor's only ECS mount point: schedule it in the host's
    /// `Startup` stage as an exclusive system. It clones the processor out of the
    /// world and hands the whole pipeline to [`IoTaskPool`]; after it returns,
    /// nothing else touches the processor from ECS.
    pub fn start(world: &mut World) {
        let processor = world.resource::<AssetProcessor>().clone();
        io_task_pool()
            .spawn(async move {
                processor
                    .initialize()
                    .await
                    .expect("the asset processor failed to initialize");

                let (new_task_sender, new_task_receiver) = async_channel::unbounded();
                processor
                    .queue_initial_processing_tasks(&new_task_sender)
                    .await;

                // The task executor outlives the initial queue: once the listeners
                // are running they keep feeding it. It downgrades its own sender so
                // that it does not keep the channel alive by itself.
                {
                    let processor = processor.clone();
                    let new_task_sender = new_task_sender.clone();
                    io_task_pool()
                        .spawn(async move {
                            processor
                                .execute_processing_tasks(new_task_sender, new_task_receiver)
                                .await;
                        })
                        .detach();
                }

                processor.data.wait_until_finished().await;
                processor.spawn_source_change_event_listeners(&new_task_sender);
            })
            .detach();
    }

    /// Sends an initial processing task for every asset in every processed
    /// source into `sender`.
    async fn queue_initial_processing_tasks(
        &self,
        sender: &async_channel::Sender<(AssetSourceId<'static>, PathBuf)>,
    ) {
        for source in self.sources().iter_processed() {
            self.queue_processing_tasks_for_folder(source, PathBuf::from(""), sender)
                .await
                .expect("failed to enumerate a source folder");
        }
    }

    /// Spawns a listener per processed source that turns source-change events
    /// into processing tasks.
    fn spawn_source_change_event_listeners(
        &self,
        sender: &async_channel::Sender<(AssetSourceId<'static>, PathBuf)>,
    ) {
        for source in self.data.sources.iter_processed() {
            let Some(receiver) = source.event_receiver().cloned() else {
                continue;
            };
            let source_id = source.id();
            let processor = self.clone();
            let sender = sender.clone();
            io_task_pool()
                .spawn(async move {
                    while let Ok(event) = receiver.recv().await {
                        let Ok(source) = processor.get_source(source_id.clone()) else {
                            return;
                        };
                        processor
                            .handle_asset_source_event(source, event, &sender)
                            .await;
                    }
                })
                .detach();
        }
    }

    /// Drains `new_task_receiver`, spawning one `process_asset` task per entry
    /// and tracking the processor's overall state.
    ///
    /// This future runs until the channel closes, not merely until it is empty,
    /// so it stays alive while the source-event listeners keep feeding it. It
    /// downgrades its sender so that it does not keep the channel alive on its
    /// own; processing tasks upgrade it again when they queue dependents.
    async fn execute_processing_tasks(
        &self,
        new_task_sender: async_channel::Sender<(AssetSourceId<'static>, PathBuf)>,
        new_task_receiver: async_channel::Receiver<(AssetSourceId<'static>, PathBuf)>,
    ) {
        let new_task_sender = {
            let weak_sender = new_task_sender.downgrade();
            drop(new_task_sender);
            weak_sender
        };

        // With an empty queue and no producers, go straight to `Finished`;
        // otherwise the processor would sit in `Processing` forever.
        if new_task_receiver.is_empty() {
            self.data
                .processing_state
                .set_state(ProcessorState::Finished)
                .await;
        }

        enum ProcessorTaskEvent {
            Start(AssetSourceId<'static>, PathBuf),
            Finished,
        }

        let (task_finished_sender, task_finished_receiver) = async_channel::unbounded::<()>();
        let mut pending_tasks = 0usize;

        while let Ok(event) = {
            // Biased towards starting tasks: otherwise we could mark the processor
            // finished while tasks are still queued.
            select_biased! {
                result = new_task_receiver.recv().fuse() => {
                    result.map(|(source_id, path)| ProcessorTaskEvent::Start(source_id, path))
                },
                result = task_finished_receiver.recv().fuse() => {
                    result.map(|()| ProcessorTaskEvent::Finished)
                }
            }
        } {
            match event {
                ProcessorTaskEvent::Start(source_id, path) => {
                    let Some(new_task_sender) = new_task_sender.upgrade() else {
                        // No task producers remain (the sources are gone), so the
                        // task cannot be run and can be dropped.
                        continue;
                    };
                    let processor = self.clone();
                    let task_finished_sender = task_finished_sender.clone();
                    pending_tasks += 1;
                    io_task_pool()
                        .spawn(async move {
                            // `task_finished_sender` must fire on every path, or
                            // `pending_tasks` never reaches zero and
                            // `wait_until_finished` hangs.
                            if let Ok(source) = processor.get_source(source_id) {
                                processor
                                    .process_asset(source, path, new_task_sender)
                                    .await;
                            }
                            let _ = task_finished_sender.send(()).await;
                        })
                        .detach();
                    self.data
                        .processing_state
                        .set_state(ProcessorState::Processing)
                        .await;
                }
                ProcessorTaskEvent::Finished => {
                    pending_tasks -= 1;
                    if pending_tasks == 0 {
                        self.data
                            .processing_state
                            .set_state(ProcessorState::Finished)
                            .await;
                    }
                }
            }
        }
    }

    /// Builds the initial in-memory view by scanning every processed source's
    /// unprocessed and processed folders.
    ///
    /// Unprocessed files are recorded as existing; a processed file whose
    /// metadata parses has its [`ProcessedInfo`] and dependency edges restored,
    /// and a processed file whose metadata is missing or unparsable (or whose
    /// source is gone) is deleted so it can be regenerated.
    async fn initialize(&self) -> Result<(), InitializeError> {
        let mut asset_infos = self.data.processing_state.asset_infos.write().await;

        /// Recursively collects the file paths under `path`. When `empty_dirs` is
        /// given, empty directories are recorded (deepest first) so the caller
        /// can clean them up.
        async fn get_asset_paths(
            reader: &dyn ErasedAssetReader,
            path: PathBuf,
            paths: &mut Vec<PathBuf>,
            mut empty_dirs: Option<&mut Vec<PathBuf>>,
        ) -> Result<bool, AssetReaderError> {
            if reader.is_directory(&path).await? {
                let mut path_stream = reader.read_directory(&path).await?;
                let mut contains_files = false;
                while let Some(child_path) = path_stream.next().await {
                    contains_files |= Box::pin(get_asset_paths(
                        reader,
                        child_path,
                        paths,
                        empty_dirs.as_deref_mut(),
                    ))
                    .await?;
                }
                if !contains_files && path.parent().is_some() && let Some(empty_dirs) = empty_dirs {
                    empty_dirs.push(path);
                }
                Ok(contains_files)
            } else {
                paths.push(path);
                Ok(true)
            }
        }

        for source in self.sources().iter_processed() {
            let Ok(processed_reader) = source.processed_reader() else {
                continue;
            };
            let Ok(processed_writer) = source.processed_writer() else {
                continue;
            };

            let mut unprocessed_paths = Vec::new();
            get_asset_paths(
                source.reader(),
                PathBuf::from(""),
                &mut unprocessed_paths,
                None,
            )
            .await
            .map_err(InitializeError::FailedToReadSourcePaths)?;

            let mut processed_paths = Vec::new();
            let mut empty_dirs = Vec::new();
            get_asset_paths(
                processed_reader,
                PathBuf::from(""),
                &mut processed_paths,
                Some(&mut empty_dirs),
            )
            .await
            .map_err(InitializeError::FailedToReadDestinationPaths)?;

            // Clean up after collecting every path: removing directories while
            // still iterating the stream would skip entries.
            for empty_dir in empty_dirs {
                let _ = processed_writer.remove_empty_directory(&empty_dir).await;
            }

            for path in unprocessed_paths {
                asset_infos.get_or_insert(AssetPath::from(path).with_source(source.id()));
            }

            for path in processed_paths {
                let mut dependencies = Vec::new();
                let asset_path = AssetPath::from(path).with_source(source.id());
                if let Some(info) = asset_infos.get_mut(&asset_path) {
                    match processed_reader.read_meta_bytes(asset_path.path()).await {
                        Ok(meta_bytes) => match ProcessedInfoMinimal::deserialize(&meta_bytes) {
                            Ok(minimal) => {
                                if let Some(processed_info) = &minimal.processed_info {
                                    for dependency in &processed_info.process_dependencies {
                                        dependencies.push(dependency.path.clone());
                                    }
                                }
                                info.set_processed_info(minimal.processed_info);
                            }
                            Err(_) => {
                                self.remove_processed_asset_and_meta(source, asset_path.path())
                                    .await;
                            }
                        },
                        Err(_) => {
                            self.remove_processed_asset_and_meta(source, asset_path.path())
                                .await;
                        }
                    }
                } else {
                    self.remove_processed_asset_and_meta(source, asset_path.path())
                        .await;
                }

                for dependency in dependencies {
                    asset_infos.add_dependent(&dependency, asset_path.clone());
                }
            }
        }

        self.data
            .processing_state
            .set_state(ProcessorState::Processing)
            .await;

        Ok(())
    }

    /// Handles one source-change event.
    ///
    /// Added/modified assets are queued for processing; removed ones have their
    /// processed output deleted; renamed ones have it moved; a removed unknown is
    /// resolved by probing the processed side.
    async fn handle_asset_source_event(
        &self,
        source: &AssetSource,
        event: AssetSourceEvent,
        new_task_sender: &async_channel::Sender<(AssetSourceId<'static>, PathBuf)>,
    ) {
        match event {
            AssetSourceEvent::AddedAsset(path)
            | AssetSourceEvent::AddedMeta(path)
            | AssetSourceEvent::ModifiedAsset(path)
            | AssetSourceEvent::ModifiedMeta(path) => {
                let _ = new_task_sender.send((source.id(), path)).await;
            }
            AssetSourceEvent::RemovedAsset(path) => {
                self.handle_removed_asset(source, path).await;
            }
            AssetSourceEvent::RemovedMeta(path) => {
                // The meta may be regenerated, and the user may be re-adding the
                // asset, so the asset itself is not deleted: only reprocessed.
                let _ = new_task_sender.send((source.id(), path)).await;
            }
            AssetSourceEvent::AddedFolder(path) => {
                self.handle_added_folder(source, path, new_task_sender)
                    .await;
            }
            AssetSourceEvent::RemovedFolder(path) => {
                self.handle_removed_folder(source, &path).await;
            }
            AssetSourceEvent::RenamedAsset { old, new } => {
                if old == new {
                    // Sometimes a rename event fires when a path is moved "back"
                    // into the source; reprocess instead of moving output.
                    let _ = new_task_sender.send((source.id(), new)).await;
                } else {
                    self.handle_renamed_asset(source, old, new, new_task_sender)
                        .await;
                }
            }
            AssetSourceEvent::RenamedMeta { old, new } => {
                if old == new {
                    let _ = new_task_sender.send((source.id(), new)).await;
                } else {
                    // A meta rename does not imply the asset was renamed; check
                    // both paths so new meta can be generated where needed.
                    let _ = new_task_sender.send((source.id(), old)).await;
                    let _ = new_task_sender.send((source.id(), new)).await;
                }
            }
            AssetSourceEvent::RenamedFolder { old, new } => {
                if old == new {
                    self.handle_added_folder(source, new, new_task_sender)
                        .await;
                } else {
                    // This reprocesses everything in the moved folder; matching
                    // moved subtrees is not possible while `AssetPath` erases
                    // relativeness.
                    self.handle_removed_folder(source, &old).await;
                    self.handle_added_folder(source, new, new_task_sender)
                        .await;
                }
            }
            AssetSourceEvent::RemovedUnknown { path, is_meta } => {
                let Ok(processed_reader) = source.processed_reader() else {
                    return;
                };
                match processed_reader.is_directory(&path).await {
                    Ok(is_directory) => {
                        if is_directory {
                            self.handle_removed_folder(source, &path).await;
                        } else if is_meta {
                            let _ = new_task_sender.send((source.id(), path)).await;
                        } else {
                            self.handle_removed_asset(source, path).await;
                        }
                    }
                    Err(AssetReaderError::NotFound(_)) => {
                        // No processed version exists, so there is nothing to do.
                    }
                    Err(_) => {}
                }
            }
        }
    }

    /// Queues every asset under `path` for processing.
    async fn handle_added_folder(
        &self,
        source: &AssetSource,
        path: PathBuf,
        new_task_sender: &async_channel::Sender<(AssetSourceId<'static>, PathBuf)>,
    ) {
        let _ = self
            .queue_processing_tasks_for_folder(source, path, new_task_sender)
            .await;
    }

    /// Recursively queues a folder's files, or the file itself if `path` is one.
    async fn queue_processing_tasks_for_folder(
        &self,
        source: &AssetSource,
        path: PathBuf,
        new_task_sender: &async_channel::Sender<(AssetSourceId<'static>, PathBuf)>,
    ) -> Result<(), AssetReaderError> {
        if source.reader().is_directory(&path).await? {
            let mut path_stream = source.reader().read_directory(&path).await?;
            while let Some(path) = path_stream.next().await {
                Box::pin(self.queue_processing_tasks_for_folder(source, path, new_task_sender))
                    .await?;
            }
        } else {
            let _ = new_task_sender.send((source.id(), path)).await;
        }
        Ok(())
    }

    /// Removes every processed file stored under `path`, then the folder itself.
    async fn handle_removed_folder(&self, source: &AssetSource, path: &Path) {
        let Ok(processed_reader) = source.processed_reader() else {
            return;
        };
        match processed_reader.read_directory(path).await {
            Ok(mut path_stream) => {
                while let Some(child_path) = path_stream.next().await {
                    self.handle_removed_asset(source, child_path).await;
                }
            }
            Err(AssetReaderError::NotFound(_)) => {
                // The processed folder does not exist; nothing to update.
            }
            Err(_) => {}
        }

        // Best-effort: a folder that is not there (or could not be removed) is
        // not an error the processor can act on.
        if let Ok(processed_writer) = source.processed_writer() {
            let _ = processed_writer.remove_directory(path).await;
        }
    }

    /// Removes an asset's processed output and its in-memory record, waiting for
    /// in-flight access first.
    async fn handle_removed_asset(&self, source: &AssetSource, path: PathBuf) {
        let asset_path = AssetPath::from(path).with_source(source.id());
        let lock = {
            // Scope the graph lock so it is not held across the file operations.
            let mut infos = self.data.processing_state.asset_infos.write().await;
            infos.remove(&asset_path)
        };
        let Some(lock) = lock else {
            return;
        };

        // Wait for uncontested access so existing readers/writers can finish.
        let _write_lock = lock.write().await;
        self.remove_processed_asset_and_meta(source, asset_path.path())
            .await;
    }

    /// Moves an asset's processed output to its new path and re-points the
    /// in-memory record, waiting for in-flight access first.
    async fn handle_renamed_asset(
        &self,
        source: &AssetSource,
        old: PathBuf,
        new: PathBuf,
        new_task_sender: &async_channel::Sender<(AssetSourceId<'static>, PathBuf)>,
    ) {
        let old = AssetPath::from(old).with_source(source.id());
        let new = AssetPath::from(new).with_source(source.id());
        let Ok(processed_writer) = source.processed_writer() else {
            return;
        };

        let locks = {
            let mut infos = self.data.processing_state.asset_infos.write().await;
            infos.rename(&old, &new, new_task_sender).await
        };
        let Some((old_lock, new_lock)) = locks else {
            return;
        };

        // Wait for uncontested access to both paths before moving the files.
        let _old_write_lock = old_lock.write().await;
        let _new_write_lock = new_lock.write().await;
        let _ = processed_writer.rename(old.path(), new.path()).await;
        let _ = processed_writer.rename_meta(old.path(), new.path()).await;
    }

    /// Removes an asset's processed bytes and `.meta`, then prunes any ancestor
    /// folders left empty. Not transactional; it also leaves the in-memory record
    /// untouched, so callers update the graph themselves.
    async fn remove_processed_asset_and_meta(&self, source: &AssetSource, path: &Path) {
        if let Ok(processed_writer) = source.processed_writer() {
            let _ = processed_writer.remove(path).await;
            let _ = processed_writer.remove_meta(path).await;
        }
        self.clean_empty_processed_ancestor_folders(source, path)
            .await;
    }

    /// Removes now-empty ancestor folders of a processed file, walking upwards
    /// until a folder cannot be removed.
    async fn clean_empty_processed_ancestor_folders(
        &self,
        source: &AssetSource,
        mut path: &Path,
    ) {
        // Never delete outside the processed root.
        if path.is_absolute() {
            return;
        }
        let Ok(processed_writer) = source.processed_writer() else {
            return;
        };
        while let Some(parent) = path.parent() {
            path = parent;
            if parent == Path::new("") {
                break;
            }
            if processed_writer.remove_empty_directory(parent).await.is_err() {
                break;
            }
        }
    }

    /// Processes `path` and folds the result into the graph.
    async fn process_asset(
        &self,
        source: &AssetSource,
        path: PathBuf,
        new_task_sender: async_channel::Sender<(AssetSourceId<'static>, PathBuf)>,
    ) {
        let asset_path = AssetPath::from(path).with_source(source.id());
        let result = self.process_asset_internal(source, &asset_path).await;
        let mut infos = self.data.processing_state.asset_infos.write().await;
        infos
            .finish_processing(asset_path, result, &new_task_sender)
            .await;
    }

    /// Runs one asset through its `.meta`'s processor (or copies it verbatim when
    /// its loader is used directly), writing the processed bytes and metadata.
    async fn process_asset_internal(
        &self,
        source: &AssetSource,
        asset_path: &AssetPath<'static>,
    ) -> Result<ProcessResult, ProcessError> {
        let server = &self.server;
        let path = asset_path.path();
        let reader = source.reader();

        let reader_err = |err| ProcessError::AssetReaderError {
            path: asset_path.clone(),
            err,
        };
        let writer_err = |err| ProcessError::AssetWriterError {
            path: asset_path.clone(),
            err,
        };

        // Resolve the processor, the source `.meta`, and the bytes to hash. Meta
        // is read first so an asset with no source file is never given a meta
        // sidecar.
        let (mut source_meta, meta_bytes, processor) = match reader.read_meta_bytes(path).await {
            Ok(meta_bytes) => {
                let minimal = AssetMetaMinimal::deserialize(&meta_bytes)
                    .map_err(ProcessError::DeserializeMetaError)?;
                let (meta, processor) = match minimal.asset {
                    AssetActionMinimal::Load { loader } => {
                        let loader = server.get_asset_loader_with_type_name(&loader)?;
                        let meta = loader
                            .deserialize_meta(&meta_bytes)
                            .map_err(ProcessError::DeserializeMetaError)?;
                        (meta, None)
                    }
                    AssetActionMinimal::Process { processor } => {
                        let processor = self.get_processor(&processor)?;
                        let meta = processor
                            .deserialize_meta(&meta_bytes)
                            .map_err(ProcessError::DeserializeMetaError)?;
                        (meta, Some(processor))
                    }
                    AssetActionMinimal::Ignore => {
                        return Ok(ProcessResult::Ignored);
                    }
                };
                (meta, meta_bytes, processor)
            }
            Err(AssetReaderError::NotFound(_)) => {
                // No meta: fall back to the extension's default processor, then
                // to the loader for the extension, and finally to ignoring it.
                let (meta, processor) = if let Some(processor) = asset_path
                    .get_full_extension()
                    .and_then(|extension| self.get_default_processor(extension))
                {
                    // Long or short name is irrelevant here: the processor is
                    // returned alongside the meta and its settings are what matter.
                    let meta = processor.default_meta(MetaTypePathKind::Long);
                    (meta, Some(processor))
                } else {
                    match server.get_path_asset_loader(asset_path) {
                        Ok(loader) => (loader.default_meta(), None),
                        Err(_) => {
                            let meta: Box<dyn AssetMetaDyn> =
                                Box::new(AssetMeta::<(), ()>::new(AssetAction::Ignore));
                            (meta, None)
                        }
                    }
                };
                let meta_bytes = meta.serialize();
                (meta, meta_bytes, processor)
            }
            Err(err) => {
                return Err(ProcessError::ReadAssetMetaError {
                    path: asset_path.clone(),
                    err,
                });
            }
        };

        let processed_writer = source
            .processed_writer()
            .map_err(ProcessError::MissingProcessedAssetWriterError)?;

        // Hash the source while streaming it, without keeping the whole file.
        let new_hash = {
            let mut reader_for_hash = reader.read(path).await.map_err(&reader_err)?;
            get_asset_hash(&meta_bytes, &mut reader_for_hash)
                .await
                .map_err(&reader_err)?
        };
        let mut new_processed_info = ProcessedInfo {
            hash: new_hash,
            full_hash: new_hash,
            process_dependencies: Vec::new(),
        };

        // Skip an asset whose own bytes/meta and every dependency's `full_hash`
        // are unchanged. `ProcessedInfo` is only ever used for this check.
        if self
            .data
            .processing_state
            .asset_infos
            .read()
            .await
            .is_up_to_date(asset_path, new_hash)
        {
            return Ok(ProcessResult::SkippedNotChanged);
        }

        // Hold the per-asset write lock until the processed bytes and meta are
        // both written, so a concurrent reader cannot interleave the pair. The
        // lock is cloned out before awaiting so one path's lock never blocks
        // another path's graph access.
        let transaction_lock = {
            let mut infos = self.data.processing_state.asset_infos.write().await;
            infos
                .get_or_insert(asset_path.clone())
                .file_transaction_lock()
        };
        let _transaction_lock = transaction_lock.write().await;

        if let Some(processor) = processor {
            let settings = source_meta
                .process_settings()
                .expect("an AssetAction::Process carries its settings");

            // A fresh reader: the hash reader was scoped and dropped above.
            let reader_for_process = reader.read(path).await.map_err(&reader_err)?;
            let mut writer = processed_writer
                .write(path)
                .await
                .map_err(&writer_err)?;

            let mut processed_meta = {
                let mut context = ProcessContext::new(
                    &self.server,
                    asset_path,
                    reader_for_process,
                    &mut new_processed_info,
                );
                processor.process(&mut context, settings, &mut *writer).await?
            };

            writer.flush().await.map_err(|error| {
                ProcessError::AssetWriterError {
                    path: asset_path.clone(),
                    err: AssetWriterError::Io(error),
                }
            })?;

            let full_hash = get_full_asset_hash(
                new_hash,
                new_processed_info
                    .process_dependencies
                    .iter()
                    .map(|dependency| dependency.full_hash),
            );
            new_processed_info.full_hash = full_hash;
            *processed_meta.processed_info_mut() = Some(new_processed_info.clone());
            let meta_bytes = processed_meta.serialize();
            processed_writer
                .write_meta_bytes(path, &meta_bytes)
                .await
                .map_err(&writer_err)?;
        } else {
            // No processor: the source is copied verbatim to the processed side,
            // with the processed info recorded in a copy of its `.meta`.
            let mut reader_for_copy = reader.read(path).await.map_err(&reader_err)?;
            let mut writer = processed_writer
                .write(path)
                .await
                .map_err(&writer_err)?;
            futures_lite::io::copy(&mut reader_for_copy, &mut writer)
                .await
                .map_err(|error| ProcessError::AssetWriterError {
                    path: asset_path.clone(),
                    err: error.into(),
                })?;

            *source_meta.processed_info_mut() = Some(new_processed_info.clone());
            let meta_bytes = source_meta.serialize();
            processed_writer
                .write_meta_bytes(path, &meta_bytes)
                .await
                .map_err(&writer_err)?;
        }

        Ok(ProcessResult::Processed(new_processed_info))
    }
}

impl AssetProcessorData {
    /// Creates the shared data for a processor over `sources`.
    pub(crate) fn new(sources: Arc<AssetSources>, processing_state: Arc<ProcessingState>) -> Self {
        Self {
            processing_state,
            processors: RwLock::new(Processors::default()),
            sources,
        }
    }

    /// Waits until the processor has finished its current pass.
    pub(crate) async fn wait_until_finished(&self) {
        self.processing_state.wait_until_finished().await;
    }
}

impl ProcessingState {
    /// Creates the state in its [`Initializing`](ProcessorState::Initializing)
    /// phase, with single-slot overflow channels for the lifecycle broadcasts.
    fn new() -> Self {
        let (mut initialized_sender, initialized_receiver) = async_broadcast::broadcast(1);
        let (mut finished_sender, finished_receiver) = async_broadcast::broadcast(1);
        // Overflow lets a late receiver read the latest state instead of a
        // broadcast blocking on an unread slot.
        initialized_sender.set_overflow(true);
        finished_sender.set_overflow(true);

        Self {
            state: async_lock::RwLock::new(ProcessorState::Initializing),
            initialized_sender,
            initialized_receiver,
            finished_sender,
            finished_receiver,
            asset_infos: async_lock::RwLock::new(ProcessorAssetInfos::default()),
        }
    }

    /// Sets the overall state, broadcasting the transitions the gated reader
    /// waits on.
    async fn set_state(&self, state: ProcessorState) {
        let mut state_guard = self.state.write().await;
        let last_state = *state_guard;
        *state_guard = state;
        if last_state != ProcessorState::Finished && state == ProcessorState::Finished {
            let _ = self.finished_sender.broadcast(()).await;
        } else if last_state != ProcessorState::Processing && state == ProcessorState::Processing {
            let _ = self.initialized_sender.broadcast(()).await;
        }
    }

    /// The processor's current state.
    pub(crate) async fn get_state(&self) -> ProcessorState {
        *self.state.read().await
    }

    /// Waits until the processor has finished its current pass.
    async fn wait_until_finished(&self) {
        let receiver = {
            let state = self.state.read().await;
            match *state {
                ProcessorState::Initializing | ProcessorState::Processing => {
                    Some(self.finished_receiver.clone())
                }
                ProcessorState::Finished => None,
            }
        };

        if let Some(mut receiver) = receiver {
            let _ = receiver.recv().await;
        }
    }
}

/// The lazy default [`IoTaskPool`].
fn io_task_pool() -> &'static IoTaskPool {
    IoTaskPool::get_or_init(TaskPool::default)
}

#[cfg(test)]
impl AssetProcessor {
    /// Runs the startup pipeline to completion without starting the long-lived
    /// event listeners, so tests can drive processing deterministically:
    /// initialize, queue every source asset, then process the queue (and any
    /// dependents it re-queues) in order. Mirrors [`AssetProcessor::start`].
    pub(crate) async fn run_initial_processing_for_test(&self) {
        self.initialize()
            .await
            .expect("the asset processor failed to initialize");
        let (new_task_sender, new_task_receiver) = async_channel::unbounded();
        self.queue_initial_processing_tasks(&new_task_sender).await;
        self.drain_queue_for_test(&new_task_sender, &new_task_receiver)
            .await;
    }

    /// Runs one source event through the real handler and then processes any
    /// tasks it queued, so tests can exercise incremental processing without the
    /// background listener loop.
    pub(crate) async fn handle_event_for_test(
        &self,
        source: &AssetSource,
        event: AssetSourceEvent,
    ) {
        let (new_task_sender, new_task_receiver) = async_channel::unbounded();
        self.handle_asset_source_event(source, event, &new_task_sender)
            .await;
        self.drain_queue_for_test(&new_task_sender, &new_task_receiver)
            .await;
    }

    /// Processes queued tasks until the queue is empty, keeping a strong sender
    /// alive so dependent assets can be re-queued.
    async fn drain_queue_for_test(&self, sender: &async_channel::Sender<(AssetSourceId<'static>, PathBuf)>, receiver: &async_channel::Receiver<(AssetSourceId<'static>, PathBuf)>) {
        while !receiver.is_empty() {
            let Ok((source_id, path)) = receiver.recv().await else {
                return;
            };
            let Ok(source) = self.get_source(source_id) else {
                continue;
            };
            self.process_asset(source, path, sender.clone()).await;
        }
    }
}
