use crate::{
    asset_loader::assets::AssetsServer, ecs::world::World,
    graphics::graphics_graph::GraphicsCommand, kairos_game::KairosGame, log::Log,
};
use egui::Visuals;

pub mod consts;
pub mod project_path_tree;
pub mod runtime;
pub mod serialize_asset;
pub mod ui;

pub struct KairosEngine {
    world: World,
    game: KairosGame,
    ui_context: ui::Context,
    log: Log,
}

impl KairosEngine {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mut world = World::new();
        let game = KairosGame::new(&mut world);
        let ui_context = ui::Context::new();
        let log = Log::new();

        Ok(Self {
            world,
            game,
            ui_context,
            log,
        })
    }

    fn update(&mut self) {
        self.game.update(&mut self.world);
    }

    fn handle_ui(&mut self, ui: &mut egui::Ui) {
        self.ui_context
            .handle(&mut self.world.assets_server, ui, &mut self.log);
    }

    fn draw_ui(&mut self, ui: &mut egui::Ui) {
        let mut visuals = Visuals::dark();
        visuals.button_frame = true;
        ui.set_visuals(visuals);

        self.ui_context.darw(ui, &mut self.log);
    }

    fn render_ui(&mut self) -> Vec<GraphicsCommand> {
        self.ui_context.render(&mut self.world, &mut self.game)
    }

    fn handle_asset_server(&mut self) {
        self.world.assets_server.handle();
    }

    fn on_exit(&mut self) {}
}
