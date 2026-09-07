#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
struct TestSchedule;

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
struct TestScheduleA;

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
struct TestScheduleB;

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
struct TestScheduleC;

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
struct TestScheduleD;

fn first_system() {}
fn second_system() {}
fn third_system() {}

fn setup() -> (Schedule, World) {
    let mut world = World::new();
    let mut schedule = Schedule::new(TestSchedule);
    schedule.add_systems((first_system, second_system).chain());
    schedule.initialize(&mut world).unwrap();
    (schedule, world)
}

// Helper for verifying skip_lists are equal, and if not, printing a human
// readable message.
macro_rules! assert_skip_list_eq {
    ($actual:expr, $expected:expr, $system_names:expr) => {
        let actual = $actual;
        let expected = $expected;
        let systems: &Vec<&str> = $system_names;

        if (actual != expected) {
            use core::fmt::Write as _;

            // mismatch, let's construct a human-readable message of what
            // was returned
            let mut msg = format!(
                "Schedule:\n    {:9} {:16}{:6} {:6} {:6}\n",
                "index", "name", "expect", "actual", "result"
            );
            for (i, name) in systems.iter().enumerate() {
                let _ = write!(msg, "    system[{:1}] {:16}", i, name);
                match (expected.contains(i), actual.contains(i)) {
                    (true, true) => msg.push_str("skip   skip   pass\n"),
                    (true, false) => {
                        msg.push_str("skip   run    FAILED; system should not have run\n")
                    }
                    (false, true) => {
                        msg.push_str("run    skip   FAILED; system should have run\n")
                    }
                    (false, false) => msg.push_str("run    run    pass\n"),
                }
            }
            assert_eq!(actual, expected, "{}", msg);
        }
    };
}

// Helper for verifying that a set of systems will be run for a given skip
// list
macro_rules! assert_systems_run {
    ($schedule:expr, $skipped_systems:expr, $($system:expr),*) => {
        // pull an ordered list of systems in the schedule, and save the
        // system TypeId, and name.
        let systems: Vec<(TypeId, alloc::string::String)> = $schedule.systems().unwrap()
            .map(|(_, system)| {
                (system.system_type(), system.name().as_string())
            })
        .collect();

        // construct a list of systems that are expected to run
        let mut expected = FixedBitSet::with_capacity(systems.len());
        $(
            let sys = IntoSystem::into_system($system);
            for (i, (type_id, _)) in systems.iter().enumerate() {
                if sys.system_type() == *type_id {
                    expected.insert(i);
                }
            }
        )*

        // flip the run list to get our skip list
        expected.toggle_range(..);

        // grab the list of skipped systems
        let actual = match $skipped_systems {
            None => FixedBitSet::with_capacity(systems.len()),
            Some(b) => b,
        };
        let system_names: Vec<&str> = systems
            .iter()
            .map(|(_,n)| n.rsplit_once("::").unwrap().1)
            .collect();

        assert_skip_list_eq!(actual, expected, &system_names);
    };
}

// Helper for verifying the expected systems will be run by the schedule
//
// This macro will construct an expected FixedBitSet for the systems that
// should be skipped, and compare it with the results from stepping the
// provided schedule.  If they don't match, it generates a human-readable
// error message and asserts.
macro_rules! assert_schedule_runs {
    ($schedule:expr, $stepping:expr, $($system:expr),*) => {
        // advance stepping to the next frame, and build the skip list for
        // this schedule
        $stepping.next_frame();
        assert_systems_run!($schedule, $stepping.skipped_systems($schedule), $($system),*);
    };
}

#[test]
fn stepping_disabled() {
    let (schedule, _world) = setup();

    let mut stepping = Stepping::new();
    stepping.add_schedule(TestSchedule).disable().next_frame();

    assert!(stepping.skipped_systems(&schedule).is_none());
    assert!(stepping.cursor().is_none());
}

#[test]
fn unknown_schedule() {
    let (schedule, _world) = setup();

    let mut stepping = Stepping::new();
    stepping.enable().next_frame();

    assert!(stepping.skipped_systems(&schedule).is_none());
}

#[test]
fn disabled_always_run() {
    let (schedule, _world) = setup();

    let mut stepping = Stepping::new();
    stepping
        .add_schedule(TestSchedule)
        .disable()
        .always_run(TestSchedule, first_system);

    assert_schedule_runs!(&schedule, &mut stepping, first_system, second_system);
}

#[test]
fn waiting_always_run() {
    let (schedule, _world) = setup();

    let mut stepping = Stepping::new();
    stepping
        .add_schedule(TestSchedule)
        .enable()
        .always_run(TestSchedule, first_system);

    assert_schedule_runs!(&schedule, &mut stepping, first_system);
}

