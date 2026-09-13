//! Tests for the [`AssetProcessor`] body: initial scanning, incremental
//! processing driven by source events, and the real `start` pipeline.
//!
//! The sources are in-memory readers and writers over a shared byte map, so the
//! tests exercise the whole processor — hashing, skipping, dependency edges,
//! delete/rename — without touching the filesystem.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use futures_io::AsyncWrite;
use futures_lite::AsyncWriteExt;
use futures_lite::future::block_on;
use kairos_ecs::error::KairosError;
use kairos_ecs::message::Messages;
use kairos_ecs::world::World;
use kairos_tasks::{BoxedFuture, ConditionalSendFuture, IoTaskPool, TaskPool};
use serde::{Deserialize, Serialize};

use crate::io::{
    AssetReader, AssetReaderError, AssetSource, AssetSourceBuilder, AssetSourceBuilders,
    AssetSourceEvent, AssetSourceId, AssetSources, AssetWatcher, AssetWriter, AssetWriterError,
    ErasedAssetReader, ErasedAssetWriter, PathStream, Reader, UnapprovedPathMode, VecReader, Writer,
    get_meta_path,
};
use crate::meta::{AssetAction, AssetActionMinimal, AssetMeta, AssetMetaCheck, AssetMetaDyn, AssetMetaMinimal};
use crate::processor::{
    AssetProcessor, LogEntry, Process, ProcessContext, ProcessError, ProcessorTransactionLog,
    ProcessorTransactionLogFactory, SetTransactionLogFactoryError,
};
use crate::{
    Asset, AssetEvent, AssetLoadFailedEvent, AssetLoader, AssetPath, AssetServer, AssetServerMode,
    Assets, LoadContext, VisitAssetDependencies, handle_internal_asset_events,
};

// ---------------------------------------------------------------------------
// In-memory asset source
// ---------------------------------------------------------------------------

/// A byte store shared by one source's reader(s) and writer(s), with a counter
/// so tests can observe that an unchanged asset is not rewritten.
#[derive(Clone, Default)]
struct MemoryStore {
    files: Arc<Mutex<HashMap<PathBuf, Vec<u8>>>>,
    writes: Arc<AtomicU64>,
}

impl MemoryStore {
    fn insert(&self, path: impl Into<PathBuf>, bytes: impl Into<Vec<u8>>) {
        self.files.lock().unwrap().insert(path.into(), bytes.into());
    }

    fn get(&self, path: &Path) -> Option<Vec<u8>> {
        self.files.lock().unwrap().get(path).cloned()
    }

    fn contains(&self, path: &Path) -> bool {
        self.files.lock().unwrap().contains_key(path)
    }

    fn remove(&self, path: &Path) {
        self.files.lock().unwrap().remove(path);
    }

    fn keys(&self) -> Vec<PathBuf> {
        self.files.lock().unwrap().keys().cloned().collect()
    }

    fn write_count(&self) -> u64 {
        self.writes.load(Ordering::Relaxed)
    }
}

/// Reads from a [`MemoryStore`], with `.meta` sidecars addressed by
/// [`get_meta_path`].
struct MemoryReader {
    store: MemoryStore,
}

impl AssetReader for MemoryReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        self.store
            .get(path)
            .map(VecReader::new)
            .ok_or_else(|| AssetReaderError::NotFound(path.to_path_buf()))
    }

    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        self.store
            .get(&get_meta_path(path))
            .map(VecReader::new)
            .ok_or_else(|| AssetReaderError::NotFound(path.to_path_buf()))
    }

    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        // Entries are source-root relative (mirroring `FileAssetReader`), so a
        // subdirectory request still yields full-relative paths. Meta sidecars
        // and hidden files are addressable but not listed, exactly as the file
        // reader skips them.
        let entries = self
            .store
            .keys()
            .into_iter()
            .filter(|key| path == Path::new("") || key.starts_with(path))
            .filter(|key| {
                let is_meta = key
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("meta"));
                let is_hidden = key
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with('.'));
                !is_meta && !is_hidden
            })
            .collect::<Vec<_>>();
        Ok(Box::new(futures_lite::stream::iter(entries)))
    }

    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
        if path == Path::new("") {
            return Ok(true);
        }
        if self.store.contains(path) {
            return Ok(false);
        }
        if self.store.keys().iter().any(|key| key.starts_with(path)) {
            return Ok(true);
        }
        Err(AssetReaderError::NotFound(path.to_path_buf()))
    }
}

/// Writes into a [`MemoryStore`].
struct MemoryWriter {
    store: MemoryStore,
}

