//! Registration and scheduling: the top-level [`install`] seam and the per-type
//! registration API.
//!
//! `kairos_asset` cannot depend on `kairos_engine` (engine reaches this crate
//! through graphics), so it does not name the engine's stages itself. Instead
//! [`install`] takes an [`AssetOptions`] carrying the three stage labels from the
//! caller and records them in the [`AssetStages`] world resource; every later
//! [`AssetWorldExt::init_asset`] call reads them when it mounts its driver
//! systems. This is the one structural deviation from `bevy_asset`, where
//! `AssetPlugin` mounts into `PreUpdate`/`PostUpdate` directly (ADR 0003).
//!
//! The work is split across the tracking and event stages so that the event face
//! lines up with `bevy_asset`:
//!
//! - Tracking stage (bevy's `PreUpdate`): [`handle_internal_asset_events`] drains
//!   the load results tasks sent back, inserts values into their stores, and
//!   writes `LoadedWithDependencies` directly. The per-type
//!   [`Assets::track_assets`](crate::Assets::track_assets) then releases the
//!   values whose last strong handle was dropped. `Added`/`Modified`/`Removed`/
//!   `Unused` are queued in the store, not yet visible.
//! - Event stage (bevy's `PostUpdate`): the per-type
//!   [`Assets::asset_events`](crate::Assets::asset_events) flushes those
//!   queued events into `Messages<AssetEvent<A>>`.
//! - Startup stage (bevy's `Startup`): where the asset processor is launched.
//!   In layout ② (a runtime processor) [`AssetProcessor::start`] is mounted
//!   here; the stage is always created so the processor can attach without
//!   re-plumbing `install`.

use std::any::TypeId;
use std::sync::Arc;

use kairos_ecs::message::MessageRegistry;
use kairos_ecs::resource::Resource;
use kairos_ecs::schedule::{
    InternedScheduleLabel, IntoScheduleConfigs, ScheduleLabel, Schedules, SystemSet,
};
use kairos_ecs::world::{FromWorld, World};

use crate::asset::Asset;
use crate::assets::{Assets, LoadedUntypedAsset};
use crate::event::{AssetEvent, AssetLoadFailedEvent, UntypedAssetLoadFailedEvent};
use crate::folder::LoadedFolder;
use crate::handle::AssetHandleProvider;
use crate::index::AssetIndexAllocator;
use crate::io::embedded::{EMBEDDED, EmbeddedAssetRegistry};
use crate::io::{AssetSourceBuilder, AssetSourceBuilders, AssetSourceId, UnapprovedPathMode};
use crate::loader::AssetLoader;
use crate::meta::AssetMetaCheck;
use crate::processor::{AssetProcessor, Process};
use crate::server::{AssetServer, AssetServerMode, handle_internal_asset_events};

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
    /// The stage the asset processor starts in (bevy's `Startup`). Layout ②
    /// mounts [`AssetProcessor::start`] here; otherwise the stage is created
    /// empty.
    pub startup: InternedScheduleLabel,
}

/// The asset pipeline layout the server runs in.
///
/// Mirrors `bevy_asset`'s `AssetMode`, but selects between the layouts kairos
/// supports today (ADR 0004):
///
/// - `Unprocessed` (layout ①): the server reads source assets directly and
///   consults their `.meta` sidecars.
/// - `Processed` (layout ②/③): the server reads processed assets. Layout ③ is
///   processed assets with no processor running — the artifacts must already
///   exist on disk. Layout ② runs a processor that produces them at runtime; it
///   is selected with [`AssetOptions::use_asset_processor`], and its gated
///   readers make the app's server wait for the processor's output.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AssetMode {
    /// Loads assets from their source's unprocessed reader, consulting `.meta`
    /// sidecars. The default.
    #[default]
    Unprocessed,
    /// Loads assets from their source's processed reader.
    Processed,
}

