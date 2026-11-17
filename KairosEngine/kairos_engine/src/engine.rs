use crate::{audio::AudioEngine, graphics, inputs::InputEngine, physics, schedule, time::Time};
use kairos_ecs::world::World;
use kairos_profile::{Category, Profiler};
use kairos_tasks::{AsyncComputeTaskPool, ComputeTaskPool, IoTaskPool, TaskPool};

pub struct Engine {
    pub world: World,
    pub input_engine: InputEngine,
    /// The process's profiling recorder.
    ///
    /// The engine owns it because a development build of a game must be able to
    /// profile itself, with no editor in the loop. It is a plain owned value
    /// (see `kairos_profile`'s ownership notes), so a host that also draws an
    /// editor UI borrows it through [`Engine::profiler`] instead of installing
    /// a second recorder.
    profiler: Profiler,
}

impl Engine {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mut world = build_world();
        // Audio goes last: its `install` mounts the per-frame driver into the
        // `Update` stage `build_world` just created and inserts the
        // `AudioEngine` as a World resource. Opening a real audio device can
        // fail, so this is the one step that keeps `Engine::new` fallible.
        crate::audio::install(&mut world, AudioEngine::new()?, schedule::Update);
        Ok(Self::from_world(world))
    }

    /// Boot the engine without opening an audio device.
    ///
    /// The same world bootstrap as [`Engine::new`] minus the audio driver: for
    /// hosts and tests that do not play sound (a UI shell, a headless run, a
    /// profiler fixture). `Engine::new` falls back to this shape when the host
    /// itself decides a missing audio device is not fatal.
    pub fn headless() -> Self {
        Self::from_world(build_world())
    }

    fn from_world(world: World) -> Self {
        let profiler = Profiler::new();
        // The timeline shows every task-pool worker, not only the ones that
        // happened to run during the capture. Only the engine knows the pools,
        // so it supplies the lanes when a capture starts.
        profiler.set_thread_lane_provider(engine_thread_lanes);
        Self {
            world,
            input_engine: InputEngine::new(),
            profiler,
        }
    }

    /// The engine's profiling recorder.
    ///
    /// Recording is off by default (`set_recording(true)` starts it). A host
    /// that draws around the engine tick — the editor's panels, for instance —
    /// reads the capture through this same handle; do not create a second
    /// [`Profiler`], because puffin's frame collector is process-wide.
    pub fn profiler(&self) -> &Profiler {
        &self.profiler
    }

    /// Read access to the engine's per-frame virtual clock.
    ///
    /// The clock is a [`Time`] World resource, registered during
    /// `schedule::install` and advanced exactly once per frame by
    /// `time_system` at the `First` stage — engine-side (non-system) code only
    /// ever reads it, through this accessor (or directly via the World
    /// resource).
    pub fn time(&self) -> &Time {
        self.world.resource::<Time>()
    }

    /// Advances the engine's ECS schedule rails by one frame.
    ///
    /// Runs the top-level [`Main`](schedule::Main) schedule — whose `run_main`
    /// driver executes the sub-schedules listed in
    /// [`MainScheduleOrder`](schedule::MainScheduleOrder) (the `First` stage's
    /// `time_system` advances the [`Time`] resource first) — and then clears
    /// the world's change-detection trackers to close the frame.
    ///
    /// # Frame boundary
    ///
    /// This is where a **game frame** starts: [`Profiler::begin_frame`] flushes
    /// the previous frame (and honours a start request), the tick runs under
    /// `Kairos.Engine.Frame`, and the frame is left open so whatever the host
    /// draws between two updates — editor panels, the render present — lands in
    /// the same recorded frame. The engine is therefore profiled by its own
    /// loop, not by a UI repaint schedule: a game build that keeps ticking keeps
    /// producing frames with no editor attached.
    ///
    /// The tick marks itself committed. A host that draws inside the frame and
    /// discards that work can call [`Profiler::end_frame`] with `false` before
    /// the next update to drop the whole frame.
    pub fn update(&mut self) {
        self.profiler.begin_frame();
        {
            kairos_profile::scope!(Category::Others, "Kairos.Engine.Frame");
            self.world.run_schedule(schedule::Main);
            self.world.clear_trackers();
        }
        self.profiler.end_frame(true);
    }
}

