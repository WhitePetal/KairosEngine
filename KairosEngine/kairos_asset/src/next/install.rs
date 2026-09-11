//! Registration and scheduling: the top-level [`install`] seam and the per-type
//! registration API.
//!
//! `kairos_asset` cannot depend on `kairos_engine` (engine reaches this crate
//! through graphics), so it does not name the engine's stages itself. Instead
//! [`install`] takes the two stage labels from the caller and records them in the
//! [`AssetStages`] world resource; every later [`AssetWorldExt::init_asset`] call
//! reads them when it mounts its driver systems. This is the one structural
//! deviation from `bevy_asset`, where `AssetPlugin` mounts into
//! `PreUpdate`/`PostUpdate` directly (ADR 0003).
//!
//! The work is split across the two stages so that the event face lines up with
//! `bevy_asset`:
//!
//! - Tracking stage (bevy's `PreUpdate`): [`handle_internal_asset_events`] drains
//!   the load results tasks sent back, inserts values into their stores, and
//!   writes `LoadedWithDependencies` directly. The per-type
//!   [`Assets::track_assets`](crate::next::Assets::track_assets) then releases the
//!   values whose last strong handle was dropped. `Added`/`Modified`/`Removed`/
//!   `Unused` are queued in the store, not yet visible.
//! - Event stage (bevy's `PostUpdate`): the per-type
//!   [`Assets::asset_events`](crate::next::Assets::asset_events) flushes those
//!   queued events into `Messages<AssetEvent<A>>`.

use kairos_ecs::message::MessageRegistry;
use kairos_ecs::resource::Resource;
use kairos_ecs::schedule::{
    InternedScheduleLabel, IntoScheduleConfigs, ScheduleLabel, Schedules, SystemSet,
};
use kairos_ecs::world::{FromWorld, World};

use crate::next::asset::Asset;
use crate::next::assets::Assets;
use crate::next::event::{AssetEvent, AssetLoadFailedEvent};
use crate::next::loader::AssetLoader;
use crate::next::server::{AssetServer, handle_internal_asset_events};

/// The schedule labels the asset drivers are installed into, recorded by
/// [`install`] so later `init_asset` calls can mount their systems without
/// naming an engine stage themselves.
#[derive(Resource, Clone, Copy, Debug)]
pub struct AssetStages {
    /// The stage that processes load results and tracks handle drops (bevy's
    /// `PreUpdate`).
    pub tracking: InternedScheduleLabel,
    /// The stage that flushes queued store events to `Messages` (bevy's
    /// `PostUpdate`).
    pub event: InternedScheduleLabel,
}

/// The system set that groups the per-type tracking-stage drivers, so callers can
/// order their own systems around them.
///
/// The exclusive [`handle_internal_asset_events`] runs before this set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AssetTrackingSystems;

/// The system set that groups the per-type event-stage flush systems.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AssetEventSystems;

// Implemented by hand rather than derived: the `SystemSet` derive lives in
// `kairos_ecs_macros`, which this crate does not depend on. The engine's
// schedule labels fold their repeated impls the same way.
macro_rules! impl_system_set {
    ($($set:ident),+ $(,)?) => {
        $(
            impl SystemSet for $set {
                fn dyn_clone(&self) -> Box<dyn SystemSet> {
                    Box::new(*self)
                }
            }
        )+
    };
}

impl_system_set!(AssetTrackingSystems, AssetEventSystems);

/// Installs the asset core into `world`: the [`AssetServer`] resource, the
/// [`AssetStages`] record, and the exclusive driver system.
///
/// The caller supplies the two stage labels because this crate cannot name the
/// engine's stages (ADR 0003). The engine calls this with `PreUpdate` and
/// `PostUpdate`; tests and other hosts may pass their own labels, which are
/// created if they do not exist yet — unlike `kairos_physics::install`, which
/// treats a missing stage as a bootstrap-order bug. Creating on demand is what
/// `bevy_asset`'s `AssetPlugin` does through `App::add_systems`.
///
/// Per-type work (stores, messages, drivers) is registered separately, after the
/// asset type's own crate is known, through [`AssetWorldExt::init_asset`].
pub fn install(
    world: &mut World,
    tracking_stage: impl ScheduleLabel,
    event_stage: impl ScheduleLabel,
) {
    world.init_resource::<AssetServer>();
    let stages = AssetStages {
        tracking: tracking_stage.intern(),
        event: event_stage.intern(),
    };
    world.insert_resource(stages);

    let mut schedules = world.get_resource_or_init::<Schedules>();
    let tracking = schedules.entry(stages.tracking);
    // The per-type tracking drivers run after the exclusive handler has drained
    // the load results, so they observe the freshly inserted values.
    tracking.configure_sets(AssetTrackingSystems.after(handle_internal_asset_events));
    tracking.add_systems(handle_internal_asset_events.ambiguous_with_all());
    schedules.entry(stages.event).configure_sets(AssetEventSystems);
}

/// The per-type registration API, the `World` counterpart to `bevy_asset`'s
/// `AssetApp`.
///
/// A type is registered once, after [`install`], by the crate that owns it.
pub trait AssetWorldExt {
    /// Registers the asset type `A`: its store, its event messages, and its
    /// tracking/event driver systems.
    ///
    /// Call once per type, after [`install`].
    fn init_asset<A: Asset>(&mut self);

    /// [`AssetWorldExt::init_asset`] with a preallocated store.
    fn init_asset_with_capacity<A: Asset>(&mut self, capacity: usize);

    /// Registers `loader` so loads of the formats it names can resolve it.
    fn register_asset_loader<L: AssetLoader>(&mut self, loader: L);

    /// Builds a loader through [`FromWorld`] and registers it.
    fn init_asset_loader<L: AssetLoader + FromWorld>(&mut self);
}

impl AssetWorldExt for World {
    fn init_asset<A: Asset>(&mut self) {
        self.init_asset_with_capacity::<A>(0);
    }

    fn init_asset_with_capacity<A: Asset>(&mut self, capacity: usize) {
        // Step 1: the server takes the store's handle provider (so both allocate
        // slots from the same allocator) and fills the two typed sender maps
        // (`LoadedWithDependencies` / load failed).
        let assets = Assets::<A>::with_capacity(capacity);
        let server = self.resource::<AssetServer>().clone();
        server.register_asset(&assets);

        // Step 2: the store itself.
        self.insert_resource(assets);

        // Step 3: the messages those senders write to must exist. Registering
        // them also wires them into the frame's message-update pass.
        MessageRegistry::register_message::<AssetEvent<A>>(self);
        MessageRegistry::register_message::<AssetLoadFailedEvent<A>>(self);

        // Step 4: the per-type drivers, mounted under the two sets.
        let stages = *self.resource::<AssetStages>();
        self.resource_scope::<Schedules, _>(|world, mut schedules| {
            schedules.allow_ambiguous_resource::<Assets<A>>(world);
            schedules
                .entry(stages.tracking)
                .add_systems(Assets::<A>::track_assets.in_set(AssetTrackingSystems));
            schedules.entry(stages.event).add_systems(
                Assets::<A>::asset_events
                    .in_set(AssetEventSystems)
                    .run_if(Assets::<A>::asset_events_condition),
            );
        });
    }

    fn register_asset_loader<L: AssetLoader>(&mut self, loader: L) {
        self.resource::<AssetServer>().register_loader(loader);
    }

    fn init_asset_loader<L: AssetLoader + FromWorld>(&mut self) {
        let loader = L::from_world(self);
        self.register_asset_loader(loader);
    }
}
