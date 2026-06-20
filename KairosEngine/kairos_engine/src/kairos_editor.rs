use crate::{
    ecs::world::World,
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
            .handle(self.world.assets_server_mut(), ui, &mut self.log);
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
        self.world.handle_assets_server();
    }

    fn on_exit(&mut self) {
        // Clear all entities before shutdown to ensure AssetHandle::drop
        // happens while the tokio runtime is still fully active.
        // This avoids spawning tasks during World::drop when the runtime
        // may be winding down, which can cause hangs.
        self.world.clear();
        self.world.handle_assets_server();
    }
}
