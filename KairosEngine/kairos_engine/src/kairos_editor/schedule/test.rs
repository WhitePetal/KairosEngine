//! Smoke tests (T1–T20) for the bevy_app-style schedule rails.
//!
//! Test list per Grilling #130 resolution D4, extended with the time-resource
//! rails (T9–T11, issue #135), the fixed-clock registration (T12, issue #136)
//! and the fixed-step driver (T13–T20, issue #137). All tests run against a
//! bare [`World`] (D2): `World::new()` → `install`, and one "frame" is
//! `world.run_schedule(Main)` followed by `world.clear_trackers()`.
//! `Engine::new()` is deliberately avoided (no audio-device dependency), so
//! these run headless in CI.
//!
//! The driver tests (T13–T20) need a deterministic per-frame virtual delta,
//! so they swap the `First`-stage wall-clock advance for [`scripted_time_system`]
//! (a per-frame [`ScriptedFrameDelta`]) — whole frames then accrue exactly the
//! scripted delta and the fixed-step counts are exact.

use super::{
    First, FixedUpdate, Last, Main, MainScheduleOrder, PostUpdate, PreUpdate, RunFixedMainLoop,
    Startup, Update, install,
};
use crate::timer::{FixedTime, Time};
use kairos_ecs::{
    resource::Resource,
    schedule::{InternedScheduleLabel, MainThreadExecutor, Schedule, ScheduleLabel, Schedules},
    system::{Res, ResMut},
    world::World,
};
use std::time::Duration;

/// Number of frames run by the smoke tests (N = 3).
const FRAMES: usize = 3;

