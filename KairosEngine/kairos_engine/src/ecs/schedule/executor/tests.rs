use kairos_ecs_macros::{Component, Resource};

use crate::ecs::{change_detection::{Res, ResMut}, schedule::{MultiThreadedExecutor, Schedule}, system::{DynParamBuilder, DynSystemParam, ExclusiveSystemParam, In, Local, ParamBuilder, ParamSet, Query, RunSystemError, Single, SystemMeta, SystemParamValidationError}, world::World};

#[derive(Component)]
struct TestComponent;

#[derive(Resource, Default)]
struct Counter(u8);

// A resource that won't be inserted, causing validation to fail.
#[derive(Resource)]
struct MissingResource;

/// An [`ExclusiveSystemParam`] that always fails validation.
struct AlwaysInvalid;

impl ExclusiveSystemParam for AlwaysInvalid {
    type State = ();
    type Item<'s> = AlwaysInvalid;

    fn init(_world: &mut World, _system_meta: &mut SystemMeta) -> Self::State {}

    fn get_param<'s>(
        _state: &'s mut Self::State,
        _system_meta: &SystemMeta,
    ) -> Result<Self::Item<'s>, SystemParamValidationError> {
        Err(SystemParamValidationError::invalid::<Self>(
            "always invalid",
        ))
    }
}

#[test]
fn function_system_validation_failure_is_error() {
    fn system(_res: Res<MissingResource>) {}

    let mut world = World::new();
    let result = world.run_system_once(system);
    assert!(
        matches!(result, Err(RunSystemError::Failed(_))),
        "Expected Failed, got {result:?}"
    );
}

#[test]
fn function_system_validation_skip() {
    fn system(_single: Single<&TestComponent>) {}

    let mut world = World::new();
    let result = world.run_system_once(system);
    assert!(
        matches!(result, Err(RunSystemError::Skipped(_))),
        "Expected Skipped, got {result:?}"
    );
}

#[test]
fn function_system_validation_success() {
    fn system(_res: Res<Counter>) {}

    let mut world = World::new();
    world.init_resource::<Counter>();
    let result = world.run_system_once(system);
    assert!(result.is_ok(), "Expected Ok, got {result:?}");
}

#[test]
fn adapter_system_validation_failure() {
    fn system(_res: Res<MissingResource>) -> u32 {
        42
    }

    let mut world = World::new();
    let result = world.run_system_once(system.map(|_x| {}));
    assert!(
        matches!(result, Err(RunSystemError::Failed(_))),
        "Expected Failed from adapter system, got {result:?}"
    );
}

#[test]
fn adapter_system_validation_skip() {
    fn system(_single: Single<&TestComponent>) -> u32 {
        42
    }

    let mut world = World::new();
    let result = world.run_system_once(system.map(|_x| {}));
    assert!(
        matches!(result, Err(RunSystemError::Skipped(_))),
        "Expected Skipped from adapter system, got {result:?}"
    );
}

#[test]
fn pipe_system_validation_failure_in_first() {
    fn first(_res: Res<MissingResource>) -> u32 {
        42
    }
    fn second(_input: In<u32>) {}

    let mut world = World::new();
    let result = world.run_system_once(first.pipe(second));
    assert!(
        matches!(result, Err(RunSystemError::Failed(_))),
        "Expected Failed from pipe first system, got {result:?}"
    );
}

#[test]
fn pipe_system_validation_failure_in_second() {
    fn first() -> u32 {
        42
    }
    fn second(_input: In<u32>, _res: Res<MissingResource>) {}

    let mut world = World::new();
    let result = world.run_system_once(first.pipe(second));
    assert!(
        matches!(result, Err(RunSystemError::Failed(_))),
        "Expected Failed from pipe second system, got {result:?}"
    );
}

#[test]
fn pipe_system_validation_skip_in_first() {
    fn first(_single: Single<&TestComponent>) -> u32 {
        42
    }
    fn second(_input: In<u32>) {}

    let mut world = World::new();
    let result = world.run_system_once(first.pipe(second));
    assert!(
        matches!(result, Err(RunSystemError::Skipped(_))),
        "Expected Skipped from pipe first system, got {result:?}"
    );
}

#[test]
fn pipe_system_validation_skip_in_second() {
    fn first() -> u32 {
        42
    }

    fn second(_input: In<u32>, _single: Single<&TestComponent>) {}

    let mut world = World::new();
    let result = world.run_system_once(first.pipe(second));
    assert!(
        matches!(result, Err(RunSystemError::Skipped(_))),
        "Expected Skipped from pipe second system, got {result:?}"
    );
}

#[test]
fn builder_system_validation_failure() {
    fn system(_res: Res<MissingResource>) {}

    let mut world = World::new();
    let result = world.run_system_once(ParamBuilder.build_system(system));
    assert!(
        matches!(result, Err(RunSystemError::Failed(_))),
        "Expected Failed from builder system, got {result:?}"
    );
}

