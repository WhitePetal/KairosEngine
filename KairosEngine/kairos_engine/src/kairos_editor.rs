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
    /// Widget name → egui Id mapping for focus operations.
    #[cfg(feature = "test-harness")]
    widget_egui_ids: std::collections::HashMap<String, egui::Id>,
    /// Egui events to inject into RawInput on the next frame.
    #[cfg(feature = "test-harness")]
    pending_egui_events: Vec<egui::Event>,
    /// Pending focus requests for the next draw_ui.
    #[cfg(feature = "test-harness")]
    pending_focus_requests: Vec<egui::Id>,
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
            #[cfg(feature = "test-harness")]
            widget_egui_ids: std::collections::HashMap::new(),
            #[cfg(feature = "test-harness")]
            pending_egui_events: Vec::new(),
            #[cfg(feature = "test-harness")]
            pending_focus_requests: Vec::new(),
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
        self.engine.world.clear();
        self.engine.assets_server.handle();
    }

    #[cfg(feature = "test-harness")]
    pub(crate) fn engine_mut(&mut self) -> &mut Engine { &mut self.engine }

    #[cfg(feature = "test-harness")]
    pub(crate) fn log_mut(&mut self) -> &mut Log { &mut self.log }

    #[cfg(feature = "test-harness")]
    pub(crate) fn record_widget_rect(&mut self, id: impl Into<String>, rect: egui::Rect) {
        self.widget_rects.insert(id.into(), rect);
    }

    #[cfg(feature = "test-harness")]
    pub(crate) fn clear_widget_rects(&mut self) {
        self.widget_rects.clear();
        self.widget_egui_ids.clear();
    }

    #[cfg(feature = "test-harness")]
    pub(crate) fn widget_rect(&self, id: &str) -> Option<egui::Rect> {
        self.widget_rects.get(id).copied()
    }

    #[cfg(feature = "test-harness")]
    pub(crate) fn record_widget_egui_id(&mut self, id: impl Into<String>, egui_id: egui::Id) {
        self.widget_egui_ids.insert(id.into(), egui_id);
    }

    #[cfg(feature = "test-harness")]
    pub(crate) fn widget_egui_id(&self, id: &str) -> Option<egui::Id> {
        self.widget_egui_ids.get(id).copied()
    }

    #[cfg(feature = "test-harness")]
    pub(crate) fn request_focus(&mut self, egui_id: egui::Id) {
        self.pending_focus_requests.push(egui_id);
    }

    #[cfg(feature = "test-harness")]
    pub(crate) fn drain_focus_requests(&mut self) -> Vec<egui::Id> {
        std::mem::take(&mut self.pending_focus_requests)
    }

    #[cfg(feature = "test-harness")]
    pub(crate) fn ui_context_mut(&mut self) -> &mut ui::Context { &mut self.ui_context }

    #[cfg(feature = "test-harness")]
    pub(crate) fn push_ui_message(&mut self, msg: crate::kairos_editor::ui::Message) {
        self.ui_context.messager.send(msg);
    }

    #[cfg(feature = "test-harness")]
    pub(crate) fn push_egui_event(&mut self, event: egui::Event) {
        self.pending_egui_events.push(event);
    }

    #[cfg(feature = "test-harness")]
    pub(crate) fn drain_egui_events(&mut self) -> Vec<egui::Event> {
        std::mem::take(&mut self.pending_egui_events)
    }
}