#[test]
fn step_always_run() {
    let (schedule, _world) = setup();

    let mut stepping = Stepping::new();
    stepping
        .add_schedule(TestSchedule)
        .enable()
        .always_run(TestSchedule, first_system)
        .step_frame();

    assert_schedule_runs!(&schedule, &mut stepping, first_system, second_system);
}

#[test]
fn continue_always_run() {
    let (schedule, _world) = setup();

    let mut stepping = Stepping::new();
    stepping
        .add_schedule(TestSchedule)
        .enable()
        .always_run(TestSchedule, first_system)
        .continue_frame();

    assert_schedule_runs!(&schedule, &mut stepping, first_system, second_system);
}

#[test]
fn disabled_never_run() {
    let (schedule, _world) = setup();

    let mut stepping = Stepping::new();
    stepping
        .add_schedule(TestSchedule)
        .never_run(TestSchedule, first_system)
        .disable();
    assert_schedule_runs!(&schedule, &mut stepping, first_system, second_system);
}

#[test]
fn waiting_never_run() {
    let (schedule, _world) = setup();

    let mut stepping = Stepping::new();
    stepping
        .add_schedule(TestSchedule)
        .enable()
        .never_run(TestSchedule, first_system);

    assert_schedule_runs!(&schedule, &mut stepping,);
}

#[test]
fn step_never_run() {
    let (schedule, _world) = setup();

    let mut stepping = Stepping::new();
    stepping
        .add_schedule(TestSchedule)
        .enable()
        .never_run(TestSchedule, first_system)
        .step_frame();

    assert_schedule_runs!(&schedule, &mut stepping, second_system);
}

#[test]
fn continue_never_run() {
    let (schedule, _world) = setup();

    let mut stepping = Stepping::new();
    stepping
        .add_schedule(TestSchedule)
        .enable()
        .never_run(TestSchedule, first_system)
        .continue_frame();

    assert_schedule_runs!(&schedule, &mut stepping, second_system);
}

#[test]
fn disabled_breakpoint() {
    let (schedule, _world) = setup();

    let mut stepping = Stepping::new();
    stepping
        .add_schedule(TestSchedule)
        .disable()
        .set_breakpoint(TestSchedule, second_system);

    assert_schedule_runs!(&schedule, &mut stepping, first_system, second_system);
}

#[test]
fn waiting_breakpoint() {
    let (schedule, _world) = setup();

    let mut stepping = Stepping::new();
    stepping
        .add_schedule(TestSchedule)
        .enable()
        .set_breakpoint(TestSchedule, second_system);

    assert_schedule_runs!(&schedule, &mut stepping,);
}

#[test]
fn step_breakpoint() {
    let (schedule, _world) = setup();

    let mut stepping = Stepping::new();
    stepping
        .add_schedule(TestSchedule)
        .enable()
        .set_breakpoint(TestSchedule, second_system)
        .step_frame();

    // since stepping stops at every system, breakpoints are ignored during
    // stepping
    assert_schedule_runs!(&schedule, &mut stepping, first_system);
    stepping.step_frame();
    assert_schedule_runs!(&schedule, &mut stepping, second_system);

    // let's go again to verify that we wrap back around to the start of
    // the frame
    stepping.step_frame();
    assert_schedule_runs!(&schedule, &mut stepping, first_system);

    // should be back in a waiting state now that it ran first_system
    assert_schedule_runs!(&schedule, &mut stepping,);
}

#[test]
fn continue_breakpoint() {
    let (schedule, _world) = setup();

    let mut stepping = Stepping::new();
    stepping
        .add_schedule(TestSchedule)
        .enable()
        .set_breakpoint(TestSchedule, second_system)
        .continue_frame();

    assert_schedule_runs!(&schedule, &mut stepping, first_system);
    stepping.continue_frame();
    assert_schedule_runs!(&schedule, &mut stepping, second_system);
    stepping.continue_frame();
    assert_schedule_runs!(&schedule, &mut stepping, first_system);
}

/// regression test for issue encountered while writing `system_stepping`
/// example
#[test]
fn continue_step_continue_with_breakpoint() {
    let mut world = World::new();
    let mut schedule = Schedule::new(TestSchedule);
    schedule.add_systems((first_system, second_system, third_system).chain());
    schedule.initialize(&mut world).unwrap();

    let mut stepping = Stepping::new();
    stepping
        .add_schedule(TestSchedule)
        .enable()
        .set_breakpoint(TestSchedule, second_system);

    stepping.continue_frame();
    assert_schedule_runs!(&schedule, &mut stepping, first_system);

    stepping.step_frame();
    assert_schedule_runs!(&schedule, &mut stepping, second_system);

    stepping.continue_frame();
    assert_schedule_runs!(&schedule, &mut stepping, third_system);
}

