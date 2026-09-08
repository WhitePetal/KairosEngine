use std::any::TypeId;

use kairos_ecs_macros::{Resource, ScheduleLabel, SystemSet};

use crate::{
    ecs::{
        change_detection::{Res, ResMut},
        error::{FallbackErrorHandler, Result, ignore, panic},
        schedule::{
            ApplyDeferred, FlattenedDependencies, IntoScheduleConfigs, IntoSystemSet, NodeId,
            Schedule, ScheduleBuildError, ScheduleBuildPass, ScheduleBuildSettings,
            ScheduleCleanupPolicy, Schedules, SystemKey, SystemSet, SystemSetKey, graph::DiGraph,
            passes::AutoInsertApplyDeferredPass,
        },
        system::Commands,
        world::World,
    },
    hash::FixedHasher,
};

#[derive(Resource)]
struct Resource1;

#[derive(Resource)]
struct Resource2;

#[test]
fn unchanged_auto_insert_apply_deferred_has_no_effect() {
    #[derive(PartialEq, Debug)]
    enum Entry {
        System(usize),
        SyncPoint(usize),
    }

    #[derive(Resource, Default)]
    struct Log(Vec<Entry>);

    fn system<const N: usize>(mut res: ResMut<Log>, mut commands: Commands) {
        res.0.push(Entry::System(N));
        commands.queue(|world: &mut World| world.resource_mut::<Log>().0.push(Entry::SyncPoint(N)));
    }

    let mut world = World::default();
    world.init_resource::<Log>();
    let mut schedule = Schedule::default();
    schedule.add_systems((system::<1>, system::<2>).chain_ignore_deferred());
    schedule.set_build_settings(ScheduleBuildSettings {
        auto_insert_apply_deferred: true,
        ..Default::default()
    });
    schedule.run(&mut world);
    let actual = world.remove_resource::<Log>().unwrap().0;

    let expected = vec![
        Entry::System(1),
        Entry::System(2),
        Entry::SyncPoint(1),
        Entry::SyncPoint(2),
    ];

    assert_eq!(actual, expected);
}

// regression test for https://github.com/bevyengine/bevy/issues/9114
#[test]
fn ambiguous_with_not_breaking_run_conditions() {
    #[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
    struct Set;

    let mut world = World::new();
    let mut schedule = Schedule::default();

    let system: fn() = || {
        panic!("This system must not run");
    };

    schedule.configure_sets(Set.run_if(|| false));
    schedule.add_systems(system.ambiguous_with(|| ()).in_set(Set));
    schedule.run(&mut world);
}

#[test]
fn inserts_a_sync_point() {
    let mut schedule = Schedule::default();
    let mut world = World::default();
    schedule.add_systems(
        (
            |mut commands: Commands| commands.insert_resource(Resource1),
            |_: Res<Resource1>| {},
        )
            .chain(),
    );
    schedule.run(&mut world);

    // inserted a sync point
    assert_eq!(schedule.executable.systems.len(), 3);
}

#[test]
fn explicit_sync_point_used_as_auto_sync_point() {
    let mut schedule = Schedule::default();
    let mut world = World::default();
    schedule.add_systems(
        (
            |mut commands: Commands| commands.insert_resource(Resource1),
            |_: Res<Resource1>| {},
        )
            .chain(),
    );
    schedule.add_systems((|| {}, ApplyDeferred, || {}).chain());
    schedule.run(&mut world);

    // No sync point was inserted, since we can reuse the explicit sync point.
    assert_eq!(schedule.executable.systems.len(), 5);
}

#[test]
fn conditional_explicit_sync_point_not_used_as_auto_sync_point() {
    let mut schedule = Schedule::default();
    let mut world = World::default();
    schedule.add_systems(
        (
            |mut commands: Commands| commands.insert_resource(Resource1),
            |_: Res<Resource1>| {},
        )
            .chain(),
    );
    schedule.add_systems((|| {}, ApplyDeferred.run_if(|| false), || {}).chain());
    schedule.run(&mut world);

    // A sync point was inserted, since the explicit sync point is not always run.
    assert_eq!(schedule.executable.systems.len(), 6);
}

