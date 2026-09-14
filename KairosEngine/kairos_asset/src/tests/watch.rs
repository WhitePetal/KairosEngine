//! Tests for the hot-reload watcher layer: the `AssetSourceEvent` drain in
//! `handle_internal_asset_events` and the end-to-end "change a source file ->
//! the loaded asset is reloaded and a `Modified` event is emitted" path.
//!
//! The injected tests feed [`AssetSourceEvent`]s into a source's channel by hand,
//! so they are deterministic and do not depend on the OS watcher. The filesystem
//! tests use a real [`FileWatcher`](crate::io::file::FileWatcher) over a
//! temporary directory and poll until the debounced change lands.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use kairos_ecs::message::Messages;
use kairos_ecs::schedule::ScheduleLabel;
use kairos_ecs::world::World;
use kairos_tasks::ConditionalSendFuture;

use crate::io::{
    AssetReader, AssetReaderError, AssetReaderFuture, AssetSourceBuilder, AssetSourceBuilders,
    AssetSourceEvent, AssetSourceId, AssetWatcher, ErasedAssetReader, PathStream, Reader,
    VecReader, empty_path_stream,
};
use crate::{
    Asset, AssetEvent, AssetLoader, AssetOptions, AssetPath, AssetServer, AssetWorldExt, Assets,
    LoadContext, LoadedFolder, ReadAssetBytesError, UntypedAssetId, VisitAssetDependencies,
    install,
};

/// The three ad-hoc stages the asset drivers are installed into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Tracking;

