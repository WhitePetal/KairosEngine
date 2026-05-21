
struct VertexOutput {
    @builtin(position) pos: vec4f,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOutput {
    var out: VertexOutput;

    let x = f32(1 - i32(vi)) * 0.5;
    let y = f32(i32(vi & 1u) * 2 - 1) * 0.5;

    out.pos = vec4f(x, y, 0.0, 0.0);

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4h {
    return vec4h(0.3, 0.2, 0.1, 1.0);
}