use kairos_ecs_macros::Resource;

use crate::ecs::{schedule::{IntoScheduleConfigs, MultiThreadedExecutor, Schedule}, system::Commands, world::World};

#[derive(Resource)]
struct R;

#[test]
fn skipped_systems_notify_dependents() {
    let mut world = World::new();
    let mut schedule = Schedule::default();
    schedule.set_executor(MultiThreadedExecutor::new());
    schedule.add_systems(
        (
            (|| {}).run_if(|| false),
            // This system depends on a system that is always skipped.
            |mut commands: Commands| {
                commands.insert_resource(R);
            },
        )
            .chain(),
    );
    schedule.run(&mut world);
    assert!(world.get_resource::<R>().is_some());
}

/// Regression test for a weird bug flagged by MIRI in
/// `spawn_exclusive_system_task`, related to a `&mut World` being captured
/// inside an `async` block and somehow remaining alive even after its last use.
#[test]
fn check_spawn_exclusive_system_task_miri() {
    let mut world = World::new();
    let mut schedule = Schedule::default();
    schedule.set_executor(MultiThreadedExecutor::new());
    schedule.add_systems(((|_: Commands| {}), |_: Commands| {}).chain());
    schedule.run(&mut world);
}
