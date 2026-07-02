use crate::{
    asset_loader::assets::AssetsServer,
    graphics::graphics_graph::GraphicsCommand,
    kairos_editor::ui::scene_window::gizmos::{
        axes_indicator::{AxesIndicatorModel, AxesIndicatorRenderer},
        grid_plane::{GridPlaneModel, GridPlaneRenderer},
    },
};

mod axes_indicator;
mod grid_plane;

pub struct GizmosModel {
    grid_plane: GridPlaneModel,
    axes_indicator: AxesIndicatorModel,
}
impl GizmosModel {
    pub fn new(assets_server: &mut AssetsServer) -> Self {
        let grid_plane = GridPlaneModel::new(assets_server);
        let axes_indicator = AxesIndicatorModel::new(assets_server);
        Self {
            grid_plane,
            axes_indicator,
        }
    }
}

pub struct GizmosRenderer {
    grid_plane_renderer: GridPlaneRenderer,
    axes_indicator_renderer: AxesIndicatorRenderer,
}

impl GizmosRenderer {
    pub fn new() -> Self {
        let grid_plane_renderer = GridPlaneRenderer::new();
        let axes_indicator_renderer = AxesIndicatorRenderer::new();
        Self {
            grid_plane_renderer,
            axes_indicator_renderer,
        }
    }
    pub fn render_gizmos(&self, model: &GizmosModel, graphics_command: &mut GraphicsCommand) {
        self.grid_plane_renderer
            .render(&model.grid_plane, graphics_command);
        self.axes_indicator_renderer
            .render(&model.axes_indicator, graphics_command);
    }
}