#[test]
fn builder_system_validation_skip() {
    fn system(_single: Single<&TestComponent>) {}

    let mut world = World::new();
    let result = world.run_system_once(ParamBuilder.build_system(system));
    assert!(
        matches!(result, Err(RunSystemError::Skipped(_))),
        "Expected Skipped from builder system, got {result:?}"
    );
}

#[test]
fn dyn_system_param_validation_failure() {
    let mut world = World::new();
    let system = (DynParamBuilder::new::<Res<MissingResource>>(ParamBuilder),)
        .build_state(&mut world)
        .build_system(|_param: DynSystemParam| {});
    let result = world.run_system_once(system);
    assert!(
        matches!(result, Err(RunSystemError::Failed(_))),
        "Expected Failed from DynSystemParam system, got {result:?}"
    );
}

#[test]
fn dyn_system_param_validation_skip() {
    let mut world = World::new();
    let system = (DynParamBuilder::new::<Single<&TestComponent>>(ParamBuilder),)
        .build_state(&mut world)
        .build_system(|_param: DynSystemParam| {});
    let result = world.run_system_once(system);
    assert!(
        matches!(result, Err(RunSystemError::Skipped(_))),
        "Expected Skipped from DynSystemParam system, got {result:?}"
    );
}

#[test]
fn dyn_system_param_validation_success() {
    let mut world = World::new();
    world.init_resource::<Counter>();
    let system = (DynParamBuilder::new::<Res<Counter>>(ParamBuilder),)
        .build_state(&mut world)
        .build_system(|_param: DynSystemParam| {});
    let result = world.run_system_once(system);
    assert!(
        result.is_ok(),
        "Expected Ok from DynSystemParam system, got {result:?}"
    );
}

#[test]
fn exclusive_system_validation_failure() {
    fn system(_world: &mut World, _param: AlwaysInvalid) {}

    let mut world = World::new();
    let result = world.run_system_once(system);
    assert!(
        matches!(result, Err(RunSystemError::Failed(_))),
        "Expected Failed from exclusive system, got {result:?}"
    );
}

#[test]
fn exclusive_system_validation_success() {
    fn system(_world: &mut World, mut _local: Local<u32>) {}

    let mut world = World::new();
    let result = world.run_system_once(system);
    assert!(
        result.is_ok(),
        "Expected Ok from exclusive system, got {result:?}"
    );
}

#[test]
fn validation_skips_system_in_schedule_singlethreaded() {
    let mut schedule = Schedule::default();
    schedule.set_executor(SingleThreadedExecutor::new());
    validation_skips_system_in_schedule("SingleThreaded", schedule);
}

#[test]
fn validation_skips_system_in_schedule_multithreaded() {
    let mut schedule = Schedule::default();
    schedule.set_executor(MultiThreadedExecutor::new());
    validation_skips_system_in_schedule("MultiThreaded", schedule);
}

fn validation_skips_system_in_schedule(name: &str, mut schedule: Schedule) {
    // Ensure the executor properly handles validation failures by skipping
    // and not running the system body.
    fn skippable_system(_single: Single<&TestComponent>, mut counter: ResMut<Counter>) {
        counter.0 += 1;
    }

    let mut world = World::new();
    world.init_resource::<Counter>();

    schedule.add_systems(skippable_system);

    // No TestComponent entity exists, so the system should be skipped.
    schedule.run(&mut world);
    assert_eq!(
        world.resource::<Counter>().0,
        0,
        "System should have been skipped with {name}"
    );
}

#[test]
fn param_set_validation_skip() {
    // A system using ParamSet with a Single sub-param should be skipped
    // when the Single has no matching entities, rather than panicking.
    fn system(mut _set: ParamSet<(Single<&TestComponent>,)>) {}

    let mut world = World::new();
    let result = world.run_system_once(system);
    assert!(
        matches!(result, Err(RunSystemError::Skipped(_))),
        "Expected Skipped from ParamSet with invalid Single, got {result:?}"
    );
}

#[test]
fn param_set_validation_failure() {
    // A system using ParamSet with a Res sub-param should fail validation
    // when the resource does not exist.
    fn system(mut _set: ParamSet<(Query<&TestComponent>, Res<MissingResource>)>) {}

    let mut world = World::new();
    let result = world.run_system_once(system);
    assert!(
        matches!(result, Err(RunSystemError::Failed(_))),
        "Expected Failed from ParamSet with missing resource, got {result:?}"
    );
}

#[test]
fn param_set_validation_success() {
    // A system using ParamSet with valid sub-params should succeed.
    fn system(mut set: ParamSet<(Query<&TestComponent>, Res<Counter>)>) {
        let _q = set.p0();
    }

    let mut world = World::new();
    world.init_resource::<Counter>();
    let result = world.run_system_once(system);
    assert!(
        result.is_ok(),
        "Expected Ok from ParamSet with valid params, got {result:?}"
    );
}
