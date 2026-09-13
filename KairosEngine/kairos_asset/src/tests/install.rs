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

use futures_lite::future::block_on;
use kairos_ecs::message::{Message, MessageReader, MessageRegistry, Messages};
use kairos_ecs::resource::Resource;
use kairos_ecs::schedule::{IntoScheduleConfigs, ScheduleLabel, Schedules};
use kairos_ecs::system::ResMut;
use kairos_ecs::world::World;
use kairos_tasks::ConditionalSendFuture;

use crate::io::{
    AssetReader, AssetReaderError, AssetReaderFuture, AssetSourceBuilder, AssetSourceBuilders,
    AssetSourceId, ErasedAssetReader, PathStream, Reader, UnapprovedPathMode, VecReader,
    empty_path_stream,
};
use crate::{
    Asset, AssetEvent, AssetEventSystems, AssetLoadFailedEvent, AssetLoader, AssetMetaCheck,
    AssetMode, AssetOptions, AssetProcessor, AssetServer, AssetServerMode, AssetStages,
    AssetWorldExt, Assets, DirectAssetAccessExt, Handle, LoadContext, VisitAssetDependencies, install,
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
    assert_eq!(options.use_asset_processor, cfg!(feature = "asset_processor"));
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

/// Reloading a path after the last handle dropped and the tracking stage ran
/// resolves to a fresh, present value.
///
/// This is the regression for the asset server keeping an [`AssetInfo`] whose
/// strong handle is dead: a later `load` used to reuse the recycled slot and
/// hand back a handle that would never resolve.
///
/// [`AssetInfo`]: crate::server::AssetInfos
#[test]
fn reloading_after_the_last_handle_dropped_loads_again() {
    let source = AssetSourceBuilder::new({
        let reader = MemoryReader::new(&[("data.bytes", b"source")]);
        move || Box::new(reader.clone()) as Box<dyn ErasedAssetReader>
    });
    let mut world = world_for_layout(source, AssetMode::Unprocessed);
    let server = world.resource::<AssetServer>().clone();

    let handle = server.load::<ByteAsset>("data.bytes");
    let first_id = handle.id();
    load_until_settled(&mut world, &server, &handle);
    assert_eq!(
        world.resource::<Assets<ByteAsset>>().get(first_id),
        Some(&ByteAsset(b"source".to_vec()))
    );

    // The holder closes: the last strong handle drops, and the tracking stage
    // releases both the value and the server's bookkeeping for it.
    drop(handle);
    world.run_schedule(Tracking);
    assert_eq!(world.resource::<Assets<ByteAsset>>().get(first_id), None);

    // Asking for the same path again must load it afresh rather than hand back
    // a handle to a slot nothing will fill.
    let handle = server.load::<ByteAsset>("data.bytes");
    load_until_settled(&mut world, &server, &handle);
    assert_eq!(
        world.resource::<Assets<ByteAsset>>().get(handle.id()),
        Some(&ByteAsset(b"source".to_vec())),
        "the value must be present again after the previous handles dropped"
    );
}

/// Re-requesting a path before the tracking stage has consumed the pending drop
/// keeps the value, because the fresh handle shares the still-pending slot.
#[test]
fn loading_again_before_tracking_consumes_the_pending_drop() {
    let source = AssetSourceBuilder::new({
        let reader = MemoryReader::new(&[("data.bytes", b"source")]);
        move || Box::new(reader.clone()) as Box<dyn ErasedAssetReader>
    });
    let mut world = world_for_layout(source, AssetMode::Unprocessed);
    let server = world.resource::<AssetServer>().clone();

    let handle = server.load::<ByteAsset>("data.bytes");
    let id = handle.id();
    load_until_settled(&mut world, &server, &handle);

    // The last handle drops, but no tracking stage runs before the path is
    // asked for again: the drop event is still queued.
    drop(handle);
    let handle = server.load::<ByteAsset>("data.bytes");
    assert_eq!(handle.id(), id, "the pending slot is reused");

    // The tracking stage now sees the stale drop; it must skip it rather than
    // release the value the fresh handle is holding.
    load_until_settled(&mut world, &server, &handle);
    assert_eq!(
        world.resource::<Assets<ByteAsset>>().get(handle.id()),
        Some(&ByteAsset(b"source".to_vec())),
        "a stale drop must not release a slot that was just re-opened"
    );
}

/// The `unapproved_path_mode` option reaches the server that `install` builds.
#[test]
fn the_unapproved_path_mode_option_reaches_the_server() {
    // A reader holding `../escape.bytes`, which only loads when an escaping path
    // is permitted.
    fn escape_source() -> AssetSourceBuilder {
        let reader = MemoryReader::new(&[("../escape.bytes", b"escaped")]);
        AssetSourceBuilder::new(move || Box::new(reader.clone()) as Box<dyn ErasedAssetReader>)
    }

    // The default is `Forbid`: the escaping path is refused outright, before
    // any handle is allocated.
    let forbidden = world_for_layout(escape_source(), AssetMode::Unprocessed);
    {
        let server = forbidden.resource::<AssetServer>();
        assert_eq!(
            server.load::<ByteAsset>("../escape.bytes"),
            Handle::<ByteAsset>::default(),
            "the default mode must refuse an escaping path"
        );
    }

    // Overriding the option on `AssetOptions` lets it load.
    let mut allowing = World::new();
    allowing
        .get_resource_or_init::<AssetSourceBuilders>()
        .insert(AssetSourceId::Default, escape_source());
    install(
        &mut allowing,
        options()
            .with_meta_check(AssetMetaCheck::Never)
            .with_unapproved_path_mode(UnapprovedPathMode::Allow),
    );
    allowing.init_asset::<ByteAsset>();
    allowing.register_asset_loader(ByteLoader);

    let server = allowing.resource::<AssetServer>().clone();
    let handle = server.load::<ByteAsset>("../escape.bytes");
    load_until_settled(&mut allowing, &server, &handle);
    assert_eq!(
        allowing.resource::<Assets<ByteAsset>>().get(handle.id()),
        Some(&ByteAsset(b"escaped".to_vec()))
    );
}

/// `register_asset_source` makes a named source resolvable, and the per-type
/// registration calls chain.
#[test]
fn register_asset_source_registers_a_named_source() {
    let mut world = World::new();
    let reader = MemoryReader::new(&[("data.bytes", b"named")]);
    world.register_asset_source(
        "named",
        AssetSourceBuilder::new(move || Box::new(reader.clone()) as Box<dyn ErasedAssetReader>),
    );

    install(&mut world, options().with_meta_check(AssetMetaCheck::Never));
    world.init_asset::<ByteAsset>().register_asset_loader(ByteLoader);

    let server = world.resource::<AssetServer>().clone();
    let handle = server.load::<ByteAsset>("named://data.bytes");
    load_until_settled(&mut world, &server, &handle);
    assert_eq!(
        world.resource::<Assets<ByteAsset>>().get(handle.id()),
        Some(&ByteAsset(b"named".to_vec()))
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
fn processed_file_path_is_configurable_and_excluded() {
    let mut world = World::new();
    install(
        &mut world,
        options()
            .with_mode(AssetMode::Processed)
            .with_processed_file_path("out/imported"),
    );
    let source = world
        .resource::<AssetServer>()
        .get_source(AssetSourceId::Default)
        .expect("the default source exists");
    assert!(
        source.processed_reader().is_ok(),
        "the custom processed root wires a processed reader"
    );
    assert_eq!(
        source.unprocessed_exclude(),
        Some(std::path::Path::new("out")),
        "the custom processed root's top-level directory is excluded from the source scan"
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
        block_on(
            processor
                .server()
                .get_asset_loader_with_type_name(crate::meta::loader_name::<ByteLoader>())
        )
        .is_ok(),
        "layout ② shares the loader registry between both servers"
    );
}

#[test]
fn watch_for_changes_override_sets_the_servers_watching_flag() {
    // Watching follows the `watch` feature unless overridden, and the default
    // feature set enables `file_watcher` (which implies `watch`).
    let mut default_world = World::new();
    install(&mut default_world, options());
    assert_eq!(
        default_world
            .resource::<AssetServer>()
            .watching_for_changes(),
        cfg!(feature = "watch"),
        "watching follows the `watch` feature when it is not overridden"
    );

    let mut unwatched_world = World::new();
    install(
        &mut unwatched_world,
        options().with_watch_for_changes_override(false),
    );
    assert!(
        !unwatched_world
            .resource::<AssetServer>()
            .watching_for_changes(),
        "an explicit `false` turns watching off"
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

#[test]
fn preregister_asset_loader_registers_through_the_world() {
    let mut world = World::new();
    install(&mut world, options());
    world.init_asset::<ByteAsset>();

    world.preregister_asset_loader::<ByteLoader>(&["bytes"]);
    world.register_asset_loader(ByteLoader);

    let server = world.resource::<AssetServer>().clone();
    let loader = block_on(server.get_asset_loader_with_extension("bytes"))
        .expect("a loader preregistered and registered through the world resolves");
    assert_eq!(loader.type_name(), crate::meta::loader_name::<ByteLoader>());
}

#[test]
fn direct_access_adds_an_asset_through_the_world() {
    let mut world = installed_world();

    let handle = world.add_asset(ByteAsset(b"direct".to_vec()));

    assert_eq!(
        world.resource::<Assets<ByteAsset>>().get(handle.id()),
        Some(&ByteAsset(b"direct".to_vec()))
    );
}

#[test]
fn direct_access_loads_an_asset_through_the_world() {
    let reader = MemoryReader::new(&[("data.bytes", b"loaded")]);
    let mut world = world_for_layout(
        AssetSourceBuilder::new(move || Box::new(reader.clone()) as Box<dyn ErasedAssetReader>),
        AssetMode::Unprocessed,
    );
    let server = world.resource::<AssetServer>().clone();

    let handle = world.load_asset::<ByteAsset>("data.bytes");
    load_until_settled(&mut world, &server, &handle);

    assert_eq!(
        world.resource::<Assets<ByteAsset>>().get(handle.id()),
        Some(&ByteAsset(b"loaded".to_vec()))
    );
}

#[test]
fn direct_access_load_builder_starts_a_load_through_the_world() {
    let reader = MemoryReader::new(&[("data.bytes", b"built")]);
    let mut world = world_for_layout(
        AssetSourceBuilder::new(move || Box::new(reader.clone()) as Box<dyn ErasedAssetReader>),
        AssetMode::Unprocessed,
    );
    let server = world.resource::<AssetServer>().clone();

    let handle = world.load_builder().load::<ByteAsset>("data.bytes");
    load_until_settled(&mut world, &server, &handle);

    assert_eq!(
        world.resource::<Assets<ByteAsset>>().get(handle.id()),
        Some(&ByteAsset(b"built".to_vec()))
    );
}
