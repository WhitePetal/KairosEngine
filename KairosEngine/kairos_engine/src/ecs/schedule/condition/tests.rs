use std::sync::{Arc, Mutex};

use kairos_ecs_macros::{Component, Message, Resource, SystemSet};

use crate::ecs::{change_detection::{Res, ResMut}, error::{ErrorContext, FallbackErrorHandler, KairosError}, query::With, schedule::{IntoScheduleConfigs, Schedule, SystemCondition, common_conditions::{any_match_filter, any_with_component, not, on_message, resource_added, resource_changed, resource_changed_or_removed, resource_exists, resource_exists_and_changed, resource_removed, run_once}}, system::{IntoSystem, Local, RunSystemOnce, Single, System}, world::World};

#[derive(Resource, Default)]
struct Counter(usize);

fn increment_counter(mut counter: ResMut<Counter>) {
    counter.0 += 1;
}

fn double_counter(mut counter: ResMut<Counter>) {
    counter.0 *= 2;
}

fn every_other_time(mut has_ran: Local<bool>) -> bool {
    *has_ran = !*has_ran;
    *has_ran
}

#[test]
fn run_condition() {
    let mut world = World::new();
    world.init_resource::<Counter>();
    let mut schedule = Schedule::default();

    // Run every other cycle
    schedule.add_systems(increment_counter.run_if(every_other_time));

    schedule.run(&mut world);
    schedule.run(&mut world);
    assert_eq!(world.resource::<Counter>().0, 1);
    schedule.run(&mut world);
    schedule.run(&mut world);
    assert_eq!(world.resource::<Counter>().0, 2);

    // Run every other cycle opposite to the last one
    schedule.add_systems(increment_counter.run_if(not(every_other_time)));

    schedule.run(&mut world);
    schedule.run(&mut world);
    assert_eq!(world.resource::<Counter>().0, 4);
    schedule.run(&mut world);
    schedule.run(&mut world);
    assert_eq!(world.resource::<Counter>().0, 6);
}

