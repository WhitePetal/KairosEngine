//! bevy_app-style "main schedule" rails for the editor engine.
//!
//! This module bootstraps the per-frame scheduling skeleton that engine and game
//! code register systems into. It mirrors the structure of bevy's
//! `MainSchedulePlugin` (`bevy_app/src/main_schedule.rs`) using only
//! `kairos_ecs` primitives:
//!
//! - [`Main`] is the top-level schedule the engine runs once per frame. It
//!   contains a single exclusive system, [`run_main`].
//! - [`run_main`] drives the labeled sub-schedules listed in the
//!   [`MainScheduleOrder`] resource in order. The [`Startup`] sub-schedule runs
//!   exactly once — on the first frame, before any other sub-stage.
//! - [`install`] registers the fixed set of sub-schedules plus the resources
//!   the driver needs. The engine's virtual clock ([`Time`]) is registered as
//!   a World resource and advanced by [`time_system`], hosted by the [`First`]
//!   sub-stage. The other sub-schedules are still empty, so a frame costs
//!   nothing beyond walking the label list and advancing the clock once.
//!
//! `FixedUpdate` is only a placeholder in the per-frame label order at this
//! stage; fixed-timestep semantics are out of scope for the skeleton.

use crate::timer::Time;
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

/// Placeholder for the fixed-timestep sub-stage (driven per frame for now).
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
    FixedUpdate,
    Update,
    PostUpdate,
    Last,
);

/// The order in which [`run_main`] drives the engine's sub-schedules.
///
/// `run_main` runs every label in [`startup_labels`](Self::startup_labels)
/// exactly once on the first frame, and then every label in
/// [`labels`](Self::labels) once per frame, in order. The lists are stored as
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
                FixedUpdate.intern(),
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

/// Bootstraps the schedule rails onto a fresh [`World`].
///
/// Registers the sub-schedules (the [`First`] one hosting [`time_system`]),
/// builds the [`Main`] schedule around [`run_main`], and inserts the driver
/// resources ([`Time`], [`MainScheduleOrder`], plus a [`MainThreadExecutor`]
/// captured on the calling thread). `Schedules` itself is created on demand by
/// the world.
pub(crate) fn install(world: &mut World) {
    log::debug!("installing the main-schedule rails");

    // The engine's virtual clock lives in the World as a resource; it is
    // advanced exactly once per frame by `time_system` below.
    world.insert_resource(Time::new());

    let mut schedules = world.get_resource_or_init::<Schedules>();

    let mut first = Schedule::new(First);
    first.add_systems(time_system);
    schedules.insert(first);

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
