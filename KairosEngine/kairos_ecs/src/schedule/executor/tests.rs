use kairos_ecs_macros::{Component, Resource};

use crate::{
    change_detection::{Res, ResMut},
    schedule::{MultiThreadedExecutor, Schedule, SingleThreadedExecutor},
    system::{In, IntoSystem, Populated, Single},
    world::World,
};

#[derive(Component)]
struct TestComponent;

#[derive(Resource, Default)]
struct TestState {
    populated_ran: bool,
    single_ran: bool,
}

#[derive(Resource, Default)]
struct Counter(u8);

fn set_single_state(mut _single: Single<&TestComponent>, mut state: ResMut<TestState>) {
    state.single_ran = true;
}

fn set_populated_state(mut _populated: Populated<&TestComponent>, mut state: ResMut<TestState>) {
    state.populated_ran = true;
}

#[test]
fn single_and_populated_skipped_and_run_singlethreaded() {
    let mut schedule = Schedule::default();
    schedule.set_executor(SingleThreadedExecutor::new());
    single_and_populated_skipped_and_run("SingleThreaded", schedule);
}

#[test]
fn single_and_populated_skipped_and_run_multithreaded() {
    let mut schedule = Schedule::default();
    schedule.set_executor(MultiThreadedExecutor::new());
    single_and_populated_skipped_and_run("MultiThreaded", schedule);
}

#[expect(clippy::print_stdout, reason = "std and println are allowed in tests")]
fn single_and_populated_skipped_and_run(name: &str, mut schedule: Schedule) {
    std::println!("Testing executor: {name}");

    let mut world = World::new();
    world.init_resource::<TestState>();

    schedule.add_systems((set_single_state, set_populated_state));
    schedule.run(&mut world);

    let state = world.get_resource::<TestState>().unwrap();
    assert!(!state.single_ran);
    assert!(!state.populated_ran);

    world.spawn(TestComponent);

    schedule.run(&mut world);
    let state = world.get_resource::<TestState>().unwrap();
    assert!(state.single_ran);
    assert!(state.populated_ran);
}

fn look_for_missing_resource(_res: Res<TestState>) {}

#[test]
#[should_panic]
fn missing_resource_panics_single_threaded() {
    let mut world = World::new();
    let mut schedule = Schedule::default();

    schedule.set_executor(SingleThreadedExecutor::new());
    schedule.add_systems(look_for_missing_resource);
    schedule.run(&mut world);
}

#[test]
#[should_panic]
fn missing_resource_panics_multi_threaded() {
    let mut world = World::new();
    let mut schedule = Schedule::default();

    schedule.set_executor(MultiThreadedExecutor::new());
    schedule.add_systems(look_for_missing_resource);
    schedule.run(&mut world);
}

#[test]
fn piped_systems_first_system_skipped() {
    // This system should be skipped when run due to no matching entity
    fn pipe_out(_single: Single<&TestComponent>) -> u8 {
        42
    }

    fn pipe_in(_input: In<u8>, mut counter: ResMut<Counter>) {
        counter.0 += 1;
    }

    let mut world = World::new();
    world.init_resource::<Counter>();
    let mut schedule = Schedule::default();

    schedule.add_systems(pipe_out.pipe(pipe_in));
    schedule.run(&mut world);

    let counter = world.resource::<Counter>();
    assert_eq!(counter.0, 0);
}

#[test]
fn piped_system_second_system_skipped() {
    // This system will be run before the second system is validated
    fn pipe_out(mut counter: ResMut<Counter>) -> u8 {
        counter.0 += 1;
        42
    }

    // This system should be skipped when run due to no matching entity
    fn pipe_in(_input: In<u8>, _single: Single<&TestComponent>, mut counter: ResMut<Counter>) {
        counter.0 += 1;
    }

    let mut world = World::new();
    world.init_resource::<Counter>();
    let mut schedule = Schedule::default();

    schedule.add_systems(pipe_out.pipe(pipe_in));
    schedule.run(&mut world);
    let counter = world.resource::<Counter>();
    assert_eq!(counter.0, 1);
}

#[test]
#[should_panic]
fn piped_system_first_system_panics() {
    // This system should panic when run because the resource is missing
    fn pipe_out(_res: Res<TestState>) -> u8 {
        42
    }

    fn pipe_in(_input: In<u8>) {}

    let mut world = World::new();
    let mut schedule = Schedule::default();

    schedule.add_systems(pipe_out.pipe(pipe_in));
    schedule.run(&mut world);
}

#[test]
#[should_panic]
fn piped_system_second_system_panics() {
    fn pipe_out() -> u8 {
        42
    }

    // This system should panic when run because the resource is missing
    fn pipe_in(_input: In<u8>, _res: Res<TestState>) {}

    let mut world = World::new();
    let mut schedule = Schedule::default();

    schedule.add_systems(pipe_out.pipe(pipe_in));
    schedule.run(&mut world);
}

// This test runs without panicking because we've
// decided to use early-out behavior for piped systems
#[test]
fn piped_system_skip_and_panic() {
    // This system should be skipped when run due to no matching entity
    fn pipe_out(_single: Single<&TestComponent>) -> u8 {
        42
    }

    // This system should panic when run because the resource is missing
    fn pipe_in(_input: In<u8>, _res: Res<TestState>) {}

    let mut world = World::new();
    let mut schedule = Schedule::default();

    schedule.add_systems(pipe_out.pipe(pipe_in));
    schedule.run(&mut world);
}

#[test]
#[should_panic]
fn piped_system_panic_and_skip() {
    // This system should panic when run because the resource is missing

    fn pipe_out(_res: Res<TestState>) -> u8 {
        42
    }

    // This system should be skipped when run due to no matching entity
    fn pipe_in(_input: In<u8>, _single: Single<&TestComponent>) {}

    let mut world = World::new();
    let mut schedule = Schedule::default();

    schedule.add_systems(pipe_out.pipe(pipe_in));
    schedule.run(&mut world);
}

#[test]
#[should_panic]
fn piped_system_panic_and_panic() {
    // This system should panic when run because the resource is missing

    fn pipe_out(_res: Res<TestState>) -> u8 {
        42
    }

    // This system should panic when run because the resource is missing
    fn pipe_in(_input: In<u8>, _res: Res<TestState>) {}

    let mut world = World::new();
    let mut schedule = Schedule::default();

    schedule.add_systems(pipe_out.pipe(pipe_in));
    schedule.run(&mut world);
}

#[test]
fn piped_system_skip_and_skip() {
    // This system should be skipped when run due to no matching entity

    fn pipe_out(_single: Single<&TestComponent>, mut counter: ResMut<Counter>) -> u8 {
        counter.0 += 1;
        42
    }

    // This system should be skipped when run due to no matching entity
    fn pipe_in(_input: In<u8>, _single: Single<&TestComponent>, mut counter: ResMut<Counter>) {
        counter.0 += 1;
    }

    let mut world = World::new();
    world.init_resource::<Counter>();
    let mut schedule = Schedule::default();

    schedule.add_systems(pipe_out.pipe(pipe_in));
    schedule.run(&mut world);

    let counter = world.resource::<Counter>();
    assert_eq!(counter.0, 0);
}
