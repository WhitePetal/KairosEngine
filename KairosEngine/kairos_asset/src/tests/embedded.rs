//! Tests for the in-memory and embedded backends: the virtual filesystem, the
//! `embedded://` source, and the embedded hot-reload loop.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use futures_lite::future::block_on;
use kairos_ecs::message::Messages;
use kairos_ecs::schedule::ScheduleLabel;
use kairos_ecs::world::World;

use crate::io::embedded::{_embedded_asset_path, EMBEDDED, EmbeddedAssetRegistry};
use crate::io::memory::{Dir, MemoryAssetReader, MemoryAssetWriter};
use crate::io::{
    AssetReader, AssetSourceBuilder, AssetSourceBuilders, AssetSourceEvent, AssetWatcher,
    AssetWriter, ErasedAssetReader, Reader,
};
use crate::{
    Asset, AssetEvent, AssetLoader, AssetOptions, AssetServer, AssetWorldExt, Assets, Handle,
    LoadContext, UntypedAssetId, VisitAssetDependencies, install,
};

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

#[derive(Debug, PartialEq, Eq)]
struct ByteAsset(Vec<u8>);

impl Asset for ByteAsset {}

impl VisitAssetDependencies for ByteAsset {}

struct ByteLoader;

impl AssetLoader for ByteLoader {
    type Asset = ByteAsset;
    type Settings = ();
    type Error = std::io::Error;

