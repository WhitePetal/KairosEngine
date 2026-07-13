use std::{path::PathBuf, sync::Arc};

use crate::{
    asset_loader::assets::{AssetHandle, AssetsServer, MaterialAssetsSystem, MeshAssetsSystem},
    graphics::{graphics_graph::GraphicsCommand, mesh::Mesh, vertex::Vertex},
    math::{self, float3, float4, float4x4},
};

/// Build a 3D arrow mesh (shaft + cone arrowhead) pointing along `direction`.
/// `length` is total arrow length; the cone occupies the last `cone_ratio`.
/// `half_width` is the shaft half-thickness; `color` is the RGBA vertex color.
fn build_arrow(
    direction: float3,
    length: f32,
    half_width: f32,
    color: float4,
    cone_ratio: f32,
    cone_segments: u32,
) -> Mesh {
    let d = math::normalize(direction);
    let tip = d * length;
    let cone_start = d * (length * (1.0 - cone_ratio));
    let cone_radius = half_width * 2.5;

    // Perpendicular basis for the cone base circle
    let right = if math::dot(&d, &float3::UP).abs() > 0.999 {
        math::normalize(math::cross(d, float3::RIGHT))
    } else {
        math::normalize(math::cross(d, float3::UP))
    };
    let up = math::cross(d, right);

    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // --- Shaft: two perpendicular quads forming a '+' cross-section ---
    let origin = float3::ZERO;
    let offsets = [right * half_width, up * half_width];
    for &offset in &offsets {
        let base = vertices.len() as u16;
        vertices.push(Vertex::with_position_color(
            float3::new(
                origin.x() + offset.x(),
                origin.y() + offset.y(),
                origin.z() + offset.z(),
            ),
            color,
        ));
        vertices.push(Vertex::with_position_color(
            float3::new(
                origin.x() - offset.x(),
                origin.y() - offset.y(),
                origin.z() - offset.z(),
            ),
            color,
        ));
        vertices.push(Vertex::with_position_color(
            float3::new(
                cone_start.x() + offset.x(),
                cone_start.y() + offset.y(),
                cone_start.z() + offset.z(),
            ),
            color,
        ));
        vertices.push(Vertex::with_position_color(
            float3::new(
                cone_start.x() - offset.x(),
                cone_start.y() - offset.y(),
                cone_start.z() - offset.z(),
            ),
            color,
        ));
        indices.extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 1, base + 3]);
    }

    // --- Cone: ring of vertices at cone_start, one vertex at tip ---
    let ring_base = vertices.len() as u16;
    for i in 0..cone_segments {
        let angle = (i as f32) * (2.0 * math::PI) / (cone_segments as f32);
        let offset = right * (angle.cos() * cone_radius) + up * (angle.sin() * cone_radius);
        vertices.push(Vertex::with_position_color(
            float3::new(
                cone_start.x() + offset.x(),
                cone_start.y() + offset.y(),
                cone_start.z() + offset.z(),
            ),
            color,
        ));
    }
    let tip_idx = vertices.len() as u16;
    vertices.push(Vertex::with_position_color(tip, color));
    for i in 0..cone_segments {
        let i0 = ring_base + i as u16;
        let i1 = ring_base + ((i + 1) % cone_segments) as u16;
        indices.extend_from_slice(&[i0, i1, tip_idx]);
    }

    Mesh { vertices, indices }
}

/// Build a combined mesh with all three world-space axes as colored arrows.
pub fn build_axes_arrows(length: f32, half_width: f32) -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let axes: [(float3, float4); 3] = [
        (float3::RIGHT, float4::new(1.0, 0.2, 0.2, 1.0)),
        (float3::UP, float4::new(0.2, 1.0, 0.2, 1.0)),
        (float3::new(0.0, 0.0, 1.0), float4::new(0.2, 0.2, 1.0, 1.0)),
    ];

    for (dir, color) in axes {
        let mut mesh = build_arrow(dir, length, half_width, color, 0.2, 8);
        let offset = vertices.len() as u16;
        for i in &mut mesh.indices {
            *i += offset;
        }
        vertices.extend(mesh.vertices);
        indices.extend(mesh.indices);
    }

    Mesh { vertices, indices }
}

pub struct AxesIndicatorModel {
    material: Arc<AssetHandle<MaterialAssetsSystem>>,
    mesh: Arc<AssetHandle<MeshAssetsSystem>>,
}

impl AxesIndicatorModel {
    pub fn new(assets_server: &mut AssetsServer) -> Self {
        let material = assets_server
            .load::<MaterialAssetsSystem>(&PathBuf::from("res/materials/gizmos/axes.mat"));
        let mesh = build_axes_arrows(3.0, 0.08);
        let mesh = assets_server.insert(
            mesh,
            PathBuf::from("runtime/scene_windo/gizmos/axes_arrows"),
        );

        Self { material, mesh }
    }
}

pub struct AxesIndicatorRenderer {}

impl AxesIndicatorRenderer {
    pub fn new() -> Self {
        Self {}
    }

    pub fn render(&self, model: &AxesIndicatorModel, graphics_command: &mut GraphicsCommand) {
        graphics_command.draw(
            model.mesh.clone(),
            model.material.clone(),
            float4x4::IDENTITY,
        );
    }
}