impl AssetWriter for MemoryWriter {
    async fn write<'a>(&'a self, path: &'a Path) -> Result<Box<Writer>, AssetWriterError> {
        self.store.writes.fetch_add(1, Ordering::Relaxed);
        // Truncate, like a real file writer opening the path for writing.
        self.store
            .files
            .lock()
            .unwrap()
            .insert(path.to_path_buf(), Vec::new());
        Ok(Box::new(MemoryWriteHandle {
            store: self.store.clone(),
            path: path.to_path_buf(),
        }))
    }

    async fn write_meta<'a>(&'a self, path: &'a Path) -> Result<Box<Writer>, AssetWriterError> {
        AssetWriter::write(self, &get_meta_path(path)).await
    }

    async fn remove<'a>(&'a self, path: &'a Path) -> Result<(), AssetWriterError> {
        self.store.remove(path);
        Ok(())
    }

    async fn remove_meta<'a>(&'a self, path: &'a Path) -> Result<(), AssetWriterError> {
        AssetWriter::remove(self, &get_meta_path(path)).await
    }

    async fn rename<'a>(&'a self, old: &'a Path, new: &'a Path) -> Result<(), AssetWriterError> {
        let mut files = self.store.files.lock().unwrap();
        if let Some(bytes) = files.remove(old) {
            files.insert(new.to_path_buf(), bytes);
        }
        Ok(())
    }

    async fn rename_meta<'a>(
        &'a self,
        old: &'a Path,
        new: &'a Path,
    ) -> Result<(), AssetWriterError> {
        AssetWriter::rename(self, &get_meta_path(old), &get_meta_path(new)).await
    }

    async fn create_directory<'a>(&'a self, _path: &'a Path) -> Result<(), AssetWriterError> {
        Ok(())
    }

    async fn remove_directory<'a>(&'a self, path: &'a Path) -> Result<(), AssetWriterError> {
        self.store
            .files
            .lock()
            .unwrap()
            .retain(|key, _| !key.starts_with(path));
        Ok(())
    }

    async fn remove_empty_directory<'a>(&'a self, _path: &'a Path) -> Result<(), AssetWriterError> {
        Ok(())
    }

    async fn remove_assets_in_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<(), AssetWriterError> {
        AssetWriter::remove_directory(self, path).await
    }
}

/// Appends every write straight into the store, so a dropped sink has committed.
struct MemoryWriteHandle {
    store: MemoryStore,
    path: PathBuf,
}

impl AsyncWrite for MemoryWriteHandle {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        self.store
            .files
            .lock()
            .unwrap()
            .entry(self.path.clone())
            .or_default()
            .extend_from_slice(buf);
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

// ---------------------------------------------------------------------------
// Watcher plumbing
// ---------------------------------------------------------------------------

/// Captures the source-event [`Sender`](async_channel::Sender) a watched source
/// is built with, so tests can push events into the processor's channel.
#[derive(Clone, Default)]
struct EventSender(Arc<Mutex<Option<async_channel::Sender<AssetSourceEvent>>>>);

impl EventSender {
    fn send(&self, event: AssetSourceEvent) {
        let sender = self
            .0
            .lock()
            .unwrap()
            .clone()
            .expect("the source is watched");
        block_on(sender.send(event)).expect("the event channel is open");
    }
}

/// A watcher handle with no behavior; the test drives events through
/// [`EventSender`] instead.
struct TestWatcher;

impl AssetWatcher for TestWatcher {}

// ---------------------------------------------------------------------------
// In-memory transaction log
// ---------------------------------------------------------------------------

/// A [`ProcessorTransactionLogFactory`] backed by in-memory entry lists, so
/// tests never touch the filesystem and can simulate a previous run that
/// crashed mid-transaction.
#[derive(Clone, Default)]
struct TestLogFactory {
    /// The entries a previous run left behind; returned by `read`.
    previous: Arc<Mutex<Vec<LogEntry>>>,
    /// The entries written by this run; recorded by the created log.
    written: Arc<Mutex<Vec<LogEntry>>>,
}

impl ProcessorTransactionLogFactory for TestLogFactory {
    fn read(&self) -> BoxedFuture<'_, Result<Vec<LogEntry>, KairosError>> {
        let entries = self.previous.lock().unwrap().clone();
        Box::pin(async move { Ok(entries) })
    }

    fn create_new_log(
        &self,
    ) -> BoxedFuture<'_, Result<Box<dyn ProcessorTransactionLog>, KairosError>> {
        self.written.lock().unwrap().clear();
        let written = self.written.clone();
        Box::pin(async move {
            Ok(Box::new(TestTransactionLog { written }) as Box<dyn ProcessorTransactionLog>)
        })
    }
}

/// A [`ProcessorTransactionLog`] that records entries in the factory's `written`
/// list.
struct TestTransactionLog {
    written: Arc<Mutex<Vec<LogEntry>>>,
}