/// The canonical per-frame order of the six sub-stages (`FixedUpdate` is not
/// one of them: the driver inside `RunFixedMainLoop` runs it 0..N times per
/// frame).
const SUB_STAGE_ORDER: [&str; 6] = [
    "First",
    "PreUpdate",
    "RunFixedMainLoop",
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
    add_trace_system(world, RunFixedMainLoop, trace_run_fixed_main_loop);
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

fn trace_run_fixed_main_loop(mut trace: ResMut<Trace>) {
    trace.0.push("RunFixedMainLoop");
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

/// T1: after `install` the world's `Schedules` contain all nine schedules —
/// including the `RunFixedMainLoop` driver stage and the `FixedUpdate` content
/// schedule that is deliberately *not* in the per-frame label list.
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
        RunFixedMainLoop.intern(),
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

/// T2: `MainScheduleOrder` is present with the default startup/label lists —
/// `RunFixedMainLoop` between `PreUpdate` and `Update`, `FixedUpdate` nowhere
/// in the per-frame list (it is the fixed content mount, driven 0..N times).
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
            RunFixedMainLoop.intern(),
            Update.intern(),
            PostUpdate.intern(),
            Last.intern(),
        ],
        "labels list must default to the canonical per-frame order"
    );
    assert!(
        !order.labels.contains(&FixedUpdate.intern()),
        "FixedUpdate must not be in the per-frame label list"
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

/// T8: removing a per-frame sub-stage must not panic; the remaining ones keep
/// running in order. `RunFixedMainLoop` stands in for any label of the frame
/// driver — a missing stage is logged and skipped, per the missing-sub-stage
/// tolerance.
#[test]
fn missing_substage_warns_and_continues() {
    let mut world = boot();
    world.insert_resource(Trace::default());
    add_sub_stage_tracers(&mut world);

    // Remove the RunFixedMainLoop sub-schedule entirely: the frame driver logs
    // the error and continues (the log itself is not asserted, per D4).
    let removed = world
        .get_resource_mut::<Schedules>()
        .expect("Schedules")
        .remove(RunFixedMainLoop);
    assert!(
        removed.is_some(),
        "RunFixedMainLoop schedule should exist before removal"
    );

    for _ in 0..FRAMES {
        run_frame(&mut world);
    }

    // With the driver gone the fixed clock is never accumulated into.
    let fixed = world.get_resource::<FixedTime>().unwrap();
    assert_eq!(fixed.overstep(), Duration::ZERO);
    assert_eq!(fixed.delta(), Duration::ZERO);
    assert_eq!(fixed.elapsed(), Duration::ZERO);

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
// T9–T12: the `Time` World resource with its `First`-stage `time_system`
// (#135), plus the registered `FixedTime` resource (#136) left untouched by
// zero-delta frames (#137)
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

/// T12: `install` also registers the `FixedTime` World resource (#136) with a
/// default 64 Hz clock. The `RunFixedMainLoop` driver consumes it every frame
/// (issue #137): under scripted zero-delta frames nothing accrues, `FixedUpdate`
/// is empty, and no first-frame step is forced — the fixed clock stays put.
#[test]
fn install_registers_the_fixed_time_resource() {
    let mut world = boot_with_scripted_frames();

    let fixed = world
        .get_resource::<FixedTime>()
        .expect("FixedTime resource missing after install");
    assert_eq!(fixed.timestep(), Duration::from_micros(15_625));
    assert_eq!(fixed.overstep(), Duration::ZERO);
    assert_eq!(fixed.delta(), Duration::ZERO);
    assert_eq!(fixed.elapsed(), Duration::ZERO);

    // Zero-delta frames: the virtual clock advances one frame per run, but the
    // driver accumulates nothing and — `FixedUpdate` being empty — must not
    // force a step (no panic, no fixed-clock movement).
    for _ in 0..FRAMES {
        run_frame(&mut world);
    }
    assert_eq!(
        world.get_resource::<Time>().unwrap().total_frame(),
        FRAMES as u64,
        "the scripted clock must still advance one frame per run"
    );
    let fixed = world
        .get_resource::<FixedTime>()
        .expect("FixedTime resource");
    assert_eq!(fixed.delta(), Duration::ZERO);
    assert_eq!(fixed.elapsed(), Duration::ZERO);
    assert_eq!(fixed.overstep(), Duration::ZERO);
}

// ---------------------------------------------------------------------------
// T13–T20: the fixed-step driver (issue #137) — scripted virtual dt
// ---------------------------------------------------------------------------
//
// The default 64 Hz timestep is 15 625 µs. Every frame below scripts an exact
// virtual delta (the `First`-stage wall-clock advance is swapped for
// [`scripted_time_system`]), so the accumulated time — and thus the number of
// `FixedUpdate` runs the driver pays for — is exact.

/// The per-frame raw delta [`scripted_time_system`] feeds to the virtual clock.
#[derive(Resource)]
struct ScriptedFrameDelta(Duration);

/// Advances the virtual clock by the scripted frame delta instead of sampling
/// the wall clock. Replaces the `First`-stage `time_system` in the driver
/// tests below.
fn scripted_time_system(mut time: ResMut<Time>, scripted: Res<ScriptedFrameDelta>) {
    time.update_with_raw_delta(scripted.0);
}

/// Boots like [`boot`], then swaps the wall-clock `First` advance for
/// [`scripted_time_system`]. The script starts at zero, so tests drive the dt
/// of every frame (including the first) from a clean slate.
fn boot_with_scripted_frames() -> World {
    let mut world = boot();
    world.insert_resource(ScriptedFrameDelta(Duration::ZERO));
    // Inserting a schedule with the `First` label replaces the booted one,
    // dropping the wall-clock `time_system`.
    let mut first = Schedule::new(First);
    first.add_systems(scripted_time_system);
    world.add_schedule(first);
    world
}

/// Scripts the virtual delta of the *next* frame.
fn set_frame_dt(world: &mut World, dt: Duration) {
    world.resource_mut::<ScriptedFrameDelta>().0 = dt;
}

/// Counts how many times the `FixedUpdate` schedule ran.
#[derive(Resource, Default)]
struct FixedUpdateRuns(usize);

fn count_fixed_update_runs(mut runs: ResMut<FixedUpdateRuns>) {
    runs.0 += 1;
}

/// Per `FixedUpdate` run: the fixed delta the content sees via `Res<FixedTime>`
/// paired with the virtual delta the same run sees via `Res<Time>`.
#[derive(Resource, Default)]
struct FixedUpdateSightings(Vec<(Duration, Duration)>);

fn sight_fixed_update(
    fixed: Res<FixedTime>,
    time: Res<Time>,
    mut sightings: ResMut<FixedUpdateSightings>,
) {
    sightings.0.push((fixed.delta(), time.delta_time()));
}

/// Adds one counting and one sighting system to the `FixedUpdate` schedule.
fn add_fixed_update_observers(world: &mut World) {
    world.insert_resource(FixedUpdateRuns::default());
    world.insert_resource(FixedUpdateSightings::default());
    world
        .get_resource_mut::<Schedules>()
        .expect("Schedules missing after install")
        .get_mut(FixedUpdate)
        .expect("FixedUpdate schedule missing after install")
        .add_systems((count_fixed_update_runs, sight_fixed_update));
}

/// Boots a scripted-frame world with the `FixedUpdate` observers installed.
fn boot_driver_world() -> World {
    let mut world = boot_with_scripted_frames();
    add_fixed_update_observers(&mut world);
    world
}

fn fixed_update_runs(world: &World) -> usize {
    world.get_resource::<FixedUpdateRuns>().unwrap().0
}

/// T13: a frame that accumulates exactly one full timestep runs `FixedUpdate`
/// exactly once; the step reports one timestep as fixed delta, and the virtual
/// clock (`Res<Time>`) keeps the per-frame value — fixed semantics come from
/// `FixedTime`, not `Time`.
#[test]
fn fixed_update_runs_once_per_full_timestep() {
    let mut world = boot_driver_world();

    set_frame_dt(&mut world, Duration::from_micros(15_625)); // one 64 Hz step

    run_frame(&mut world);

    assert_eq!(
        fixed_update_runs(&world),
        1,
        "a full-step frame must run FixedUpdate exactly once"
    );
    let fixed = world.get_resource::<FixedTime>().unwrap();
    assert_eq!(fixed.overstep(), Duration::ZERO);
    assert_eq!(fixed.delta(), Duration::from_micros(15_625));
    assert_eq!(fixed.elapsed(), Duration::from_micros(15_625));
    assert_eq!(
        world.get_resource::<FixedUpdateSightings>().unwrap().0,
        vec![(Duration::from_micros(15_625), Duration::from_micros(15_625))],
        "the FixedUpdate content must see one fixed timestep and the frame's virtual delta"
    );

    // A second identical frame steps again: nothing was carried over.
    run_frame(&mut world);
    assert_eq!(fixed_update_runs(&world), 2);
    assert_eq!(
        world.get_resource::<FixedTime>().unwrap().elapsed(),
        Duration::from_micros(31_250)
    );
}

/// T14: a sub-step frame never runs `FixedUpdate`; the remainder is carried
/// across frames and only spent once it finally covers a whole step.
#[test]
fn fixed_update_remainder_carries_across_frames() {
    let mut world = boot_driver_world();

    // Frame 1: 10 ms < one 15 625 µs step → no run, the 10 ms stay over.
    set_frame_dt(&mut world, Duration::from_millis(10));
    run_frame(&mut world);
    assert_eq!(fixed_update_runs(&world), 0);
    assert_eq!(
        world.get_resource::<FixedTime>().unwrap().overstep(),
        Duration::from_millis(10)
    );

    // Frame 2: 10 ms more → 20 ms covers one step; 4 375 µs remain over.
    run_frame(&mut world);
    assert_eq!(fixed_update_runs(&world), 1);
    let fixed = world.get_resource::<FixedTime>().unwrap();
    assert_eq!(fixed.overstep(), Duration::from_micros(4_375));
    assert_eq!(fixed.elapsed(), Duration::from_micros(15_625));

    // Frame 3: 4 ms → 8 375 µs < one step → still just the one run so far.
    set_frame_dt(&mut world, Duration::from_millis(4));
    run_frame(&mut world);
    assert_eq!(fixed_update_runs(&world), 1);
    assert_eq!(
        world.get_resource::<FixedTime>().unwrap().overstep(),
        Duration::from_micros(8_375)
    );
}

/// T15: one long frame can chain several steps — 50 ms pays for three 64 Hz
/// steps (46 875 µs), and every step reports one timestep as fixed delta while
/// `Res<Time>` still shows the whole 50 ms virtual delta.
#[test]
fn fixed_update_one_frame_can_run_several_steps() {
    let mut world = boot_driver_world();

    set_frame_dt(&mut world, Duration::from_millis(50));
    run_frame(&mut world);

    assert_eq!(fixed_update_runs(&world), 3);
    let fixed = world.get_resource::<FixedTime>().unwrap();
    assert_eq!(fixed.overstep(), Duration::from_micros(3_125));
    assert_eq!(fixed.elapsed(), Duration::from_micros(46_875));
    assert_eq!(
        world.get_resource::<FixedUpdateSightings>().unwrap().0,
        vec![
            (Duration::from_micros(15_625), Duration::from_millis(50)),
            (Duration::from_micros(15_625), Duration::from_millis(50)),
            (Duration::from_micros(15_625), Duration::from_millis(50)),
        ],
        "each step must report one fixed timestep while Res<Time> stays the 50 ms virtual delta"
    );
}

/// T16: an overlong frame is clamped by the virtual clock's 250 ms `max_delta`
/// before it reaches the accumulator, so one frame runs at most 16 steps at
/// 64 Hz (spiral-of-death protection lives on the virtual side).
#[test]
fn fixed_update_overlong_frame_is_capped_by_the_max_delta_clamp() {
    let mut world = boot_driver_world();

    // A 1 s stall: `Time` clamps the raw delta to 250 ms → exactly 16 steps.
    set_frame_dt(&mut world, Duration::from_secs(1));
    run_frame(&mut world);

    assert_eq!(
        world.get_resource::<Time>().unwrap().delta_time(),
        Duration::from_millis(250),
        "the virtual clock must clamp the raw delta to 250 ms"
    );
    assert_eq!(fixed_update_runs(&world), 16);
    let fixed = world.get_resource::<FixedTime>().unwrap();
    assert_eq!(fixed.overstep(), Duration::ZERO);
    assert_eq!(fixed.elapsed(), Duration::from_millis(250));

    // A second equally long frame catches up another 16 steps — never more.
    run_frame(&mut world);
    assert_eq!(fixed_update_runs(&world), 32);
    assert_eq!(
        world.get_resource::<FixedTime>().unwrap().elapsed(),
        Duration::from_millis(500)
    );
}

/// T17: while the virtual clock is paused a frame feeds zero delta, so
/// `FixedUpdate` does not run (starting from no leftover full step).
#[test]
fn fixed_update_does_not_run_while_paused() {
    let mut world = boot_driver_world();

    // One full step first: stepping works while the clock runs.
    set_frame_dt(&mut world, Duration::from_micros(15_625));
    run_frame(&mut world);
    assert_eq!(fixed_update_runs(&world), 1);

    // Pause: even a 1 s scripted stall feeds delta = 0, so no step runs.
    world.resource_mut::<Time>().pause();
    set_frame_dt(&mut world, Duration::from_secs(1));
    run_frame(&mut world);

    assert_eq!(
        fixed_update_runs(&world),
        1,
        "paused frames must not run FixedUpdate"
    );
    assert_eq!(
        world.get_resource::<Time>().unwrap().delta_time(),
        Duration::ZERO,
        "a paused frame must report zero virtual delta"
    );
    let fixed = world.get_resource::<FixedTime>().unwrap();
    assert_eq!(fixed.overstep(), Duration::ZERO);
    assert_eq!(fixed.elapsed(), Duration::from_micros(15_625));

    // Resume: fixed stepping picks up where it stopped.
    world.resource_mut::<Time>().resume();
    set_frame_dt(&mut world, Duration::from_micros(15_625));
    run_frame(&mut world);
    assert_eq!(fixed_update_runs(&world), 2);
}

/// T18: a zero-speed frame (`time_scale = 0`) also feeds zero delta — the
/// same fixed behavior as a paused frame.
#[test]
fn fixed_update_does_not_run_at_zero_speed() {
    let mut world = boot_driver_world();

    world.resource_mut::<Time>().set_time_scale(0.0);
    set_frame_dt(&mut world, Duration::from_millis(20));
    run_frame(&mut world);

    assert_eq!(fixed_update_runs(&world), 0);
    assert_eq!(
        world.get_resource::<Time>().unwrap().delta_time(),
        Duration::ZERO
    );
    let fixed = world.get_resource::<FixedTime>().unwrap();
    assert_eq!(fixed.overstep(), Duration::ZERO);
    assert_eq!(fixed.elapsed(), Duration::ZERO);

    // Back to full speed, the same scripted frame length steps again.
    world.resource_mut::<Time>().set_time_scale(1.0);
    run_frame(&mut world);
    assert_eq!(fixed_update_runs(&world), 1);
    assert_eq!(
        world.get_resource::<FixedTime>().unwrap().elapsed(),
        Duration::from_micros(15_625)
    );
}

/// T19: an *empty* `FixedUpdate` (no systems) runs silently once a frame's
/// accumulated time covers a step — the fixed clock advances by the expected
/// number of steps, no systems run, and there is no panic/warn/forced step.
#[test]
fn empty_fixed_update_runs_silently_when_a_step_is_due() {
    // No observers: `FixedUpdate` content is empty here, unlike T13–T18.
    let mut world = boot_with_scripted_frames();

    set_frame_dt(&mut world, Duration::from_millis(50)); // 3 steps + 3 125 µs
    run_frame(&mut world);

    let fixed = world.get_resource::<FixedTime>().unwrap();
    assert_eq!(
        fixed.elapsed(),
        Duration::from_micros(46_875),
        "an empty FixedUpdate must still advance the fixed clock once per step"
    );
    assert_eq!(fixed.overstep(), Duration::from_micros(3_125));

    // A second full-step frame is equally silent and steps again.
    run_frame(&mut world);
    assert_eq!(
        world.get_resource::<FixedTime>().unwrap().elapsed(),
        Duration::from_micros(93_750)
    );
}

/// T20: removing the `FixedUpdate` schedule entirely must not break a frame —
/// the driver logs and moves on (the log is not asserted, per D4), the rest of
/// the sub-stages keep running, and the fixed clock only advances through real
/// steps (nothing is drained into a missing schedule).
#[test]
fn missing_fixed_update_schedule_is_tolerated() {
    let mut world = boot_with_scripted_frames();
    world.insert_resource(Trace::default());
    add_sub_stage_tracers(&mut world);

    let removed = world
        .get_resource_mut::<Schedules>()
        .expect("Schedules")
        .remove(FixedUpdate);
    assert!(
        removed.is_some(),
        "FixedUpdate schedule should exist before removal"
    );

    set_frame_dt(&mut world, Duration::from_millis(50));
    run_frame(&mut world);

    // The frame's 50 ms were accumulated but could not be spent.
    let fixed = world.get_resource::<FixedTime>().unwrap();
    assert_eq!(fixed.overstep(), Duration::from_millis(50));
    assert_eq!(fixed.elapsed(), Duration::ZERO);

    // The remaining per-frame sub-stages still ran once, in canonical order.
    let trace = &world.get_resource::<Trace>().unwrap().0;
    assert_eq!(trace, SUB_STAGE_ORDER.as_slice());
}
