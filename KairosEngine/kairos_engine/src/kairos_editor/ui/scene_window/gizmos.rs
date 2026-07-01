use crate::{asset_loader::assets::AssetsServer, graphics::graphics_graph::GraphicsCommand, kairos_editor::ui::scene_window::gizmos::grid_plane::{GridPlaneModel, GridPlaneRenderer}};

mod grid_plane;

pub struct GizmosModel {
    grid_plane: GridPlaneModel,
}
impl GizmosModel {
    pub fn new(assets_server: &mut AssetsServer) -> Self {
        let grid_plane = GridPlaneModel::new(assets_server);

        Self { 
            grid_plane,
        }
    }
}

pub struct GizmosRenderer {
    grid_plane_renderer: GridPlaneRenderer
}

impl GizmosRenderer {
    pub fn new() -> Self {
        let grid_plane_renderer = GridPlaneRenderer::new();

        Self {  
            grid_plane_renderer,
        }
    }
    pub fn render_gizmos(&self, model: &GizmosModel, graphics_command: &mut GraphicsCommand) {
        self.grid_plane_renderer.render(&model.grid_plane, graphics_command);
    }
}