impl ProcessorTransactionLog for TestTransactionLog {
    fn begin_processing<'a>(
        &'a mut self,
        asset: &'a AssetPath<'_>,
    ) -> BoxedFuture<'a, Result<(), KairosError>> {
        self.written
            .lock()
            .unwrap()
            .push(LogEntry::BeginProcessing(asset.clone_owned()));
        Box::pin(async move { Ok(()) })
    }

    fn end_processing<'a>(
        &'a mut self,
        asset: &'a AssetPath<'_>,
    ) -> BoxedFuture<'a, Result<(), KairosError>> {
        self.written
            .lock()
            .unwrap()
            .push(LogEntry::EndProcessing(asset.clone_owned()));
        Box::pin(async move { Ok(()) })
    }

    fn unrecoverable(&mut self) -> BoxedFuture<'_, Result<(), KairosError>> {
        self.written
            .lock()
            .unwrap()
            .push(LogEntry::UnrecoverableError);
        Box::pin(async move { Ok(()) })
    }
}

// ---------------------------------------------------------------------------
// A text asset, loader, and processor
// ---------------------------------------------------------------------------

/// An asset whose value is the text it was loaded from.
#[derive(Debug, PartialEq, Eq)]
struct TextAsset(String);

impl Asset for TextAsset {}
impl VisitAssetDependencies for TextAsset {}

/// Loads the reader's bytes as text.
struct TextLoader;

impl AssetLoader for TextLoader {
    type Asset = TextAsset;
    type Settings = ();
    type Error = std::io::Error;

    fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext,
    ) -> impl ConditionalSendFuture<Output = Result<TextAsset, std::io::Error>> {
        async move {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;
            Ok(TextAsset(String::from_utf8_lossy(&bytes).into_owned()))
        }
    }

    fn extensions(&self) -> &[&str] {
        &["txt"]
    }
}

/// The processor's per-asset settings: a prefix prepended to the source text.
#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
struct TextSettings {
    prefix: String,
}

/// Prefixes the source text and writes it back out.
struct TextProcessor;

impl Process for TextProcessor {
    type Settings = TextSettings;
    type OutputLoader = TextLoader;

    fn process(
        &self,
        context: &mut ProcessContext,
        settings: &Self::Settings,
        writer: &mut Writer,
    ) -> impl ConditionalSendFuture<Output = Result<(), ProcessError>> {
        async move {
            let loaded = context.load_source_asset::<TextLoader>(&()).await?;
            let value = loaded
                .get::<TextAsset>()
                .expect("the source loader produced a TextAsset");
            let text = format!("{}{}", settings.prefix, value.0);
            writer.write_all(text.as_bytes()).await.map_err(|err| {
                ProcessError::AssetWriterError {
                    path: context.path().clone(),
                    err: err.into(),
                }
            })?;
            Ok(())
        }
    }
}

/// Serializes a source `.meta` that processes `path` with [`TextProcessor`] and
/// `prefix`.
fn process_meta(prefix: &str) -> Vec<u8> {
    let meta = AssetMeta::<(), TextSettings>::new(AssetAction::Process {
        processor: core::any::type_name::<TextProcessor>().to_string(),
        settings: TextSettings {
            prefix: prefix.to_string(),
        },
    });
    AssetMetaDyn::serialize(&meta)
}

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

/// A processor over one in-memory source, with the unprocessed and processed
/// halves exposed for assertions.
struct Harness {
    processor: AssetProcessor,
    /// The frozen sources shared with the app-facing server (layout ②).
    sources: Arc<AssetSources>,
    unprocessed: MemoryStore,
    processed: MemoryStore,
    events: EventSender,
    log_factory: TestLogFactory,
}

impl Harness {
    fn new() -> Self {
        Self::with_stores(
            MemoryStore::default(),
            MemoryStore::default(),
            TestLogFactory::default(),
        )
    }

    /// Builds a processor over the given stores and log factory, so a test can
    /// reuse the on-disk state of a previous processor (a restart).
    fn with_stores(
        unprocessed: MemoryStore,
        processed: MemoryStore,
        log_factory: TestLogFactory,
    ) -> Self {
        let events = EventSender::default();

        let source_reader = unprocessed.clone();
        let source_writer = unprocessed.clone();
        let processed_reader = processed.clone();
        let processed_writer = processed.clone();
        let watched_events = events.clone();

        let builder = AssetSourceBuilder::new(move || {
            Box::new(MemoryReader {
                store: source_reader.clone(),
            }) as Box<dyn ErasedAssetReader>
        })
        .with_writer(move |_create_root| {
            Some(Box::new(MemoryWriter {
                store: source_writer.clone(),
            }) as Box<dyn ErasedAssetWriter>)
        })
        .with_processed_reader(move || {
            Box::new(MemoryReader {
                store: processed_reader.clone(),
            }) as Box<dyn ErasedAssetReader>
        })
        .with_processed_writer(move |_create_root| {
            Some(Box::new(MemoryWriter {
                store: processed_writer.clone(),
            }) as Box<dyn ErasedAssetWriter>)
        })
        .with_watcher(move |sender| {
            *watched_events.0.lock().unwrap() = Some(sender);
            Some(Box::new(TestWatcher) as Box<dyn AssetWatcher>)
        });

        let mut builders = AssetSourceBuilders::default();
        builders.insert(AssetSourceId::Default, builder);
        let (processor, sources) = AssetProcessor::new(&mut builders, false);

        processor
            .data()
            .set_log_factory(Box::new(log_factory.clone()))
            .expect("the log factory is set before the processor starts");

        processor.server().register_loader(TextLoader);
        processor.register_processor(TextProcessor);
        processor.set_default_processor::<TextProcessor>("txt");

        Self {
            processor,
            sources,
            unprocessed,
            processed,
            events,
            log_factory,
        }
    }

