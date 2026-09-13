//! Tests for the asset server: registration, the loading pipeline, and the
//! A-tier public API.

use std::{
    any::TypeId,
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use kairos_ecs::message::Messages;
use kairos_ecs::world::World;
use kairos_tasks::ConditionalSendFuture;

use crate::io::{
    AssetReader, AssetReaderError, AssetReaderFuture, AssetSourceBuilder, AssetSourceBuilders,
    AssetSourceId, ErasedAssetReader, PathStream, Reader, UnapprovedPathMode, VecReader,
    empty_path_stream, file::FileAssetReader, get_meta_path,
};
use crate::{
    Asset, AssetEvent, AssetLoadError, AssetLoadFailedEvent, AssetLoader, AssetMetaCheck,
    AssetPath, AssetServer, AssetServerMode, Assets, Handle, LoadContext, LoadState, LoadedFolder,
    LoadedUntypedAsset, ReadAssetBytesError, UntypedAssetId, VisitAssetDependencies,
    handle_internal_asset_events,
};
use crate::meta::{ProcessedInfo, ProcessedInfoMinimal};

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
        // Meta sidecars are stored under their real `<path>.meta` name.
        let meta_path = get_meta_path(path);
        let bytes = self.0.get(&meta_path).cloned();
        async move {
            bytes
                .map(VecReader::new)
                .ok_or_else(|| AssetReaderError::NotFound(meta_path))
        }
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

/// An in-memory reader with real directories, so a folder load has a tree to
/// walk. Files and directories are declared explicitly; a path that is neither
/// is a miss.
#[derive(Clone, Default)]
struct TreeReader {
    files: Arc<HashMap<PathBuf, Vec<u8>>>,
    dirs: Arc<HashSet<PathBuf>>,
}

impl TreeReader {
    fn new(files: &[(&str, &[u8])], dirs: &[&str]) -> Self {
        Self {
            files: Arc::new(
                files
                    .iter()
                    .map(|(path, bytes)| (PathBuf::from(path), bytes.to_vec()))
                    .collect(),
            ),
            dirs: Arc::new(dirs.iter().map(PathBuf::from).collect()),
        }
    }
}

impl AssetReader for TreeReader {
    fn read<'a>(&'a self, path: &'a Path) -> impl AssetReaderFuture<Value: Reader + 'a> {
        let bytes = self.files.get(path).cloned();
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
        path: &'a Path,
    ) -> impl ConditionalSendFuture<Output = Result<Box<PathStream>, AssetReaderError>> {
        let mut entries: Vec<PathBuf> = self
            .files
            .keys()
            .chain(self.dirs.iter())
            .filter(|entry| entry.parent() == Some(path))
            .cloned()
            .collect();
        entries.sort();
        let is_dir = self.dirs.contains(path);
        async move {
            if is_dir {
                Ok(Box::new(futures_lite::stream::iter(entries)) as Box<PathStream>)
            } else {
                Err(AssetReaderError::NotFound(path.to_path_buf()))
            }
        }
    }

    fn is_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> impl ConditionalSendFuture<Output = Result<bool, AssetReaderError>> {
        let is_dir = self.dirs.contains(path);
        let is_file = self.files.contains_key(path);
        async move {
            if is_dir {
                Ok(true)
            } else if is_file {
                Ok(false)
            } else {
                Err(AssetReaderError::NotFound(path.to_path_buf()))
            }
        }
    }
}

/// A server whose default source is the file/directory `tree`.
fn server_with_tree(files: &[(&str, &[u8])], dirs: &[&str]) -> AssetServer {
    let reader = TreeReader::new(files, dirs);
    let mut builders = AssetSourceBuilders::default();
    builders.insert(
        AssetSourceId::Default,
        AssetSourceBuilder::new(move || Box::new(reader.clone()) as Box<dyn ErasedAssetReader>),
    );
    let sources = Arc::new(builders.build_sources(false, false));
    AssetServer::new_with_meta_check(
        sources,
        AssetServerMode::Unprocessed,
        AssetMetaCheck::Never,
        false,
        UnapprovedPathMode::Forbid,
    )
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
    let sources = Arc::new(builders.build_sources(false, false));
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
    server_with_files_and_mode(files, UnapprovedPathMode::Forbid)
}

/// A server whose default source is `files`, in `unapproved` path mode.
fn server_with_files_and_mode(
    files: &[(&str, &[u8])],
    unapproved: UnapprovedPathMode,
) -> AssetServer {
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
    let sources = Arc::new(builders.build_sources(false, false));
    AssetServer::new_with_meta_check(
        sources,
        AssetServerMode::Unprocessed,
        AssetMetaCheck::Never,
        false,
        unapproved,
    )
}

/// A world holding the server, a [`ByteAsset`] store, a
/// [`LoadedUntypedAsset`] store, a [`LoadedFolder`] store, and the asset
/// messages.
fn world_for(server: &AssetServer) -> World {
    let assets = Assets::<ByteAsset>::default();
    server.register_asset(&assets);
    let untyped = Assets::<LoadedUntypedAsset>::default();
    server.register_asset(&untyped);
    let folders = Assets::<LoadedFolder>::default();
    server.register_asset(&folders);

    let mut world = World::new();
    world.insert_resource(assets);
    world.insert_resource(untyped);
    world.insert_resource(folders);
    world.insert_resource(server.clone());
    world.insert_resource(Messages::<AssetEvent<ByteAsset>>::default());
    world.insert_resource(Messages::<AssetLoadFailedEvent<ByteAsset>>::default());
    world.insert_resource(Messages::<AssetEvent<LoadedUntypedAsset>>::default());
    world.insert_resource(Messages::<AssetLoadFailedEvent<LoadedUntypedAsset>>::default());
    world.insert_resource(Messages::<AssetEvent<LoadedFolder>>::default());
    world.insert_resource(Messages::<AssetLoadFailedEvent<LoadedFolder>>::default());
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
    assert_ne!(first.id(), crate::AssetId::<ByteAsset>::default());

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
fn add_async_hands_out_a_handle_before_the_future_resolves() {
    let server = server_with_files(&[]);
    let mut world = world_for(&server);

    let handle =
        server.add_async::<ByteAsset, std::io::Error>(async { Ok(ByteAsset(vec![1, 2, 3])) });
    let id = handle.id().untyped();

    // The handle exists and the asset is loading straight away, before the
    // spawned task has had a chance to send its result.
    assert!(server.is_managed(id));
    assert!(server.load_state(id).is_loading());

    wait_for(&mut world, &server, id);

    assert!(server.is_loaded_with_dependencies(id));
    assert_eq!(
        world.resource::<Assets<ByteAsset>>().get(handle.id()),
        Some(&ByteAsset(vec![1, 2, 3]))
    );
}

#[test]
fn add_async_marks_the_asset_failed_when_the_future_errors() {
    let server = server_with_files(&[]);
    let mut world = world_for(&server);

    let handle = server.add_async::<ByteAsset, std::io::Error>(async {
        Err(std::io::Error::new(std::io::ErrorKind::Other, "nope"))
    });
    let id = handle.id().untyped();
    wait_for(&mut world, &server, id);

    assert_eq!(world.resource::<Assets<ByteAsset>>().get(handle.id()), None);
    let LoadState::Failed(error) = server.load_state(id) else {
        panic!("the asset should have failed");
    };
    assert!(matches!(&*error, AssetLoadError::AddAsyncError(_)));
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
fn the_untyped_accessors_expose_the_loaded_asset() {
    let server = server_with_files(&[("data.bytes", b"hello")]);
    server.register_loader(ByteLoader);
    let mut world = world_for(&server);

    let handle = server.load::<ByteAsset>("data.bytes");
    let id = handle.id().untyped();
    wait_for(&mut world, &server, id);

    // Handle accessors, by id and by path.
    assert_eq!(server.get_id_handle_untyped(id).map(|h| h.id()), Some(id));
    assert_eq!(
        server
            .get_path_and_type_id_handle(&AssetPath::from("data.bytes"), TypeId::of::<ByteAsset>())
            .map(|h| h.id()),
        Some(id)
    );
    assert_eq!(server.get_handle_untyped("data.bytes").map(|h| h.id()), Some(id));
    assert_eq!(server.get_handles_untyped("data.bytes").len(), 1);

    // Path id accessors.
    assert_eq!(server.get_path_id("data.bytes"), Some(id));
    assert_eq!(server.get_path_ids("data.bytes"), vec![id]);

    // Source accessor.
    assert!(server.get_source(AssetSourceId::Default).is_ok());
    assert!(server.get_source("nope").is_err());

    // The handle visits its own id, whose dependency states are loaded.
    assert!(server.are_dependencies_loaded(&handle));
    assert!(server.are_direct_dependencies_loaded(&handle));
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
    let uuid_id = crate::AssetId::<ByteAsset>::invalid().untyped();
    assert!(!server.is_managed(uuid_id));
}

/// Sets its flag when dropped, so a load's guard lifetime can be observed.
struct DropFlag(Arc<AtomicBool>);

impl Drop for DropFlag {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/// A loader that announces it has started and then blocks until the gate is
/// released, so a test can inspect the world while a load is in flight.
struct GatedLoader {
    started: async_channel::Sender<()>,
    gate: async_channel::Receiver<()>,
}

impl AssetLoader for GatedLoader {
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
            self.started
                .send(())
                .await
                .expect("the started channel is open");
            self.gate.recv().await.expect("the gate channel is open");
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;
            Ok(ByteAsset(bytes))
        }
    }

    fn extensions(&self) -> &[&str] {
        &["bytes"]
    }
}

/// A loader whose load always fails, so the failure paths can be exercised.
struct FailingLoader;

impl AssetLoader for FailingLoader {
    type Asset = ByteAsset;
    type Settings = ();
    type Error = std::io::Error;

    fn load(
        &self,
        _reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext,
    ) -> impl ConditionalSendFuture<Output = Result<ByteAsset, std::io::Error>> {
        async move { Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "boom")) }
    }

    fn extensions(&self) -> &[&str] {
        &["failing"]
    }
}

