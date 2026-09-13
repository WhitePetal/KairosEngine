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
//! [`AssetSources`] and loaders; layout ② wires them together through
//! [`AssetProcessor::new`], which gates every source's processed reader on this
//! processor's [`ProcessingState`] before the app's server reads them.
//!
//! The processor's own reads of processed files go through the
//! [`ungated_processed_reader`](AssetSource::ungated_processed_reader) (the
//! original reader kept alongside the gated one), so it never waits on its own
//! output.
//!
//! The processor's own write-ahead log ([`ProcessorTransactionLog`] and the
//! [`FileTransactionLogFactory`] that backs it) makes processing transactional:
//! [`initialize`](AssetProcessor::initialize) validates the previous run's log
//! and recovers any transaction that did not finish, and every processing pass
//! brackets its writes with begin/end entries.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

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
    AssetActionMinimal, AssetMetaCheck, AssetMetaMinimal, ProcessedInfo, ProcessedInfoMinimal,
    get_asset_hash, get_full_asset_hash,
};
use crate::path::AssetPath;
use crate::server::{AssetServer, AssetServerMode};

use super::info::{ProcessStatus, ProcessorAssetInfos};
use super::log::{
    FileTransactionLogFactory, LogEntry, LogEntryError, ProcessorTransactionLog,
    ProcessorTransactionLogFactory, SetTransactionLogFactoryError, ValidateLogError, WriteLogError,
    validate_transaction_log,
};
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
    /// The factory that builds the transaction log.
    ///
    /// A plain [`Mutex`], not an async one: the factory is set once, before the
    /// processor starts, and no guard is ever held across an `await`.
    log_factory: Mutex<Option<Box<dyn ProcessorTransactionLogFactory>>>,
    /// The active transaction log, created once [`AssetProcessor::initialize`]
    /// has validated and recovered the previous run's log.
    log: async_lock::RwLock<Option<Box<dyn ProcessorTransactionLog>>>,
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
    /// The receiving end of the initialization broadcast; cloned by
    /// [`ProcessingState::wait_until_initialized`].
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
    /// The asset's `.meta` says [`Ignore`](crate::meta::AssetAction::Ignore).
    Ignored,
}

