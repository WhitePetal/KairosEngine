use crate::graphics::graphics_graph::graphics_node::{GizmoGridDraw, GizmoVertex};

/// Generate a large quad on the XZ plane (Y=0) for the infinite grid effect.
/// The actual grid lines are computed in the fragment shader.
pub fn create_grid_quad(half_extent: f32) -> GizmoGridDraw {
    let vertices = vec![
        GizmoVertex {
            position: [-half_extent, 0.0, -half_extent],
        },
        GizmoVertex {
            position: [half_extent, 0.0, -half_extent],
        },
        GizmoVertex {
            position: [-half_extent, 0.0, half_extent],
        },
        GizmoVertex {
            position: [half_extent, 0.0, half_extent],
        },
    ];

    // Two triangles covering the quad: 0-1-2, 2-1-3
    let indices = vec![0, 1, 2, 2, 1, 3];

    GizmoGridDraw { vertices, indices }
}