#[test]
fn conditional_explicit_sync_point_not_used_as_auto_sync_point_condition_on_chain() {
    let mut schedule = Schedule::default();
    let mut world = World::default();
    schedule.add_systems(
        (
            |mut commands: Commands| commands.insert_resource(Resource1),
            |_: Res<Resource1>| {},
        )
            .chain(),
    );
    schedule.add_systems((|| {}, ApplyDeferred, || {}).chain().run_if(|| false));
    schedule.run(&mut world);

    // A sync point was inserted, since the explicit sync point is not always run.
    assert_eq!(schedule.executable.systems.len(), 6);
}

#[test]
fn conditional_explicit_sync_point_not_used_as_auto_sync_point_condition_on_system_set() {
    #[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
    struct Set;

    let mut schedule = Schedule::default();
    let mut world = World::default();
    schedule.configure_sets(Set.run_if(|| false));
    schedule.add_systems(
        (
            |mut commands: Commands| commands.insert_resource(Resource1),
            |_: Res<Resource1>| {},
        )
            .chain(),
    );
    schedule.add_systems((|| {}, ApplyDeferred.in_set(Set), || {}).chain());
    schedule.run(&mut world);

    // A sync point was inserted, since the explicit sync point is not always run.
    assert_eq!(schedule.executable.systems.len(), 6);
}

#[test]
fn conditional_explicit_sync_point_not_used_as_auto_sync_point_condition_on_nested_system_set() {
    #[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
    struct Set1;
    #[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
    struct Set2;

    let mut schedule = Schedule::default();
    let mut world = World::default();
    schedule.configure_sets(Set2.run_if(|| false));
    schedule.configure_sets(Set1.in_set(Set2));
    schedule.add_systems(
        (
            |mut commands: Commands| commands.insert_resource(Resource1),
            |_: Res<Resource1>| {},
        )
            .chain(),
    );
    schedule.add_systems((|| {}, ApplyDeferred, || {}).chain().in_set(Set1));
    schedule.run(&mut world);

    // A sync point was inserted, since the explicit sync point is not always run.
    assert_eq!(schedule.executable.systems.len(), 6);
}

#[test]
fn merges_sync_points_into_one() {
    let mut schedule = Schedule::default();
    let mut world = World::default();
    // insert two parallel command systems, it should only create one sync point
    schedule.add_systems(
        (
            (
                |mut commands: Commands| commands.insert_resource(Resource1),
                |mut commands: Commands| commands.insert_resource(Resource2),
            ),
            |_: Res<Resource1>, _: Res<Resource2>| {},
        )
            .chain(),
    );
    schedule.run(&mut world);

    // inserted sync points
    assert_eq!(schedule.executable.systems.len(), 4);

    // merges sync points on rebuild
    schedule.add_systems(((
        (
            |mut commands: Commands| commands.insert_resource(Resource1),
            |mut commands: Commands| commands.insert_resource(Resource2),
        ),
        |_: Res<Resource1>, _: Res<Resource2>| {},
    )
        .chain(),));
    schedule.run(&mut world);

    assert_eq!(schedule.executable.systems.len(), 7);
}

#[test]
fn adds_multiple_consecutive_syncs() {
    let mut schedule = Schedule::default();
    let mut world = World::default();
    // insert two consecutive command systems, it should create two sync points
    schedule.add_systems(
        (
            |mut commands: Commands| commands.insert_resource(Resource1),
            |mut commands: Commands| commands.insert_resource(Resource2),
            |_: Res<Resource1>, _: Res<Resource2>| {},
        )
            .chain(),
    );
    schedule.run(&mut world);

    assert_eq!(schedule.executable.systems.len(), 5);
}