    async fn load(
        &self,
        reader: &mut dyn crate::io::Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<ByteAsset, Self::Error> {
        let mut bytes = Vec::new();
        futures_lite::AsyncReadExt::read_to_end(reader, &mut bytes).await?;
        Ok(ByteAsset(bytes))
    }

    fn extensions(&self) -> &[&str] {
        &["bytes"]
    }
}

/// An [`AssetWatcher`] handle that only keeps the event channel alive, so a test
/// can inject [`AssetSourceEvent`]s without a filesystem backend.
struct InjectedWatcher;

impl AssetWatcher for InjectedWatcher {}

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

// --- `_embedded_asset_path` ---

#[test]
fn embedded_asset_path_from_local_crate() {
    let asset_path = _embedded_asset_path(
        "my_crate",
        "src".as_ref(),
        "src/foo/plugin.rs".as_ref(),
        "the/asset.png".as_ref(),
    );
    assert_eq!(asset_path, Path::new("my_crate/foo/the/asset.png"));
}

#[test]
fn embedded_asset_path_from_local_crate_blank_src_path() {
    let asset_path = _embedded_asset_path(
        "my_crate",
        "".as_ref(),
        "src/foo/some/deep/path/plugin.rs".as_ref(),
        "the/asset.png".as_ref(),
    );
    assert_eq!(asset_path, Path::new("my_crate/the/asset.png"));
}

#[test]
fn embedded_asset_path_from_external_crate() {
    let asset_path = _embedded_asset_path(
        "my_crate",
        "src".as_ref(),
        "/path/to/crate/src/foo/plugin.rs".as_ref(),
        "the/asset.png".as_ref(),
    );
    assert_eq!(asset_path, Path::new("my_crate/foo/the/asset.png"));
}

#[test]
fn embedded_asset_path_from_local_example_crate() {
    let asset_path = _embedded_asset_path(
        "example_name",
        "examples/foo".as_ref(),
        "examples/foo/example.rs".as_ref(),
        "the/asset.png".as_ref(),
    );
    assert_eq!(asset_path, Path::new("example_name/the/asset.png"));
}

#[test]
#[should_panic(expected = "Failed to find src_prefix \"NOT-THERE\" in \"src")]
fn embedded_asset_path_from_local_crate_bad_src() {
    let _asset_path = _embedded_asset_path(
        "my_crate",
        "NOT-THERE".as_ref(),
        "src/foo/plugin.rs".as_ref(),
        "the/asset.png".as_ref(),
    );
}

// --- the in-memory filesystem ---

#[test]
fn memory_dir_stores_assets_and_metadata() {
    let dir = Dir::default();
    let a_path = Path::new("a.txt");
    let a_data = b"a".to_vec();
    let a_meta = b"ameta".to_vec();

    dir.insert_asset(a_path, a_data.clone());
    let asset = dir.get_asset(a_path).unwrap();
    assert_eq!(asset.path(), a_path);
    assert_eq!(asset.value(), a_data);

    dir.insert_meta(a_path, a_meta.clone());
    let meta = dir.get_metadata(a_path).unwrap();
    assert_eq!(meta.path(), a_path);
    assert_eq!(meta.value(), a_meta);

    // Nested paths create intermediate directories.
    let b_path = Path::new("x/y/b.txt");
    let b_data = b"b".to_vec();
    dir.insert_asset(b_path, b_data.clone());
    assert_eq!(dir.get_asset(b_path).unwrap().value(), b_data);
    assert!(dir.get_dir(Path::new("x/y")).is_some());

    assert_eq!(dir.remove_asset(a_path).unwrap().value(), a_data);
    assert!(dir.get_asset(a_path).is_none());
    assert!(dir.remove_asset(a_path).is_none());
    assert_eq!(dir.remove_metadata(a_path).unwrap().value(), a_meta);
}

#[test]
fn memory_writer_round_trips_bytes_meta_and_rename() {
    let dir = Dir::default();
    let writer = MemoryAssetWriter { root: dir.clone() };

    let write = |path: &Path, bytes: &[u8]| {
        block_on(async {
            let mut sink = AssetWriter::write(&writer, path).await.unwrap();
            futures_lite::AsyncWriteExt::write_all(&mut sink, bytes)
                .await
                .unwrap();
            futures_lite::AsyncWriteExt::flush(&mut sink).await.unwrap();
        });
    };

    write(Path::new("data.bytes"), b"one");
    assert_eq!(
        dir.get_asset(Path::new("data.bytes")).unwrap().value(),
        b"one"
    );

    block_on(async {
        let mut sink = AssetWriter::write_meta(&writer, Path::new("data.bytes"))
            .await
            .unwrap();
        futures_lite::AsyncWriteExt::write_all(&mut sink, b"meta")
            .await
            .unwrap();
        futures_lite::AsyncWriteExt::flush(&mut sink).await.unwrap();
    });
    assert_eq!(
        dir.get_metadata(Path::new("data.bytes")).unwrap().value(),
        b"meta"
    );

    block_on(AssetWriter::rename(
        &writer,
        Path::new("data.bytes"),
        Path::new("renamed.bytes"),
    ))
    .unwrap();
    assert!(dir.get_asset(Path::new("data.bytes")).is_none());
    assert_eq!(
        dir.get_asset(Path::new("renamed.bytes")).unwrap().value(),
        b"one"
    );

    block_on(AssetWriter::remove(&writer, Path::new("renamed.bytes"))).unwrap();
    assert!(dir.get_asset(Path::new("renamed.bytes")).is_none());
    assert!(block_on(AssetWriter::remove(&writer, Path::new("renamed.bytes"))).is_err());
}

#[test]
fn memory_reader_lists_directories_and_assets() {
    let dir = Dir::default();
    dir.insert_asset(Path::new("folder/a.bytes"), b"a".to_vec());
    dir.insert_asset(Path::new("folder/b.bytes"), b"b".to_vec());
    let reader = MemoryAssetReader { root: dir };

    let entries = block_on(async {
        let mut stream = AssetReader::read_directory(&reader, Path::new("folder"))
            .await
            .unwrap();
        let mut entries = Vec::new();
        use futures_lite::StreamExt;
        while let Some(entry) = stream.next().await {
            entries.push(entry);
        }
        entries
    });
    assert!(entries.contains(&PathBuf::from("folder/a.bytes")));
    assert!(entries.contains(&PathBuf::from("folder/b.bytes")));
    assert!(block_on(AssetReader::is_directory(&reader, Path::new("folder"))).unwrap());
    assert!(!block_on(AssetReader::is_directory(&reader, Path::new("nope"))).unwrap());
}

// --- the embedded source ---

/// A fixture embedded by the macro tests; `include_bytes!`/`include_str!` resolve
/// it relative to this file.
const FIXTURE_BYTES: &[u8] = b"hello embedded";

#[test]
fn embedded_path_macro_matches_the_registered_asset_path() {
    let path = crate::embedded_path!("fixtures/embedded.txt");
    let components: Vec<_> = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        components,
        ["kairos_asset", "tests", "fixtures", "embedded.txt"]
    );
}

