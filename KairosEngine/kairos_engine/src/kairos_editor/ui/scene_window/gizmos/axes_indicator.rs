use std::{path::PathBuf, sync::Arc};

use crate::{
    asset_loader::assets::{AssetHandle, AssetsServer, MaterialAssetsSystem},
    graphics::{graphics_graph::GraphicsCommand, vertex::Vertex},
    math::{self, float3, float4, float4x4},
};

/// Minimal vertex with position + color, zero-filled for unused attributes.
fn vertex(pos: float3, color: float4) -> Vertex {
    Vertex {
        position: float4::new(pos.x(), pos.y(), pos.z(), 1.0),
        color,
        texcoord: crate::math::float2::ZERO,
        normal: float3::ZERO,
        tangent: float4::new(0.0, 0.0, 0.0, 0.0),
    }
}

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
) -> (Vec<Vertex>, Vec<u16>) {
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
        vertices.push(vertex(
            float3::new(
                origin.x() + offset.x(),
                origin.y() + offset.y(),
                origin.z() + offset.z(),
            ),
            color,
        ));
        vertices.push(vertex(
            float3::new(
                origin.x() - offset.x(),
                origin.y() - offset.y(),
                origin.z() - offset.z(),
            ),
            color,
        ));
        vertices.push(vertex(
            float3::new(
                cone_start.x() + offset.x(),
                cone_start.y() + offset.y(),
                cone_start.z() + offset.z(),
            ),
            color,
        ));
        vertices.push(vertex(
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
        vertices.push(vertex(
            float3::new(
                cone_start.x() + offset.x(),
                cone_start.y() + offset.y(),
                cone_start.z() + offset.z(),
            ),
            color,
        ));
    }
    let tip_idx = vertices.len() as u16;
    vertices.push(vertex(tip, color));
    for i in 0..cone_segments {
        let i0 = ring_base + i as u16;
        let i1 = ring_base + ((i + 1) % cone_segments) as u16;
        indices.extend_from_slice(&[i0, i1, tip_idx]);
    }

    (vertices, indices)
}

/// Build a combined mesh with all three world-space axes as colored arrows.
pub fn build_axes_arrows(length: f32, half_width: f32) -> (Vec<Vertex>, Vec<u16>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let axes: [(float3, float4); 3] = [
        (float3::RIGHT, float4::new(1.0, 0.2, 0.2, 1.0)),
        (float3::UP, float4::new(0.2, 1.0, 0.2, 1.0)),
        (float3::new(0.0, 0.0, 1.0), float4::new(0.2, 0.2, 1.0, 1.0)),
    ];

    for (dir, color) in axes {
        let (v, mut idx) = build_arrow(dir, length, half_width, color, 0.2, 8);
        let offset = vertices.len() as u16;
        for i in &mut idx {
            *i += offset;
        }
        vertices.extend(v);
        indices.extend(idx);
    }

    (vertices, indices)
}

// ---- Model & Renderer ----

pub struct AxesArrows {
    pub vertices: Arc<Vec<Vertex>>,
    pub indices: Arc<Vec<u16>>,
}

pub struct AxesIndicatorModel {
    material: Arc<AssetHandle<MaterialAssetsSystem>>,
    mesh: AxesArrows,
}

impl AxesIndicatorModel {
    pub fn new(assets_server: &mut AssetsServer) -> Self {
        let material = assets_server
            .load::<MaterialAssetsSystem>(PathBuf::from("res/materials/gizmos/axes.mat"));
        let (vertices, indices) = build_axes_arrows(3.0, 0.08);
        let mesh = AxesArrows {
            vertices: Arc::new(vertices),
            indices: Arc::new(indices),
        };
        Self { material, mesh }
    }
}

pub struct AxesIndicatorRenderer {}

impl AxesIndicatorRenderer {
    pub fn new() -> Self {
        Self {}
    }

    pub fn render(&self, model: &AxesIndicatorModel, graphics_command: &mut GraphicsCommand) {
        graphics_command.draw_simple_mesh(
            model.mesh.vertices.clone(),
            model.mesh.indices.clone(),
            model.material.clone(),
            float4x4::IDENTITY,
        );
    }
}