#[test]
fn untyped_load_resolves_the_asset_under_its_synthetic_source() {
    let server = server_with_files(&[("data.bytes", b"hello")]);
    server.register_loader(ByteLoader);
    let mut world = world_for(&server);

    let untyped_handle = server.load_builder().load_untyped("data.bytes");
    let untyped_id = untyped_handle.id().untyped();
    wait_for(&mut world, &server, untyped_id);

    // The wrapper resolved the real asset's handle.
    let resolved = world
        .resource::<Assets<LoadedUntypedAsset>>()
        .get(untyped_handle.id())
        .expect("the untyped wrapper loaded")
        .handle
        .clone();
    let typed = server
        .get_handle::<ByteAsset>("data.bytes")
        .expect("the resolved asset has a typed handle");
    assert_eq!(resolved.id(), typed.id().untyped());
    assert_eq!(
        world.resource::<Assets<ByteAsset>>().get(&typed),
        Some(&ByteAsset(b"hello".to_vec()))
    );

    // It is addressed under the synthetic `--untyped` source, so it cannot
    // collide with the typed load of the same path.
    let wrapper_path = server.get_path(untyped_id).expect("the wrapper has a path");
    assert_eq!(wrapper_path.source().as_str(), Some("--untyped"));
    assert_eq!(wrapper_path.path(), Path::new("data.bytes"));
}