    /// A fresh processor over the same stores, as if the process had restarted
    /// with `previous` as the transaction log the crashed run left behind.
    fn restart_with_log(&self, previous: Vec<LogEntry>) -> Self {
        let log_factory = TestLogFactory::default();
        *log_factory.previous.lock().unwrap() = previous;
        Self::with_stores(
            self.unprocessed.clone(),
            self.processed.clone(),
            log_factory,
        )
    }

    fn source(&self) -> &AssetSource {
        self.processor
            .get_source(AssetSourceId::Default)
            .expect("the default source is registered")
    }

    /// Runs the initial scan and processing pass to completion.
    fn run_initial(&self) {
        block_on(self.processor.run_initial_processing());
    }

    /// Routes one source event through the processor's handler and drains any
    /// tasks it queued.
    fn handle(&self, event: AssetSourceEvent) {
        block_on(self.processor.handle_event_for_test(self.source(), event));
    }

    /// Builds the app-facing server for layout ②: Processed mode over the
    /// processor's sources, sharing its loaders (exactly what `install` does).
    fn main_server(&self) -> AssetServer {
        AssetServer::new_sharing_loaders_with(
            self.processor.server(),
            self.sources.clone(),
            AssetServerMode::Processed,
            AssetMetaCheck::Always,
            false,
            UnapprovedPathMode::Forbid,
        )
    }
}

