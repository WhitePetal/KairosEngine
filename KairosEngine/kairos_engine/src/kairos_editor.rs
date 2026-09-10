use crate::{
    asset_loader::assets::AssetsServer,
    audio::AudioEngine,
    graphics::{self, graphics_graph::GraphicsCommand},
    inputs::InputEngine,
    kairos_game::KairosGame,
    log::Log,
    physics::PhysicsEngine,
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
pub mod runtime;
pub mod schedule;
pub mod serialize_asset;
pub mod syntax;
pub mod ui;

pub struct Engine {
    pub world: World,
    pub assets_server: AssetsServer,
    pub audio_engine: AudioEngine,
    pub physics_engine: PhysicsEngine,
    pub input_engine: InputEngine,
}

impl Engine {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mut world = World::new();
        // Bootstrap the bevy_app-style schedule rails — including the `Time`
        // World resource and its `First`-stage `time_system` — before any
        // game/editor logic gets a chance to register systems.
        schedule::install(&mut world);
        // Then the render rails, which register into the `Extract` stage the
        // skeleton above just created. The order is a precondition, not a
        // preference: game assembly runs after `Engine::new` and binds the game
        // camera to `GameView`, so the view resources must already exist.
        graphics::install(&mut world);
        // Finally the editor camera controller, whose system the extract stage
        // must find already written to when it reads the frame. It consumes
        // `SceneViewInput`, an editor concept, so it is installed here rather
        // than by the engine-level `graphics::install`.
        camera::install(&mut world);
        let assets_server = AssetsServer::new();
        let audio_engine = AudioEngine::new()?;
        let physics_engine = PhysicsEngine::new();
        let input_engine = InputEngine::new();

        Ok(Self {
            world,
            assets_server,
            audio_engine,
            physics_engine,
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

pub struct KairosEngine {
    engine: Engine,
    game: KairosGame,
    ui_context: ui::Context,
    log: Log,
}

impl KairosEngine {
    pub fn new(egui_ctx: &egui::Context) -> Result<Self, Box<dyn std::error::Error>> {
        let mut engine = Engine::new()?;
        let game = KairosGame::new(&mut engine);
        let ui_context = ui::Context::new(egui_ctx)?;
        let log = Log::new();
        Ok(Self {
            engine,
            game,
            ui_context,
            log,
        })
    }

    fn update_keyboard_input(&mut self, event: KeyEvent) {
        self.engine.input_engine.update_keyboard_input(event)
    }

    fn update(&mut self) {
        // Frame start: drive the engine's schedule rails first.
        self.engine.update();
        self.game.update(&mut self.engine);
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
        self.ui_context.render(&mut self.engine, &mut self.game)
    }

    fn handle_asset_server(&mut self) {
        self.engine.assets_server.handle();
    }

    fn on_exit(&mut self) {
        // TODO!
        // self.engine.world.clear();
        self.engine.assets_server.handle();
    }
}