#[test]
fn load_erased_matches_the_typed_load() {
    let server = server_with_files(&[("data.bytes", b"hello")]);
    server.register_loader(ByteLoader);
    let mut world = world_for(&server);

    let untyped = server
        .load_builder()
        .load_erased(TypeId::of::<ByteAsset>(), "data.bytes");
    assert_eq!(untyped.type_id(), TypeId::of::<ByteAsset>());
    wait_for(&mut world, &server, untyped.id());

    let typed: Handle<ByteAsset> = untyped.typed_debug_checked();
    assert_eq!(
        world.resource::<Assets<ByteAsset>>().get(&typed),
        Some(&ByteAsset(b"hello".to_vec()))
    );
    assert_eq!(
        server.get_handle::<ByteAsset>("data.bytes").map(|handle| handle.id()),
        Some(typed.id())
    );
}

#[test]
fn load_untyped_async_resolves_the_handle() {
    let server = server_with_files(&[("data.bytes", b"hello")]);
    server.register_loader(ByteLoader);
    let mut world = world_for(&server);

    let resolved =
        futures_lite::future::block_on(server.load_builder().load_untyped_async("data.bytes"))
            .expect("the untyped load resolves");
    assert_eq!(resolved.type_id(), TypeId::of::<ByteAsset>());
    wait_for(&mut world, &server, resolved.id());

    assert_eq!(
        server.get_handle::<ByteAsset>("data.bytes").map(|handle| handle.id().untyped()),
        Some(resolved.id())
    );
    let typed: Handle<ByteAsset> = resolved.typed_debug_checked();
    assert_eq!(
        world.resource::<Assets<ByteAsset>>().get(&typed),
        Some(&ByteAsset(b"hello".to_vec()))
    );
}