#[test]
fn combinators_with_maybe_failing_condition() {
    #![allow(
        clippy::nonminimal_bool,
        clippy::overly_complex_bool_expr,
        reason = "Trailing `|| false` and `&& true` are used in this test to visually remain consistent with the combinators"
    )]

    // Things that should be tested:
    // - the final result of the combinator is correct
    // - the systems that are expected to run do run
    // - the systems that are expected to not run do not run

    #[derive(Component)]
    struct Vacant;

    // SystemConditions don't have mutable access to the world, so we use a
    // `Res<AtomicCounter>` to count invocations.
    #[derive(Resource, Default)]
    struct Counter(Arc<Mutex<usize>>);

    // The following constants are used to represent a system having run.
    // both are prime so that when multiplied they give a unique value for any TRUE^n*FALSE^m
    const FALSE: usize = 2;
    const TRUE: usize = 3;

    // this is a system, but has the same side effect as `test_true`
    fn is_true_inc(counter: Res<Counter>) -> bool {
        test_true(&counter)
    }

    // this is a system, but has the same side effect as `test_false`
    fn is_false_inc(counter: Res<Counter>) -> bool {
        test_false(&counter)
    }

    // This condition will always yield `false`, because `Vacant` is never present.
    fn vacant(_: Single<&Vacant>) -> bool {
        true
    }

    fn test_true(counter: &Counter) -> bool {
        *counter.0.lock().unwrap() *= TRUE;
        true
    }

    fn test_false(counter: &Counter) -> bool {
        *counter.0.lock().unwrap() *= FALSE;
        false
    }

    // Helper function that runs a logic call and returns the result, as
    // well as the composite number of the calls.
    fn logic_call_result(f: impl FnOnce(&Counter) -> bool) -> (usize, bool) {
        let counter = Counter(Arc::new(Mutex::new(1)));
        let result = f(&counter);
        (*counter.0.lock().unwrap(), result)
    }

    // `test_true` and `test_false` can't fail like the systems can, and so
    // we use them to model the short circuiting behavior of rust's logical
    // operators. The goal is to end up with a composite number that
    // describes rust's behavior and compare that to the result of the
    // combinators.

    // we expect `true() || false()` to yield `true`, and short circuit
    // after `true()`
    assert_eq!(
        logic_call_result(|c| test_true(c) || test_false(c)),
        (TRUE.pow(1) * FALSE.pow(0), true)
    );

    let mut world = World::new();
    world.init_resource::<Counter>();

    // ensure there are no `Vacant` entities
    assert!(world.query::<&Vacant>().iter(&world).next().is_none());
    assert!(matches!(
        world.run_system_once((|| true).or_else(vacant)),
        Ok(true)
    ));

    // This system should fail
    assert!(RunSystemOnce::run_system_once(&mut world, vacant).is_err());

    #[track_caller]
    fn assert_system<Marker>(
        world: &mut World,
        system: impl IntoSystem<(), bool, Marker>,
        equivalent_to: impl FnOnce(&Counter) -> bool,
    ) {
        *world.resource::<Counter>().0.lock().unwrap() = 1;

        let system = IntoSystem::into_system(system);
        let name = system.name();

        let out = RunSystemOnce::run_system_once(&mut *world, system).unwrap_or(false);

        let (expected_counter, expected) = logic_call_result(equivalent_to);
        let caller = core::panic::Location::caller();
        let counter = *world.resource::<Counter>().0.lock().unwrap();

        assert_eq!(
            out,
            expected,
            "At {}:{} System `{name}` yielded unexpected value `{out}`, expected `{expected}`",
            caller.file(),
            caller.line(),
        );

        assert_eq!(
            counter, expected_counter,
            "At {}:{} System `{name}` did not increment counter as expected: expected `{expected_counter}`, got `{counter}`",
            caller.file(),
            caller.line(),
        );
    }

    assert_system(&mut world, is_true_inc.or_else(vacant), |c| {
        test_true(c) || false
    });
    assert_system(&mut world, is_true_inc.nor_else(vacant), |c| {
        !(test_true(c) || false)
    });
    assert_system(&mut world, is_true_inc.xor(vacant), |c| {
        test_true(c) ^ false
    });
    assert_system(&mut world, is_true_inc.xnor(vacant), |c| {
        !(test_true(c) ^ false)
    });
    assert_system(&mut world, is_true_inc.and_then(vacant), |c| {
        test_true(c) && false
    });
    assert_system(&mut world, is_true_inc.nand_then(vacant), |c| {
        !(test_true(c) && false)
    });

    // even if `vacant` fails as the first condition, where applicable (or,
    // xor), `is_true_inc` should still be called. `and` and `nand` short
    // circuit on an initial `false`.
    assert_system(&mut world, vacant.or_else(is_true_inc), |c| {
        false || test_true(c)
    });
    assert_system(&mut world, vacant.nor_else(is_true_inc), |c| {
        !(false || test_true(c))
    });
    assert_system(&mut world, vacant.xor(is_true_inc), |c| {
        false ^ test_true(c)
    });
    assert_system(&mut world, vacant.xnor(is_true_inc), |c| {
        !(false ^ test_true(c))
    });
    assert_system(&mut world, vacant.and_then(is_true_inc), |c| {
        false && test_true(c)
    });
    assert_system(&mut world, vacant.nand_then(is_true_inc), |c| {
        !(false && test_true(c))
    });

    // the same logic ought to be the case with a condition that runs, but yields `false`:
    assert_system(&mut world, is_true_inc.or_else(is_false_inc), |c| {
        test_true(c) || test_false(c)
    });
    assert_system(&mut world, is_true_inc.nor_else(is_false_inc), |c| {
        !(test_true(c) || test_false(c))
    });
    assert_system(&mut world, is_true_inc.xor(is_false_inc), |c| {
        test_true(c) ^ test_false(c)
    });
    assert_system(&mut world, is_true_inc.xnor(is_false_inc), |c| {
        !(test_true(c) ^ test_false(c))
    });
    assert_system(&mut world, is_true_inc.and_then(is_false_inc), |c| {
        test_true(c) && test_false(c)
    });
    assert_system(&mut world, is_true_inc.nand_then(is_false_inc), |c| {
        !(test_true(c) && test_false(c))
    });

    // and where one condition yields `false` and the other fails:
    assert_system(&mut world, is_false_inc.or_else(vacant), |c| {
        test_false(c) || false
    });
    assert_system(&mut world, is_false_inc.nor_else(vacant), |c| {
        !(test_false(c) || false)
    });
    assert_system(&mut world, is_false_inc.xor(vacant), |c| {
        test_false(c) ^ false
    });
    assert_system(&mut world, is_false_inc.xnor(vacant), |c| {
        !(test_false(c) ^ false)
    });
    assert_system(&mut world, is_false_inc.and_then(vacant), |c| {
        test_false(c) && false
    });
    assert_system(&mut world, is_false_inc.nand_then(vacant), |c| {
        !(test_false(c) && false)
    });

    // and where both conditions yield `true`:
    assert_system(&mut world, is_true_inc.or_else(is_true_inc), |c| {
        test_true(c) || test_true(c)
    });
    assert_system(&mut world, is_true_inc.nor_else(is_true_inc), |c| {
        !(test_true(c) || test_true(c))
    });
    assert_system(&mut world, is_true_inc.xor(is_true_inc), |c| {
        test_true(c) ^ test_true(c)
    });
    assert_system(&mut world, is_true_inc.xnor(is_true_inc), |c| {
        !(test_true(c) ^ test_true(c))
    });
    assert_system(&mut world, is_true_inc.and_then(is_true_inc), |c| {
        test_true(c) && test_true(c)
    });
    assert_system(&mut world, is_true_inc.nand_then(is_true_inc), |c| {
        !(test_true(c) && test_true(c))
    });

    // and where both conditions yield `false`:
    assert_system(&mut world, is_false_inc.or_else(is_false_inc), |c| {
        test_false(c) || test_false(c)
    });
    assert_system(&mut world, is_false_inc.nor_else(is_false_inc), |c| {
        !(test_false(c) || test_false(c))
    });
    assert_system(&mut world, is_false_inc.xor(is_false_inc), |c| {
        test_false(c) ^ test_false(c)
    });
    assert_system(&mut world, is_false_inc.xnor(is_false_inc), |c| {
        !(test_false(c) ^ test_false(c))
    });
    assert_system(&mut world, is_false_inc.and_then(is_false_inc), |c| {
        test_false(c) && test_false(c)
    });
    assert_system(&mut world, is_false_inc.nand_then(is_false_inc), |c| {
        !(test_false(c) && test_false(c))
    });
}

