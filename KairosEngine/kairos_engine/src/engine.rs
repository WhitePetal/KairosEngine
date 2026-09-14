use crate::{audio::AudioEngine, graphics, inputs::InputEngine, physics, schedule, time::Time};
use kairos_ecs::world::World;

pub struct Engine {
    pub world: World,
    pub input_engine: InputEngine,
}

impl Engine {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mut world = build_world();
        // Audio goes last: its `install` mounts the per-frame driver into the
        // `Update` stage `build_world` just created and inserts the
        // `AudioEngine` as a World resource. Opening a real audio device can
        // fail, so this is the one step that keeps `Engine::new` fallible.
        crate::audio::install(&mut world, AudioEngine::new()?, schedule::Update);
        let input_engine = InputEngine::new();

        Ok(Self {
            world,
            input_engine,
        })
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
    pub fn update(&mut self) {
        self.world.run_schedule(schedule::Main);
        self.world.clear_trackers();
    }
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