/// The configuration [`install`] takes: the caller's stage labels plus the
/// server options `bevy_asset`'s `AssetPlugin` carries.
///
/// The stage labels are the one structural deviation from `bevy_asset` (ADR
/// 0003); the rest mirrors `AssetPlugin`'s fields. `kairos_asset` cannot name
/// the engine's stages, so the host injects them here rather than the crate
/// reaching back up into `kairos_engine`.
#[derive(Clone, Debug)]
pub struct AssetOptions {
    /// The stage that processes load results (bevy's `PreUpdate`).
    pub tracking_stage: InternedScheduleLabel,
    /// The stage that flushes queued store events (bevy's `PostUpdate`).
    pub event_stage: InternedScheduleLabel,
    /// The stage a runtime processor would start in (bevy's `Startup`).
    pub startup_stage: InternedScheduleLabel,
    /// Which pipeline layout to run in. Defaults to [`AssetMode::Unprocessed`].
    pub mode: AssetMode,
    /// How and where `.meta` sidecars are consulted. Ignored in
    /// [`AssetMode::Processed`], which always reads meta (ADR 0002).
    pub meta_check: AssetMetaCheck,
    /// How paths that escape their source root are treated. Defaults to
    /// [`UnapprovedPathMode::Forbid`], so a `..` that climbs out of a source is
    /// rejected (ADR 0004).
    pub unapproved_path_mode: UnapprovedPathMode,
    /// Whether a runtime processor should produce the processed store. Defaults
    /// to the `asset_processor` cargo feature (off by default).
    ///
    /// Only meaningful with [`AssetMode::Processed`], where it selects layout ②
    /// (a processor producing the outputs) over layout ③ (outputs shipped ahead
    /// of time).
    pub use_asset_processor: bool,
    /// Overrides whether the server watches its sources for changes. When
    /// `None`, watching follows the `watch` cargo feature, which the default
    /// feature set turns on through `file_watcher`.
    pub watch_for_changes_override: Option<bool>,
    /// The processed root, relative to the working directory. `None` keeps the
    /// default `imported_assets/Default` (ADR 0004). Only consulted in
    /// [`AssetMode::Processed`].
    pub processed_file_path: Option<String>,
}

impl AssetOptions {
    /// The default unprocessed root: the process working directory (ADR 0004).
    const DEFAULT_UNPROCESSED_FILE_PATH: &'static str = "";
    /// The default processed root, relative to the working directory.
    pub const DEFAULT_PROCESSED_FILE_PATH: &'static str = "imported_assets/Default";

    /// Options for the three stages, with bevy's defaults for everything else:
    /// unprocessed mode, `.meta` always checked, no processor, no watch
    /// override.
    pub fn new(
        tracking_stage: impl ScheduleLabel,
        event_stage: impl ScheduleLabel,
        startup_stage: impl ScheduleLabel,
    ) -> Self {
        Self {
            tracking_stage: tracking_stage.intern(),
            event_stage: event_stage.intern(),
            startup_stage: startup_stage.intern(),
            mode: AssetMode::default(),
            meta_check: AssetMetaCheck::default(),
            unapproved_path_mode: UnapprovedPathMode::default(),
            use_asset_processor: cfg!(feature = "asset_processor"),
            watch_for_changes_override: None,
            processed_file_path: None,
        }
    }

    /// Sets the pipeline layout.
    pub fn with_mode(mut self, mode: AssetMode) -> Self {
        self.mode = mode;
        self
    }

    /// Sets how and where `.meta` sidecars are consulted.
    pub fn with_meta_check(mut self, meta_check: AssetMetaCheck) -> Self {
        self.meta_check = meta_check;
        self
    }

    /// Sets how paths that escape their source root are treated.
    pub fn with_unapproved_path_mode(mut self, mode: UnapprovedPathMode) -> Self {
        self.unapproved_path_mode = mode;
        self
    }

    /// Sets whether a runtime processor should produce the processed store.
    pub fn with_use_asset_processor(mut self, use_asset_processor: bool) -> Self {
        self.use_asset_processor = use_asset_processor;
        self
    }

    /// Overrides whether the server watches its sources for changes.
    pub fn with_watch_for_changes_override(mut self, watch: bool) -> Self {
        self.watch_for_changes_override = Some(watch);
        self
    }

