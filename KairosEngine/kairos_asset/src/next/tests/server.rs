//! Tests for the asset server: registration, the loading pipeline, and the
//! A-tier public API.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use kairos_ecs::message::Messages;
use kairos_ecs::world::World;
use kairos_tasks::ConditionalSendFuture;

use crate::next::io::{
    AssetReader, AssetReaderError, AssetReaderFuture, AssetSourceBuilder, AssetSourceBuilders,
    AssetSourceId, ErasedAssetReader, PathStream, Reader, UnapprovedPathMode, VecReader,
    empty_path_stream,
};
use crate::next::{
    Asset, AssetEvent, AssetLoadFailedEvent, AssetLoader, AssetMetaCheck, AssetServer,
    AssetServerMode, Assets, Handle, LoadContext, UntypedAssetId, VisitAssetDependencies,
    handle_internal_asset_events,
};

/// An asset whose value is the bytes it was loaded from.
#[derive(Debug, PartialEq, Eq)]
struct ByteAsset(Vec<u8>);

impl Asset for ByteAsset {}
impl VisitAssetDependencies for ByteAsset {}

/// An asset that holds a handle to another asset, so the pipeline has a real
/// dependency to wait for.
struct ParentAsset {
    body: Vec<u8>,
    dep: Handle<ByteAsset>,
}

impl Asset for ParentAsset {}
impl VisitAssetDependencies for ParentAsset {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        self.dep.visit_dependencies(visit);
    }
}

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

/// A loader that declares a dependency and keeps its handle in the value.
struct ParentLoader;

impl AssetLoader for ParentLoader {
    type Asset = ParentAsset;
    type Settings = ();
    type Error = std::io::Error;

    fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        load_context: &mut LoadContext,
    ) -> impl ConditionalSendFuture<Output = Result<ParentAsset, std::io::Error>> {
        let dep: Handle<ByteAsset> = load_context.load("dep.bytes");
        async move {
            let mut body = Vec::new();
            reader.read_to_end(&mut body).await?;
            Ok(ParentAsset { body, dep })
        }
    }

    fn extensions(&self) -> &[&str] {
        &["parent"]
    }
}

/// A loader that produces a labeled asset beside its root value.
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

/// An in-memory [`AssetReader`], so tests do not touch the filesystem.
#[derive(Clone)]
struct MemoryReader(Arc<HashMap<PathBuf, Vec<u8>>>);