#[test]
fn embedded_asset_macro_registers_a_loadable_asset() {
    let mut world = World::new();
    install(&mut world, options().with_watch_for_changes_override(false));
    crate::embedded_asset!(&mut world, "fixtures/embedded.txt");
    world.init_asset::<ByteAsset>();
    world.register_asset_loader(ByteLoader);

    let server = world.resource::<AssetServer>().clone();
    let handle: Handle<ByteAsset> = crate::load_embedded_asset!(&world, "fixtures/embedded.txt");
    settle(&mut world, &server, handle.id().untyped());

    assert_eq!(
        world.resource::<Assets<ByteAsset>>().get(handle.id()),
        Some(&ByteAsset(FIXTURE_BYTES.to_vec()))
    );
}

#[test]
fn load_internal_asset_macro_inserts_into_the_store() {
    let mut world = World::new();
    install(&mut world, options().with_watch_for_changes_override(false));
    world.init_asset::<ByteAsset>();

    let handle = world
        .resource_mut::<Assets<ByteAsset>>()
        .add(ByteAsset(Vec::new()));
    {
        let world = &mut world;
        crate::load_internal_asset!(
            world,
            handle,
            "fixtures/embedded.txt",
            |text: &str, _path: std::borrow::Cow<'_, str>| ByteAsset(text.as_bytes().to_vec())
        );
    }

    assert_eq!(
        world.resource::<Assets<ByteAsset>>().get(handle.id()),
        Some(&ByteAsset(FIXTURE_BYTES.to_vec()))
    );
}

#[test]
fn load_internal_binary_asset_macro_inserts_into_the_store() {
    let mut world = World::new();
    install(&mut world, options().with_watch_for_changes_override(false));
    world.init_asset::<ByteAsset>();

    let handle = world
        .resource_mut::<Assets<ByteAsset>>()
        .add(ByteAsset(Vec::new()));
    {
        let world = &mut world;
        crate::load_internal_binary_asset!(
            world,
            handle,
            "fixtures/embedded.txt",
            |bytes: &[u8], _path: String| ByteAsset(bytes.to_vec())
        );
    }

    assert_eq!(
        world.resource::<Assets<ByteAsset>>().get(handle.id()),
        Some(&ByteAsset(FIXTURE_BYTES.to_vec()))
    );
}

#[test]
fn embedded_assets_load_through_the_embedded_source() {
    let registry = EmbeddedAssetRegistry::default();
    registry.insert_asset(
        PathBuf::from("src/thing.bytes"),
        Path::new("my_crate/thing.bytes"),
        b"one".to_vec(),
    );

    let mut world = World::new();
    world.insert_resource(registry);
    install(&mut world, options().with_watch_for_changes_override(false));
    world.init_asset::<ByteAsset>();
    world.register_asset_loader(ByteLoader);

    let server = world.resource::<AssetServer>().clone();
    let handle = server.load::<ByteAsset>("embedded://my_crate/thing.bytes");
    settle(&mut world, &server, handle.id().untyped());

    assert_eq!(
        world.resource::<Assets<ByteAsset>>().get(handle.id()),
        Some(&ByteAsset(b"one".to_vec()))
    );
}

#[test]
fn embedded_source_reads_through_the_registry_dir() {
    let registry = EmbeddedAssetRegistry::default();
    registry.insert_asset(
        PathBuf::from("src/thing.bytes"),
        Path::new("my_crate/thing.bytes"),
        b"one".to_vec(),
    );
    registry.insert_meta(
        Path::new("src/thing.bytes"),
        Path::new("my_crate/thing.bytes"),
        b"meta",
    );

    let reader = MemoryAssetReader {
        root: registry.dir.clone(),
    };
    let bytes = block_on(async {
        let mut handle = AssetReader::read(&reader, Path::new("my_crate/thing.bytes"))
            .await
            .unwrap();
        let mut bytes = Vec::new();
        handle.read_to_end(&mut bytes).await.unwrap();
        bytes
    });
    assert_eq!(bytes, b"one");

    let meta = block_on(AssetReader::read_meta_bytes(
        &reader,
        Path::new("my_crate/thing.bytes"),
    ))
    .unwrap();
    assert_eq!(meta, b"meta");

    assert_eq!(
        registry
            .remove_asset(Path::new("my_crate/thing.bytes"))
            .unwrap()
            .value(),
        b"one"
    );
    assert!(
        block_on(AssetReader::read(
            &reader,
            Path::new("my_crate/thing.bytes")
        ))
        .is_err()
    );
}

