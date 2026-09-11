//! Tests for registration and scheduling: `install`, the per-type `init_asset`
//! API, and the tracking/event stage split.
//!
//! Core invariants that need the loading pipeline — same-path loads reuse one id,
//! and a missing file fails without panicking — live beside the pipeline in
//! `tests/server.rs`. The tests here exercise registration and event timing
//! deterministically, through `AssetServer::add` rather than a load task.

use kairos_ecs::message::{Message, MessageReader, MessageRegistry, Messages};
use kairos_ecs::resource::Resource;
use kairos_ecs::schedule::{IntoScheduleConfigs, ScheduleLabel, Schedules};
use kairos_ecs::system::ResMut;
use kairos_ecs::world::World;

use crate::next::{
    Asset, AssetEvent, AssetEventSystems, AssetLoadFailedEvent, AssetServer, AssetStages,
    AssetWorldExt, Assets, VisitAssetDependencies, install,
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
    install(&mut world, Tracking, Events);
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
    install(&mut world, Tracking, Events);

    let stages = world.resource::<AssetStages>();
    assert_eq!(stages.tracking, Tracking.intern());
    assert_eq!(stages.event, Events.intern());

    let schedules = world.resource::<Schedules>();
    assert!(schedules.contains(Tracking));
    assert!(schedules.contains(Events));
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
