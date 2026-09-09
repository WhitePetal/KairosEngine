//! bevy_app-style "main schedule" rails for the editor engine.
//!
//! This module bootstraps the per-frame scheduling skeleton that engine and
//! game code register systems into. It mirrors the structure of bevy's
//! `MainSchedulePlugin` (`bevy_app/src/main_schedule.rs`) using only
//! `kairos_ecs` primitives:
//!
//! - [`Main`] is the top-level schedule the engine runs once per frame. It
//!   contains a single exclusive system, [`run_main`].
//! - [`run_main`] drives the labeled sub-schedules listed in the
//!   [`MainScheduleOrder`] resource in order. The [`Startup`] sub-schedule runs
//!   exactly once — on the first frame, before any other sub-stage. A missing
//!   sub-schedule is tolerated: its error is logged and the remaining labels
//!   still run.
//! - Fixed-rate work mounts onto the [`FixedUpdate`] sub-schedule, which is
//!   *not* part of the per-frame label list. Between `PreUpdate` and `Update`
//!   sits [`RunFixedMainLoop`], a stage hosting a single exclusive driver
//!   system, [`run_fixed_main_loop`]. Every frame the driver feeds the virtual
//!   clock's delta into the [`FixedTime`] accumulator and then re-runs
//!   `FixedUpdate` once per full timestep — 0..N runs per frame (0 when the
//!   accumulated time covers no full step). `FixedUpdate` systems read the
//!   fixed semantics from `Res<FixedTime>` (constant timestep, fixed elapsed);
//!   `Res<Time>` stays the per-frame virtual clock and carries no fixed
//!   meaning.
//!
//! [`install`] registers the fixed set of sub-schedules plus the resources the
//! drivers need: the engine's virtual clock ([`Time`]) and the fixed clock
//! ([`FixedTime`]) as World resources — [`time_system`], hosted by the
//! [`First`] sub-stage, advances the former exactly once per frame. When
//! `FixedUpdate` has no systems a frame costs the driver a few resource
//! accesses and, whenever the accumulated time covers a full step, one run of
//! an empty schedule — no user systems, no warn, and no forced first step.

use crate::time::{FixedTime, Time};
use kairos_ecs::{
    resource::Resource,
    schedule::{InternedScheduleLabel, MainThreadExecutor, Schedule, ScheduleLabel, Schedules},
    system::{Local, ResMut},
    world::World,
};

/// The top-level schedule driven once per editor frame; hosts [`run_main`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Main;

/// The schedule run exactly once, on the first frame, before all sub-stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Startup;

/// The first per-frame sub-stage (bevy parity: runs before `PreUpdate`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct First;

/// Sub-stage for work that must happen before the fixed step / main update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PreUpdate;

/// Sub-stage between `PreUpdate` and `Update` (bevy's `RunFixedMainLoop`).
///
/// Runs exactly once per frame and hosts the sole exclusive system
/// [`run_fixed_main_loop`], the fixed-step driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RunFixedMainLoop;

/// The fixed-timestep content sub-schedule: the single mount point for
/// fixed-rate systems.
///
/// Not listed in the per-frame [`MainScheduleOrder`]; instead
/// [`run_fixed_main_loop`] runs it 0..N times per frame — once per full
/// [`FixedTime`] timestep that the accumulated virtual time pays for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FixedUpdate;

/// The main per-frame sub-stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Update;

/// Sub-stage for work that reacts to the results of `Update`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PostUpdate;

/// The last per-frame sub-stage (bevy parity: runs after `PostUpdate`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Last;

// The `ScheduleLabel` derive macro adds `intern` (the trait default) plus a
// `dyn_clone` identical to the one below; we implement it by hand to keep the
// skeleton free of a `kairos_ecs_macros` dependency.
macro_rules! impl_schedule_label {
    ($($label:ident),+ $(,)?) => {
        $(
            impl ScheduleLabel for $label {
                fn dyn_clone(&self) -> Box<dyn ScheduleLabel> {
                    Box::new(*self)
                }
            }
        )+
    };
}

impl_schedule_label!(
    Main,
    Startup,
    First,
    PreUpdate,
    RunFixedMainLoop,
    FixedUpdate,
    Update,
    PostUpdate,
    Last,
);

/// The order in which [`run_main`] drives the engine's sub-schedules.
///
/// `run_main` runs every label in [`startup_labels`](Self::startup_labels)
/// exactly once on the first frame, and then every label in
/// [`labels`](Self::labels) once per frame, in order. The default per-frame
/// list walks `First` (clock advance) … `PreUpdate` … `RunFixedMainLoop`
/// (fixed-step driver) … `Update` … `Last`; `FixedUpdate` is deliberately not
/// here — the driver inside `RunFixedMainLoop` re-runs it 0..N times per
/// frame. The lists are stored as interned labels so iteration and equality
/// checks are cheap.
#[derive(Resource)]
pub struct MainScheduleOrder {
    /// Schedules run once, in order, on the first frame (currently `[Startup]`).
    pub startup_labels: Vec<InternedScheduleLabel>,
    /// Schedules run once per frame, in order (bevy's `First … Last` set).
    pub labels: Vec<InternedScheduleLabel>,
}