/// Builds a world whose `embedded` source has an injected watcher, returning the
/// world and the channel source changes can be posted on.
fn watched_embedded_world(
    registry: EmbeddedAssetRegistry,
) -> (World, async_channel::Sender<AssetSourceEvent>, PathBuf) {
    let watched = PathBuf::from("watched/thing.bytes");
    let asset_path = Path::new("my_crate/thing.bytes");
    registry.insert_asset(watched.clone(), asset_path, b"one".to_vec());

    // `register_source` installs the real (opt-in) embedded watcher; replace it
    // with one that hands the test the event channel instead.
    let mut builders = AssetSourceBuilders::default();
    registry.register_source(&mut builders);
    let captured = Arc::new(Mutex::new(None));
    let captured_by_watcher = captured.clone();
    builders.get_mut(EMBEDDED).unwrap().watcher = Some(Box::new(move |sender| {
        *captured_by_watcher.lock().unwrap() = Some(sender);
        Some(Box::new(InjectedWatcher) as Box<dyn AssetWatcher>)
    }));

    let mut world = World::new();
    world.insert_resource(registry);
    world.insert_resource(builders);
    install(&mut world, options().with_watch_for_changes_override(true));
    world.init_asset::<ByteAsset>();
    world.register_asset_loader(ByteLoader);

    let sender = captured
        .lock()
        .unwrap()
        .clone()
        .expect("the embedded watcher constructor received the event channel");
    (world, sender, watched)
}

#[test]
fn embedded_asset_hot_reloads_after_the_source_changes() {
    let (mut world, sender, watched) = watched_embedded_world(EmbeddedAssetRegistry::default());
    let server = world.resource::<AssetServer>().clone();
    let asset_path = Path::new("my_crate/thing.bytes");

    let handle: Handle<ByteAsset> = server.load("embedded://my_crate/thing.bytes");
    let id = handle.id().untyped();
    settle(&mut world, &server, id);
    assert_eq!(
        world.resource::<Assets<ByteAsset>>().get(handle.id()),
        Some(&ByteAsset(b"one".to_vec()))
    );
    let _ = drain_events(&mut world);

    // The embedded watcher overwrote the compiled-in bytes with the changed file
    // contents; only the event is left to drive the reload.
    world.resource::<EmbeddedAssetRegistry>().insert_asset(
        watched,
        asset_path,
        b"two, longer".to_vec(),
    );
    sender
        .send_blocking(AssetSourceEvent::ModifiedAsset(asset_path.to_path_buf()))
        .unwrap();

    let mut saw_modified = false;
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        pump(&mut world);
        saw_modified |= drain_events(&mut world)
            .iter()
            .any(|event| matches!(event, AssetEvent::Modified { id: event_id } if *event_id == handle.id()));
        if saw_modified
            && world.resource::<Assets<ByteAsset>>().get(handle.id())
                == Some(&ByteAsset(b"two, longer".to_vec()))
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!(
        "the embedded asset never hot-reloaded (modified={saw_modified}, value={:?})",
        world.resource::<Assets<ByteAsset>>().get(handle.id())
    );
}

#[test]
fn install_keeps_a_host_registered_embedded_source() {
    // A host that pre-registers `embedded` keeps its builder: `install` only
    // fills the source in when it is absent.
    let registry = EmbeddedAssetRegistry::default();
    let mut builders = AssetSourceBuilders::default();
    builders.insert(
        EMBEDDED,
        AssetSourceBuilder::new(|| {
            Box::new(MemoryAssetReader::default()) as Box<dyn ErasedAssetReader>
        }),
    );

    let mut world = World::new();
    world.insert_resource(registry);
    world.insert_resource(builders);
    install(&mut world, options().with_watch_for_changes_override(false));

    let server = world.resource::<AssetServer>().clone();
    assert!(server.get_source(EMBEDDED).is_ok());
}