#[test]
fn load_untyped_async_rejects_an_empty_path() {
    let server = server_with_files(&[]);

    let error = futures_lite::future::block_on(server.load_builder().load_untyped_async(""))
        .expect_err("an empty path is rejected");
    assert!(matches!(error, crate::AssetLoadError::EmptyPath(_)));
}

#[test]
fn load_untyped_async_reports_a_loader_error() {
    let server = server_with_files(&[("bad.failing", b"")]);
    server.register_loader(FailingLoader);
    let _world = world_for(&server);

    let error = futures_lite::future::block_on(
        server.load_builder().load_untyped_async("bad.failing"),
    )
    .expect_err("the loader fails");
    assert!(matches!(error, crate::AssetLoadError::AssetLoaderError(_)));
}

#[test]
fn a_failed_untyped_load_marks_the_resolved_handle_failed() {
    let server = server_with_files(&[("bad.failing", b"")]);
    server.register_loader(FailingLoader);
    let mut world = world_for(&server);

    let handle = server.load_builder().load_untyped("bad.failing");
    let wrapper_id = handle.id().untyped();
    wait_for(&mut world, &server, wrapper_id);
    assert!(server.load_state(wrapper_id).is_failed());

    // The resolved typed handle is failed too, not left loading, so a later load
    // of the same path can retry instead of joining a stuck handle.
    let resolved_id = server
        .get_path_id("bad.failing")
        .expect("the resolved handle is registered");
    assert_eq!(resolved_id.type_id(), TypeId::of::<ByteAsset>());
    assert!(server.load_state(resolved_id).is_failed());
}

#[test]
fn override_unapproved_loads_denied_paths_but_never_forbidden_ones() {
    // `Forbid` rejects an escaping path even when the builder overrides.
    let forbidden = server_with_files_and_mode(
        &[("../escape.bytes", b"no")],
        UnapprovedPathMode::Forbid,
    );
    forbidden.register_loader(ByteLoader);
    let rejected = forbidden
        .load_builder()
        .override_unapproved()
        .load::<ByteAsset>("../escape.bytes");
    assert_eq!(rejected, Handle::<ByteAsset>::default());

    // `Deny` rejects the plain load but yields to the builder override.
    let denied =
        server_with_files_and_mode(&[("../escape.bytes", b"ok")], UnapprovedPathMode::Deny);
    denied.register_loader(ByteLoader);
    let mut world = world_for(&denied);

    let rejected = denied.load::<ByteAsset>("../escape.bytes");
    assert_eq!(rejected, Handle::<ByteAsset>::default());

    let handle = denied
        .load_builder()
        .override_unapproved()
        .load::<ByteAsset>("../escape.bytes");
    assert_ne!(handle, Handle::<ByteAsset>::default());
    wait_for(&mut world, &denied, handle.id().untyped());
    assert_eq!(
        world.resource::<Assets<ByteAsset>>().get(&handle),
        Some(&ByteAsset(b"ok".to_vec()))
    );
}

