use crate::{
    asset_loader::assets::AssetsServer, audio::AudioEngine, ecs::world::World,
    graphics::graphics_graph::GraphicsCommand, inputs::InputEngine, kairos_game::KairosGame,
    log::Log, physics::PhysicsEngine, timer::Time,
};
use egui::Visuals;
use winit::event::KeyEvent;

pub mod asset_registry;
pub mod consts;
pub mod editor_assets;
pub mod project_path_tree;
pub mod runtime;
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
        let world = World::new();
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
}

pub struct KairosEngine {
    engine: Engine,
    game: KairosGame,
    ui_context: ui::Context,
    log: Log,
    #[cfg(feature = "test-harness")]
    widget_rects: std::collections::HashMap<String, egui::Rect>,
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
            #[cfg(feature = "test-harness")]
            widget_rects: std::collections::HashMap::new(),
        })
    }

    fn update_keyboard_input(&mut self, event: KeyEvent) {
        self.engine.input_engine.update_keyboard_input(event)
    }

    fn update(&mut self) {
        self.game.update(&mut self.engine);
    }

    fn handle_ui(&mut self, ui: &mut egui::Ui) {
        self.ui_context.handle(&mut self.engine, ui, &mut self.log);
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
        // Clear all entities before shutdown to ensure AssetHandle::drop
        // happens while the tokio runtime is still fully active.
        // This avoids spawning tasks during World::drop when the runtime
        // may be winding down, which can cause hangs.
        self.engine.world.clear();
        self.engine.assets_server.handle();
    }

    /// Access the underlying `Engine` (crate-visible for test harness).
    #[cfg(feature = "test-harness")]
    pub(crate) fn engine_mut(&mut self) -> &mut Engine {
        &mut self.engine
    }

    /// Access the log buffer (crate-visible for test harness).
    #[cfg(feature = "test-harness")]
    pub(crate) fn log_mut(&mut self) -> &mut Log {
        &mut self.log
    }

    /// Record a widget's screen-space rectangle for the current frame.
    /// Called from the egui draw callback.
    #[cfg(feature = "test-harness")]
    pub(crate) fn record_widget_rect(&mut self, id: impl Into<String>, rect: egui::Rect) {
        self.widget_rects.insert(id.into(), rect);
    }

    /// Clear widget rects at the start of a new frame.
    #[cfg(feature = "test-harness")]
    pub(crate) fn clear_widget_rects(&mut self) {
        self.widget_rects.clear();
    }

    /// Query a widget's screen-space rectangle by ID.
    #[cfg(feature = "test-harness")]
    pub(crate) fn widget_rect(&self, id: &str) -> Option<egui::Rect> {
        self.widget_rects.get(id).copied()
    }

    /// Access the UI context (crate-visible for test harness dispatching).
    #[cfg(feature = "test-harness")]
    pub(crate) fn ui_context_mut(&mut self) -> &mut ui::Context {
        &mut self.ui_context
    }
}
