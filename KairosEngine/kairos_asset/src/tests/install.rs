//! Tests for registration and scheduling: `install`, the per-type `init_asset`
//! API, and the tracking/event/startup stage split.
//!
//! Core invariants that need the loading pipeline — same-path loads reuse one id,
//! and a missing file fails without panicking — live beside the pipeline in
//! `tests/server.rs`. Most of the tests here exercise registration and event
//! timing deterministically, through `AssetServer::add` rather than a load task;
//! the two layout tests below do drive a real `load` so they can prove which
//! reader each `AssetMode` selects.

use std::sync::Arc;

use kairos_ecs::message::{Message, MessageReader, MessageRegistry, Messages};
use kairos_ecs::resource::Resource;
use kairos_ecs::schedule::{IntoScheduleConfigs, ScheduleLabel, Schedules};
use kairos_ecs::system::ResMut;
use kairos_ecs::world::World;
use kairos_tasks::ConditionalSendFuture;

use crate::io::{
    AssetReader, AssetReaderError, AssetReaderFuture, AssetSourceBuilder, AssetSourceBuilders,
    AssetSourceId, ErasedAssetReader, PathStream, Reader, VecReader, empty_path_stream,
};
use crate::{
    Asset, AssetEvent, AssetEventSystems, AssetLoadFailedEvent, AssetLoader, AssetMetaCheck,
    AssetMode, AssetOptions, AssetProcessor, AssetServer, AssetServerMode, AssetStages,
    AssetWorldExt, Assets, Handle, LoadContext, VisitAssetDependencies, install,
};

/// The two ad-hoc stages the asset drivers are installed into.
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

/// The ad-hoc stage the startup record points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Boot;

impl ScheduleLabel for Boot {
    fn dyn_clone(&self) -> Box<dyn ScheduleLabel> {
        Box::new(*self)
    }
}

/// The default options over the three ad-hoc stages.
fn options() -> AssetOptions {
    AssetOptions::new(Tracking, Events, Boot)
}

/// An asset whose value is a byte payload, registered without a loader.
#[derive(Debug, PartialEq, Eq)]
struct ByteAsset(Vec<u8>);

impl Asset for ByteAsset {}
impl VisitAssetDependencies for ByteAsset {}

/// A second asset type, to show stores are independent.
#[derive(Debug, PartialEq, Eq)]
struct OtherAsset(u32);

impl Asset for OtherAsset {}
impl VisitAssetDependencies for OtherAsset {}

/// A world with the asset core installed and `ByteAsset` registered.
fn installed_world() -> World {
    let mut world = World::new();
    install(&mut world, options());
    world.init_asset::<ByteAsset>();
    world
}

/// Drains every message of type `M` currently buffered.
fn drain<M: Message>(world: &mut World) -> Vec<M> {
    world.resource_mut::<Messages<M>>().drain().collect()
}

#[test]
fn install_records_the_caller_stages() {
    let mut world = World::new();
    install(&mut world, options());

    let stages = world.resource::<AssetStages>();
    assert_eq!(stages.tracking, Tracking.intern());
    assert_eq!(stages.event, Events.intern());
    assert_eq!(stages.startup, Boot.intern());

    let schedules = world.resource::<Schedules>();
    assert!(schedules.contains(Tracking));
    assert!(schedules.contains(Events));
    assert!(schedules.contains(Boot));
}

#[test]
fn init_asset_registers_the_store_the_messages_and_the_drivers() {
    let mut world = installed_world();

    assert!(world.get_resource::<Assets<ByteAsset>>().is_some());
    assert!(world.get_resource::<Messages<AssetEvent<ByteAsset>>>().is_some());
    assert!(world.get_resource::<Messages<AssetLoadFailedEvent<ByteAsset>>>().is_some());
    assert!(world.get_resource::<MessageRegistry>().is_some());

    // The handle provider and both sender maps are registered with the server:
    // an `add` produces a handle whose value reaches the store, and a
    // `LoadedWithDependencies` event, once the drivers run.
    let handle = world.resource::<AssetServer>().add(ByteAsset(b"hi".to_vec()));
    let id = handle.id();
    world.run_schedule(Tracking);

    assert!(world.resource::<Assets<ByteAsset>>().get(id).is_some());
    let events = drain::<AssetEvent<ByteAsset>>(&mut world);
    assert!(events.iter().any(|event| event.is_loaded_with_dependencies(id)));
}