/// Polls `condition` until it holds, panicking after a generous timeout. Used by
/// the tests that exercise the real background `start` pipeline.
fn wait_for(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if condition() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("condition was not met before the timeout");
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn initial_processing_processes_existing_assets() {
    let harness = Harness::new();
    harness.unprocessed.insert("model.txt", b"hello".to_vec());

    harness.run_initial();

    assert_eq!(
        harness.processed.get(Path::new("model.txt")),
        Some(b"hello".to_vec())
    );
    // The processed side's `.meta` names the output loader, never the processor.
    let meta = harness
        .processed
        .get(Path::new("model.txt.meta"))
        .expect("the processor wrote a processed meta");
    let minimal = AssetMetaMinimal::deserialize(&meta).expect("the processed meta is valid ron");
    assert!(
        matches!(minimal.asset, AssetActionMinimal::Load { .. }),
        "a processed meta's action is always a load"
    );
}

#[test]
fn a_source_meta_configures_the_processor() {
    let harness = Harness::new();
    harness.unprocessed.insert("model.txt", b"hello".to_vec());
    harness
        .unprocessed
        .insert("model.txt.meta", process_meta("> "));

    harness.run_initial();

    assert_eq!(
        harness.processed.get(Path::new("model.txt")),
        Some(b"> hello".to_vec())
    );
}

#[test]
fn modified_assets_are_reprocessed() {
    let harness = Harness::new();
    harness.unprocessed.insert("model.txt", b"first".to_vec());
    harness.run_initial();
    assert_eq!(
        harness.processed.get(Path::new("model.txt")),
        Some(b"first".to_vec())
    );

    harness.unprocessed.insert("model.txt", b"second".to_vec());
    harness.handle(AssetSourceEvent::ModifiedAsset(PathBuf::from("model.txt")));

    assert_eq!(
        harness.processed.get(Path::new("model.txt")),
        Some(b"second".to_vec())
    );
}

#[test]
fn added_assets_are_processed() {
    let harness = Harness::new();
    harness.run_initial();
    assert!(!harness.processed.contains(Path::new("fresh.txt")));

    harness.unprocessed.insert("fresh.txt", b"new".to_vec());
    harness.handle(AssetSourceEvent::AddedAsset(PathBuf::from("fresh.txt")));

    assert_eq!(
        harness.processed.get(Path::new("fresh.txt")),
        Some(b"new".to_vec())
    );
}

#[test]
fn removed_assets_delete_their_processed_output() {
    let harness = Harness::new();
    harness.unprocessed.insert("model.txt", b"bye".to_vec());
    harness.run_initial();
    assert!(harness.processed.contains(Path::new("model.txt")));

    harness.unprocessed.remove(Path::new("model.txt"));
    harness.handle(AssetSourceEvent::RemovedAsset(PathBuf::from("model.txt")));

    assert!(!harness.processed.contains(Path::new("model.txt")));
    assert!(!harness.processed.contains(Path::new("model.txt.meta")));
}

#[test]
fn renamed_assets_move_their_processed_output() {
    let harness = Harness::new();
    harness.unprocessed.insert("old.txt", b"content".to_vec());
    harness.run_initial();
    assert!(harness.processed.contains(Path::new("old.txt")));

    // The source is renamed underneath the processor.
    harness.unprocessed.remove(Path::new("old.txt"));
    harness.unprocessed.insert("new.txt", b"content".to_vec());
    harness.handle(AssetSourceEvent::RenamedAsset {
        old: PathBuf::from("old.txt"),
        new: PathBuf::from("new.txt"),
    });

    assert!(!harness.processed.contains(Path::new("old.txt")));
    assert!(!harness.processed.contains(Path::new("old.txt.meta")));
    assert_eq!(
        harness.processed.get(Path::new("new.txt")),
        Some(b"content".to_vec())
    );
}

#[test]
fn unchanged_assets_are_not_rewritten() {
    let harness = Harness::new();
    harness.unprocessed.insert("model.txt", b"stable".to_vec());
    harness.run_initial();
    let writes = harness.processed.write_count();
    assert!(writes > 0, "the first pass wrote the asset");

    // A spurious modify event with unchanged bytes must skip processing.
    harness.handle(AssetSourceEvent::ModifiedAsset(PathBuf::from("model.txt")));

    assert_eq!(
        harness.processed.write_count(),
        writes,
        "an unchanged asset is not rewritten"
    );
}

#[test]
fn a_removed_unknown_resolves_to_a_removed_asset() {
    let harness = Harness::new();
    harness
        .unprocessed
        .insert("model.txt", b"gone soon".to_vec());
    harness.run_initial();
    assert!(harness.processed.contains(Path::new("model.txt")));

    harness.unprocessed.remove(Path::new("model.txt"));
    harness.handle(AssetSourceEvent::RemovedUnknown {
        path: PathBuf::from("model.txt"),
        is_meta: false,
    });

    assert!(!harness.processed.contains(Path::new("model.txt")));
}

#[test]
fn start_scans_and_reacts_to_watcher_events() {
    let harness = Harness::new();
    harness.unprocessed.insert("boot.txt", b"boot".to_vec());

    let mut world = World::default();
    world.insert_resource(harness.processor.clone());
    AssetProcessor::start(&mut world);

    // The startup scan processes the existing asset.
    wait_for(|| harness.processed.get(Path::new("boot.txt")) == Some(b"boot".to_vec()));

    // A watcher event on the unprocessed side drives incremental processing.
    harness.unprocessed.insert("boot.txt", b"booted".to_vec());
    harness
        .events
        .send(AssetSourceEvent::ModifiedAsset(PathBuf::from("boot.txt")));
    wait_for(|| harness.processed.get(Path::new("boot.txt")) == Some(b"booted".to_vec()));

    // And a removal deletes the processed output.
    harness.unprocessed.remove(Path::new("boot.txt"));
    harness
        .events
        .send(AssetSourceEvent::RemovedAsset(PathBuf::from("boot.txt")));
    wait_for(|| !harness.processed.contains(Path::new("boot.txt")));
}

#[test]
fn startup_recovery_reprocesses_unfinished_transactions() {
    let first = Harness::new();
    first.unprocessed.insert("recover.txt", b"recover".to_vec());
    first.unprocessed.insert("keep.txt", b"keep".to_vec());
    first.run_initial();
    assert_eq!(
        first.processed.get(Path::new("recover.txt")),
        Some(b"recover".to_vec())
    );
    assert_eq!(
        first.processed.get(Path::new("keep.txt")),
        Some(b"keep".to_vec())
    );

    // Simulate a crash mid-write: both processed payloads are torn, but only
    // `recover.txt`'s transaction was left open without an `End`.
    first.processed.insert("recover.txt", b"CORRUPT".to_vec());
    first.processed.insert("keep.txt", b"CORRUPT".to_vec());

    let restarted = first.restart_with_log(vec![
        LogEntry::BeginProcessing(AssetPath::from("recover.txt")),
        LogEntry::BeginProcessing(AssetPath::from("keep.txt")),
        LogEntry::EndProcessing(AssetPath::from("keep.txt")),
    ]);
    restarted.run_initial();

    // The unfinished asset's output was deleted and regenerated from source...
    assert_eq!(
        restarted.processed.get(Path::new("recover.txt")),
        Some(b"recover".to_vec())
    );
    // ...while the completed transaction's output was left exactly as it was:
    // recovery only removes assets whose transaction never ended.
    assert_eq!(
        restarted.processed.get(Path::new("keep.txt")),
        Some(b"CORRUPT".to_vec())
    );

    // The fresh log records exactly the one reprocess as a balanced transaction.
    assert_eq!(
        *restarted.log_factory.written.lock().unwrap(),
        vec![
            LogEntry::BeginProcessing(AssetPath::from("recover.txt")),
            LogEntry::EndProcessing(AssetPath::from("recover.txt")),
        ],
    );
}

#[test]
fn an_unrecoverable_log_invalidates_every_processed_asset() {
    let first = Harness::new();
    first.unprocessed.insert("a.txt", b"a".to_vec());
    first.unprocessed.insert("b.txt", b"b".to_vec());
    first.run_initial();

    // Both outputs are torn, so a rebuild is the only safe response.
    first.processed.insert("a.txt", b"CORRUPT".to_vec());
    first.processed.insert("b.txt", b"CORRUPT".to_vec());

    let restarted = first.restart_with_log(vec![LogEntry::UnrecoverableError]);
    restarted.run_initial();

    // The whole processed folder was dropped and regenerated, not trusted.
    assert_eq!(
        restarted.processed.get(Path::new("a.txt")),
        Some(b"a".to_vec())
    );
    assert_eq!(
        restarted.processed.get(Path::new("b.txt")),
        Some(b"b".to_vec())
    );
}

#[test]
fn setting_the_log_factory_after_start_is_rejected() {
    let harness = Harness::new();
    harness.run_initial();

    assert_eq!(
        harness
            .processor
            .data()
            .set_log_factory(Box::new(TestLogFactory::default())),
        Err(SetTransactionLogFactoryError::AlreadyInUse)
    );
}

// ---------------------------------------------------------------------------
// Processor gating (layout ②)
// ---------------------------------------------------------------------------

/// A sidecar naming a processor that is never registered, so processing fails.
fn missing_processor_meta() -> Vec<u8> {
    let meta = AssetMeta::<(), ()>::new(AssetAction::Process {
        processor: "kairos_asset::tests::asset_processor::NeverRegistered".to_string(),
        settings: (),
    });
    AssetMetaDyn::serialize(&meta)
}

/// A world holding `server` and a [`TextAsset`] store, so a load can be driven
/// to completion through the tracking half of the pipeline.
fn text_world(server: &AssetServer) -> World {
    let assets = Assets::<TextAsset>::default();
    server.register_asset(&assets);
    let mut world = World::new();
    world.insert_resource(assets);
    world.insert_resource(server.clone());
    world.insert_resource(Messages::<AssetEvent<TextAsset>>::default());
    world.insert_resource(Messages::<AssetLoadFailedEvent<TextAsset>>::default());
    world
}

/// Drives the tracking stage until the load for `id` settles.
fn wait_for_load(
    world: &mut World,
    server: &AssetServer,
    id: impl Into<crate::UntypedAssetId>,
) {
    let id = id.into();
    for _ in 0..5000 {
        handle_internal_asset_events(world);
        if server.load_state(id).is_failed() || server.is_loaded_with_dependencies(id) {
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("asset {id:?} never settled: {:?}", server.load_state(id));
}

#[test]
fn gated_reader_serves_the_processed_output() {
    let harness = Harness::new();
    harness.unprocessed.insert("model.txt", b"hello".to_vec());
    harness.unprocessed.insert("model.txt.meta", process_meta("P:"));
    harness.run_initial();

    let reader = harness
        .source()
        .processed_reader()
        .expect("the source is gated");
    let mut bytes = Vec::new();
    block_on(async {
        let mut asset = reader
            .read(Path::new("model.txt"))
            .await
            .expect("the processed read resolves");
        asset.read_to_end(&mut bytes).await.expect("read to end");
    });
    assert_eq!(bytes, b"P:hello");
}

#[test]
fn main_server_reads_the_processor_output_end_to_end() {
    let harness = Harness::new();
    harness.unprocessed.insert("model.txt", b"hello".to_vec());
    harness.unprocessed.insert("model.txt.meta", process_meta("P:"));
    harness.run_initial();

    // The app-facing server (as `install` builds it in layout ②) reads the
    // processor's output, waiting on the gate rather than reading the source.
    let server = harness.main_server();
    let mut world = text_world(&server);
    let handle = server.load::<TextAsset>("model.txt");
    wait_for_load(&mut world, &server, handle.id());

    assert_eq!(
        world.resource::<Assets<TextAsset>>().get(handle.id()),
        Some(&TextAsset("P:hello".to_string()))
    );
}

/// A unique directory under the system temp dir, removed first so a rerun starts
/// clean.
fn file_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "kairos_asset_next_processor_{name}_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The real-filesystem counterpart of
/// [`main_server_reads_the_processor_output_end_to_end`]: a `platform_default`
/// source over absolute temp roots, so the product actually lands under
/// `imported_assets/Default` and is loaded from disk through layout ②.
#[test]
fn file_source_writes_the_product_under_imported_assets_and_layout_2_loads_it() {
    let root = file_dir("layout2");
    let unprocessed = root.join("res");
    let processed = root.join("imported_assets/Default");
    std::fs::create_dir_all(&unprocessed).unwrap();
    std::fs::write(unprocessed.join("model.txt"), b"hello").unwrap();
    std::fs::write(unprocessed.join("model.txt.meta"), process_meta("P:")).unwrap();

    // The platform-default file source, rooted at the temp dirs.
    let builder = AssetSourceBuilder::platform_default(
        unprocessed.to_str().unwrap(),
        Some(processed.to_str().unwrap()),
    );
    let mut builders = AssetSourceBuilders::default();
    builders.insert(AssetSourceId::Default, builder);
    let (processor, sources) = AssetProcessor::new(&mut builders, false);

    // Keep the transaction log in memory so the test writes nothing outside its
    // temp tree.
    processor
        .data()
        .set_log_factory(Box::new(TestLogFactory::default()))
        .expect("the log factory is set before the processor starts");
    processor.server().register_loader(TextLoader);
    processor.register_processor(TextProcessor);
    processor.set_default_processor::<TextProcessor>("txt");

    // The fix: the default source now reports itself processed and exposes both
    // the unprocessed and processed writers.
    let source = processor
        .get_source(AssetSourceId::Default)
        .expect("the default source is registered");
    assert!(source.should_process());
    assert!(source.writer().is_ok());
    assert!(source.processed_writer().is_ok());

    block_on(processor.run_initial_processing());

    // The product and its sidecar landed under `imported_assets/Default`.
    assert_eq!(
        std::fs::read(processed.join("model.txt")).unwrap(),
        b"P:hello"
    );
    assert!(processed.join("model.txt.meta").is_file());

    // The app-facing server (layout ②) loads the on-disk product end-to-end.
    let server = AssetServer::new_sharing_loaders_with(
        processor.server(),
        sources.clone(),
        AssetServerMode::Processed,
        AssetMetaCheck::Always,
        false,
        UnapprovedPathMode::Forbid,
    );
    let mut world = text_world(&server);
    let handle = server.load::<TextAsset>("model.txt");
    wait_for_load(&mut world, &server, handle.id());

    assert_eq!(
        world.resource::<Assets<TextAsset>>().get(handle.id()),
        Some(&TextAsset("P:hello".to_string()))
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// The default source's unprocessed root is the working directory and the
/// processed root lives inside it (ADR 0004), so the processor must skip its own
/// output. Without the exclusion a second pass would scan
/// `imported_assets/Default/...` as source and nest a copy of the whole tree.
#[test]
fn nested_processed_root_is_not_scanned_as_source() {
    let root = file_dir("nested_layout");
    let processed = root.join("imported_assets/Default");
    std::fs::create_dir_all(root.join("res")).unwrap();
    std::fs::write(root.join("res/model.txt"), b"hello").unwrap();
    std::fs::write(root.join("res/model.txt.meta"), process_meta("P:")).unwrap();

    // The unprocessed root is the whole temp tree, with the processed root nested
    // under it - exactly the default source's shape.
    let builder = AssetSourceBuilder::platform_default(
        root.to_str().unwrap(),
        Some(processed.to_str().unwrap()),
    );
    let mut builders = AssetSourceBuilders::default();
    builders.insert(AssetSourceId::Default, builder);
    let (processor, _sources) = AssetProcessor::new(&mut builders, false);

    processor
        .data()
        .set_log_factory(Box::new(TestLogFactory::default()))
        .expect("the log factory is set before the processor starts");
    processor.server().register_loader(TextLoader);
    processor.register_processor(TextProcessor);
    processor.set_default_processor::<TextProcessor>("txt");

    let source = processor
        .get_source(AssetSourceId::Default)
        .expect("the default source is registered");
    assert_eq!(
        source.unprocessed_exclude(),
        Some(Path::new("imported_assets"))
    );

    block_on(processor.run_initial_processing());
    assert_eq!(
        std::fs::read(processed.join("res/model.txt")).unwrap(),
        b"P:hello"
    );

    // The exclusion kept the product out of the source view: only the real source
    // path is known, not the copy the first pass wrote under the processed root
    // (which a second pass with no exclusion would treat as a source and reprocess
    // into `imported_assets/Default/imported_assets/...`).
    let infos = block_on(processor.data().processing_state.asset_infos.read());
    let product_path = AssetPath::from(PathBuf::from("imported_assets/Default/res/model.txt"))
        .with_source(AssetSourceId::Default);
    assert!(
        infos.get(&product_path).is_none(),
        "the processed root was scanned as source content"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn main_server_reports_a_missing_asset_as_a_failed_load() {
    let harness = Harness::new();
    harness.run_initial();

    let server = harness.main_server();
    let mut world = text_world(&server);
    let handle = server.load::<TextAsset>("ghost.txt");
    wait_for_load(&mut world, &server, handle.id());

    assert!(
        server.load_state(handle.id()).is_failed(),
        "a non-existent asset is a failed load, not a hang"
    );
}

#[test]
fn main_server_reports_a_failed_asset_as_a_failed_load() {
    let harness = Harness::new();
    harness.unprocessed.insert("bad.txt", b"bad".to_vec());
    harness
        .unprocessed
        .insert("bad.txt.meta", missing_processor_meta());
    harness.run_initial();

    let server = harness.main_server();
    let mut world = text_world(&server);
    let handle = server.load::<TextAsset>("bad.txt");
    wait_for_load(&mut world, &server, handle.id());

    assert!(
        server.load_state(handle.id()).is_failed(),
        "a failed processed asset is reported, not read"
    );
}

#[test]
fn gated_read_of_an_ignored_asset_reports_not_found() {
    let harness = Harness::new();
    harness.unprocessed.insert("skip.txt", b"skip".to_vec());
    let meta = AssetMeta::<(), ()>::new(AssetAction::Ignore);
    harness
        .unprocessed
        .insert("skip.txt.meta", AssetMetaDyn::serialize(&meta));
    harness.run_initial();

    // An ignored asset produces no processed output; the gate must resolve to a
    // clear "not found" rather than wait forever.
    let sources = harness.sources.clone();
    let result = Arc::new(Mutex::new(None::<bool>));
    let task_result = result.clone();
    IoTaskPool::get_or_init(TaskPool::default)
        .spawn(async move {
            let source = sources
                .get(AssetSourceId::Default)
                .expect("the default source exists");
            let read = source
                .processed_reader()
                .expect("the source is gated")
                .read(Path::new("skip.txt"))
                .await;
            *task_result.lock().unwrap() = Some(matches!(read, Err(AssetReaderError::NotFound(_))));
        })
        .detach();

    wait_for(|| result.lock().unwrap().is_some());
    assert_eq!(*result.lock().unwrap(), Some(true));
}

#[test]
fn a_gated_read_waits_until_the_asset_is_processed() {
    let harness = Harness::new();
    harness.unprocessed.insert("model.txt", b"hello".to_vec());
    harness.unprocessed.insert("model.txt.meta", process_meta("P:"));

    let sources = harness.sources.clone();
    let result = Arc::new(Mutex::new(None::<Vec<u8>>));
    let task_result = result.clone();
    IoTaskPool::get_or_init(TaskPool::default)
        .spawn(async move {
            let source = sources
                .get(AssetSourceId::Default)
                .expect("the default source exists");
            let mut asset = source
                .processed_reader()
                .expect("the source is gated")
                .read(Path::new("model.txt"))
                .await
                .expect("the read resolves once processed");
            let mut bytes = Vec::new();
            asset.read_to_end(&mut bytes).await.expect("read to end");
            *task_result.lock().unwrap() = Some(bytes);
        })
        .detach();

    // Nothing has been processed yet, so the gated read cannot resolve.
    std::thread::sleep(Duration::from_millis(50));
    assert!(
        result.lock().unwrap().is_none(),
        "the gated read returned before processing finished"
    );

    harness.run_initial();
    wait_for(|| result.lock().unwrap().is_some());
    assert_eq!(
        result.lock().unwrap().as_deref(),
        Some(b"P:hello".as_slice())
    );
}

#[test]
fn a_held_gated_reader_blocks_a_concurrent_rewrite() {
    let harness = Harness::new();
    harness.unprocessed.insert("model.txt", b"hello".to_vec());
    harness.run_initial();

    // Taking a gated read acquires the asset's file transaction lock for as long
    // as the reader lives.
    let reader = block_on(
        harness
            .source()
            .processed_reader()
            .expect("the source is gated")
            .read(Path::new("model.txt")),
    )
    .expect("the processed read resolves");

    let asset_path = AssetPath::from(PathBuf::from("model.txt"));
    let lock = {
        let infos = block_on(harness.processor.data().processing_state.asset_infos.read());
        infos
            .get(&asset_path)
            .expect("the asset is in the graph")
            .file_transaction_lock()
    };

    // The processor's write side cannot be taken while the reader lives, so it
    // cannot rewrite (or half-write) the bytes the reader is streaming.
    assert!(
        block_on(futures_lite::future::poll_once(lock.write())).is_none(),
        "a held reader must block a rewrite"
    );

    // Dropping the reader releases the lock for the writer.
    drop(reader);
    let _write = block_on(lock.write());
}