impl AssetReader for MemoryReader {
    fn read<'a>(&'a self, path: &'a Path) -> impl AssetReaderFuture<Value: Reader + 'a> {
        async move {
            self.0
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

/// A source whose bytes can change between loads, for testing `reload`.
#[derive(Clone, Default)]
struct MutableReader(Arc<Mutex<HashMap<PathBuf, Vec<u8>>>>);

impl MutableReader {
    fn new(files: &[(&str, &[u8])]) -> Self {
        let this = Self::default();
        for (path, bytes) in files {
            this.set(path, bytes);
        }
        this
    }

    fn set(&self, path: &str, bytes: &[u8]) {
        self.0
            .lock()
            .expect("the reader lock is not poisoned")
            .insert(PathBuf::from(path), bytes.to_vec());
    }
}

impl AssetReader for MutableReader {
    fn read<'a>(&'a self, path: &'a Path) -> impl AssetReaderFuture<Value: Reader + 'a> {
        // Read before the future so the lock is not held across an await.
        let bytes = self
            .0
            .lock()
            .expect("the reader lock is not poisoned")
            .get(path)
            .cloned();
        async move {
            bytes
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

/// A server whose default source is `reader`.
fn server_with_reader(reader: MutableReader) -> AssetServer {
    let mut builders = AssetSourceBuilders::default();
    builders.insert(
        AssetSourceId::Default,
        AssetSourceBuilder::new(
            move || Box::new(reader.clone()) as Box<dyn ErasedAssetReader>,
        ),
    );
    let sources = Arc::new(builders.build_sources());
    AssetServer::new_with_meta_check(
        sources,
        AssetServerMode::Unprocessed,
        AssetMetaCheck::Never,
        false,
        UnapprovedPathMode::Forbid,
    )
}

/// A server whose default source is `files`, with no meta sidecars.
fn server_with_files(files: &[(&str, &[u8])]) -> AssetServer {
    let files: HashMap<PathBuf, Vec<u8>> = files
        .iter()
        .map(|(path, bytes)| (PathBuf::from(path), bytes.to_vec()))
        .collect();
    let files = Arc::new(files);

    let mut builders = AssetSourceBuilders::default();
    builders.insert(
        AssetSourceId::Default,
        AssetSourceBuilder::new(
            move || Box::new(MemoryReader(files.clone())) as Box<dyn ErasedAssetReader>,
        ),
    );
    let sources = Arc::new(builders.build_sources());
    AssetServer::new_with_meta_check(
        sources,
        AssetServerMode::Unprocessed,
        AssetMetaCheck::Never,
        false,
        UnapprovedPathMode::Forbid,
    )
}

/// A world holding the server, a [`ByteAsset`] store, and the asset messages.
fn world_for(server: &AssetServer) -> World {
    let assets = Assets::<ByteAsset>::default();
    server.register_asset(&assets);

    let mut world = World::new();
    world.insert_resource(assets);
    world.insert_resource(server.clone());
    world.insert_resource(Messages::<AssetEvent<ByteAsset>>::default());
    world.insert_resource(Messages::<AssetLoadFailedEvent<ByteAsset>>::default());
    world
}

/// Runs the main-thread half of the pipeline until `id` settles.
fn wait_for(world: &mut World, server: &AssetServer, id: UntypedAssetId) {
    for _ in 0..5000 {
        handle_internal_asset_events(world);
        let state = server.load_state(id);
        if state.is_failed() || server.is_loaded_with_dependencies(id) {
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("asset {id:?} never settled; state was {:?}", server.load_state(id));
}

#[test]
fn repeated_loads_reuse_the_same_id() {
    let server = server_with_files(&[("data.bytes", b"hello")]);
    server.register_loader(ByteLoader);
    let mut world = world_for(&server);

    let first = server.load::<ByteAsset>("data.bytes");
    let second = server.load::<ByteAsset>("data.bytes");

    assert_eq!(first.id(), second.id());
    assert_ne!(first.id(), crate::next::AssetId::<ByteAsset>::default());

    // Let the single load task finish so it is not left dangling.
    wait_for(&mut world, &server, first.id().untyped());
}

#[test]
fn missing_file_fails_without_panicking() {
    let server = server_with_files(&[]);
    server.register_loader(ByteLoader);
    let mut world = world_for(&server);

    let handle = server.load::<ByteAsset>("missing.bytes");
    let id = handle.id().untyped();
    wait_for(&mut world, &server, id);

    assert!(server.load_state(id).is_failed());
    assert_eq!(server.get_load_state(id).map(|state| state.is_failed()), Some(true));
    assert_eq!(world.resource::<Assets<ByteAsset>>().get(handle.id()), None);
}

#[test]
fn a_loaded_asset_is_inserted_into_its_store() {
    let server = server_with_files(&[("data.bytes", b"hello")]);
    server.register_loader(ByteLoader);
    let mut world = world_for(&server);

    let handle = server.load::<ByteAsset>("data.bytes");
    let id = handle.id().untyped();
    wait_for(&mut world, &server, id);

    assert!(server.is_loaded(id));
    assert!(server.is_loaded_with_dependencies(id));
    assert_eq!(
        world.resource::<Assets<ByteAsset>>().get(handle.id()),
        Some(&ByteAsset(b"hello".to_vec()))
    );
}

#[test]
fn a_declared_dependency_is_waited_on() {
    let server = server_with_files(&[("main.parent", b"body"), ("dep.bytes", b"dep")]);
    server.register_loader(ByteLoader);
    server.register_loader(ParentLoader);

    let parent_assets = Assets::<ParentAsset>::default();
    let byte_assets = Assets::<ByteAsset>::default();
    server.register_asset(&parent_assets);
    server.register_asset(&byte_assets);
    let mut world = World::new();
    world.insert_resource(parent_assets);
    world.insert_resource(byte_assets);
    world.insert_resource(server.clone());
    world.insert_resource(Messages::<AssetEvent<ParentAsset>>::default());
    world.insert_resource(Messages::<AssetEvent<ByteAsset>>::default());
    world.insert_resource(Messages::<AssetLoadFailedEvent<ParentAsset>>::default());
    world.insert_resource(Messages::<AssetLoadFailedEvent<ByteAsset>>::default());

    let handle = server.load::<ParentAsset>("main.parent");
    let id = handle.id().untyped();
    wait_for(&mut world, &server, id);

    assert!(server.is_loaded_with_dependencies(id));
    let loaded = world.resource::<Assets<ParentAsset>>().get(handle.id()).unwrap();
    assert_eq!(loaded.body, b"body");
    // The dependency was loaded too, and the parent's handle names it.
    let dep_id = loaded.dep.id();
    assert!(server.is_loaded(dep_id.untyped()));
    assert_eq!(
        world.resource::<Assets<ByteAsset>>().get(dep_id),
        Some(&ByteAsset(b"dep".to_vec()))
    );
}

#[test]
fn a_labeled_asset_is_loaded_and_inserted() {
    let server = server_with_files(&[("main.labeled", b"ignored")]);
    server.register_loader(LabeledLoader);
    let mut world = world_for(&server);

    let handle = server.load::<ByteAsset>("main.labeled#sub");
    let id = handle.id().untyped();
    wait_for(&mut world, &server, id);

    // The labeled asset is inserted under its own `path#label` id.
    assert!(server.is_loaded_with_dependencies(id));
    assert_eq!(
        world.resource::<Assets<ByteAsset>>().get(handle.id()),
        Some(&ByteAsset(b"sub".to_vec()))
    );
    assert_eq!(
        server.get_path(id).map(|path| path.path().to_path_buf()),
        Some(PathBuf::from("main.labeled"))
    );
}

#[test]
fn add_inserts_a_runtime_asset() {
    let server = server_with_files(&[]);
    let mut world = world_for(&server);

    let handle = server.add(ByteAsset(vec![1, 2, 3]));
    let id = handle.id().untyped();
    wait_for(&mut world, &server, id);

    assert!(server.is_managed(id));
    assert!(server.is_loaded_with_dependencies(id));
    assert_eq!(
        world.resource::<Assets<ByteAsset>>().get(handle.id()),
        Some(&ByteAsset(vec![1, 2, 3]))
    );
}

#[test]
fn handles_and_paths_are_reported_for_a_loaded_asset() {
    let server = server_with_files(&[("data.bytes", b"hello")]);
    server.register_loader(ByteLoader);
    let mut world = world_for(&server);

    let handle = server.load::<ByteAsset>("data.bytes");
    let id = handle.id();
    wait_for(&mut world, &server, id.untyped());

    assert!(server.is_managed(id));
    assert_eq!(server.get_id_handle(id).map(|handle| handle.id()), Some(id));
    assert_eq!(
        server.get_handle::<ByteAsset>("data.bytes").map(|handle| handle.id()),
        Some(id)
    );
    assert_eq!(
        server.get_path(id).map(|path| path.path().to_path_buf()),
        Some(PathBuf::from("data.bytes"))
    );
}

#[test]
fn reload_picks_up_new_bytes() {
    let files = MutableReader::new(&[("data.bytes", b"one")]);
    let server = server_with_reader(files.clone());
    server.register_loader(ByteLoader);
    let mut world = world_for(&server);

    let handle = server.load::<ByteAsset>("data.bytes");
    let id = handle.id().untyped();
    wait_for(&mut world, &server, id);
    assert_eq!(
        world.resource::<Assets<ByteAsset>>().get(handle.id()),
        Some(&ByteAsset(b"one".to_vec()))
    );

    files.set("data.bytes", b"two");
    server.reload("data.bytes");

    for _ in 0..5000 {
        handle_internal_asset_events(&mut world);
        if world.resource::<Assets<ByteAsset>>().get(handle.id())
            == Some(&ByteAsset(b"two".to_vec()))
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("the reload was never applied");
}

#[test]
fn dropping_the_handle_discards_the_load() {
    let server = server_with_files(&[("data.bytes", b"hello")]);
    server.register_loader(ByteLoader);
    let mut world = world_for(&server);

    let handle = server.load::<ByteAsset>("data.bytes");
    let id = handle.id();
    // Drop the only strong handle: nobody wants the result any more.
    drop(handle);

    for _ in 0..5000 {
        handle_internal_asset_events(&mut world);
        if !server.is_managed(id) {
            assert_eq!(world.resource::<Assets<ByteAsset>>().get(id), None);
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("a load with no live handle was not discarded");
}

#[test]
fn a_uuid_handle_is_not_managed() {
    let server = server_with_files(&[]);
    let uuid_id = crate::next::AssetId::<ByteAsset>::invalid().untyped();
    assert!(!server.is_managed(uuid_id));
}
