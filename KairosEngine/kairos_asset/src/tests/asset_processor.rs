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
use futures_lite::future::block_on;
use futures_lite::AsyncWriteExt;
use kairos_ecs::world::World;
use kairos_tasks::ConditionalSendFuture;
use serde::{Deserialize, Serialize};

use crate::io::{
    AssetReader, AssetReaderError, AssetSource, AssetSourceBuilder, AssetSourceBuilders,
    AssetSourceEvent, AssetSourceId, AssetWatcher, AssetWriter, AssetWriterError,
    ErasedAssetReader, ErasedAssetWriter, PathStream, Reader, VecReader, Writer, get_meta_path,
};
use crate::meta::{AssetAction, AssetActionMinimal, AssetMeta, AssetMetaDyn, AssetMetaMinimal};
use crate::processor::{AssetProcessor, Process, ProcessContext, ProcessError};
use crate::{Asset, AssetLoader, LoadContext, VisitAssetDependencies};

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
        self.files
            .lock()
            .unwrap()
            .insert(path.into(), bytes.into());
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
        // subdirectory request still yields full-relative paths.
        let entries = self
            .store
            .keys()
            .into_iter()
            .filter(|key| path == Path::new("") || key.starts_with(path))
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

    async fn rename_meta<'a>(&'a self, old: &'a Path, new: &'a Path) -> Result<(), AssetWriterError> {
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
        let sender = self.0.lock().unwrap().clone().expect("the source is watched");
        block_on(sender.send(event)).expect("the event channel is open");
    }
}

/// A watcher handle with no behavior; the test drives events through
/// [`EventSender`] instead.
struct TestWatcher;

impl AssetWatcher for TestWatcher {}

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
            writer
                .write_all(text.as_bytes())
                .await
                .map_err(|err| ProcessError::AssetWriterError {
                    path: context.path().clone(),
                    err: err.into(),
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
    unprocessed: MemoryStore,
    processed: MemoryStore,
    events: EventSender,
}

impl Harness {
    fn new() -> Self {
        let unprocessed = MemoryStore::default();
        let processed = MemoryStore::default();
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
        let (processor, _sources) = AssetProcessor::new(&mut builders, false);

        processor.server().register_loader(TextLoader);
        processor.register_processor(TextProcessor);
        processor.set_default_processor::<TextProcessor>("txt");

        Self {
            processor,
            unprocessed,
            processed,
            events,
        }
    }

    fn source(&self) -> &AssetSource {
        self.processor
            .get_source(AssetSourceId::Default)
            .expect("the default source is registered")
    }

    /// Runs the initial scan and processing pass to completion.
    fn run_initial(&self) {
        block_on(self.processor.run_initial_processing_for_test());
    }

    /// Routes one source event through the processor's handler and drains any
    /// tasks it queued.
    fn handle(&self, event: AssetSourceEvent) {
        block_on(self.processor.handle_event_for_test(self.source(), event));
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
    harness.unprocessed.insert("model.txt.meta", process_meta("> "));

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
    harness.unprocessed.insert("model.txt", b"gone soon".to_vec());
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
    harness.events.send(AssetSourceEvent::ModifiedAsset(PathBuf::from("boot.txt")));
    wait_for(|| harness.processed.get(Path::new("boot.txt")) == Some(b"booted".to_vec()));

    // And a removal deletes the processed output.
    harness.unprocessed.remove(Path::new("boot.txt"));
    harness.events.send(AssetSourceEvent::RemovedAsset(PathBuf::from("boot.txt")));
    wait_for(|| !harness.processed.contains(Path::new("boot.txt")));
}
