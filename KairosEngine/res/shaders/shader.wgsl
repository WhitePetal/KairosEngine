// enable f16;

struct a2v {
    @location(0) vertex: vec4f,
    @location(1) color: vec4f,
    @location(2) texcoord: vec2f,
}

struct v2f {
    @builtin(position) pos: vec4f,
    @location(0) color: vec4f,
    @location(1) uv: vec2f,
};

@group(1) @binding(0)
var<uniform> matrix_vp: mat4x4f;

@vertex
fn vs_main(v: a2v) -> v2f {
    var o: v2f;

    o.pos = matrix_vp * vec4f(v.vertex.xyz, 1.0);
    o.color = v.color;
    o.uv = v.texcoord;

    return o;
}

struct gbuffer {
    @location(0) color: vec4f
}

@group(0) @binding(0)
var texture: texture_2d<f32>;
@group(0) @binding(1)
var s_texture: sampler;

@fragment
fn fs_main(i: v2f) -> gbuffer {
    var out: gbuffer;
    let tex = textureSample(texture, s_texture, i.uv);
    let color = i.color * tex;
    out.color = color;
    return out;
}