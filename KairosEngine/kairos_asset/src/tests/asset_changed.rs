//! Tests for the [`AssetChanged`] query filter and the [`AssetChanges`] resource.
//!
//! The `added` and `changed` cases are ported from `bevy_asset` 0.19.1's
//! `asset_changed.rs`, adapted to this crate's `World` + schedule harness: the
//! `Boot`/`Update`/`Events` schedules stand in for Bevy's
//! `Startup`/`Update`/`PostUpdate`, with the event stage playing the role of
//! `AssetEventSystems` in `PostUpdate`.

use kairos_ecs::component::Component;
use kairos_ecs::query::Without;
use kairos_ecs::resource::Resource;
use kairos_ecs::schedule::{IntoScheduleConfigs, ScheduleLabel, Schedules};
use kairos_ecs::system::{Commands, Local, Query, Res, ResMut, assert_is_system};
use kairos_ecs::world::World;

use crate::asset_changed::{AsAssetId, AssetChanges};
use crate::{
    Asset, AssetChanged, AssetEventSystems, AssetId, AssetOptions, AssetWorldExt, Assets, Handle,
    VisitAssetDependencies, install,
};

/// Stands in for Bevy's `Startup` schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Boot;

impl ScheduleLabel for Boot {
    fn dyn_clone(&self) -> Box<dyn ScheduleLabel> {
        Box::new(*self)
    }
}

/// Stands in for Bevy's `PreUpdate`-mounted `AssetTrackingSystems`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Tracking;

impl ScheduleLabel for Tracking {
    fn dyn_clone(&self) -> Box<dyn ScheduleLabel> {
        Box::new(*self)
    }
}

/// Stands in for Bevy's `Update` schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Update;

impl ScheduleLabel for Update {
    fn dyn_clone(&self) -> Box<dyn ScheduleLabel> {
        Box::new(*self)
    }
}

/// Stands in for Bevy's `PostUpdate`-mounted `AssetEventSystems`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Events;

impl ScheduleLabel for Events {
    fn dyn_clone(&self) -> Box<dyn ScheduleLabel> {
        Box::new(*self)
    }
}

