//! Smoke tests (T1–T11) for the bevy_app-style schedule rails.
//!
//! Test list per Grilling #130 resolution D4, extended with the time-resource
//! rails (T9–T11, issue #135); all tests run against a bare
//! [`World`] (D2): `World::new()` → `install`, and one "frame" is
//! `world.run_schedule(Main)` followed by `world.clear_trackers()`.
//! `Engine::new()` is deliberately avoided (no audio-device dependency), so
//! these run headless in CI.

use super::{
    First, FixedUpdate, Last, Main, MainScheduleOrder, PostUpdate, PreUpdate, Startup, Update,
    install,
};
use crate::timer::Time;
use kairos_ecs::{
    resource::Resource,
    schedule::{InternedScheduleLabel, MainThreadExecutor, ScheduleLabel, Schedules},
    system::ResMut,
    world::World,
};

/// Number of frames run by the smoke tests (N = 3).
const FRAMES: usize = 3;

/// The canonical per-frame order of the six sub-stages.
const SUB_STAGE_ORDER: [&str; 6] = [
    "First",
    "PreUpdate",
    "FixedUpdate",
    "Update",
    "PostUpdate",
    "Last",
];

/// Records which systems ran, in order.
#[derive(Resource, Default)]
struct Trace(Vec<&'static str>);

/// Counts executions of the `Startup` system attached before the first frame.
#[derive(Resource, Default)]
struct StartupRuns(usize);

/// Counts executions of the `Startup` system appended after the first frame.
#[derive(Resource, Default)]
struct LaterStartupRuns(usize);

fn boot() -> World {
    let mut world = World::new();
    install(&mut world);
    world
}

/// One frame: run the `Main` schedule, then clear change-detection trackers.
fn run_frame(world: &mut World) {
    world.run_schedule(Main);
    world.clear_trackers();
}

/// Adds a `ResMut<Trace>` system to the schedule with `label`.
fn add_trace_system(world: &mut World, label: impl ScheduleLabel, system: fn(ResMut<Trace>)) {
    world
        .get_resource_mut::<Schedules>()
        .expect("Schedules missing after install")
        .get_mut(label)
        .expect("schedule missing after install")
        .add_systems(system);
}

/// Attaches the six canonical sub-stage tracers (one per sub-stage).
fn add_sub_stage_tracers(world: &mut World) {
    add_trace_system(world, First, trace_first);
    add_trace_system(world, PreUpdate, trace_pre_update);
    add_trace_system(world, FixedUpdate, trace_fixed_update);
    add_trace_system(world, Update, trace_update);
    add_trace_system(world, PostUpdate, trace_post_update);
    add_trace_system(world, Last, trace_last);
}

// ---------------------------------------------------------------------------
// Tracing systems
// ---------------------------------------------------------------------------

fn trace_startup(mut trace: ResMut<Trace>) {
    trace.0.push("startup");
}

fn trace_first(mut trace: ResMut<Trace>) {
    trace.0.push("First");
}

fn trace_pre_update(mut trace: ResMut<Trace>) {
    trace.0.push("PreUpdate");
}

fn trace_fixed_update(mut trace: ResMut<Trace>) {
    trace.0.push("FixedUpdate");
}

fn trace_update(mut trace: ResMut<Trace>) {
    trace.0.push("Update");
}

fn trace_post_update(mut trace: ResMut<Trace>) {
    trace.0.push("PostUpdate");
}

fn trace_last(mut trace: ResMut<Trace>) {
    trace.0.push("Last");
}

fn trace_startup_runs(mut runs: ResMut<StartupRuns>) {
    runs.0 += 1;
}

fn trace_later_startup_runs(mut runs: ResMut<LaterStartupRuns>) {
    runs.0 += 1;
}

// ---------------------------------------------------------------------------
// T1–T8
// ---------------------------------------------------------------------------

/// T1: after `install` the world's `Schedules` contain all eight schedules.
#[test]
fn boot_installs_all_schedules() {
    let world = boot();
    let schedules = world
        .get_resource::<Schedules>()
        .expect("Schedules resource missing after install");
    for label in [
        Startup.intern(),
        Main.intern(),
        First.intern(),
        PreUpdate.intern(),
        FixedUpdate.intern(),
        Update.intern(),
        PostUpdate.intern(),
        Last.intern(),
    ] {
        assert!(
            schedules.contains(label),
            "schedule `{label:?}` was not installed"
        );
    }
}

/// T2: `MainScheduleOrder` is present with the default startup/label lists.
#[test]
fn main_schedule_order_defaults() {
    let world = boot();
    let order = world
        .get_resource::<MainScheduleOrder>()
        .expect("MainScheduleOrder resource missing after install");

    assert_eq!(
        order.startup_labels,
        vec![Startup.intern()],
        "startup list must default to exactly [Startup]"
    );
    assert_eq!(
        order.labels,
        vec![
            First.intern(),
            PreUpdate.intern(),
            FixedUpdate.intern(),
            Update.intern(),
            PostUpdate.intern(),
            Last.intern(),
        ],
        "labels list must default to the canonical per-frame order"
    );
}

/// T3: `MainThreadExecutor` is inserted as a world resource by `install`.
#[test]
fn main_thread_executor_installed() {
    let world = boot();
    assert!(
        world.get_resource::<MainThreadExecutor>().is_some(),
        "MainThreadExecutor resource missing after install"
    );
}

/// T4: `Startup` runs exactly once, on the first frame, before `First`.
#[test]
fn startup_runs_exactly_once() {
    let mut world = boot();
    world.insert_resource(Trace::default());
    add_trace_system(&mut world, Startup, trace_startup);
    add_trace_system(&mut world, First, trace_first);

    for _ in 0..FRAMES {
        run_frame(&mut world);
    }

    let trace = &world.get_resource::<Trace>().expect("Trace resource").0;
    assert_eq!(
        trace.iter().filter(|entry| **entry == "startup").count(),
        1,
        "Startup must run exactly once across {FRAMES} frames"
    );
    assert_eq!(
        trace.iter().filter(|entry| **entry == "First").count(),
        FRAMES,
        "First must run on every frame"
    );
    assert_eq!(
        trace[0], "startup",
        "Startup must run before First on the first frame"
    );
}

/// T5: the six sub-stages run on every frame, in canonical order.
#[test]
fn substages_run_every_frame_in_order() {
    let mut world = boot();
    world.insert_resource(Trace::default());
    add_sub_stage_tracers(&mut world);

    for _ in 0..FRAMES {
        run_frame(&mut world);
    }

    let trace = &world.get_resource::<Trace>().expect("Trace resource").0;
    assert_eq!(trace.len(), FRAMES * SUB_STAGE_ORDER.len());
    for (frame, chunk) in trace.chunks_exact(SUB_STAGE_ORDER.len()).enumerate() {
        assert_eq!(
            chunk,
            SUB_STAGE_ORDER.as_slice(),
            "frame {frame} ran the sub-stages out of order"
        );
    }
}

/// T6: driving an empty world (no user systems) for N frames does not panic.
#[test]
fn empty_frame_no_panic() {
    let mut world = boot();

    for _ in 0..FRAMES {
        run_frame(&mut world);
    }

    // Sanity checks: the rails are still intact after the empty frames.
    assert!(world.get_resource::<Schedules>().is_some());
    assert!(world.get_resource::<MainScheduleOrder>().is_some());
    assert!(world.get_resource::<MainThreadExecutor>().is_some());
}

/// T7: a `Startup` system appended after the first frame never runs (bevy parity).
#[test]
fn startup_appended_later_does_not_rerun() {
    let mut world = boot();
    world.insert_resource(StartupRuns(0));
    world.insert_resource(LaterStartupRuns(0));

    {
        let mut schedules = world.get_resource_mut::<Schedules>().expect("Schedules");
        schedules
            .get_mut(Startup)
            .expect("Startup schedule missing")
            .add_systems(trace_startup_runs);
    }

    run_frame(&mut world);
    assert_eq!(
        world.get_resource::<StartupRuns>().unwrap().0,
        1,
        "Startup must run exactly once on the first frame"
    );

    // Append a second system to `Startup` after the first frame has passed.
    {
        let mut schedules = world.get_resource_mut::<Schedules>().expect("Schedules");
        schedules
            .get_mut(Startup)
            .expect("Startup schedule missing")
            .add_systems(trace_later_startup_runs);
    }

    for _ in 0..FRAMES {
        run_frame(&mut world);
    }

    assert_eq!(
        world.get_resource::<StartupRuns>().unwrap().0,
        1,
        "Startup must not re-run after the first frame"
    );
    assert_eq!(
        world.get_resource::<LaterStartupRuns>().unwrap().0,
        0,
        "systems appended to Startup after the first frame must not run"
    );
}

/// T8: removing a sub-stage must not panic; the remaining ones keep running.
#[test]
fn missing_substage_warns_and_continues() {
    let mut world = boot();
    world.insert_resource(Trace::default());
    add_sub_stage_tracers(&mut world);

    // Remove the FixedUpdate sub-schedule entirely: the driver must warn and
    // continue (the warning itself is not asserted, per D4).
    let removed = world
        .get_resource_mut::<Schedules>()
        .expect("Schedules")
        .remove(FixedUpdate);
    assert!(
        removed.is_some(),
        "FixedUpdate schedule should exist before removal"
    );

    for _ in 0..FRAMES {
        run_frame(&mut world);
    }

    let remaining: [&str; 5] = ["First", "PreUpdate", "Update", "PostUpdate", "Last"];
    let trace = &world.get_resource::<Trace>().expect("Trace resource").0;
    assert_eq!(trace.len(), FRAMES * remaining.len());
    for (frame, chunk) in trace.chunks_exact(remaining.len()).enumerate() {
        assert_eq!(
            chunk,
            remaining.as_slice(),
            "frame {frame}: remaining sub-stages must still run in order"
        );
    }
}

// ---------------------------------------------------------------------------
// T9–T11: the `Time` World resource and its `First`-stage `time_system` (#135)
// ---------------------------------------------------------------------------

/// T9: `install` registers the `Time` World resource with a zeroed clock.
#[test]
fn install_registers_the_time_resource() {
    let world = boot();

    let time = world
        .get_resource::<Time>()
        .expect("Time resource missing after install");
    assert_eq!(time.total_frame(), 0, "clock must start before frame one");
    assert_eq!(time.delta_time(), std::time::Duration::ZERO);
    assert_eq!(time.total_time(), std::time::Duration::ZERO);
}

/// T10: a full frame advances the clock exactly once — `time_system` runs in
/// `First`, the only stage that advances it.
#[test]
fn time_advances_exactly_once_per_frame() {
    let mut world = boot();

    for _ in 0..FRAMES {
        run_frame(&mut world);
    }

    let time = world.get_resource::<Time>().expect("Time resource");
    assert_eq!(
        time.total_frame(),
        FRAMES as u64,
        "clock must advance exactly once per frame"
    );
}

/// T11: no sub-stage other than `First` advances the clock; running `First`
/// alone advances it exactly once.
#[test]
fn only_first_advances_the_clock() {
    let mut world = boot();

    // Run every sub-stage the frame driver runs except `First` directly: the
    // clock must not move. The stage list comes from the `MainScheduleOrder`
    // resource itself (startup + per-frame labels) so it stays in sync with
    // the canonical order instead of duplicating it.
    let other_stages: Vec<InternedScheduleLabel> = {
        let order = world
            .get_resource::<MainScheduleOrder>()
            .expect("MainScheduleOrder resource");
        order
            .startup_labels
            .iter()
            .chain(order.labels.iter())
            .copied()
            .filter(|label| *label != First.intern())
            .collect()
    };
    for label in other_stages {
        world.run_schedule(label);
    }
    assert_eq!(
        world.get_resource::<Time>().expect("Time resource").total_frame(),
        0,
        "stages other than First must not advance the clock"
    );

    // Running `First` alone is one frame's worth of advancement.
    world.run_schedule(First);
    assert_eq!(
        world.get_resource::<Time>().expect("Time resource").total_frame(),
        1,
        "First must advance the clock exactly once per run"
    );
}