/// The thread lanes a capture starts with: the calling (frame) thread and every
/// worker of every task pool that exists.
///
/// Called by the profiler when recording starts, on the frame thread. Pools are
/// process-global and created lazily, so this asks for the ones that exist; a
/// pool created later still appears once one of its threads runs, but it is
/// not reserved a row in advance.
fn engine_thread_lanes() -> Vec<String> {
    let main = std::thread::current().name().unwrap_or("main").to_owned();
    let mut lanes = vec![main];

    fn push_pool(lanes: &mut Vec<String>, pool: Option<&'static TaskPool>) {
        if let Some(pool) = pool {
            lanes.extend(pool.thread_names().map(str::to_owned));
        }
    }
    push_pool(&mut lanes, ComputeTaskPool::try_get().map(|pool| &**pool));
    push_pool(
        &mut lanes,
        AsyncComputeTaskPool::try_get().map(|pool| &**pool),
    );
    push_pool(&mut lanes, IoTaskPool::try_get().map(|pool| &**pool));
    lanes
}

/// Boots the World half of an [`Engine`]: the schedule rails, the asset core,
/// physics, the render rails, and the audio asset types, in the order their
/// preconditions require.
///
/// The audio engine is deliberately *not* here: it belongs to [`Engine::new`],
/// which owns the fallible device open. Keeping this function infallible and
/// audio-device-free is what lets a host that does not want an audio device — a
/// test, or a headless run — drive the exact bootstrap order.
///
/// The editor's own assets and camera are not here either: they belong to the
/// host, which adds them through `kairos_editor::install` once [`Engine::new`]
/// returns. This function bootstraps engine subsystems only.
fn build_world() -> World {
    let mut world = World::new();
    // Bootstrap the bevy_app-style schedule rails — including the `Time` World
    // resource and its `First`-stage `time_system` — before any game/editor
    // logic gets a chance to register systems.
    schedule::install(&mut world);
    // The asset core: its `AssetServer` becomes a World resource, its per-type
    // driver systems mount into the engine's `PreUpdate`/`PostUpdate`, and the
    // `AssetEvent`s it writes ride the frame's message pass. All asset insertion
    // happens in `PreUpdate`, so no editor step needs to pump a `handle()`.
    // `AssetOptions` also carries the `Startup` stage the (deferred) processor
    // track will launch from.
    crate::asset::install(
        &mut world,
        crate::asset::AssetOptions::new(
            schedule::PreUpdate,
            schedule::PostUpdate,
            schedule::Startup,
        ),
    );
    // The first migrated asset type: `Font` registers its store, loader, and
    // driver systems with the core just installed. P2 slices add the rest here.
    crate::kairos_ui::font::install(&mut world);
    // Physics comes next: its step system registers into the `FixedUpdate` stage
    // the schedule rails just created, so this order is a precondition. It is a
    // World resource, not an `Engine` field, so from here on every `Engine`
    // carries physics in its world.
    physics::install(&mut world, schedule::FixedUpdate);
    // Then the render rails, which register into the `Extract` stage the schedule
    // rails just created. The order is a precondition, not a preference: game
    // assembly runs after `Engine::new` and binds the game camera to `GameView`,
    // so the view resources must already exist.
    graphics::install(&mut world, schedule::Extract);
    // The graphics asset types ride the same core: their stores, loaders, and
    // the extract-stage system that collects their change events for render
    // cache invalidation.
    graphics::install_assets(&mut world, schedule::Extract);
    // The audio asset types: `PcmData` and `AudioAsset` are the leaves, and the
    // `AudioExt` composite declares both through its loader, so it is registered
    // after the two stores it targets.
    crate::audio::audio_ext::pcm::install(&mut world);
    crate::audio::audio::install(&mut world);
    crate::audio::audio_ext::install(&mut world);
    world
}

#[cfg(test)]
mod test;