#[test]
fn do_not_consider_ignore_deferred_before_exclusive_system() {
    let mut schedule = Schedule::default();
    let mut world = World::default();
    // chain_ignore_deferred adds no sync points usually but an exception is made for exclusive systems
    schedule.add_systems(
        (
            |_: Commands| {},
            // <- no sync point is added here because the following system is not exclusive
            |mut commands: Commands| commands.insert_resource(Resource1),
            // <- sync point is added here because the following system is exclusive which expects to see all commands to that point
            |world: &mut World| assert!(world.contains_resource::<Resource1>()),
            // <- no sync point is added here because the previous system has no deferred parameters
            |_: &mut World| {},
            // <- no sync point is added here because the following system is not exclusive
            |_: Commands| {},
        )
            .chain_ignore_deferred(),
    );
    schedule.run(&mut world);

    assert_eq!(schedule.executable.systems.len(), 6); // 5 systems + 1 sync point
}

#[test]
fn bubble_sync_point_through_ignore_deferred_node() {
    let mut schedule = Schedule::default();
    let mut world = World::default();

    let insert_resource_config = (
        // the first system has deferred commands
        |mut commands: Commands| commands.insert_resource(Resource1),
        // the second system has no deferred commands
        || {},
    )
        // the first two systems are chained without a sync point in between
        .chain_ignore_deferred();

    schedule.add_systems(
        (
            insert_resource_config,
            // the third system would panic if the command of the first system was not applied
            |_: Res<Resource1>| {},
        )
            // the third system is chained after the first two, possibly with a sync point in between
            .chain(),
    );

    // To add a sync point between the second and third system despite the second having no commands,
    // the first system has to signal the second system that there are unapplied commands.
    // With that the second system will add a sync point after it so the third system will find the resource.

    schedule.run(&mut world);

    assert_eq!(schedule.executable.systems.len(), 4); // 3 systems + 1 sync point
}

#[test]
fn disable_auto_sync_points() {
    let mut schedule = Schedule::default();
    schedule.set_build_settings(ScheduleBuildSettings {
        auto_insert_apply_deferred: false,
        ..Default::default()
    });
    let mut world = World::default();
    schedule.add_systems(
        (
            |mut commands: Commands| commands.insert_resource(Resource1),
            |res: Option<Res<Resource1>>| assert!(res.is_none()),
        )
            .chain(),
    );
    schedule.run(&mut world);

    assert_eq!(schedule.executable.systems.len(), 2);
}

mod no_sync_chain {
    use super::*;

    #[derive(Resource)]
    struct Ra;

    #[derive(Resource)]
    struct Rb;

    #[derive(Resource)]
    struct Rc;

    fn run_schedule(expected_num_systems: usize, add_systems: impl FnOnce(&mut Schedule)) {
        let mut schedule = Schedule::default();
        let mut world = World::default();
        add_systems(&mut schedule);

        schedule.run(&mut world);

        assert_eq!(schedule.executable.systems.len(), expected_num_systems);
    }

    #[test]
    fn only_chain_outside() {
        run_schedule(5, |schedule: &mut Schedule| {
            schedule.add_systems(
                (
                    (
                        |mut commands: Commands| commands.insert_resource(Ra),
                        |mut commands: Commands| commands.insert_resource(Rb),
                    ),
                    (
                        |res_a: Option<Res<Ra>>, res_b: Option<Res<Rb>>| {
                            assert!(res_a.is_some());
                            assert!(res_b.is_some());
                        },
                        |res_a: Option<Res<Ra>>, res_b: Option<Res<Rb>>| {
                            assert!(res_a.is_some());
                            assert!(res_b.is_some());
                        },
                    ),
                )
                    .chain(),
            );
        });

        run_schedule(4, |schedule: &mut Schedule| {
            schedule.add_systems(
                (
                    (
                        |mut commands: Commands| commands.insert_resource(Ra),
                        |mut commands: Commands| commands.insert_resource(Rb),
                    ),
                    (
                        |res_a: Option<Res<Ra>>, res_b: Option<Res<Rb>>| {
                            assert!(res_a.is_none());
                            assert!(res_b.is_none());
                        },
                        |res_a: Option<Res<Ra>>, res_b: Option<Res<Rb>>| {
                            assert!(res_a.is_none());
                            assert!(res_b.is_none());
                        },
                    ),
                )
                    .chain_ignore_deferred(),
            );
        });
    }

