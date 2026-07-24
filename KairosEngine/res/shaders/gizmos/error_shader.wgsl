// Error / fallback shader — outputs pure purple (1.0, 0.0, 1.0, 1.0).
// Matches the standard vertex input layout so it can be used as a
// drop-in replacement when the real shader fails to load.

struct InstanceInput {
    @location(5) model_matrix_0: vec4f,
    @location(6) model_matrix_1: vec4f,
    @location(7) model_matrix_2: vec4f,
    @location(8) model_matrix_3: vec4f,
};

struct a2v {
    @location(0) vertex: vec4f,
    @location(1) color: vec4f,
    @location(2) texcoord: vec2f,
    @location(3) normal: vec3f,
    @location(4) tangent: vec4f,
}

struct v2f {
    @builtin(position) pos: vec4f,
}

@group(0) @binding(0)
var<uniform> matrix_vp: mat4x4f;

@vertex
fn vs_main(v: a2v, instancing: InstanceInput) -> v2f {
    var o: v2f;

    var local_to_world = mat4x4f(
        instancing.model_matrix_0,
        instancing.model_matrix_1,
        instancing.model_matrix_2,
        instancing.model_matrix_3,
    );

    o.pos = matrix_vp * local_to_world * vec4f(v.vertex.xyz, 1.0);

    return o;
}

struct gbuffer {
    @location(0) color: vec4f
}

@fragment
fn fs_main() -> gbuffer {
    var out: gbuffer;
    out.color = vec4f(1.0, 0.0, 1.0, 1.0); // pure purple
    return out;
}
