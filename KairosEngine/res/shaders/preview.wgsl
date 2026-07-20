// Preview shader — solid white diffuse, no texture.
// Used by the model inspector to show a clean 3D preview.

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
    // Diffuse lighting: warm directional light from upper-right-front.
    let light_dir = normalize(vec3f(0.6, 0.8, 0.5));
    let ndotl = max(dot(normalize(v.normal), light_dir), 0.0);
    let brightness = ndotl * 0.65 + 0.35; // ambient + diffuse
    o.color = vec4f(brightness, brightness, brightness, 1.0);
    return o;
}

@fragment
fn fs_main(i: VertexOutput) -> @location(0) vec4f {
    return i.color;
}