    /// Sets the processed root, relative to the working directory.
    ///
    /// Defaults to `imported_assets/Default`; only consulted in
    /// [`AssetMode::Processed`].
    pub fn with_processed_file_path(mut self, path: impl Into<String>) -> Self {
        self.processed_file_path = Some(path.into());
        self
    }
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
/// The caller supplies the stage labels through [`AssetOptions`] because this
/// crate cannot name the engine's stages (ADR 0003). The engine passes
/// `PreUpdate`/`PostUpdate`/`Startup`; tests and other hosts may pass their own
/// labels, which are created if they do not exist yet — unlike
/// `kairos_physics::install`, which treats a missing stage as a bootstrap-order
/// bug. Creating on demand is what `bevy_asset`'s `AssetPlugin` does through
/// `App::add_systems`.
///
/// The [`AssetServer`] is built from the [`AssetSourceBuilders`] resource, so a
/// host may register named sources (or a custom default source) before calling
/// this; a missing default is filled in per [`AssetOptions::mode`].
///
/// Per-type work (stores, messages, drivers) is registered separately, after the
/// asset type's own crate is known, through [`AssetWorldExt::init_asset`].
pub fn install(world: &mut World, options: AssetOptions) {
    let stages = AssetStages {
        tracking: options.tracking_stage,
        event: options.event_stage,
        startup: options.startup_stage,
    };

    // The effective value follows ADR 0006: the caller's override, else
    // `cfg!(feature = "watch")`. `watch` is on by default (the default feature set
    // turns on `file_watcher`, which implies `watch`), so a host opts *out* to
    // disable watching.
    let watch = options
        .watch_for_changes_override
        .unwrap_or(cfg!(feature = "watch"));
    let use_processor = options.mode == AssetMode::Processed && options.use_asset_processor;

    // The embedded source is registered through the world's
    // [`EmbeddedAssetRegistry`], so `embedded://` paths resolve and the macros can
    // populate the registry before or after `install`. A host that pre-registered
    // an `embedded` source keeps its builder.
    world.get_resource_or_init::<EmbeddedAssetRegistry>();
    let embedded_registry = world.resource::<EmbeddedAssetRegistry>().clone();

    // Freeze the sources exactly as `AssetPlugin` does: in processed mode the
    // default source gains a processed root, and the watcher slots are chosen by
    // the mode (unprocessed sources watch the source side, processed mode the
    // processed side). Layout ② builds the processor over the same sources and
    // lets it gate their processed readers.
    let (sources, server_mode, meta_check, processor) = {
        let mut builders = world.get_resource_or_init::<AssetSourceBuilders>();
        let processed_path = match options.mode {
            AssetMode::Unprocessed => None,
            AssetMode::Processed => Some(
                options
                    .processed_file_path
                    .as_deref()
                    .unwrap_or(AssetOptions::DEFAULT_PROCESSED_FILE_PATH),
            ),
        };
        builders.init_default_source(AssetOptions::DEFAULT_UNPROCESSED_FILE_PATH, processed_path);
        if builders.get_mut(EMBEDDED).is_none() {
            embedded_registry.register_source(&mut builders);
        }
        match options.mode {
            AssetMode::Unprocessed => (
                Arc::new(builders.build_sources(watch, false)),
                AssetServerMode::Unprocessed,
                options.meta_check.clone(),
                None,
            ),
            AssetMode::Processed if use_processor => {
                // Layout ②: the processor freezes the sources (watching the
                // unprocessed side) and gates their processed readers, so the
                // app's server waits on the processor's output.
                let (processor, sources) = AssetProcessor::new(&mut builders, watch);
                (
                    sources,
                    AssetServerMode::Processed,
                    AssetMetaCheck::Always,
                    Some(processor),
                )
            }
            AssetMode::Processed => (
                // Layout ③: processed assets shipped ahead of time, so nothing
                // watches the source side; the processed side may watch.
                Arc::new(builders.build_sources(false, watch)),
                AssetServerMode::Processed,
                // Processed assets always carry meta (bevy parity).
                AssetMetaCheck::Always,
                None,
            ),
        }
    };

    // In layout ② the app's server shares the processor's sources and loaders, so
    // a loader registered on one is visible to the other and reads wait on the
    // processor's output.
    let server = match &processor {
        Some(processor) => AssetServer::new_sharing_loaders_with(
            processor.server(),
            sources,
            server_mode,
            meta_check,
            watch,
            options.unapproved_path_mode,
        ),
        None => AssetServer::new_with_meta_check(
            sources,
            server_mode,
            meta_check,
            watch,
            options.unapproved_path_mode,
        ),
    };
    let has_processor = processor.is_some();
    world.insert_resource(server);
    if let Some(processor) = processor {
        world.insert_resource(processor);
    }
    world.insert_resource(stages);

    {
        let mut schedules = world.get_resource_or_init::<Schedules>();
        let tracking = schedules.entry(stages.tracking);
        // The per-type tracking drivers run after the exclusive handler has drained
        // the load results, so they observe the freshly inserted values.
        tracking.configure_sets(AssetTrackingSystems.after(handle_internal_asset_events));
        tracking.add_systems(handle_internal_asset_events.ambiguous_with_all());
        schedules
            .entry(stages.event)
            .configure_sets(AssetEventSystems);
        // Create the startup schedule (and, in layout ②, mount the processor) so
        // the host does not have to build the stage itself.
        let startup = schedules.entry(stages.startup);
        if has_processor {
            startup.add_systems(AssetProcessor::start);
        }
    }

    // The untyped-load wrapper and the folder value are core asset types, so
    // their stores and drivers are registered with the rest of the core (bevy
    // does the same in `AssetPlugin`).
    world.init_asset::<LoadedUntypedAsset>();
    world.init_asset::<LoadedFolder>();

    // The untyped failure message is core too: one stream carries every asset
    // type's load failures, whatever the type.
    MessageRegistry::register_message::<UntypedAssetLoadFailedEvent>(world);
}

/// The per-type registration API, the `World` counterpart to `bevy_asset`'s
/// `AssetApp`.
///
/// A type is registered once, after [`install`], by the crate that owns it.
/// Every method returns `&mut Self` so registrations chain, matching `AssetApp`.
pub trait AssetWorldExt {
    /// Registers the asset type `A`: its store, its event messages, and its
    /// tracking/event driver systems.
    ///
    /// Call once per type, after [`install`].
    fn init_asset<A: Asset>(&mut self) -> &mut Self;