#[test]
fn loaded_with_dependencies_lands_in_tracking_and_added_waits_for_the_event_stage() {
    let mut world = installed_world();

    let handle = world.resource::<AssetServer>().add(ByteAsset(b"hi".to_vec()));
    let id = handle.id();

    // `add` only queues the value; nothing is stored until the tracking stage
    // drains it.
    assert!(world.resource::<Assets<ByteAsset>>().get(id).is_none());

    world.run_schedule(Tracking);
    assert_eq!(
        world.resource::<Assets<ByteAsset>>().get(id),
        Some(&ByteAsset(b"hi".to_vec()))
    );

    let after_tracking = drain::<AssetEvent<ByteAsset>>(&mut world);
    assert!(
        after_tracking
            .iter()
            .any(|event| event.is_loaded_with_dependencies(id)),
        "LoadedWithDependencies is written directly during tracking"
    );
    assert!(
        !after_tracking.iter().any(|event| event.is_added(id)),
        "Added is still queued in the store until the event stage flushes it"
    );

    world.run_schedule(Events);
    let after_events = drain::<AssetEvent<ByteAsset>>(&mut world);
    assert!(after_events.iter().any(|event| event.is_added(id)));
}

#[derive(Resource, Default)]
struct Observed(Vec<AssetEvent<ByteAsset>>);

fn observe(mut reader: MessageReader<AssetEvent<ByteAsset>>, mut observed: ResMut<Observed>) {
    observed.0.extend(reader.read().copied());
}

#[test]
fn asset_events_reach_a_message_reader_after_the_event_stage() {
    let mut world = installed_world();
    world.insert_resource(Observed::default());
    world
        .get_resource_or_init::<Schedules>()
        .entry(Events)
        .add_systems(observe.after(AssetEventSystems));

    let handle = world.resource::<AssetServer>().add(ByteAsset(b"hi".to_vec()));
    let id = handle.id();

    world.run_schedule(Tracking);
    world.run_schedule(Events);

    let observed = world.resource::<Observed>();
    assert!(observed.0.iter().any(|event| event.is_added(id)));
    assert!(observed.0.iter().any(|event| event.is_loaded_with_dependencies(id)));
}

#[test]
fn insert_is_immediately_visible_through_the_registered_store() {
    let mut world = installed_world();

    let handle = world
        .resource_mut::<Assets<ByteAsset>>()
        .add(ByteAsset(b"now".to_vec()));
    assert_eq!(
        world.resource::<Assets<ByteAsset>>().get(handle.id()),
        Some(&ByteAsset(b"now".to_vec()))
    );
}

#[test]
fn dropping_the_last_strong_handle_releases_the_asset_in_the_tracking_stage() {
    let mut world = installed_world();

    let handle = world.resource::<AssetServer>().add(ByteAsset(b"x".to_vec()));
    let id = handle.id();
    world.run_schedule(Tracking);
    assert!(world.resource::<Assets<ByteAsset>>().contains(id));

    // Dropping the last strong handle sends a drop event; the next tracking
    // stage releases the value.
    drop(handle);
    world.run_schedule(Tracking);
    assert!(!world.resource::<Assets<ByteAsset>>().contains(id));

    world.run_schedule(Events);
    let events = drain::<AssetEvent<ByteAsset>>(&mut world);
    assert!(events.iter().any(|event| event.is_unused(id)));
    assert!(events.iter().any(|event| event.is_removed(id)));
}

#[test]
fn stores_are_independent_per_registered_type() {
    let mut world = installed_world();
    world.init_asset::<OtherAsset>();

    let bytes = world
        .resource_mut::<Assets<ByteAsset>>()
        .add(ByteAsset(b"b".to_vec()));
    let other = world.resource_mut::<Assets<OtherAsset>>().add(OtherAsset(7));

    assert!(world.resource::<Assets<ByteAsset>>().get(bytes.id()).is_some());
    assert!(world.resource::<Assets<OtherAsset>>().get(other.id()).is_some());
    assert_eq!(world.resource::<Assets<ByteAsset>>().len(), 1);
    assert_eq!(world.resource::<Assets<OtherAsset>>().len(), 1);
}