#[test]
fn clear_breakpoint() {
    let (schedule, _world) = setup();

    let mut stepping = Stepping::new();
    stepping
        .add_schedule(TestSchedule)
        .enable()
        .set_breakpoint(TestSchedule, second_system)
        .continue_frame();

    assert_schedule_runs!(&schedule, &mut stepping, first_system);
    stepping.continue_frame();
    assert_schedule_runs!(&schedule, &mut stepping, second_system);

    stepping.clear_breakpoint(TestSchedule, second_system);
    stepping.continue_frame();
    assert_schedule_runs!(&schedule, &mut stepping, first_system, second_system);
}

#[test]
fn clear_system() {
    let (schedule, _world) = setup();

    let mut stepping = Stepping::new();
    stepping
        .add_schedule(TestSchedule)
        .enable()
        .never_run(TestSchedule, second_system)
        .continue_frame();
    assert_schedule_runs!(&schedule, &mut stepping, first_system);

    stepping.clear_system(TestSchedule, second_system);
    stepping.continue_frame();
    assert_schedule_runs!(&schedule, &mut stepping, first_system, second_system);
}

#[test]
fn clear_schedule() {
    let (schedule, _world) = setup();

    let mut stepping = Stepping::new();
    stepping
        .add_schedule(TestSchedule)
        .enable()
        .never_run(TestSchedule, first_system)
        .never_run(TestSchedule, second_system)
        .continue_frame();
    assert_schedule_runs!(&schedule, &mut stepping,);

    stepping.clear_schedule(TestSchedule);
    stepping.continue_frame();
    assert_schedule_runs!(&schedule, &mut stepping, first_system, second_system);
}

/// This was discovered in code-review, ensure that `clear_schedule` also
/// clears any pending changes too.
#[test]
fn set_behavior_then_clear_schedule() {
    let (schedule, _world) = setup();

    let mut stepping = Stepping::new();
    stepping
        .add_schedule(TestSchedule)
        .enable()
        .continue_frame();
    assert_schedule_runs!(&schedule, &mut stepping, first_system, second_system);

    stepping.never_run(TestSchedule, first_system);
    stepping.clear_schedule(TestSchedule);
    stepping.continue_frame();
    assert_schedule_runs!(&schedule, &mut stepping, first_system, second_system);
}

/// Ensure that if they `clear_schedule` then make further changes to the
/// schedule, those changes after the clear are applied.
#[test]
fn clear_schedule_then_set_behavior() {
    let (schedule, _world) = setup();

    let mut stepping = Stepping::new();
    stepping
        .add_schedule(TestSchedule)
        .enable()
        .continue_frame();
    assert_schedule_runs!(&schedule, &mut stepping, first_system, second_system);

    stepping.clear_schedule(TestSchedule);
    stepping.never_run(TestSchedule, first_system);
    stepping.continue_frame();
    assert_schedule_runs!(&schedule, &mut stepping, second_system);
}

// Schedules such as FixedUpdate can be called multiple times in a single
// render frame.  Ensure we only run steppable systems the first time the
// schedule is run
#[test]
fn multiple_calls_per_frame_continue() {
    let (schedule, _world) = setup();

    let mut stepping = Stepping::new();
    stepping
        .add_schedule(TestSchedule)
        .enable()
        .always_run(TestSchedule, second_system)
        .continue_frame();

    // start a new frame, then run the schedule two times; first system
    // should only run on the first one
    stepping.next_frame();
    assert_systems_run!(
        &schedule,
        stepping.skipped_systems(&schedule),
        first_system,
        second_system
    );
    assert_systems_run!(
        &schedule,
        stepping.skipped_systems(&schedule),
        second_system
    );
}
#[test]
fn multiple_calls_per_frame_step() {
    let (schedule, _world) = setup();

    let mut stepping = Stepping::new();
    stepping.add_schedule(TestSchedule).enable().step_frame();

    // start a new frame, then run the schedule two times; first system
    // should only run on the first one
    stepping.next_frame();
    assert_systems_run!(&schedule, stepping.skipped_systems(&schedule), first_system);
    assert_systems_run!(&schedule, stepping.skipped_systems(&schedule),);
}

