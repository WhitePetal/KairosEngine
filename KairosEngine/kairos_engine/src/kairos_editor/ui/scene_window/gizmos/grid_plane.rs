use std::path::PathBuf;

use kairos_asset::next::{AssetServer, Handle};
use kairos_ecs::world::World;

use crate::{
    graphics::{graphics_graph::GraphicsCommand, material::Material, mesh::Mesh, vertex::Vertex},
    math::{float3, float4x4},
};

pub struct GridPlaneModel {
    material: Handle<Material>,
    mesh: Handle<Mesh>,
}

/// Generate line geometry for a grid on the XZ plane (Y=0).
/// Each integer coordinate from `-half_extent` to `+half_extent` gets an
/// X-parallel and Z-parallel line, producing a hollow grid.
fn build_grid_lines(half_extent: i32) -> Mesh {
    let extent = half_extent as f32;
    let line_count = (2 * half_extent + 1) as usize;
    let total_vertices = line_count * 4; // 2 lines/cell × 2 verts/line

    let mut vertices = Vec::with_capacity(total_vertices);
    let mut indices = Vec::with_capacity(total_vertices);

    // X-parallel lines
    for z in -half_extent..=half_extent {
        let z = z as f32;
        let i0 = vertices.len() as u16;
        vertices.push(Vertex::with_position(float3::new(-extent, 0.0, z)));
        let i1 = vertices.len() as u16;
        vertices.push(Vertex::with_position(float3::new(extent, 0.0, z)));
        indices.push(i0);
        indices.push(i1);
    }
    // Z-parallel lines
    for x in -half_extent..=half_extent {
        let x = x as f32;
        let i0 = vertices.len() as u16;
        vertices.push(Vertex::with_position(float3::new(x, 0.0, -extent)));
        let i1 = vertices.len() as u16;
        vertices.push(Vertex::with_position(float3::new(x, 0.0, extent)));
        indices.push(i0);
        indices.push(i1);
    }

    Mesh { vertices, indices }
}

impl GridPlaneModel {
    pub fn new(world: &World) -> Self {
        let material = world
            .resource::<AssetServer>()
            .load::<Material>(PathBuf::from("res/materials/gizmos/grid_plane.mat"));
        let mesh = world
            .resource::<AssetServer>()
            .add(build_grid_lines(100));

        Self { material, mesh }
    }
}

pub struct GridPlaneRenderer {}
impl GridPlaneRenderer {
    pub fn new() -> Self {
        Self {}
    }

    pub fn render(&self, model: &GridPlaneModel, graphics_command: &mut GraphicsCommand) {
        graphics_command.draw(
            model.mesh.clone(),
            model.material.clone(),
            float4x4::IDENTITY,
        );
    }
}