/// A loader that returns the reader's bytes, so the layout tests can prove a
/// load reaches the source the mode selects.
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

/// An in-memory [`AssetReader`] over a fixed path-to-bytes map.
#[derive(Clone)]
struct MemoryReader(Arc<std::collections::HashMap<std::path::PathBuf, Vec<u8>>>);

impl MemoryReader {
    fn new(files: &[(&str, &[u8])]) -> Self {
        Self(Arc::new(
            files
                .iter()
                .map(|(path, bytes)| (std::path::PathBuf::from(path), bytes.to_vec()))
                .collect(),
        ))
    }
}

impl AssetReader for MemoryReader {
    fn read<'a>(&'a self, path: &'a std::path::Path) -> impl AssetReaderFuture<Value: Reader + 'a> {
        async move {
            self.0
                .get(path)
                .cloned()
                .map(VecReader::new)
                .ok_or_else(|| AssetReaderError::NotFound(path.to_path_buf()))
        }
    }

    fn read_meta<'a>(
        &'a self,
        path: &'a std::path::Path,
    ) -> impl AssetReaderFuture<Value: Reader + 'a> {
        async move { Err::<VecReader, _>(AssetReaderError::NotFound(path.to_path_buf())) }
    }

    fn read_directory<'a>(
        &'a self,
        _path: &'a std::path::Path,
    ) -> impl ConditionalSendFuture<Output = Result<Box<PathStream>, AssetReaderError>> {
        async move { Ok(empty_path_stream()) }
    }

    fn is_directory<'a>(
        &'a self,
        _path: &'a std::path::Path,
    ) -> impl ConditionalSendFuture<Output = Result<bool, AssetReaderError>> {
        async move { Ok(false) }
    }
}

/// Builds a world whose pre-registered default source is `builder`, then installs
/// the core in `mode`. Pre-registering a default source is what lets the layout
/// tests stay off the filesystem: `install` only fills a default in when the
/// host has not supplied one.
fn world_for_layout(builder: AssetSourceBuilder, mode: AssetMode) -> World {
    let mut world = World::new();
    world
        .get_resource_or_init::<AssetSourceBuilders>()
        .insert(AssetSourceId::Default, builder);
    install(
        &mut world,
        options().with_mode(mode).with_meta_check(AssetMetaCheck::Never),
    );
    world.init_asset::<ByteAsset>();
    world.register_asset_loader(ByteLoader);
    world
}

