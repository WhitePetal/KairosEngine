// Grid line gizmo shader — draws grid lines on the XZ plane.
// Vertices are pre-generated line segments; fragment outputs solid color.

struct VertexInput {
    @location(0) position: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> matrix_vp: mat4x4<f32>;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = matrix_vp * in.position;
    return out;
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(0.3, 0.3, 0.3, 0.7);
}