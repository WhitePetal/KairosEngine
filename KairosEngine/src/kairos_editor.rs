use crate::{
    graphics::{graphics_graph::GraphicsCommand, render_pipeline::RenderPipeline},
    log::Log,
};
use egui::Visuals;

pub mod consts;
pub mod project_path_tree;
pub mod runtime;
pub mod serialize_asset;
pub mod ui;

pub struct KairosEngine {
    ui_context: ui::Context,
    log: Log,
}

impl KairosEngine {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let ui_context = ui::Context::new();
        let log = Log::new();

        Ok(Self { ui_context, log })
    }

    fn update(&mut self) {}

    fn handle_ui(&mut self, ui: &mut egui::Ui) {
        self.ui_context.handle(ui, &mut self.log);
    }

    fn draw_ui(&mut self, ui: &mut egui::Ui) {
        let mut visuals = Visuals::dark();
        visuals.button_frame = true;
        ui.set_visuals(visuals);

        self.ui_context.darw(ui, &mut self.log);
    }

    fn render_ui(&mut self) -> Vec<GraphicsCommand> {
        self.ui_context.render()
    }

    fn on_exit(&mut self) {}
}