#[test]
fn run_condition_combinators() {
    let mut world = World::new();
    world.init_resource::<Counter>();
    let mut schedule = Schedule::default();

    schedule.add_systems(
        (
            increment_counter.run_if(every_other_time.and_eager(|| true)), // Run every odd cycle.
            increment_counter.run_if(every_other_time.nand_eager(|| false)), // Always run.
            double_counter.run_if(every_other_time.nor_eager(|| false)), // Run every even cycle.
            increment_counter.run_if(every_other_time.or_eager(|| true)), // Always run.
            increment_counter.run_if(every_other_time.xnor(|| true)),    // Run every odd cycle.
            double_counter.run_if(every_other_time.xnor(|| false)), // Run every even cycle.
            increment_counter.run_if(every_other_time.xor(|| false)), // Run every odd cycle.
            double_counter.run_if(every_other_time.xor(|| true)),   // Run every even cycle.
        )
            .chain(),
    );

    schedule.run(&mut world);
    assert_eq!(world.resource::<Counter>().0, 5);
    schedule.run(&mut world);
    assert_eq!(world.resource::<Counter>().0, 52);
}

#[test]
fn multiple_run_conditions() {
    let mut world = World::new();
    world.init_resource::<Counter>();
    let mut schedule = Schedule::default();

    // Run every other cycle
    schedule.add_systems(increment_counter.run_if(every_other_time).run_if(|| true));
    // Never run
    schedule.add_systems(increment_counter.run_if(every_other_time).run_if(|| false));

    schedule.run(&mut world);
    assert_eq!(world.resource::<Counter>().0, 1);
    schedule.run(&mut world);
    assert_eq!(world.resource::<Counter>().0, 1);
}

#[test]
fn multiple_run_conditions_is_and_operation() {
    let mut world = World::new();
    world.init_resource::<Counter>();

    let mut schedule = Schedule::default();

    // This should never run, if multiple run conditions worked
    // like an OR condition then it would always run
    schedule.add_systems(
        increment_counter
            .run_if(every_other_time)
            .run_if(not(every_other_time)),
    );

    schedule.run(&mut world);
    assert_eq!(world.resource::<Counter>().0, 0);
    schedule.run(&mut world);
    assert_eq!(world.resource::<Counter>().0, 0);
}
#[derive(Component)]
struct TestComponent;

#[derive(Message)]
struct TestMessage;

#[derive(Resource)]
struct TestResource(());

fn test_system() {}

// Ensure distributive_run_if compiles with the common conditions.
#[test]
fn distributive_run_if_compiles() {
    Schedule::default().add_systems(
        (test_system, test_system)
            .distributive_run_if(run_once)
            .distributive_run_if(resource_exists::<TestResource>)
            .distributive_run_if(resource_added::<TestResource>)
            .distributive_run_if(resource_changed::<TestResource>)
            .distributive_run_if(resource_exists_and_changed::<TestResource>)
            .distributive_run_if(resource_changed_or_removed::<TestResource>)
            .distributive_run_if(resource_removed::<TestResource>)
            .distributive_run_if(on_message::<TestMessage>)
            .distributive_run_if(any_with_component::<TestComponent>)
            .distributive_run_if(any_match_filter::<With<TestComponent>>)
            .distributive_run_if(not(run_once)),
    );
}

#[test]
fn run_if_error_contains_system() {
    let mut world = World::new();
    world.insert_resource(FallbackErrorHandler(my_error_handler));

    #[derive(Resource)]
    struct MyResource;

    fn condition(_res: Res<MyResource>) -> bool {
        true
    }

    fn my_error_handler(_: KairosError, ctx: ErrorContext) {
        let a = IntoSystem::into_system(system_a);
        let b = IntoSystem::into_system(system_b);
        assert!(
            matches!(ctx, ErrorContext::RunCondition { system, on_set, .. } if (on_set && system == b.name()) || (!on_set && system == a.name()))
        );
    }

    fn system_a() {}
    fn system_b() {}

    let mut schedule = Schedule::default();
    schedule.add_systems(system_a.run_if(condition));
    schedule.run(&mut world);

    #[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
    struct Set;

    let mut schedule = Schedule::default();
    schedule
        .add_systems((system_b,).in_set(Set))
        .configure_sets(Set.run_if(condition));
    schedule.run(&mut world);
}