#[derive(Debug)]
struct MyAsset(usize, &'static str);

impl Asset for MyAsset {}
impl VisitAssetDependencies for MyAsset {}

#[derive(Component)]
struct MyComponent(Handle<MyAsset>);

impl AsAssetId for MyComponent {
    type Asset = MyAsset;

    fn as_asset_id(&self) -> AssetId<Self::Asset> {
        self.0.id()
    }
}

#[derive(Default, PartialEq, Debug, Resource)]
struct Counter(Vec<u32>);

/// A world with the asset core installed, `MyAsset` registered, and an empty
/// `Counter` sized for the test.
fn asset_world(counter_len: usize) -> World {
    let mut world = World::new();
    install(&mut world, AssetOptions::new(Tracking, Events, Boot));
    world.init_asset::<MyAsset>();
    world.insert_resource(Counter(vec![0; counter_len]));
    world
}

/// Runs one frame's schedules, in Bevy's `PreUpdate` -> `Update` -> `PostUpdate`
/// order. `Boot` is deliberately not run here: each test chooses when (or
/// whether) to run the startup stage.
fn frame(world: &mut World) {
    world.run_schedule(Tracking);
    world.run_schedule(Update);
    world.run_schedule(Events);
}

#[track_caller]
fn assert_counter(world: &World, expected: Counter) {
    assert_eq!(*world.resource::<Counter>(), expected);
}

fn count_update(
    mut counter: ResMut<Counter>,
    assets: Res<Assets<MyAsset>>,
    query: Query<&MyComponent, AssetChanged<MyComponent>>,
) {
    for handle in query.iter() {
        let asset = assets.get(&handle.0).unwrap();
        counter.0[asset.0] += 1;
    }
}

fn update_some(mut assets: ResMut<Assets<MyAsset>>, mut run_count: Local<u32>) {
    let mut update_index = |i| {
        let id = assets
            .iter()
            .find_map(|(h, a)| (a.0 == i).then_some(h))
            .unwrap();
        let mut asset = assets.get_mut(id).unwrap();
        asset.1 = "new_value";
    };
    match *run_count {
        0 | 1 => update_index(0),
        2 => {}
        3 => {
            update_index(0);
            update_index(1);
        }
        4.. => update_index(1),
    };
    *run_count += 1;
}

fn add_some(mut assets: ResMut<Assets<MyAsset>>, mut cmds: Commands, mut run_count: Local<u32>) {
    match *run_count {
        1 => {
            cmds.spawn(MyComponent(assets.add(MyAsset(0, "init"))));
        }
        0 | 2 => {}
        3 => {
            cmds.spawn(MyComponent(assets.add(MyAsset(1, "init"))));
            cmds.spawn(MyComponent(assets.add(MyAsset(2, "init"))));
        }
        4.. => {
            cmds.spawn(MyComponent(assets.add(MyAsset(3, "init"))));
        }
    };
    *run_count += 1;
}

#[test]
#[should_panic]
fn should_conflict() {
    #[derive(Component)]
    struct Foo;

    fn system(
        _: Query<&Foo, AssetChanged<MyComponent>>,
        _: Query<&mut AssetChanges<MyAsset>, Without<Foo>>,
    ) {
    }
    assert_is_system(system);
}

// According to a comment in `QueryState::new`, components on the filter position
// shouldn't conflict with components on the query position.
#[test]
fn handle_filter_pos_ok() {
    fn compatible_filter(_query: Query<&mut MyComponent, AssetChanged<MyComponent>>) {}
    assert_is_system(compatible_filter);
}

#[test]
fn added() {
    let mut world = asset_world(4);
    world
        .get_resource_or_init::<Schedules>()
        .entry(Update)
        .add_systems(add_some);
    world
        .get_resource_or_init::<Schedules>()
        .entry(Events)
        .add_systems(count_update.after(AssetEventSystems));

    frame(&mut world); // run_count == 0
    assert_counter(&world, Counter(vec![0, 0, 0, 0]));
    frame(&mut world); // run_count == 1
    assert_counter(&world, Counter(vec![1, 0, 0, 0]));
    frame(&mut world); // run_count == 2
    assert_counter(&world, Counter(vec![1, 0, 0, 0]));
    frame(&mut world); // run_count == 3
    assert_counter(&world, Counter(vec![1, 1, 1, 0]));
    frame(&mut world); // run_count == 4
    assert_counter(&world, Counter(vec![1, 1, 1, 1]));
}

#[test]
fn changed() {
    let mut world = asset_world(2);

    world
        .get_resource_or_init::<Schedules>()
        .entry(Boot)
        .add_systems(
            |mut cmds: Commands, mut assets: ResMut<Assets<MyAsset>>| {
                let asset0 = assets.add(MyAsset(0, "init"));
                let asset1 = assets.add(MyAsset(1, "init"));
                cmds.spawn(MyComponent(asset0.clone()));
                cmds.spawn(MyComponent(asset0));
                cmds.spawn(MyComponent(asset1.clone()));
                cmds.spawn(MyComponent(asset1.clone()));
                cmds.spawn(MyComponent(asset1));
            },
        );
    world
        .get_resource_or_init::<Schedules>()
        .entry(Update)
        .add_systems(update_some);
    world
        .get_resource_or_init::<Schedules>()
        .entry(Events)
        .add_systems(count_update.after(AssetEventSystems));

    // First frame runs `Startup`, then `Update` and the event stage.
    world.run_schedule(Boot);
    frame(&mut world); // run_count == 0

    // First run: count the entities that were added in `Boot`.
    assert_counter(&world, Counter(vec![2, 3]));

    // Second run: `update_some` updates the first asset, which is associated
    // with two entities, so `count_update` picks up two updates.
    frame(&mut world); // run_count == 1
    assert_counter(&world, Counter(vec![4, 3]));

    // Third run: `update_some` doesn't update anything, so the values hold.
    frame(&mut world); // run_count == 2
    assert_counter(&world, Counter(vec![4, 3]));

    // Fourth run: update both assets (asset 0: 2 entities, asset 1: 3).
    frame(&mut world); // run_count == 3
    assert_counter(&world, Counter(vec![6, 6]));

    // Fifth run: only update the second asset.
    frame(&mut world); // run_count == 4
    assert_counter(&world, Counter(vec![6, 9]));

    // Sixth run: again only the second asset.
    frame(&mut world); // run_count == 5
    assert_counter(&world, Counter(vec![6, 12]));
}

/// The `AssetChanges` resource is created lazily, by `AssetChanged`'s
/// `init_state`: a program that never uses the filter never pays for it.
#[test]
fn asset_changes_is_created_lazily() {
    let mut world = asset_world(2);

    // Create the (empty) update stage so `frame` can run it.
    world.get_resource_or_init::<Schedules>().entry(Update);

    // Only the store and its drivers are registered so far.
    assert!(world.get_resource::<AssetChanges<MyAsset>>().is_none());
    frame(&mut world);
    assert!(
        world.get_resource::<AssetChanges<MyAsset>>().is_none(),
        "the store's event driver must not force the resource into existence"
    );

    // Registering a system that uses `AssetChanged` creates it on first run.
    world
        .get_resource_or_init::<Schedules>()
        .entry(Events)
        .add_systems(count_update.after(AssetEventSystems));
    frame(&mut world);
    assert!(world.get_resource::<AssetChanges<MyAsset>>().is_some());
}