    #[test]
    fn chain_first() {
        run_schedule(6, |schedule: &mut Schedule| {
            schedule.add_systems(
                (
                    (
                        |mut commands: Commands| commands.insert_resource(Ra),
                        |mut commands: Commands, res_a: Option<Res<Ra>>| {
                            commands.insert_resource(Rb);
                            assert!(res_a.is_some());
                        },
                    )
                        .chain(),
                    (
                        |res_a: Option<Res<Ra>>, res_b: Option<Res<Rb>>| {
                            assert!(res_a.is_some());
                            assert!(res_b.is_some());
                        },
                        |res_a: Option<Res<Ra>>, res_b: Option<Res<Rb>>| {
                            assert!(res_a.is_some());
                            assert!(res_b.is_some());
                        },
                    ),
                )
                    .chain(),
            );
        });

        run_schedule(5, |schedule: &mut Schedule| {
            schedule.add_systems(
                (
                    (
                        |mut commands: Commands| commands.insert_resource(Ra),
                        |mut commands: Commands, res_a: Option<Res<Ra>>| {
                            commands.insert_resource(Rb);
                            assert!(res_a.is_some());
                        },
                    )
                        .chain(),
                    (
                        |res_a: Option<Res<Ra>>, res_b: Option<Res<Rb>>| {
                            assert!(res_a.is_some());
                            assert!(res_b.is_none());
                        },
                        |res_a: Option<Res<Ra>>, res_b: Option<Res<Rb>>| {
                            assert!(res_a.is_some());
                            assert!(res_b.is_none());
                        },
                    ),
                )
                    .chain_ignore_deferred(),
            );
        });
    }

    #[test]
    fn chain_second() {
        run_schedule(6, |schedule: &mut Schedule| {
            schedule.add_systems(
                (
                    (
                        |mut commands: Commands| commands.insert_resource(Ra),
                        |mut commands: Commands| commands.insert_resource(Rb),
                    ),
                    (
                        |mut commands: Commands, res_a: Option<Res<Ra>>, res_b: Option<Res<Rb>>| {
                            commands.insert_resource(Rc);
                            assert!(res_a.is_some());
                            assert!(res_b.is_some());
                        },
                        |res_a: Option<Res<Ra>>, res_b: Option<Res<Rb>>, res_c: Option<Res<Rc>>| {
                            assert!(res_a.is_some());
                            assert!(res_b.is_some());
                            assert!(res_c.is_some());
                        },
                    )
                        .chain(),
                )
                    .chain(),
            );
        });

        run_schedule(5, |schedule: &mut Schedule| {
            schedule.add_systems(
                (
                    (
                        |mut commands: Commands| commands.insert_resource(Ra),
                        |mut commands: Commands| commands.insert_resource(Rb),
                    ),
                    (
                        |mut commands: Commands, res_a: Option<Res<Ra>>, res_b: Option<Res<Rb>>| {
                            commands.insert_resource(Rc);
                            assert!(res_a.is_none());
                            assert!(res_b.is_none());
                        },
                        |res_a: Option<Res<Ra>>, res_b: Option<Res<Rb>>, res_c: Option<Res<Rc>>| {
                            assert!(res_a.is_some());
                            assert!(res_b.is_some());
                            assert!(res_c.is_some());
                        },
                    )
                        .chain(),
                )
                    .chain_ignore_deferred(),
            );
        });
    }

