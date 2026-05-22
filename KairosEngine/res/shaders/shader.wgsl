// enable f16;

struct a2v {
    @location(0) vertex: vec4f,
    @location(1) color: vec4f
}

struct v2f {
    @builtin(position) pos: vec4f,
    @location(0) color: vec4f,
};

@vertex
fn vs_main(v: a2v) -> v2f {
    var out: v2f;

    out.pos = v.vertex;
    out.color = v.color;

    return out;
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