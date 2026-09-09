use crate::{
    asset_loader::assets::AssetsServer, audio::AudioEngine,
    graphics::graphics_graph::GraphicsCommand, inputs::InputEngine, kairos_game::KairosGame,
    log::Log, physics::PhysicsEngine, timer::Time,
};
use egui::Visuals;
use kairos_ecs::world::World;
use winit::event::KeyEvent;

pub mod asset_registry;
pub mod consts;
pub mod editor_assets;
pub mod project_path_tree;
pub mod runtime;
pub mod schedule;
pub mod serialize_asset;
pub mod syntax;
pub mod ui;

pub struct Engine {
    pub time: Time,
    pub world: World,
    pub assets_server: AssetsServer,
    pub audio_engine: AudioEngine,
    pub physics_engine: PhysicsEngine,
    pub input_engine: InputEngine,
}

impl Engine {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let time = Time::new();
        let mut world = World::new();
        // Bootstrap the (currently empty) bevy_app-style schedule rails before
        // any game/editor logic gets a chance to register systems.
        schedule::install(&mut world);
        let assets_server = AssetsServer::new();
        let audio_engine = AudioEngine::new()?;
        let physics_engine = PhysicsEngine::new();
        let input_engine = InputEngine::new();

        Ok(Self {
            time,
            world,
            assets_server,
            audio_engine,
            physics_engine,
            input_engine,
        })
    }

    /// Advances the engine's ECS schedule rails by one frame.
    ///
    /// Runs the top-level [`Main`](schedule::Main) schedule — whose `run_main`
    /// driver executes the sub-schedules listed in
    /// [`MainScheduleOrder`](schedule::MainScheduleOrder) — and then clears the
    /// world's change-detection trackers to close the frame.
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