    #[test]
    fn chain_all() {
        run_schedule(7, |schedule: &mut Schedule| {
            schedule.add_systems(
                (
                    (
                        |mut commands: Commands| commands.insert_resource(Ra),
                        |mut commands: Commands, res_a: Option<Res<Ra>>| {
                            commands.insert_resource(Rb);
                            assert!(res_a.is_some());
                        },
                    )
                        .chain(),
                    (
                        |mut commands: Commands, res_a: Option<Res<Ra>>, res_b: Option<Res<Rb>>| {
                            commands.insert_resource(Rc);
                            assert!(res_a.is_some());
                            assert!(res_b.is_some());
                        },
                        |res_a: Option<Res<Ra>>, res_b: Option<Res<Rb>>, res_c: Option<Res<Rc>>| {
                            assert!(res_a.is_some());
                            assert!(res_b.is_some());
                            assert!(res_c.is_some());
                        },
                    )
                        .chain(),
                )
                    .chain(),
            );
        });

        run_schedule(6, |schedule: &mut Schedule| {
            schedule.add_systems(
                (
                    (
                        |mut commands: Commands| commands.insert_resource(Ra),
                        |mut commands: Commands, res_a: Option<Res<Ra>>| {
                            commands.insert_resource(Rb);
                            assert!(res_a.is_some());
                        },
                    )
                        .chain(),
                    (
                        |mut commands: Commands, res_a: Option<Res<Ra>>, res_b: Option<Res<Rb>>| {
                            commands.insert_resource(Rc);
                            assert!(res_a.is_some());
                            assert!(res_b.is_none());
                        },
                        |res_a: Option<Res<Ra>>, res_b: Option<Res<Rb>>, res_c: Option<Res<Rc>>| {
                            assert!(res_a.is_some());
                            assert!(res_b.is_some());
                            assert!(res_c.is_some());
                        },
                    )
                        .chain(),
                )
                    .chain_ignore_deferred(),
            );
        });
    }
}

#[derive(ScheduleLabel, Hash, Debug, Clone, PartialEq, Eq)]
struct TestSchedule;

#[derive(Resource)]
struct CheckSystemRan(usize);

#[test]
fn add_systems_to_existing_schedule() {
    let mut schedules = Schedules::default();
    let schedule = Schedule::new(TestSchedule);

    schedules.insert(schedule);
    schedules.add_systems(TestSchedule, |mut ran: ResMut<CheckSystemRan>| ran.0 += 1);

    let mut world = World::new();

    world.insert_resource(CheckSystemRan(0));
    world.insert_resource(schedules);
    world.run_schedule(TestSchedule);

    let value = world
        .get_resource::<CheckSystemRan>()
        .expect("CheckSystemRan Resource Should Exist");
    assert_eq!(value.0, 1);
}

#[test]
fn add_systems_to_non_existing_schedule() {
    let mut schedules = Schedules::default();

    schedules.add_systems(TestSchedule, |mut ran: ResMut<CheckSystemRan>| ran.0 += 1);

    let mut world = World::new();

    world.insert_resource(CheckSystemRan(0));
    world.insert_resource(schedules);
    world.run_schedule(TestSchedule);

    let value = world
        .get_resource::<CheckSystemRan>()
        .expect("CheckSystemRan Resource Should Exist");
    assert_eq!(value.0, 1);
}

#[derive(SystemSet, Debug, Hash, Clone, PartialEq, Eq)]
enum TestSet {
    First,
    Second,
}

#[test]
fn configure_set_on_existing_schedule() {
    let mut schedules = Schedules::default();
    let schedule = Schedule::new(TestSchedule);

    schedules.insert(schedule);

    schedules.configure_sets(TestSchedule, (TestSet::First, TestSet::Second).chain());
    schedules.add_systems(
        TestSchedule,
        (|mut ran: ResMut<CheckSystemRan>| {
            assert_eq!(ran.0, 0);
            ran.0 += 1;
        })
        .in_set(TestSet::First),
    );

    schedules.add_systems(
        TestSchedule,
        (|mut ran: ResMut<CheckSystemRan>| {
            assert_eq!(ran.0, 1);
            ran.0 += 1;
        })
        .in_set(TestSet::Second),
    );

    let mut world = World::new();

    world.insert_resource(CheckSystemRan(0));
    world.insert_resource(schedules);
    world.run_schedule(TestSchedule);

    let value = world
        .get_resource::<CheckSystemRan>()
        .expect("CheckSystemRan Resource Should Exist");
    assert_eq!(value.0, 2);
}

