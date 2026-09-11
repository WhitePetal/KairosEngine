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
//! - [`Extract`] is the per-frame extraction stage, sitting between
//!   [`PostUpdate`] and [`Last`]: everything that *modifies* the world has
//!   finished by the time it runs, so "this runs before the frame is read" is
//!   a scheduling boundary rather than a per-system `.after()` promise.
//! - Fixed-rate work mounts onto the [`FixedUpdate`] sub-schedule, which is
//!   *not* part of the per-frame label list. Between `PreUpdate` and `Update`
//!   sits [`RunFixedMainLoop`], a stage hosting a single exclusive driver
//!   system, [`run_fixed_main_loop`]. Every frame the driver feeds the virtual
//!   clock's delta into the [`FixedTime`] accumulator and then re-runs
//!   `FixedUpdate` once per full timestep — 0..N runs per frame (0 when the
//!   accumulated time covers no full step). `FixedUpdate` systems read the
//!   fixed semantics from `Res<FixedTime>` (constant timestep, fixed elapsed);
//!   `Res<Time>` stays the per-frame virtual clock and carries no fixed
//!   meaning. Each step then runs [`FixedPostUpdate`] — the fixed-step
//!   post-update seam — mirroring bevy's `FixedMain` order.
//!
//! [`install`] registers the fixed set of sub-schedules plus the resources the
//! drivers need: the engine's virtual clock ([`Time`]) and the fixed clock
//! ([`FixedTime`]) as World resources — [`time_system`], hosted by the
//! [`First`] sub-stage, advances the former exactly once per frame. The
//! [`First`] stage also hosts the frame's message pass
//! ([`message_update_system`]), gated on [`MessageRegistry`] and signalled by
//! [`singnal_message_update_system`] in [`FixedPostUpdate`]. When `FixedUpdate`
//! has no systems a frame costs the driver a few resource accesses and,
//! whenever the accumulated time covers a full step, one run of an empty
//! schedule — no user systems, no warn, and no forced first step.

use crate::time::{FixedTime, Time};
use kairos_ecs::{
    message::{
        MessageRegistry, MessageUpdateSystems, ShouldUpdateMessages, message_update_comdition,
        message_update_system, singnal_message_update_system,
    },
    resource::Resource,
    schedule::{
        InternedScheduleLabel, IntoScheduleConfigs, MainThreadExecutor, Schedule, ScheduleLabel,
        Schedules,
    },
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

/// The fixed-step post-update sub-schedule: work that reacts to a fixed step
/// once the step's content has run (bevy parity: `FixedPostUpdate`).
///
/// Like [`FixedUpdate`], it is not in the per-frame [`MainScheduleOrder`];
/// [`run_fixed_main_loop`] runs it once per fixed step, immediately after that
/// step's `FixedUpdate`. It hosts
/// [`singnal_message_update_system`](kairos_ecs::message::singnal_message_update_system),
/// which releases the frame's messages to the next frame's message update, and
/// is the seam later fixed-rate modules (physics) hook into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FixedPostUpdate;

/// The main per-frame sub-stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Update;

/// Sub-stage for work that reacts to the results of `Update`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PostUpdate;

/// The per-frame extraction stage: after every modifier, before [`Last`].
///
/// Everything that mutates the world lives in the stages up to and including
/// [`PostUpdate`]; stage systems that *read* the frame's final state — the
/// render extraction rails, for instance — register here. The ordering is a
/// property of the schedule, not of the individual systems: a system that is
/// not registered into this stage cannot accidentally run after it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Extract;

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
    FixedPostUpdate,
    Update,
    PostUpdate,
    Extract,
    Last,
);

/// The order in which [`run_main`] drives the engine's sub-schedules.
///
/// `run_main` runs every label in [`startup_labels`](Self::startup_labels)
/// exactly once on the first frame, and then every label in
/// [`labels`](Self::labels) once per frame, in order. The default per-frame
/// list walks `First` (clock advance) … `PreUpdate` … `RunFixedMainLoop`
/// (fixed-step driver) … `Update` … `PostUpdate` … `Extract` (frame read) …
/// `Last`; `FixedUpdate` is deliberately not here — the driver inside
/// `RunFixedMainLoop` re-runs it 0..N times per frame. The lists are stored as
/// interned labels so iteration and equality checks are cheap.
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
                Extract.intern(),
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

    // Run the fixed schedules until the accumulated time runs out (0..N times).
    // Each step runs `FixedUpdate` and then `FixedPostUpdate`, matching bevy's
    // `FixedMain` order (`FixedUpdate` before `FixedPostUpdate`); the post step
    // is where the message signal lives. Both schedules are scoped around the
    // whole loop so the 0..N consecutive runs share one system-state cache, and
    // either may be missing without breaking the frame: the other stage still
    // runs and the fixed clock still advances.
    let outcome = world.try_schedule_scope(FixedUpdate, |world, fixed_update| {
        let post_update = world.try_schedule_scope(FixedPostUpdate, |world, fixed_post_update| {
            while world.resource_mut::<FixedTime>().expend() {
                fixed_update.run(world);
                fixed_post_update.run(world);
            }
        });

        match post_update {
            Ok(()) => None,
            Err(error) => {
                // The post stage is missing: still spend every due step on
                // `FixedUpdate` alone, then report the missing schedule.
                while world.resource_mut::<FixedTime>().expend() {
                    fixed_update.run(world);
                }
                Some(error)
            }
        }
    });

    match outcome {
        Ok(None) => {}
        Ok(Some(error)) => {
            log::error!("skipping fixed-post-update stage: {error}");
        }
        Err(error) => {
            log::error!("skipping fixed-update schedule `{FixedUpdate:?}`: {error}");
        }
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

    // Messages are held back until a fixed step has run: the registry starts in
    // `Waiting`, and the `FixedPostUpdate` signal below releases them. This is
    // bevy's `TimePlugin` posture, and it gives fixed-rate systems a chance to
    // observe a frame's messages before the message pass clears them.
    let mut messages = world.get_resource_or_init::<MessageRegistry>();
    messages.should_update = ShouldUpdateMessages::Waiting;

    let mut schedules = world.get_resource_or_init::<Schedules>();

    let mut first = Schedule::new(First);
    first.add_systems((
        // The clock advance and the message pass are independent of each other.
        time_system.ambiguous_with(message_update_system),
        // Messages are polled once per frame, but only once a fixed step has
        // signalled the registry (see `FixedPostUpdate` below).
        message_update_system
            .in_set(MessageUpdateSystems)
            .run_if(message_update_comdition),
    ));
    schedules.insert(first);

    let mut fixed_loop = Schedule::new(RunFixedMainLoop);
    fixed_loop.add_systems(run_fixed_main_loop);
    schedules.insert(fixed_loop);

    let mut fixed_post_update = Schedule::new(FixedPostUpdate);
    fixed_post_update.add_systems(singnal_message_update_system);
    schedules.insert(fixed_post_update);

    for label in [
        Startup.intern(),
        PreUpdate.intern(),
        FixedUpdate.intern(),
        Update.intern(),
        PostUpdate.intern(),
        Extract.intern(),
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
mod asset_test;

#[cfg(test)]
mod test;
