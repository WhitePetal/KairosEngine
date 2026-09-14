use crate::{
    audio::AudioEngine,
    graphics::{self, graphics_graph::GraphicsCommand},
    inputs::InputEngine,
    kairos_game::KairosGame,
    log::Log,
    physics,
    time::Time,
};
use egui::Visuals;
use kairos_ecs::world::World;
use winit::event::KeyEvent;

pub mod asset_registry;
pub mod camera;
pub mod consts;
pub mod editor_assets;
pub mod project_path_tree;
pub mod project_watcher;
pub mod runtime;
pub mod schedule;
pub mod syntax;
pub mod ui;

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
/// physics, the render rails, and the editor camera controller, in the order
/// their preconditions require.
///
/// Audio is deliberately *not* here: it belongs to [`Engine::new`], which owns
/// the fallible device open. Keeping this function infallible and audio-free is
/// what lets a host that does not want an audio device — a test, or a headless
/// run — drive the exact bootstrap order.
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
    // The next S2 leaf types: `Text` (scripts/documents/shader sources), `Toml`,
    // and the `SyntaxHighlightSettings` their editors consume. They are mutually
    // independent, so the order here is a convenience grouping, not a
    // precondition.
    crate::kairos_editor::editor_assets::text::install(&mut world);
    crate::kairos_editor::editor_assets::toml::install(&mut world);
    crate::kairos_editor::syntax::install(&mut world);
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
    // The editor's texture inspector loads its `TextureEdit` composite through
    // the asset server (`add_async`), so its store is registered here. Audio
    // comes next: `PcmData` and `AudioAsset` are the leaves, and the editor's
    // `AudioExt` composite declares both through its loader, so it is registered
    // after the two stores it targets.
    crate::kairos_editor::ui::inspector::texture::install(&mut world);
    crate::audio::audio_ext::pcm::install(&mut world);
    crate::audio::audio::install(&mut world);
    crate::audio::audio_ext::install(&mut world);
    // Finally the editor camera controller, whose system the extract stage must
    // find already written to when it reads the frame. It consumes
    // `SceneViewInput`, an editor concept, so it is installed here rather than by
    // the engine-level `graphics::install`.
    camera::install(&mut world);
    world
}

pub struct KairosEngine {
    engine: Engine,
    ui_context: ui::Context,
    log: Log,
}

impl KairosEngine {
    pub fn new(egui_ctx: &egui::Context) -> Result<Self, Box<dyn std::error::Error>> {
        let mut engine = Engine::new()?;
        // The game is a stateless assembly entry: its `new` spawns the demo
        // scene and registers the drifting-listener system into the engine's
        // rails. The value it returns is not kept — per-frame work rides the
        // schedule now.
        let _ = KairosGame::new(&mut engine);
        let ui_context = ui::Context::new(egui_ctx)?;
        let log = Log::new();
        Ok(Self {
            engine,
            ui_context,
            log,
        })
    }

    fn update_keyboard_input(&mut self, event: KeyEvent) {
        self.engine.input_engine.update_keyboard_input(event)
    }

    fn update(&mut self) {
        // Frame start: the game's per-frame work (the drifting listener, the
        // audio driver) is scheduled into the rails, so driving the rails *is*
        // the frame — there is no second hand-driven step.
        self.engine.update();
    }

    fn handle_ui(&mut self, ui: &mut egui::Ui) {
        self.ui_context.handle(&mut self.engine, ui);
    }

    fn draw_ui(&mut self, ui: &mut egui::Ui) {
        let mut visuals = Visuals::dark();
        visuals.button_frame = true;
        ui.set_visuals(visuals);
        self.ui_context.darw(ui, &self.engine, &mut self.log);
    }

    fn render_ui(&mut self) -> Vec<GraphicsCommand> {
        self.ui_context.render(&mut self.engine)
    }

    fn on_exit(&mut self) {
        // TODO!
        // self.engine.world.clear();
    }
}