#[test]
fn configure_set_on_new_schedule() {
    let mut schedules = Schedules::default();

    schedules.configure_sets(TestSchedule, (TestSet::First, TestSet::Second).chain());
    schedules.add_systems(
        TestSchedule,
        (|mut ran: ResMut<CheckSystemRan>| {
            assert_eq!(ran.0, 0);
            ran.0 += 1;
        })
        .in_set(TestSet::First),
    );

    schedules.add_systems(
        TestSchedule,
        (|mut ran: ResMut<CheckSystemRan>| {
            assert_eq!(ran.0, 1);
            ran.0 += 1;
        })
        .in_set(TestSet::Second),
    );

    let mut world = World::new();

    world.insert_resource(CheckSystemRan(0));
    world.insert_resource(schedules);
    world.run_schedule(TestSchedule);

    let value = world
        .get_resource::<CheckSystemRan>()
        .expect("CheckSystemRan Resource Should Exist");
    assert_eq!(value.0, 2);
}

#[test]
fn test_default_error_handler() {
    #[derive(Resource, Default)]
    struct Ran(bool);

    fn system(mut ran: ResMut<Ran>) -> Result {
        ran.0 = true;
        Err("I failed!".into())
    }

    // Test that the fallback error handler is used
    let mut world = World::default();
    world.init_resource::<Ran>();
    world.insert_resource(FallbackErrorHandler(ignore));
    let mut schedule = Schedule::default();
    schedule.add_systems(system).run(&mut world);
    assert!(world.resource::<Ran>().0);

    // Test that the handler doesn't change within the schedule
    schedule.add_systems(
        (|world: &mut World| {
            world.insert_resource(FallbackErrorHandler(panic));
        })
        .before(system),
    );
    schedule.run(&mut world);
}

#[test]
fn get_a_system_key() {
    fn test_system() {}

    let mut schedule = Schedule::default();
    schedule.add_systems(test_system);
    let mut world = World::default();
    let _ = schedule.initialize(&mut world);

    let keys = schedule
        .graph()
        .systems_in_set(test_system.into_system_set().intern())
        .unwrap();
    assert_eq!(keys.len(), 1);
}

#[test]
fn get_system_keys_in_set() {
    fn system_1() {}
    fn system_2() {}

    let mut schedule = Schedule::default();
    schedule.add_systems((system_1, system_2).in_set(TestSet::First));
    let mut world = World::default();
    let _ = schedule.initialize(&mut world);

    let keys = schedule
        .graph()
        .systems_in_set(TestSet::First.into_system_set().intern())
        .unwrap();
    assert_eq!(keys.len(), 2);
}

#[test]
fn get_system_keys_with_same_name() {
    fn test_system() {}

    let mut schedule = Schedule::default();
    schedule.add_systems((test_system, test_system));
    let mut world = World::default();
    let _ = schedule.initialize(&mut world);

    let keys = schedule
        .graph()
        .systems_in_set(test_system.into_system_set().intern())
        .unwrap();
    assert_eq!(keys.len(), 2);
}

#[test]
fn remove_a_system() {
    fn system() {}

    let mut schedule = Schedule::default();
    schedule.add_systems(system);
    let mut world = World::default();

    let remove_count = schedule.remove_systems_in_set(
        system,
        &mut world,
        ScheduleCleanupPolicy::RemoveSetAndSystemsAllowBreakages,
    );
    assert_eq!(remove_count.unwrap(), 1);

    // schedule has changed, so we check initializing again
    schedule.initialize(&mut world).unwrap();
    assert_eq!(schedule.graph().systems.len(), 0);
}