/// An error from [`AssetProcessor::initialize`].
#[derive(Debug, thiserror::Error)]
pub enum InitializeError {
    /// Reading the unprocessed folder failed.
    #[error(transparent)]
    FailedToReadSourcePaths(AssetReaderError),
    /// Reading the processed folder failed.
    #[error(transparent)]
    FailedToReadDestinationPaths(AssetReaderError),
    /// Validating the previous run's transaction log failed.
    #[error("Failed to validate asset log: {0}")]
    ValidateLogError(#[from] ValidateLogError),
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
        let mut sources = sources.build_sources(true, watch_processed);
        // Gate every processed source's reader on this processor's state: the
        // app's server (layout ②) waits for the processor's output instead of
        // reading a partial file, while the original reader is kept as the
        // ungated copy the processor reads itself.
        sources.gate_on_processor(state.clone());
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

    /// Logs an unrecoverable error. On the next run of the processor, all
    /// assets will be regenerated. This should only be used as a last resort.
    /// Every call to this should be considered with scrutiny and ideally
    /// replaced with something more granular.
    async fn log_unrecoverable(&self) {
        let mut log = self.data.log.write().await;
        let log = log.as_mut().expect("the transaction log is started");
        log.unrecoverable()
            .await
            .map_err(|error| WriteLogError {
                log_entry: LogEntry::UnrecoverableError,
                error,
            })
            .unwrap();
    }

    /// Logs the start of an asset being processed. If this is not followed at
    /// some point by a closing [`AssetProcessor::log_end_processing`], the next
    /// run of the processor treats the asset as incompletely processed and
    /// reprocesses it.
    async fn log_begin_processing(&self, path: &AssetPath<'_>) {
        let mut log = self.data.log.write().await;
        let log = log.as_mut().expect("the transaction log is started");
        log.begin_processing(path)
            .await
            .map_err(|error| WriteLogError {
                log_entry: LogEntry::BeginProcessing(path.clone_owned()),
                error,
            })
            .unwrap();
    }

    /// Logs the end of an asset being successfully processed. See
    /// [`AssetProcessor::log_begin_processing`].
    async fn log_end_processing(&self, path: &AssetPath<'_>) {
        let mut log = self.data.log.write().await;
        let log = log.as_mut().expect("the transaction log is started");
        log.end_processing(path)
            .await
            .map_err(|error| WriteLogError {
                log_entry: LogEntry::EndProcessing(path.clone_owned()),
                error,
            })
            .unwrap();
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
                        // The processor's server has no `Assets` store, so it
                        // flushes its handle-drop bookkeeping here instead of
                        // through `track_assets`.
                        self.server.write_infos().consume_handle_drop_events();
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
    /// This first validates the previous run's transaction log and recovers any
    /// half-finished transactions (see
    /// [`validate_transaction_log_and_recover`](AssetProcessor::validate_transaction_log_and_recover)),
    /// then scans: unprocessed files are recorded as existing; a processed file
    /// whose metadata parses has its [`ProcessedInfo`] and dependency edges
    /// restored, and a processed file whose metadata is missing or unparsable
    /// (or whose source is gone) is deleted so it can be regenerated.
    async fn initialize(&self) -> Result<(), InitializeError> {
        self.validate_transaction_log_and_recover().await;
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
            let Some(processed_reader) = source.ungated_processed_reader() else {
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
                // The processed root can sit inside the unprocessed root (ADR
                // 0004 roots the default source at the working directory), so
                // drop this source's own output before it is mistaken for source
                // content.
                if source.is_excluded_from_unprocessed(&path) {
                    continue;
                }
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
        if Self::event_is_excluded(source, &event) {
            return;
        }
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
                let Some(processed_reader) = source.ungated_processed_reader() else {
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
        if source.is_excluded_from_unprocessed(&path) {
            return Ok(());
        }
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
        let Some(processed_reader) = source.ungated_processed_reader() else {
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
            Err(_) => {
                // The processed folder could not be read, so the in-memory view
                // can no longer be trusted: mark the run unrecoverable so the
                // next start regenerates everything.
                self.log_unrecoverable().await;
            }
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
            infos.remove(&asset_path).await
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

    /// Whether an event names a path inside the source's processed subtree.
    ///
    /// A rename is skipped if either side lies in the subtree: the scan-based initial
    /// pass is the authority on what counts as source content, so no watcher event
    /// may drag processed output back in as a source.
    fn event_is_excluded(source: &AssetSource, event: &AssetSourceEvent) -> bool {
        let excluded = |path: &Path| source.is_excluded_from_unprocessed(path);
        match event {
            AssetSourceEvent::AddedAsset(path)
            | AssetSourceEvent::ModifiedAsset(path)
            | AssetSourceEvent::RemovedAsset(path)
            | AssetSourceEvent::AddedMeta(path)
            | AssetSourceEvent::ModifiedMeta(path)
            | AssetSourceEvent::RemovedMeta(path)
            | AssetSourceEvent::AddedFolder(path)
            | AssetSourceEvent::RemovedFolder(path)
            | AssetSourceEvent::RemovedUnknown { path, .. } => excluded(path),
            AssetSourceEvent::RenamedAsset { old, new }
            | AssetSourceEvent::RenamedMeta { old, new }
            | AssetSourceEvent::RenamedFolder { old, new } => excluded(old) || excluded(new),
        }
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
                        let loader = server
                            .get_asset_loader_with_type_name(&loader)
                            .await
                            .map_err(crate::AssetLoadError::from)?;
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
                    match server.get_path_asset_loader(asset_path).await {
                        Ok(loader) => (loader.default_meta(), None),
                        // Nothing claims the source, so it is ignored - exactly
                        // what an explicit `AssetAction::Ignore` means. Falling
                        // through to the copy below would turn every unclaimed
                        // file in the source tree into a product, and
                        // `finish_processing` would then record it as processed
                        // instead of non-existent (ADR 0005 deviation 10).
                        Err(_) => return Ok(ProcessResult::Ignored),
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

        // Bracketing the write with begin/end entries is what makes a crash
        // mid-write detectable: the next run deletes any output whose `Begin`
        // never got its `End`.
        self.log_begin_processing(asset_path).await;

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

        self.log_end_processing(asset_path).await;

        Ok(ProcessResult::Processed(new_processed_info))
    }

    /// Validates the previous run's transaction log and recovers from any
    /// transactions that did not complete, then starts a fresh log.
    ///
    /// A half-finished transaction means the processor crashed (or failed)
    /// between a `Begin` and its matching `End`: the asset's processed bytes and
    /// `.meta` may be torn, so they are deleted here and regenerated by the
    /// ordinary initial pass, rather than being trusted because the asset was
    /// "already processed".
    ///
    /// Anything unrecoverable — an unreadable log, an explicit unrecoverable
    /// entry, or a log that is not a valid begin/end sequence — invalidates the
    /// whole processed folder so a full rebuild replaces any partial output.
    async fn validate_transaction_log_and_recover(&self) {
        let log_factory = self
            .data
            .log_factory
            .lock()
            .expect("the transaction log factory lock poisoned")
            // Taking the factory marks startup as done, so a factory can no
            // longer be swapped in.
            .take()
            .expect("the asset processor only starts once");

        if let Err(err) = validate_transaction_log(log_factory.as_ref()).await {
            let state_is_valid = match err {
                ValidateLogError::ReadLogError(_) | ValidateLogError::UnrecoverableError => false,
                ValidateLogError::EntryErrors(entry_errors) => {
                    let mut state_is_valid = true;
                    for entry_error in entry_errors {
                        match entry_error {
                            LogEntryError::DuplicateTransaction(_)
                            | LogEntryError::EndedMissingTransaction(_) => {
                                state_is_valid = false;
                                break;
                            }
                            LogEntryError::UnfinishedTransaction(path) => {
                                let Ok(source) = self.get_source(path.source()) else {
                                    state_is_valid = false;
                                    continue;
                                };
                                let Ok(processed_writer) = source.processed_writer() else {
                                    state_is_valid = false;
                                    continue;
                                };
                                // NotFound is fine (the crash may have happened
                                // before the write); any other failure means the
                                // processed folder cannot be made consistent.
                                for result in [
                                    processed_writer.remove(path.path()).await,
                                    processed_writer.remove_meta(path.path()).await,
                                ] {
                                    if let Err(AssetWriterError::Io(err)) = result
                                        && err.kind() != ErrorKind::NotFound
                                    {
                                        state_is_valid = false;
                                    }
                                }
                            }
                        }
                    }
                    state_is_valid
                }
            };

            if !state_is_valid {
                // The log cannot be trusted, so every processed asset is dropped
                // and regenerated instead of serving a possibly-torn output.
                for source in self.sources().iter_processed() {
                    let Ok(processed_writer) = source.processed_writer() else {
                        continue;
                    };
                    processed_writer
                        .remove_assets_in_directory(Path::new(""))
                        .await
                        .expect(
                            "processed assets were in a bad state and could not be removed to \
                             restart from scratch",
                        );
                }
            }
        }

        let mut log = self.data.log.write().await;
        *log = Some(
            log_factory
                .create_new_log()
                .await
                .expect("failed to initialize the asset processor transaction log"),
        );
    }
}

impl AssetProcessorData {
    /// Creates the shared data for a processor over `sources`.
    pub(crate) fn new(sources: Arc<AssetSources>, processing_state: Arc<ProcessingState>) -> Self {
        Self {
            processing_state,
            // The default log is file-backed; hosts can replace it before start.
            log_factory: Mutex::new(Some(Box::new(FileTransactionLogFactory::default()))),
            log: Default::default(),
            processors: RwLock::new(Processors::default()),
            sources,
        }
    }

    /// Sets the transaction log factory for the processor.
    ///
    /// If this is called after asset processing has begun (in the `Startup`
    /// stage), it returns an error and does nothing. If it is never called, the
    /// default file-backed log is used.
    pub fn set_log_factory(
        &self,
        factory: Box<dyn ProcessorTransactionLogFactory>,
    ) -> Result<(), SetTransactionLogFactoryError> {
        let mut log_factory = self
            .log_factory
            .lock()
            .expect("the transaction log factory lock poisoned");
        if log_factory.is_none() {
            // This indicates the asset processor has already started, so setting
            // the factory does nothing here.
            return Err(SetTransactionLogFactoryError::AlreadyInUse);
        }

        *log_factory = Some(factory);
        Ok(())
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

    /// Waits until initialization has finished, so a gate never reads a
    /// half-built in-memory view.
    pub(crate) async fn wait_until_initialized(&self) {
        let receiver = {
            let state = self.state.read().await;
            match *state {
                ProcessorState::Initializing => Some(self.initialized_receiver.clone()),
                ProcessorState::Processing | ProcessorState::Finished => None,
            }
        };

        if let Some(mut receiver) = receiver {
            let _ = receiver.recv().await;
        }
    }

    /// Waits until `path` reaches a final [`ProcessStatus`], then returns it.
    ///
    /// An asset absent from the graph is [`NonExistent`](ProcessStatus::NonExistent).
    /// This is what the gated reader blocks on before opening a processed file.
    pub(crate) async fn wait_until_processed(&self, path: AssetPath<'static>) -> ProcessStatus {
        self.wait_until_initialized().await;

        // Hold the lock while checking the status and cloning the receiver: if
        // the status flips in between, the broadcast would be missed.
        let mut receiver = {
            let infos = self.asset_infos.write().await;
            match infos.get(&path) {
                Some(info) => match info.status() {
                    Some(status) => return status,
                    None => info.status_receiver(),
                },
                None => return ProcessStatus::NonExistent,
            }
        };

        receiver
            .recv()
            .await
            .unwrap_or(ProcessStatus::NonExistent)
    }

    /// The read guard for `path`'s file transaction lock.
    ///
    /// The gated reader holds this for as long as it is reading, so the
    /// processor cannot overwrite the asset's bytes or `.meta` underneath it. A
    /// path absent from the graph has no output to lock, so this is
    /// [`NotFound`](AssetReaderError::NotFound).
    pub(crate) async fn get_transaction_lock(
        &self,
        path: &AssetPath<'static>,
    ) -> Result<async_lock::RwLockReadGuardArc<()>, AssetReaderError> {
        // Clone the lock out before awaiting it: holding the graph read lock
        // while waiting for one path's transaction lock could block every other
        // path (and deadlock against a writer).
        let lock = {
            let infos = self.asset_infos.read().await;
            let info = infos
                .get(path)
                .ok_or_else(|| AssetReaderError::NotFound(path.path().to_owned()))?;
            info.file_transaction_lock()
        };
        Ok(lock.read_arc().await)
    }

    /// Waits until the processor has finished its current pass.
    pub(crate) async fn wait_until_finished(&self) {
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
        self.drain_queue(&new_task_sender, &new_task_receiver)
            .await;
    }
}

impl AssetProcessor {
    /// Runs the startup pipeline to completion without starting the long-lived
    /// event listeners, so a caller can drive processing deterministically:
    /// initialize, queue every source asset, then process the queue (and any
    /// dependents it re-queues) in order. Mirrors [`AssetProcessor::start`].
    ///
    /// This is the deterministic entry point for tests and for tools that want
    /// to process on demand rather than on the processor's background schedule.
    pub async fn run_initial_processing(&self) {
        self.initialize()
            .await
            .expect("the asset processor failed to initialize");
        let (new_task_sender, new_task_receiver) = async_channel::unbounded();
        self.queue_initial_processing_tasks(&new_task_sender).await;
        self.drain_queue(&new_task_sender, &new_task_receiver)
            .await;
    }

    /// Processes queued tasks until the queue is empty, keeping a strong sender
    /// alive so dependent assets can be re-queued.
    async fn drain_queue(
        &self,
        sender: &async_channel::Sender<(AssetSourceId<'static>, PathBuf)>,
        receiver: &async_channel::Receiver<(AssetSourceId<'static>, PathBuf)>,
    ) {
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
