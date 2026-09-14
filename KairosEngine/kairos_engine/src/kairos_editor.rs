use crate::{Engine, graphics::graphics_graph::GraphicsCommand, kairos_game::KairosGame, log::Log};
use egui::Visuals;
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