    /// [`AssetWorldExt::init_asset`] with a preallocated store.
    fn init_asset_with_capacity<A: Asset>(&mut self, capacity: usize) -> &mut Self;

    /// Registers `loader` so loads of the formats it names can resolve it.
    fn register_asset_loader<L: AssetLoader>(&mut self, loader: L) -> &mut Self;

    /// Pre-registers a loader for `extensions`, blocking loads of those formats
    /// until a real loader named [`loader_name::<L>()`](crate::meta::loader_name)
    /// is registered.
    fn preregister_asset_loader<L: AssetLoader>(&mut self, extensions: &[&str]) -> &mut Self;

    /// Builds a loader through [`FromWorld`] and registers it.
    fn init_asset_loader<L: AssetLoader + FromWorld>(&mut self) -> &mut Self;

    /// Registers `source` under `id`, so paths of the `id://…` form resolve
    /// against it.
    ///
    /// Sources must be registered *before* [`install`], which builds the
    /// [`AssetServer`] from the [`AssetSourceBuilders`] resource and freezes
    /// them; a source registered afterwards is logged and never read.
    fn register_asset_source(
        &mut self,
        id: impl Into<AssetSourceId<'static>>,
        source: AssetSourceBuilder,
    ) -> &mut Self;

    /// Registers a [`Process`] implementation with the runtime
    /// [`AssetProcessor`], when one is installed (layout ②).
    ///
    /// A no-op for a host that runs without a processor (layout ① or ③).
    fn register_asset_processor<P: Process>(&mut self, processor: P) -> &mut Self;

    /// Makes `P` the default [`Process`] for `extension` in the runtime
    /// [`AssetProcessor`], when one is installed (layout ②).
    fn set_default_asset_processor<P: Process>(&mut self, extension: &str) -> &mut Self;
}

impl AssetWorldExt for World {
    fn init_asset<A: Asset>(&mut self) -> &mut Self {
        self.init_asset_with_capacity::<A>(0)
    }

    fn init_asset_with_capacity<A: Asset>(&mut self, capacity: usize) -> &mut Self {
        // Step 1: the server takes the store's handle provider (so both allocate
        // slots from the same allocator) and fills the two typed sender maps
        // (`LoadedWithDependencies` / load failed).
        let assets = Assets::<A>::with_capacity(capacity);
        let server = self.resource::<AssetServer>().clone();
        server.register_asset(&assets);

        // In layout ② the processor's own server is not backed by a store, so it
        // needs a provider for this type too: that keeps its id space separate
        // from the store's (bevy parity).
        if let Some(processor) = self.get_resource::<AssetProcessor>() {
            processor
                .server()
                .register_handle_provider(AssetHandleProvider::new(
                    TypeId::of::<A>(),
                    Arc::new(AssetIndexAllocator::default()),
                ));
        }

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
        self
    }

    fn register_asset_loader<L: AssetLoader>(&mut self, loader: L) -> &mut Self {
        self.resource::<AssetServer>().register_loader(loader);
        self
    }

    fn preregister_asset_loader<L: AssetLoader>(&mut self, extensions: &[&str]) -> &mut Self {
        self.resource::<AssetServer>()
            .preregister_loader::<L>(extensions);
        self
    }

    fn init_asset_loader<L: AssetLoader + FromWorld>(&mut self) -> &mut Self {
        let loader = L::from_world(self);
        self.register_asset_loader(loader)
    }

    fn register_asset_source(
        &mut self,
        id: impl Into<AssetSourceId<'static>>,
        source: AssetSourceBuilder,
    ) -> &mut Self {
        let id = id.into();
        if self.get_resource::<AssetServer>().is_some() {
            tracing::error!(
                "asset source '{id}' was registered after `install`; the server's sources are \
                 already frozen, so it will never be read"
            );
        }
        self.get_resource_or_init::<AssetSourceBuilders>()
            .insert(id, source);
        self
    }

    fn register_asset_processor<P: Process>(&mut self, processor: P) -> &mut Self {
        if let Some(asset_processor) = self.get_resource::<AssetProcessor>() {
            asset_processor.register_processor(processor);
        }
        self
    }

    fn set_default_asset_processor<P: Process>(&mut self, extension: &str) -> &mut Self {
        if let Some(asset_processor) = self.get_resource::<AssetProcessor>() {
            asset_processor.set_default_processor::<P>(extension);
        }
        self
    }
}
