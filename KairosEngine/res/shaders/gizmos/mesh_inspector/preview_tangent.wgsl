// Preview shader — Tangent mode: visualize tangents as RGB colors.
// Maps tangent.x → R, tangent.y → G, tangent.z → B.

struct VertexInput {
    @location(0) position: vec4f,
    @location(1) color: vec4f,
    @location(2) texcoord: vec2f,
    @location(3) normal: vec3f,
    @location(4) tangent: vec4f,
}

struct VertexOutput {
    @builtin(position) pos: vec4f,
    @location(0) color: vec4f,
}

@group(0) @binding(0)
var<uniform> matrix_vp: mat4x4f;

@vertex
fn vs_main(v: VertexInput) -> VertexOutput {
    var o: VertexOutput;
    o.pos = matrix_vp * vec4f(v.position.xyz, 1.0);
    // Map tangent.xyz from [-1, 1] to [0, 1] range.
    let t = normalize(v.tangent.xyz);
    o.color = vec4f(t * 0.5 + 0.5, 1.0);
    return o;
}

@fragment
fn fs_main(i: VertexOutput) -> @location(0) vec4f {
    return i.color;
}