impl Default for MainScheduleOrder {
    fn default() -> Self {
        Self {
            startup_labels: vec![Startup.intern()],
            labels: vec![
                First.intern(),
                PreUpdate.intern(),
                RunFixedMainLoop.intern(),
                Update.intern(),
                PostUpdate.intern(),
                Last.intern(),
            ],
        }
    }
}

/// Advances the engine's virtual clock ([`Time`]) exactly once per frame.
///
/// Registered into the [`First`] sub-stage — the first per-frame stage — so a
/// fresh `delta_time` is available to every later stage and to engine-side
/// code that reads the clock after the frame's schedules have run. This is the
/// only place the clock is advanced (bevy `TimePlugin` parity: `time_system`
/// hangs off `First`).
fn time_system(mut time: ResMut<Time>) {
    time.update();
}

/// The exclusive driver system hosted by the [`Main`] schedule.
///
/// On its first execution it runs the `startup_labels` schedules once; after
/// that (and on every execution) it runs the `labels` schedules in order. A
/// missing sub-schedule is tolerated: it logs a warning and execution moves on
/// to the next label.
fn run_main(world: &mut World, mut run_at_least_once: Local<bool>) {
    if !*run_at_least_once {
        world.resource_scope::<MainScheduleOrder, _>(|world, order| {
            for &label in &order.startup_labels {
                if let Err(error) = world.try_run_schedule(label) {
                    log::error!("skipping startup schedule `{label:?}`: {error}");
                }
            }
        });
        *run_at_least_once = true;
    }

    world.resource_scope::<MainScheduleOrder, _>(|world, order| {
        for &label in &order.labels {
            if let Err(error) = world.try_run_schedule(label) {
                log::error!("skipping sub-stage schedule `{label:?}`: {error}");
            }
        }
    });
}

/// The exclusive fixed-step driver system hosted by the [`RunFixedMainLoop`]
/// schedule.
///
/// Runs exactly once per frame, after the `First`-stage [`time_system`] has
/// advanced the virtual clock:
///
/// 1. read the frame's virtual delta from [`Time`] — it is already
///    `max_delta`-clamped, `time_scale`-scaled and `paused`-aware, so a
///    paused or zero-speed frame feeds `Duration::ZERO`;
/// 2. accumulate that delta into [`FixedTime`];
/// 3. while the accumulator still covers a full timestep, run the
///    [`FixedUpdate`] schedule exactly once per step.
///
/// The per-frame step count is therefore 0..N with no explicit loop bound —
/// the implicit cap is the virtual clock's default 250 ms `max_delta` clamp
/// (~16 steps per frame at the default 64 Hz). The `FixedUpdate` schedule is
/// temporarily pulled out of `Schedules` for the loop (kairos_ecs schedule
/// scope) so the 0..N consecutive runs share one system-state cache. A missing
/// `FixedUpdate` schedule is tolerated like any other missing sub-stage: the
/// error is logged and the frame moves on; nothing is drained in that case, so
/// the fixed clock only ever advances through real steps.
fn run_fixed_main_loop(world: &mut World) {
    let delta = world.resource::<Time>().delta_time();
    world.resource_mut::<FixedTime>().accumulate(delta);

    // Run the fixed schedule until the accumulated time runs out (0..N times).
    if let Err(error) = world.try_schedule_scope(FixedUpdate, |world, schedule| {
        while world.resource_mut::<FixedTime>().expend() {
            schedule.run(world);
        }
    }) {
        log::error!("skipping fixed-update schedule `{FixedUpdate:?}`: {error}");
    }
}

/// Bootstraps the schedule rails onto a fresh [`World`].
///
/// Registers the sub-schedules — the [`First`] one hosting [`time_system`], the
/// [`RunFixedMainLoop`] one hosting the fixed-step [`run_fixed_main_loop`]
/// driver, and the empty [`FixedUpdate`] content schedule — builds the [`Main`]
/// schedule around [`run_main`], and inserts the driver resources ([`Time`],
/// [`FixedTime`], [`MainScheduleOrder`], plus a [`MainThreadExecutor`]
/// captured on the calling thread). `Schedules` itself is created on demand by
/// the world.
pub(crate) fn install(world: &mut World) {
    log::debug!("installing the main-schedule rails");

    // The engine's virtual clock lives in the World as a resource and is
    // advanced exactly once per frame by `time_system` below. The fixed clock
    // sits alongside it; `run_fixed_main_loop` consumes it every frame inside
    // `RunFixedMainLoop`.
    world.insert_resource(Time::new());
    world.insert_resource(FixedTime::new());

    let mut schedules = world.get_resource_or_init::<Schedules>();

    let mut first = Schedule::new(First);
    first.add_systems(time_system);
    schedules.insert(first);

    let mut fixed_loop = Schedule::new(RunFixedMainLoop);
    fixed_loop.add_systems(run_fixed_main_loop);
    schedules.insert(fixed_loop);

    for label in [
        Startup.intern(),
        PreUpdate.intern(),
        FixedUpdate.intern(),
        Update.intern(),
        PostUpdate.intern(),
        Last.intern(),
    ] {
        schedules.insert(Schedule::new(label));
    }

    let mut main = Schedule::new(Main);
    main.add_systems(run_main);
    schedules.insert(main);

    world.insert_resource(MainScheduleOrder::default());
    // The driver may eventually contain `!Send` systems; capture this (main)
    // thread's executor at bootstrap so they always run where they must.
    world.insert_resource(MainThreadExecutor::new());
}

#[cfg(test)]
mod test;