#[test]
fn load_untyped_async_enforces_the_unapproved_gate() {
    // `Forbid` rejects, and cannot be overridden.
    let forbidden =
        server_with_files_and_mode(&[("../escape.bytes", b"no")], UnapprovedPathMode::Forbid);
    let _world = world_for(&forbidden);
    let error = futures_lite::future::block_on(
        forbidden
            .load_builder()
            .override_unapproved()
            .load_untyped_async("../escape.bytes"),
    )
    .expect_err("forbid rejects an unapproved path");
    assert!(matches!(error, crate::AssetLoadError::UnapprovedPath { .. }));

    // `Deny` rejects by default and yields to the builder override.
    let denied =
        server_with_files_and_mode(&[("../escape.bytes", b"ok")], UnapprovedPathMode::Deny);
    denied.register_loader(ByteLoader);
    let mut world = world_for(&denied);

    let rejected = futures_lite::future::block_on(
        denied.load_builder().load_untyped_async("../escape.bytes"),
    );
    assert!(matches!(
        rejected,
        Err(crate::AssetLoadError::UnapprovedPath { .. })
    ));

    let resolved = futures_lite::future::block_on(
        denied
            .load_builder()
            .override_unapproved()
            .load_untyped_async("../escape.bytes"),
    )
    .expect("the override loads the denied path");
    wait_for(&mut world, &denied, resolved.id());
}

#[test]
fn a_guard_is_held_until_the_load_settles() {
    let server = server_with_files(&[("data.bytes", b"hello")]);
    let (started, started_rx) = async_channel::bounded(1);
    let (gate, gate_rx) = async_channel::bounded(1);
    server.register_loader(GatedLoader {
        started,
        gate: gate_rx,
    });
    let mut world = world_for(&server);

    let dropped = Arc::new(AtomicBool::new(false));
    let handle = server
        .load_builder()
        .with_guard(DropFlag(dropped.clone()))
        .load::<ByteAsset>("data.bytes");

    // Wait until the loader is running: the guard must still be held while the
    // load is in flight.
    started_rx.recv_blocking().expect("the loader started");
    assert!(
        !dropped.load(Ordering::SeqCst),
        "the guard is held while the load runs"
    );

    // Release the loader; once the load settles the guard drops.
    gate.send_blocking(()).expect("release the loader gate");
    wait_for(&mut world, &server, handle.id().untyped());
    assert!(
        dropped.load(Ordering::SeqCst),
        "the guard is dropped once the load settles"
    );
}

#[test]
fn a_guard_is_dropped_when_the_load_fails() {
    let server = server_with_files(&[]);
    server.register_loader(ByteLoader);
    let mut world = world_for(&server);

    let dropped = Arc::new(AtomicBool::new(false));
    let handle = server
        .load_builder()
        .with_guard(DropFlag(dropped.clone()))
        .load::<ByteAsset>("missing.bytes");
    wait_for(&mut world, &server, handle.id().untyped());

    assert!(server.load_state(handle.id().untyped()).is_failed());
    assert!(
        dropped.load(Ordering::SeqCst),
        "the guard is dropped when the load fails"
    );
}

