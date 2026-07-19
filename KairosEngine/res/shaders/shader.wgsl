// enable f16;

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
    @location(0) color: vec4f,
    @location(1) uv: vec2f,
    @location(2) normal: vec3f,
};

@group(0) @binding(0)
var<uniform> matrix_vp: mat4x4f;

@group(1) @binding(0)
var texture: texture_2d<f32>;
@group(1) @binding(1)
var s_texture: sampler;


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
    var normal_world = normalize(local_to_world * vec4f(v.normal.xyz, 0.0));
    o.color = v.color;
    o.uv = v.texcoord;
    o.normal = normal_world.xyz;

    return o;
}

struct gbuffer {
    @location(0) color: vec4f
}

@fragment
fn fs_main(i: v2f) -> gbuffer {
    var out: gbuffer;
    let tex = textureSample(texture, s_texture, i.uv);
    let color = i.color * tex;
    let l = normalize(vec3f(0.0, 1.0, 1.0));
    let ndotl = dot(i.normal, l) * 0.5 + 0.5;
    out.color = color * ndotl;
    // out.color = vec4f(ndotl);
    return out;
}
