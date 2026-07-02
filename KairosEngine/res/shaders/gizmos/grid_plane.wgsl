// Grid gizmo shader — draws an infinite-looking grid on the XZ plane.
// The grid is computed in the fragment shader from world-space coordinates,
// so density can later be adjusted via a uniform without regenerating geometry.

struct a2v {
    @location(0) vertex: vec4f,
}

struct v2f {
    @builtin(position) pos: vec4f,
    @location(0) world_position: vec3f,
}

@group(0) @binding(0)
var<uniform> matrix_vp: mat4x4f;

@vertex
fn vs_main(v: a2v) -> v2f {
    var o: v2f;
    // The quad vertices are already in world space (XZ plane, Y=0).
    o.world_position = v.vertex.xyz;
    o.pos = matrix_vp * vec4f(v.vertex.xyz, 1.0);
    return o;
}

struct gbuffer {
    @location(0) color: vec4f
}

@fragment
fn fs_main(i: v2f) -> gbuffer {
    var out: gbuffer;
    let grid_size: f32 = 1.0;  // one unit per cell
    let line_width: f32 = 0.01;

    let pos = i.world_position.xz;
    let fract_pos = fract(pos / grid_size);
    let dx = min(fract_pos.x, 1.0 - fract_pos.x);
    let dz = min(fract_pos.y, 1.0 - fract_pos.y);
    let dist_to_line = min(dx, dz);

    let line = 1.0 - smoothstep(0.0, line_width, dist_to_line);
    let grid_color = vec4f(0.3, 0.3, 0.3, 0.4);
    out.color = grid_color * line;
    return out;
}
