// Axes indicator shader — per-vertex colored arrows.
// No lighting, no texture — just VP transform + vertex color passthrough.

struct a2v {
    @location(0) vertex: vec4f,
    @location(1) color: vec4f,
}

struct v2f {
    @builtin(position) pos: vec4f,
    @location(0) color: vec4f,
}

@group(0) @binding(0)
var<uniform> matrix_vp: mat4x4f;

@vertex
fn vs_main(v: a2v) -> v2f {
    var o: v2f;
    o.pos = matrix_vp * vec4f(v.vertex.xyz * 0.1, 1.0);
    o.color = v.color;
    return o;
}

struct gbuffer {
    @location(0) color: vec4f
}

@fragment
fn fs_main(i: v2f) -> gbuffer {
    var out: gbuffer;
    out.color = i.color;
    return out;
}
