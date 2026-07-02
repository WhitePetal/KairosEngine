use std::{path::PathBuf, sync::Arc};

use crate::{
    asset_loader::assets::{AssetHandle, AssetsServer, MaterialAssetsSystem},
    graphics::{graphics_graph::GraphicsCommand, vertex::Vertex},
    math::{float3, float4x4},
};

/// Grid gizmo draw data — vertices + indices for a quad on the XZ plane.
pub struct GridPlaneQuad {
    pub vertices: Arc<Vec<Vertex>>,
    pub indices: Arc<Vec<u16>>,
}

pub struct GridPlaneModel {
    material: Arc<AssetHandle<MaterialAssetsSystem>>,
    mesh: GridPlaneQuad,
}
impl GridPlaneModel {
    pub fn new(assets_server: &mut AssetsServer) -> Self {
        let material = assets_server
            .load::<MaterialAssetsSystem>(PathBuf::from("res/materials/gizmos/grid_plane.mat"));
        let half_extent = 100.0;
        let vertices = vec![
            Vertex::with_position(float3::new(-half_extent, 0.0, half_extent)),
            Vertex::with_position(float3::new(half_extent, 0.0, half_extent)),
            Vertex::with_position(float3::new(-half_extent, 0.0, -half_extent)),
            Vertex::with_position(float3::new(half_extent, 0.0, -half_extent)),
        ];
        let indices = vec![0, 1, 2, 2, 1, 3];

        let vertices = Arc::new(vertices);
        let indices = Arc::new(indices);

        let mesh = GridPlaneQuad { vertices, indices };
        Self { material, mesh }
    }
}

pub struct GridPlaneRenderer {}
impl GridPlaneRenderer {
    pub fn new() -> Self {
        Self {}
    }

    pub fn render(&self, model: &GridPlaneModel, graphics_command: &mut GraphicsCommand) {
        graphics_command.draw_simple_mesh(
            model.mesh.vertices.clone(),
            model.mesh.indices.clone(),
            model.material.clone(),
            float4x4::IDENTITY,
        );
    }
}