#[test]
fn load_folder_collects_every_asset_recursively() {
    let server = server_with_tree(
        &[
            ("folder/a.bytes", b"a"),
            ("folder/sub/b.bytes", b"b"),
            ("folder/sub/deeper/c.bytes", b"c"),
        ],
        &["folder", "folder/sub", "folder/sub/deeper"],
    );
    server.register_loader(ByteLoader);
    let mut world = world_for(&server);

    let handle = server.load_folder("folder");
    let id = handle.id().untyped();
    wait_for(&mut world, &server, id);

    assert!(server.is_loaded_with_dependencies(id));
    let folder = world
        .resource::<Assets<LoadedFolder>>()
        .get(handle.id())
        .expect("the folder asset loaded");
    assert_eq!(folder.handles.len(), 3, "every nested asset is collected");

    let bytes = world.resource::<Assets<ByteAsset>>();
    let mut values: Vec<Vec<u8>> = folder
        .handles
        .iter()
        .map(|handle| {
            assert_eq!(handle.type_id(), TypeId::of::<ByteAsset>());
            bytes
                .get(&handle.clone().typed_debug_checked::<ByteAsset>())
                .expect("every discovered asset is loaded")
                .0
                .clone()
        })
        .collect();
    values.sort();
    assert_eq!(values, vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]);
}

#[test]
fn load_folder_of_an_empty_directory_loads_with_no_handles() {
    let server = server_with_tree(&[], &["empty"]);
    server.register_loader(ByteLoader);
    let mut world = world_for(&server);

    let handle = server.load_folder("empty");
    let id = handle.id().untyped();
    wait_for(&mut world, &server, id);

    assert!(server.is_loaded_with_dependencies(id));
    let folder = world
        .resource::<Assets<LoadedFolder>>()
        .get(handle.id())
        .expect("the empty folder loaded");
    assert!(folder.handles.is_empty());
}

#[test]
fn load_folder_of_a_missing_directory_fails() {
    let server = server_with_tree(&[], &[]);
    server.register_loader(ByteLoader);
    let mut world = world_for(&server);

    let handle = server.load_folder("missing");
    let id = handle.id().untyped();
    wait_for(&mut world, &server, id);

    assert!(server.load_state(id).is_failed());
    assert!(
        world
            .resource::<Assets<LoadedFolder>>()
            .get(handle.id())
            .is_none()
    );
}

#[test]
fn load_folder_skips_files_without_a_loader() {
    let server = server_with_tree(
        &[("mixed/a.bytes", b"a"), ("mixed/readme.txt", b"text")],
        &["mixed"],
    );
    server.register_loader(ByteLoader);
    let mut world = world_for(&server);

    let handle = server.load_folder("mixed");
    let id = handle.id().untyped();
    wait_for(&mut world, &server, id);

    let folder = world
        .resource::<Assets<LoadedFolder>>()
        .get(handle.id())
        .expect("the folder loaded despite the unloadable file");
    assert_eq!(folder.handles.len(), 1);
    assert_eq!(folder.handles[0].type_id(), TypeId::of::<ByteAsset>());
}

#[test]
fn repeated_folder_loads_reuse_the_same_handle() {
    let server = server_with_tree(&[("folder/a.bytes", b"a")], &["folder"]);
    server.register_loader(ByteLoader);
    let mut world = world_for(&server);

    let first = server.load_folder("folder");
    let second = server.load_folder("folder");
    assert_eq!(first.id(), second.id());

    wait_for(&mut world, &server, first.id().untyped());
    assert!(server.is_loaded_with_dependencies(first.id().untyped()));
}