/// Runs the tracking stage until the load for `handle` settles.
fn load_until_settled(world: &mut World, server: &AssetServer, handle: &Handle<ByteAsset>) {
    let id = handle.id();
    for _ in 0..5000 {
        world.run_schedule(Tracking);
        let state = server.load_state(id);
        if state.is_failed() || server.is_loaded_with_dependencies(id) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    panic!("asset {id:?} never settled; state was {:?}", server.load_state(id));
}

#[test]
fn asset_options_default_to_unprocessed_with_the_processor_feature_off() {
    let options = options();
    assert_eq!(options.mode, AssetMode::Unprocessed);
    assert_eq!(options.use_asset_processor, cfg!(feature = "use_asset_processor"));
    assert!(options.watch_for_changes_override.is_none());
}

/// Layout ①: `AssetMode::Unprocessed` reads the source-side reader.
#[test]
fn unprocessed_mode_loads_from_the_source_reader() {
    let source = AssetSourceBuilder::new({
        let reader = MemoryReader::new(&[("data.bytes", b"source")]);
        move || Box::new(reader.clone()) as Box<dyn ErasedAssetReader>
    });
    let mut world = world_for_layout(source, AssetMode::Unprocessed);

    let server = world.resource::<AssetServer>().clone();
    assert_eq!(server.mode(), AssetServerMode::Unprocessed);

    let handle = server.load::<ByteAsset>("data.bytes");
    load_until_settled(&mut world, &server, &handle);

    assert_eq!(
        world.resource::<Assets<ByteAsset>>().get(handle.id()),
        Some(&ByteAsset(b"source".to_vec()))
    );
}

/// Layout ③: `AssetMode::Processed` without a processor reads the pre-built
/// processed reader, not the source side.
#[test]
fn processed_mode_without_a_processor_loads_from_the_processed_reader() {
    let source = AssetSourceBuilder::new({
        let reader = MemoryReader::new(&[("data.bytes", b"source")]);
        move || Box::new(reader.clone()) as Box<dyn ErasedAssetReader>
    })
    .with_processed_reader({
        let reader = MemoryReader::new(&[("data.bytes", b"processed")]);
        move || Box::new(reader.clone()) as Box<dyn ErasedAssetReader>
    });
    let mut world = world_for_layout(source, AssetMode::Processed);

    let server = world.resource::<AssetServer>().clone();
    assert_eq!(server.mode(), AssetServerMode::Processed);

    let handle = server.load::<ByteAsset>("data.bytes");
    load_until_settled(&mut world, &server, &handle);

    assert_eq!(
        world.resource::<Assets<ByteAsset>>().get(handle.id()),
        Some(&ByteAsset(b"processed".to_vec()))
    );
}

#[test]
fn processed_mode_gives_the_default_source_a_processed_reader() {
    // With no host-supplied default source, `install` roots the processed reader
    // at the crate's default processed path.
    let mut processed_world = World::new();
    install(&mut processed_world, options().with_mode(AssetMode::Processed));
    let source = processed_world
        .resource::<AssetServer>()
        .get_source(AssetSourceId::Default)
        .expect("the default source exists");
    assert!(
        source.processed_reader().is_ok(),
        "processed mode wires the default source's processed reader"
    );

    let mut unprocessed_world = World::new();
    install(&mut unprocessed_world, options());
    let source = unprocessed_world
        .resource::<AssetServer>()
        .get_source(AssetSourceId::Default)
        .expect("the default source exists");
    assert!(
        source.processed_reader().is_err(),
        "unprocessed mode leaves the processed reader empty"
    );
}

#[test]
fn processed_mode_with_a_processor_wires_layout_2() {
    let source = AssetSourceBuilder::new({
        let reader = MemoryReader::new(&[("data.bytes", b"source")]);
        move || Box::new(reader.clone()) as Box<dyn ErasedAssetReader>
    });
    let mut world = World::new();
    world
        .get_resource_or_init::<AssetSourceBuilders>()
        .insert(AssetSourceId::Default, source);
    install(
        &mut world,
        options()
            .with_mode(AssetMode::Processed)
            .with_use_asset_processor(true)
            .with_meta_check(AssetMetaCheck::Never),
    );
    world.init_asset::<ByteAsset>();
    world.register_asset_loader(ByteLoader);

    // The processor is mounted and the app's server runs in Processed mode.
    let processor = world
        .get_resource::<AssetProcessor>()
        .expect("layout ② mounts the processor");
    assert_eq!(
        world.resource::<AssetServer>().mode(),
        AssetServerMode::Processed
    );

    // The two servers share one loader registry: a loader registered on the
    // app's server is visible to the processor's server (which needs it to
    // resolve source-asset loaders while processing).
    assert!(
        processor
            .server()
            .get_asset_loader_with_type_name(crate::meta::loader_name::<ByteLoader>())
            .is_ok(),
        "layout ② shares the loader registry between both servers"
    );
}

#[test]
fn watch_for_changes_override_sets_the_servers_watching_flag() {
    let mut default_world = World::new();
    install(&mut default_world, options());
    assert!(
        !default_world
            .resource::<AssetServer>()
            .watching_for_changes(),
        "watching defaults to off until the watch feature lands"
    );

    let mut watched_world = World::new();
    install(
        &mut watched_world,
        options().with_watch_for_changes_override(true),
    );
    assert!(watched_world
        .resource::<AssetServer>()
        .watching_for_changes());
}