#[test]
fn step_duplicate_systems() {
    let mut world = World::new();
    let mut schedule = Schedule::new(TestSchedule);
    schedule.add_systems((first_system, first_system, second_system).chain());
    schedule.initialize(&mut world).unwrap();

    let mut stepping = Stepping::new();
    stepping.add_schedule(TestSchedule).enable();

    // needed for assert_skip_list_eq!
    let system_names = vec!["first_system", "first_system", "second_system"];
    // we're going to step three times, and each system in order should run
    // only once
    for system_index in 0..3 {
        // build the skip list by setting all bits, then clearing our the
        // one system that should run this step
        let mut expected = FixedBitSet::with_capacity(3);
        expected.set_range(.., true);
        expected.set(system_index, false);

        // step the frame and get the skip list
        stepping.step_frame();
        stepping.next_frame();
        let skip_list = stepping
            .skipped_systems(&schedule)
            .expect("TestSchedule has been added to Stepping");

        assert_skip_list_eq!(skip_list, expected, &system_names);
    }
}

#[test]
fn step_run_if_false() {
    let mut world = World::new();
    let mut schedule = Schedule::new(TestSchedule);

    // This needs to be a system test to confirm the interaction between
    // the skip list and system conditions in Schedule::run().  That means
    // all of our systems need real bodies that do things.
    //
    // first system will be configured as `run_if(|| false)`, so it can
    // just panic if called
    let first_system: fn() = move || {
        panic!("first_system should not be run");
    };

    // The second system, we need to know when it has been called, so we'll
    // add a resource for tracking if it has been run.  The system will
    // increment the run count.
    #[derive(Resource)]
    struct RunCount(usize);
    world.insert_resource(RunCount(0));
    let second_system = |mut run_count: ResMut<RunCount>| {
        println!("I have run!");
        run_count.0 += 1;
    };

    // build our schedule; first_system should never run, followed by
    // second_system.
    schedule.add_systems((first_system.run_if(|| false), second_system).chain());
    schedule.initialize(&mut world).unwrap();

    // set up stepping
    let mut stepping = Stepping::new();
    stepping.add_schedule(TestSchedule).enable();
    world.insert_resource(stepping);

    // if we step, and the run condition is false, we should not run
    // second_system.  The stepping cursor is at first_system, and if
    // first_system wasn't able to run, that's ok.
    let mut stepping = world.resource_mut::<Stepping>();
    stepping.step_frame();
    stepping.next_frame();
    schedule.run(&mut world);
    assert_eq!(
        world.resource::<RunCount>().0,
        0,
        "second_system should not have run"
    );

    // now on the next step, second_system should run
    let mut stepping = world.resource_mut::<Stepping>();
    stepping.step_frame();
    stepping.next_frame();
    schedule.run(&mut world);
    assert_eq!(
        world.resource::<RunCount>().0,
        1,
        "second_system should have run"
    );
}

#[test]
fn remove_schedule() {
    let (schedule, _world) = setup();
    let mut stepping = Stepping::new();
    stepping.add_schedule(TestSchedule).enable();

    // run the schedule once and verify all systems are skipped
    assert_schedule_runs!(&schedule, &mut stepping,);
    assert!(!stepping.schedules().unwrap().is_empty());

    // remove the test schedule
    stepping.remove_schedule(TestSchedule);
    assert_schedule_runs!(&schedule, &mut stepping, first_system, second_system);
    assert!(stepping.schedules().unwrap().is_empty());
}

// verify that Stepping can construct an ordered list of schedules
#[test]
fn schedules() {
    let mut world = World::new();

    // build & initialize a few schedules
    let mut schedule_a = Schedule::new(TestScheduleA);
    schedule_a.initialize(&mut world).unwrap();
    let mut schedule_b = Schedule::new(TestScheduleB);
    schedule_b.initialize(&mut world).unwrap();
    let mut schedule_c = Schedule::new(TestScheduleC);
    schedule_c.initialize(&mut world).unwrap();
    let mut schedule_d = Schedule::new(TestScheduleD);
    schedule_d.initialize(&mut world).unwrap();

    // setup stepping and add all the schedules
    let mut stepping = Stepping::new();
    stepping
        .add_schedule(TestScheduleA)
        .add_schedule(TestScheduleB)
        .add_schedule(TestScheduleC)
        .add_schedule(TestScheduleD)
        .enable()
        .next_frame();

    assert!(stepping.schedules().is_err());

    stepping.skipped_systems(&schedule_b);
    assert!(stepping.schedules().is_err());
    stepping.skipped_systems(&schedule_a);
    assert!(stepping.schedules().is_err());
    stepping.skipped_systems(&schedule_c);
    assert!(stepping.schedules().is_err());

    // when we call the last schedule, Stepping should have enough data to
    // return an ordered list of schedules
    stepping.skipped_systems(&schedule_d);
    assert!(stepping.schedules().is_ok());

    assert_eq!(
        *stepping.schedules().unwrap(),
        vec![
            TestScheduleB.intern(),
            TestScheduleA.intern(),
            TestScheduleC.intern(),
            TestScheduleD.intern(),
        ]
    );
}