impl ScheduleLabel for Tracking {
    fn dyn_clone(&self) -> Box<dyn ScheduleLabel> {
        Box::new(*self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Events;

impl ScheduleLabel for Events {
    fn dyn_clone(&self) -> Box<dyn ScheduleLabel> {
        Box::new(*self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Boot;

impl ScheduleLabel for Boot {
    fn dyn_clone(&self) -> Box<dyn ScheduleLabel> {
        Box::new(*self)
    }
}

fn options() -> AssetOptions {
    AssetOptions::new(Tracking, Events, Boot)
}

/// An asset whose value is the bytes it was loaded from.
#[derive(Debug, PartialEq, Eq)]
struct ByteAsset(Vec<u8>);

impl Asset for ByteAsset {}
impl VisitAssetDependencies for ByteAsset {}

/// A loader that returns the reader's bytes.
struct ByteLoader;

impl AssetLoader for ByteLoader {
    type Asset = ByteAsset;
    type Settings = ();
    type Error = std::io::Error;

    fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext,
    ) -> impl ConditionalSendFuture<Output = Result<ByteAsset, std::io::Error>> {
        async move {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;
            Ok(ByteAsset(bytes))
        }
    }

    fn extensions(&self) -> &[&str] {
        &["bytes"]
    }
}

/// A loader whose value is the bytes of the `shared.bytes` file it reads while
/// loading. The read is a *loader dependency*, not a handle dependency, which is
/// the edge the reload drain's `queue_ancestors` walks.
struct DependentLoader;

impl AssetLoader for DependentLoader {
    type Asset = ByteAsset;
    type Settings = ();
    type Error = std::io::Error;

    fn load(
        &self,
        _reader: &mut dyn Reader,
        _settings: &(),
        load_context: &mut LoadContext,
    ) -> impl ConditionalSendFuture<Output = Result<ByteAsset, std::io::Error>> {
        let read = futures_lite::future::block_on(load_context.read_asset_bytes("shared.bytes"));
        async move {
            let bytes = read
                .map_err(|error: ReadAssetBytesError| std::io::Error::other(error.to_string()))?;
            Ok(ByteAsset(bytes))
        }
    }

    fn extensions(&self) -> &[&str] {
        &["dep"]
    }
}

/// A loader that produces a labeled sub-asset beside its root value.
struct LabeledLoader;

impl AssetLoader for LabeledLoader {
    type Asset = ByteAsset;
    type Settings = ();
    type Error = std::io::Error;

    fn load(
        &self,
        _reader: &mut dyn Reader,
        _settings: &(),
        load_context: &mut LoadContext,
    ) -> impl ConditionalSendFuture<Output = Result<ByteAsset, std::io::Error>> {
        load_context.add_labeled_asset("sub", ByteAsset(b"sub".to_vec()));
        async move { Ok(ByteAsset(b"root".to_vec())) }
    }

    fn extensions(&self) -> &[&str] {
        &["labeled"]
    }
}

/// An [`AssetWatcher`] handle that only keeps the event channel alive, so a test
/// can inject [`AssetSourceEvent`]s without a filesystem backend.
struct InjectedWatcher;

impl AssetWatcher for InjectedWatcher {}

/// A cloneable in-memory reader, so the injected tests can change bytes between
/// loads.
#[derive(Clone)]
struct MemoryReader {
    files: Arc<Mutex<HashMap<PathBuf, Vec<u8>>>>,
}

impl MemoryReader {
    fn new(files: &[(&str, &[u8])]) -> Self {
        let files = files
            .iter()
            .map(|(path, bytes)| (PathBuf::from(path), bytes.to_vec()))
            .collect();
        Self {
            files: Arc::new(Mutex::new(files)),
        }
    }
}

impl AssetReader for MemoryReader {
    fn read<'a>(&'a self, path: &'a Path) -> impl AssetReaderFuture<Value: Reader + 'a> {
        async move {
            self.files
                .lock()
                .unwrap()
                .get(path)
                .cloned()
                .map(VecReader::new)
                .ok_or_else(|| AssetReaderError::NotFound(path.to_path_buf()))
        }
    }

    fn read_meta<'a>(&'a self, path: &'a Path) -> impl AssetReaderFuture<Value: Reader + 'a> {
        async move { Err::<VecReader, _>(AssetReaderError::NotFound(path.to_path_buf())) }
    }

    fn read_directory<'a>(
        &'a self,
        _path: &'a Path,
    ) -> impl ConditionalSendFuture<Output = Result<Box<PathStream>, AssetReaderError>> {
        async move { Ok(empty_path_stream()) }
    }

    fn is_directory<'a>(
        &'a self,
        _path: &'a Path,
    ) -> impl ConditionalSendFuture<Output = Result<bool, AssetReaderError>> {
        async move { Ok(false) }
    }
}

/// An in-memory default source whose watcher slot hands the test the event
/// channel, so it can post source changes by hand.
fn injected_builder(
    files: &[(&str, &[u8])],
) -> (
    AssetSourceBuilder,
    Arc<Mutex<Option<async_channel::Sender<AssetSourceEvent>>>>,
) {
    let reader = MemoryReader::new(files);
    let captured = Arc::new(Mutex::new(None));
    let captured_by_watcher = captured.clone();
    let builder =
        AssetSourceBuilder::new(move || Box::new(reader.clone()) as Box<dyn ErasedAssetReader>)
            .with_watcher(move |sender| {
                *captured_by_watcher.lock().unwrap() = Some(sender);
                Some(Box::new(InjectedWatcher) as Box<dyn AssetWatcher>)
            });
    (builder, captured)
}

/// A world with the asset core installed over `builder`, `ByteAsset` registered,
/// and the two stages wired.
fn watched_world(builder: AssetSourceBuilder, watch: bool) -> World {
    let mut world = World::new();
    world
        .get_resource_or_init::<AssetSourceBuilders>()
        .insert(AssetSourceId::Default, builder);
    install(&mut world, options().with_watch_for_changes_override(watch));
    world.init_asset::<ByteAsset>();
    world.register_asset_loader(ByteLoader);
    world.register_asset_loader(DependentLoader);
    world.register_asset_loader(LabeledLoader);
    world
}

/// Runs both asset stages once.
fn pump(world: &mut World) {
    world.run_schedule(Tracking);
    world.run_schedule(Events);
}

/// The `Messages<AssetEvent<ByteAsset>>` resource, drained.
fn drain_events(world: &mut World) -> Vec<AssetEvent<ByteAsset>> {
    world
        .resource_mut::<Messages<AssetEvent<ByteAsset>>>()
        .drain()
        .collect()
}

/// Runs both stages until `id` finishes loading (or fails).
fn settle(world: &mut World, server: &AssetServer, id: UntypedAssetId) {
    for _ in 0..5000 {
        pump(world);
        if server.load_state(id).is_failed() || server.is_loaded_with_dependencies(id) {
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!(
        "asset {id:?} never settled; state was {:?}",
        server.load_state(id)
    );
}

/// A unique temporary directory for one test, recreated empty.
fn temp_dir(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("kairos_asset_watch_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The platform-default source over `dir` (absolute), so its watcher watches it.
fn file_builder(dir: &Path) -> AssetSourceBuilder {
    AssetSourceBuilder::platform_default(dir.to_str().unwrap(), None)
}

/// Builds an injected world and returns it with the channel to post events on.
fn injected_world(files: &[(&str, &[u8])]) -> (World, async_channel::Sender<AssetSourceEvent>) {
    let (builder, captured) = injected_builder(files);
    let world = watched_world(builder, true);
    let sender = captured
        .lock()
        .unwrap()
        .clone()
        .expect("the watcher constructor received the event channel");
    (world, sender)
}

#[test]
fn injected_modified_event_reloads_the_asset_and_emits_modified() {
    let (mut world, sender) = injected_world(&[("data.bytes", b"one")]);
    let server = world.resource::<AssetServer>().clone();

    let handle = server.load::<ByteAsset>("data.bytes");
    let id = handle.id().untyped();
    settle(&mut world, &server, id);
    assert_eq!(
        world.resource::<Assets<ByteAsset>>().get(handle.id()),
        Some(&ByteAsset(b"one".to_vec()))
    );
    let _ = drain_events(&mut world);

    sender
        .send_blocking(AssetSourceEvent::ModifiedAsset(PathBuf::from("data.bytes")))
        .unwrap();

    let mut saw_modified = false;
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        pump(&mut world);
        for event in drain_events(&mut world) {
            if matches!(event, AssetEvent::Modified { id: event_id } if event_id == handle.id()) {
                saw_modified = true;
            }
        }
        if saw_modified {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }

    assert!(saw_modified, "the reloaded asset emitted `Modified`");
    assert_eq!(
        world.resource::<Assets<ByteAsset>>().get(handle.id()),
        Some(&ByteAsset(b"one".to_vec()))
    );
}

#[test]
fn rename_events_are_ignored_by_the_main_server() {
    let (mut world, sender) = injected_world(&[("data.bytes", b"one")]);
    let server = world.resource::<AssetServer>().clone();

    let handle = server.load::<ByteAsset>("data.bytes");
    let id = handle.id().untyped();
    settle(&mut world, &server, id);
    let _ = drain_events(&mut world);

    // A pure rename must not reload the main server (the processor owns renames).
    sender
        .send_blocking(AssetSourceEvent::RenamedAsset {
            old: PathBuf::from("data.bytes"),
            new: PathBuf::from("other.bytes"),
        })
        .unwrap();

    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(500) {
        pump(&mut world);
        for event in drain_events(&mut world) {
            assert!(
                !matches!(event, AssetEvent::Modified { .. }),
                "a rename must not emit `Modified` on the main server"
            );
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn real_file_change_emits_modified_and_reloads() {
    let dir = temp_dir("file_change");
    std::fs::write(dir.join("data.bytes"), b"one").unwrap();
    let mut world = watched_world(file_builder(&dir), true);
    let server = world.resource::<AssetServer>().clone();

    let handle = server.load::<ByteAsset>("data.bytes");
    let id = handle.id().untyped();
    settle(&mut world, &server, id);
    assert_eq!(
        world.resource::<Assets<ByteAsset>>().get(handle.id()),
        Some(&ByteAsset(b"one".to_vec()))
    );
    let _ = drain_events(&mut world);

    std::fs::write(dir.join("data.bytes"), b"two, longer").unwrap();

    let mut saw_modified = false;
    let mut value_updated = false;
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(20) {
        pump(&mut world);
        for event in drain_events(&mut world) {
            if matches!(event, AssetEvent::Modified { id: event_id } if event_id == handle.id()) {
                saw_modified = true;
            }
        }
        value_updated = world.resource::<Assets<ByteAsset>>().get(handle.id())
            == Some(&ByteAsset(b"two, longer".to_vec()));
        if saw_modified && value_updated {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!(
        "the file change never reloaded the asset (modified={saw_modified}, updated={value_updated})"
    );
}

#[test]
fn real_folder_change_rewalks_the_loaded_folder() {
    let dir = temp_dir("folder_change");
    let folder = dir.join("folder");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("a.bytes"), b"a").unwrap();
    let mut world = watched_world(file_builder(&dir), true);
    let server = world.resource::<AssetServer>().clone();

    let handle = server.load_folder("folder");
    let id = handle.id().untyped();
    settle(&mut world, &server, id);
    assert_eq!(
        world
            .resource::<Assets<LoadedFolder>>()
            .get(handle.id())
            .map(|loaded| loaded.handles.len()),
        Some(1)
    );

    // A new file appears in the folder: the folder handle must be re-walked.
    std::fs::write(folder.join("b.bytes"), b"b").unwrap();

    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(20) {
        pump(&mut world);
        let count = world
            .resource::<Assets<LoadedFolder>>()
            .get(handle.id())
            .map(|loaded| loaded.handles.len());
        if count == Some(2) {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("the folder was never re-walked after a file was added");
}

#[test]
fn real_dependency_change_reloads_the_dependent() {
    let dir = temp_dir("dependency_change");
    std::fs::write(dir.join("shared.bytes"), b"one").unwrap();
    std::fs::write(dir.join("main.dep"), b"body").unwrap();
    let mut world = watched_world(file_builder(&dir), true);
    let server = world.resource::<AssetServer>().clone();

    let handle = server.load::<ByteAsset>("main.dep");
    let id = handle.id().untyped();
    settle(&mut world, &server, id);
    assert_eq!(
        world.resource::<Assets<ByteAsset>>().get(handle.id()),
        Some(&ByteAsset(b"one".to_vec()))
    );

    // Change the loader dependency. The dependent must reload even though its own
    // file did not change.
    std::fs::write(dir.join("shared.bytes"), b"two, longer").unwrap();

    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(20) {
        pump(&mut world);
        if world.resource::<Assets<ByteAsset>>().get(handle.id())
            == Some(&ByteAsset(b"two, longer".to_vec()))
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("the loader dependency change never reloaded the dependent");
}

#[test]
fn labeled_subasset_reloads_when_only_its_handle_lives() {
    let (mut world, sender) = injected_world(&[("base.labeled", b"ignored")]);
    let server = world.resource::<AssetServer>().clone();

    // Load the labeled sub-asset directly. Its internal base handle is dropped
    // when the load finishes, so only the labeled handle remains alive.
    let labeled = server.load::<ByteAsset>("base.labeled#sub");
    let labeled_id = labeled.id().untyped();
    settle(&mut world, &server, labeled_id);
    assert_eq!(
        world.resource::<Assets<ByteAsset>>().get(labeled.id()),
        Some(&ByteAsset(b"sub".to_vec()))
    );
    let _ = drain_events(&mut world);

    sender
        .send_blocking(AssetSourceEvent::ModifiedAsset(PathBuf::from(
            "base.labeled",
        )))
        .unwrap();

    // The base path itself has no live handle; the reload is triggered by the
    // `living_labeled_assets` bookkeeping and the untyped fallback.
    let mut saw_modified = false;
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        pump(&mut world);
        for event in drain_events(&mut world) {
            if matches!(event, AssetEvent::Modified { id } if id == labeled.id()) {
                saw_modified = true;
            }
        }
        if saw_modified {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(saw_modified, "the labeled sub-asset was reloaded");
}

#[test]
fn watching_stays_on_when_a_source_has_no_watcher() {
    // A source whose root does not exist gets no watcher, but the server still
    // reports that it is watching: a per-source failure must not flip the flag.
    let missing = temp_dir("missing_root").join("does_not_exist");
    let world = watched_world(file_builder(&missing), true);
    let server = world.resource::<AssetServer>().clone();

    assert!(server.watching_for_changes());
    let source = server.get_source(AssetSourceId::Default).unwrap();
    assert!(
        source.event_receiver().is_none(),
        "a missing root leaves the source without an event channel"
    );
}

#[test]
fn get_asset_path_strips_meta_and_rejects_outside_paths() {
    use crate::io::file::file_watcher::get_asset_path;

    let root = Path::new("/root");
    let (asset, is_meta) = get_asset_path(root, Path::new("/root/a/b.bytes")).unwrap();
    assert_eq!(asset, PathBuf::from("a/b.bytes"));
    assert!(!is_meta);

    let (asset, is_meta) = get_asset_path(root, Path::new("/root/a/b.bytes.meta")).unwrap();
    assert_eq!(asset, PathBuf::from("a/b.bytes"));
    assert!(is_meta);

    assert!(get_asset_path(root, Path::new("/elsewhere/a.bytes")).is_none());
}

#[test]
fn asset_path_helpers_used_by_the_reload_drain_work() {
    // Guards the path construction the drain relies on.
    let path = AssetPath::from(PathBuf::from("folder")).with_source(AssetSourceId::Default);
    assert_eq!(path.path(), Path::new("folder"));
}

/// On macOS the OS watcher reports event paths with symlinks resolved
/// (`/private/tmp/...` for a root spelled `/tmp`, and `/private/var/...` for the
/// non-canonical `/var/folders/...` a login session hands out), so a watcher must
/// fall back to the canonical root. Before that, a symlinked root silently
/// dropped every event.
#[test]
#[cfg(unix)]
fn symlinked_root_maps_canonical_event_paths() {
    use crate::io::file::file_watcher::get_asset_path;

    let base = temp_dir("symlink_roots");
    let real = base.join("real");
    std::fs::create_dir_all(&real).unwrap();
    let link = base.join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();

    let canonical = std::fs::canonicalize(&real).unwrap();

    // The canonical event path still maps back onto a source-relative path.
    let (asset, is_meta) = get_asset_path(&link, &canonical.join("folder/b.bytes")).unwrap();
    assert_eq!(asset, PathBuf::from("folder/b.bytes"));
    assert!(!is_meta);

    // So does the path spelled exactly as the root was given.
    let (asset, _) = get_asset_path(&link, &link.join("folder/b.bytes")).unwrap();
    assert_eq!(asset, PathBuf::from("folder/b.bytes"));
}

/// The end-to-end shape of the same bug: a folder loaded through a symlinked
/// root must still be re-walked when a file is added to it.
#[test]
#[cfg(unix)]
fn real_folder_change_under_a_symlinked_root_rewalks_the_folder() {
    let base = temp_dir("symlink_folder");
    let real = base.join("real");
    let folder = real.join("folder");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("a.bytes"), b"a").unwrap();
    let link = base.join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();

    let mut world = watched_world(file_builder(&link), true);
    let server = world.resource::<AssetServer>().clone();

    let handle = server.load_folder("folder");
    let id = handle.id().untyped();
    settle(&mut world, &server, id);
    assert_eq!(
        world
            .resource::<Assets<LoadedFolder>>()
            .get(handle.id())
            .map(|loaded| loaded.handles.len()),
        Some(1)
    );

    std::fs::write(folder.join("b.bytes"), b"b").unwrap();

    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(20) {
        pump(&mut world);
        let count = world
            .resource::<Assets<LoadedFolder>>()
            .get(handle.id())
            .map(|loaded| loaded.handles.len());
        if count == Some(2) {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("the folder was never re-walked through a symlinked root");
}