#[test]
fn remove_multiple_systems() {
    fn system() {}

    let mut schedule = Schedule::default();
    schedule.add_systems((system, system));
    let mut world = World::default();

    let remove_count = schedule.remove_systems_in_set(
        system,
        &mut world,
        ScheduleCleanupPolicy::RemoveSetAndSystemsAllowBreakages,
    );
    assert_eq!(remove_count.unwrap(), 2);

    // schedule has changed, so we check initializing again
    schedule.initialize(&mut world).unwrap();
    assert_eq!(schedule.graph().systems.len(), 0);
}

#[test]
fn remove_a_system_with_dependencies() {
    fn system_1() {}
    fn system_2() {}

    let mut schedule = Schedule::default();
    schedule.add_systems((system_1, system_2).chain());
    let mut world = World::default();

    let remove_count = schedule.remove_systems_in_set(
        system_1,
        &mut world,
        ScheduleCleanupPolicy::RemoveSetAndSystemsAllowBreakages,
    );
    assert_eq!(remove_count.unwrap(), 1);

    // schedule has changed, so we check initializing again
    schedule.initialize(&mut world).unwrap();
    assert_eq!(schedule.graph().systems.len(), 1);
}

#[test]
fn remove_a_system_and_still_ordered() {
    #[derive(Resource)]
    struct A;

    fn system_1(_: ResMut<A>) {}
    fn system_2() {}
    fn system_3(_: ResMut<A>) {}

    let mut schedule = Schedule::default();
    schedule.add_systems((system_1, system_2, system_3).chain());
    let mut world = World::new();

    let _ = schedule.remove_systems_in_set(
        system_2,
        &mut world,
        ScheduleCleanupPolicy::RemoveSetAndSystems,
    );

    let result = schedule.initialize(&mut world);
    assert!(result.is_ok());
    let conflicts = schedule.graph().conflicting_systems();
    assert!(conflicts.is_empty());
}

#[test]
fn remove_a_set_and_still_ordered() {
    #[derive(Resource)]
    struct A;

    #[derive(SystemSet, Hash, PartialEq, Eq, Clone, Debug)]
    struct B;

    fn system_1(_: ResMut<A>) {}
    fn system_2() {}
    fn system_3(_: ResMut<A>) {}

    let mut schedule = Schedule::default();
    schedule.add_systems((system_1.before(B), system_2, system_3.after(B)));
    let mut world = World::new();

    let _ =
        schedule.remove_systems_in_set(B, &mut world, ScheduleCleanupPolicy::RemoveSetAndSystems);

    let result = schedule.initialize(&mut world);
    assert!(result.is_ok());
    let conflicts = schedule.graph().conflicting_systems();
    assert!(conflicts.is_empty());
}

#[test]
fn build_pass_iteration_order() {
    #[derive(Debug)]
    struct Pass<const N: usize>;

    impl<const N: usize> ScheduleBuildPass for Pass<N> {
        type EdgeOptions = ();
        fn add_dependency(
            &mut self,
            _from: NodeId,
            _to: NodeId,
            _options: Option<&Self::EdgeOptions>,
        ) {
        }
        fn build(
            &mut self,
            _world: &mut World,
            _graph: &mut super::ScheduleGraph,
            _dependency_flattened: FlattenedDependencies<'_>,
        ) -> core::result::Result<(), ScheduleBuildError> {
            Ok(())
        }
        fn collapse_set(
            &mut self,
            _set: SystemSetKey,
            _systems: &indexmap::IndexSet<SystemKey, FixedHasher>,
            _dependency_flattening: &DiGraph<NodeId>,
        ) -> impl Iterator<Item = (NodeId, NodeId)> {
            core::iter::empty()
        }
    }

    let mut schedule = Schedule::default();
    schedule.add_build_pass(Pass::<0>);
    schedule.add_build_pass(Pass::<1>);
    schedule.add_build_pass(Pass::<2>);

    let pass_order: Vec<TypeId> = schedule.graph().passes.keys().cloned().collect();

    assert_eq!(
        pass_order,
        vec![
            TypeId::of::<AutoInsertApplyDeferredPass>(),
            TypeId::of::<Pass<0>>(),
            TypeId::of::<Pass<1>>(),
            TypeId::of::<Pass<2>>()
        ]
    );
}