#[test]
fn verify_cursor() {
    // helper to build a cursor tuple for the supplied schedule
    fn cursor(schedule: &Schedule, index: usize) -> (InternedScheduleLabel, NodeId) {
        let node_id = schedule.executable().system_ids[index];
        (schedule.label(), NodeId::System(node_id))
    }

    let mut world = World::new();
    let mut slotmap = SlotMap::<SystemKey, ()>::with_key();

    // create two schedules with a number of systems in them
    let mut schedule_a = Schedule::new(TestScheduleA);
    schedule_a.add_systems((|| {}, || {}, || {}, || {}).chain());
    schedule_a.initialize(&mut world).unwrap();
    let mut schedule_b = Schedule::new(TestScheduleB);
    schedule_b.add_systems((|| {}, || {}, || {}, || {}).chain());
    schedule_b.initialize(&mut world).unwrap();

    // setup stepping and add all schedules
    let mut stepping = Stepping::new();
    stepping
        .add_schedule(TestScheduleA)
        .add_schedule(TestScheduleB)
        .enable();

    assert!(stepping.cursor().is_none());

    // step the system nine times, and verify the cursor before & after
    // each step
    let mut cursors = Vec::new();
    for _ in 0..9 {
        stepping.step_frame().next_frame();
        cursors.push(stepping.cursor());
        stepping.skipped_systems(&schedule_a);
        stepping.skipped_systems(&schedule_b);
        cursors.push(stepping.cursor());
    }

    #[rustfmt::skip]
    assert_eq!(
        cursors,
        vec![
            // before render frame        // after render frame
            None,                         Some(cursor(&schedule_a, 1)),
            Some(cursor(&schedule_a, 1)), Some(cursor(&schedule_a, 2)),
            Some(cursor(&schedule_a, 2)), Some(cursor(&schedule_a, 3)),
            Some(cursor(&schedule_a, 3)), Some(cursor(&schedule_b, 0)),
            Some(cursor(&schedule_b, 0)), Some(cursor(&schedule_b, 1)),
            Some(cursor(&schedule_b, 1)), Some(cursor(&schedule_b, 2)),
            Some(cursor(&schedule_b, 2)), Some(cursor(&schedule_b, 3)),
            Some(cursor(&schedule_b, 3)), None,
            Some(cursor(&schedule_a, 0)), Some(cursor(&schedule_a, 1)),
        ]
    );

    let sys0 = slotmap.insert(());
    let sys1 = slotmap.insert(());
    let _sys2 = slotmap.insert(());
    let sys3 = slotmap.insert(());

    // reset our cursor (disable/enable), and update stepping to test if the
    // cursor properly skips over AlwaysRun & NeverRun systems.  Also set
    // a Break system to ensure that shows properly in the cursor
    stepping
        // disable/enable to reset cursor
        .disable()
        .enable()
        .set_breakpoint_node(TestScheduleA, NodeId::System(sys1))
        .always_run_node(TestScheduleA, NodeId::System(sys3))
        .never_run_node(TestScheduleB, NodeId::System(sys0));

    let mut cursors = Vec::new();
    for _ in 0..9 {
        stepping.step_frame().next_frame();
        cursors.push(stepping.cursor());
        stepping.skipped_systems(&schedule_a);
        stepping.skipped_systems(&schedule_b);
        cursors.push(stepping.cursor());
    }

    #[rustfmt::skip]
    assert_eq!(
        cursors,
        vec![
            // before render frame        // after render frame
            Some(cursor(&schedule_a, 0)), Some(cursor(&schedule_a, 1)),
            Some(cursor(&schedule_a, 1)), Some(cursor(&schedule_a, 2)),
            Some(cursor(&schedule_a, 2)), Some(cursor(&schedule_b, 1)),
            Some(cursor(&schedule_b, 1)), Some(cursor(&schedule_b, 2)),
            Some(cursor(&schedule_b, 2)), Some(cursor(&schedule_b, 3)),
            Some(cursor(&schedule_b, 3)), None,
            Some(cursor(&schedule_a, 0)), Some(cursor(&schedule_a, 1)),
            Some(cursor(&schedule_a, 1)), Some(cursor(&schedule_a, 2)),
            Some(cursor(&schedule_a, 2)), Some(cursor(&schedule_b, 1)),
        ]
    );
}