#[test]
fn load_folder_walks_a_real_directory() {
    let dir = std::env::temp_dir().join(format!(
        "kairos_asset_load_folder_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("a.bytes"), b"a").unwrap();
    std::fs::write(dir.join("sub").join("b.bytes"), b"b").unwrap();
    std::fs::write(dir.join("sub").join("note.txt"), b"unloadable").unwrap();
    // A meta sidecar is addressable but must not be collected as an asset.
    std::fs::write(dir.join("a.bytes.meta"), b"unused").unwrap();

    let root = dir.clone();
    let mut builders = AssetSourceBuilders::default();
    builders.insert(
        AssetSourceId::Default,
        AssetSourceBuilder::new(move || {
            Box::new(FileAssetReader::new(&root)) as Box<dyn ErasedAssetReader>
        }),
    );
    let sources = Arc::new(builders.build_sources(false, false));
    let server = AssetServer::new_with_meta_check(
        sources,
        AssetServerMode::Unprocessed,
        AssetMetaCheck::Never,
        false,
        UnapprovedPathMode::Forbid,
    );
    server.register_loader(ByteLoader);
    let mut world = world_for(&server);

    let handle = server.load_folder("");
    let id = handle.id().untyped();
    wait_for(&mut world, &server, id);

    let folder = world
        .resource::<Assets<LoadedFolder>>()
        .get(handle.id())
        .expect("the folder loaded");
    assert_eq!(
        folder.handles.len(),
        2,
        "the nested byte assets load; the meta sidecar is not listed and the unloadable text file is skipped"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn read_asset_bytes_returns_bytes_and_records_a_loader_dependency() {
    let server = server_with_files(&[("data.bytes", b"hello")]);
    let mut context = LoadContext::new(&server, AssetPath::from("root.bytes"), true, false);

    let bytes = futures_lite::future::block_on(context.read_asset_bytes("data.bytes"))
        .expect("the bytes are read");
    assert_eq!(bytes, b"hello");

    // The read is a loader dependency (the processor's input), not a handle
    // dependency: nothing was added to `dependencies`.
    let loaded = context.finish(ByteAsset(vec![]));
    assert!(loaded.dependencies.is_empty());
    assert_eq!(
        loaded.loader_dependencies.get(&AssetPath::from("data.bytes")),
        Some(&[0u8; 32]),
        "without hash population the dependency records the zero hash"
    );
}

#[test]
fn read_asset_bytes_records_the_processed_full_hash() {
    let full_hash = [7u8; 32];
    let meta_bytes = ron::ser::to_string(&ProcessedInfoMinimal {
        processed_info: Some(ProcessedInfo {
            hash: [1u8; 32],
            full_hash,
            process_dependencies: vec![],
        }),
    })
    .expect("the processed info serializes")
    .into_bytes();

    let server = server_with_files(&[
        ("data.bytes", b"hello"),
        ("data.bytes.meta", &meta_bytes),
    ]);
    // A context that populates hashes, as the processor's would.
    let mut context = LoadContext::new(&server, AssetPath::from("root.bytes"), false, true);

    let bytes = futures_lite::future::block_on(context.read_asset_bytes("data.bytes"))
        .expect("the bytes are read");
    assert_eq!(bytes, b"hello");

    let loaded = context.finish(ByteAsset(vec![]));
    assert_eq!(
        loaded.loader_dependencies.get(&AssetPath::from("data.bytes")),
        Some(&full_hash),
        "the processed asset's full_hash is what a dependent records"
    );
}

#[test]
fn read_asset_bytes_rejects_an_empty_path() {
    let server = server_with_files(&[]);
    let mut context = LoadContext::new(&server, AssetPath::from("root.bytes"), true, false);

    let error = futures_lite::future::block_on(context.read_asset_bytes(""))
        .expect_err("an empty path is rejected");
    assert!(matches!(error, ReadAssetBytesError::EmptyPath(_)));
}

#[test]
fn read_asset_bytes_requires_hash_metadata_when_populating_hashes() {
    let meta_bytes = ron::ser::to_string(&ProcessedInfoMinimal {
        processed_info: None,
    })
    .expect("the processed info serializes")
    .into_bytes();
    let server = server_with_files(&[
        ("data.bytes", b"hello"),
        ("data.bytes.meta", &meta_bytes),
    ]);
    let mut context = LoadContext::new(&server, AssetPath::from("root.bytes"), false, true);

    let error = futures_lite::future::block_on(context.read_asset_bytes("data.bytes"))
        .expect_err("hash metadata is required");
    assert!(matches!(error, ReadAssetBytesError::MissingAssetHash));
}
