// Prototype viewport scene shader.
//
// A single fullscreen triangle is traced per pixel: the fragment shader
// reconstructs the primary ray from the camera basis in the uniform block and
// sphere-traces the scene's signed distance field. There are no vertex or index
// buffers, so the whole demo scene lives in this file.
//
// Scene (mirrors pick.rs):
//   * ground: the plane y = 0, drawn as an anti-aliased checker grid;
//   * cube:   axis-aligned box centred at (0, 0.5, 0) with half extent 0.5.
//
// Coordinate system: right-handed, +y up. wgpu clip space has z in [0, 1].
// The host writes one SceneCamera uniform per viewport (see renderer.rs); the
// uniform layout must stay in sync with SceneCameraUniform there.
//
// Colour: the host renders into an Rgba8UnormSrgb attachment, so this shader
// writes linear light and the hardware applies the sRGB transfer function on
// store. The registered egui view then samples those gamma-encoded bytes.

struct SceneCamera {
    // xyz: eye position in world space, w: viewport aspect ratio (width / height).
    eye_aspect: vec4<f32>,
    // xyz: normalized camera forward, w: tan(fov_y / 2).
    forward_tan: vec4<f32>,
    // xyz: normalized camera right axis.
    right: vec4<f32>,
    // xyz: normalized camera up axis.
    up: vec4<f32>,
    // x: selected object id (-1.0 when nothing is selected);
    // yz: viewport size in pixels (needed for supersampling);
    // w: reserved for future parameters.
    params: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: SceneCamera;

// Scene constants; keep in sync with pick.rs.
const CUBE_CENTER: vec3<f32> = vec3<f32>(0.0, 0.5, 0.0);
const CUBE_HALF_EXTENT: vec3<f32> = vec3<f32>(0.5, 0.5, 0.5);
const CUBE_ID: f32 = 1.0;
const GROUND_ID: f32 = 2.0;

// Simple directional key light; the prototype does not model engine lighting.
const LIGHT_DIRECTION: vec3<f32> = normalize(vec3<f32>(0.45, 0.85, 0.30));

// Raymarch tuning. The scene is small and mostly analytic, so a low step budget
// is enough and keeps the prototype viewport fast on integrated GPUs.
const MAX_STEPS: i32 = 96;
const MAX_DISTANCE: f32 = 48.0;
const HIT_EPSILON: f32 = 0.0008;

// Supersampling: AA_SAMPLES x AA_SAMPLES rays per pixel. 2 doubles the memory
// bandwidth and roughly quadruples the trace cost, which is acceptable for a
// viewport-sized target; use 1 for a low-end/software fallback.
const AA_SAMPLES: i32 = 2;

// ------------------------------------------
// Signed distance field
// ------------------------------------------

// Signed distance to a box: negative inside, positive outside.
fn sd_box(p: vec3<f32>, center: vec3<f32>, half_extent: vec3<f32>) -> f32 {
    let q = abs(p - center) - half_extent;
    return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}

// Signed distance to the whole scene: the cube unioned with the ground
// half-space y >= 0. The camera is clamped above the ground by the host, so the
// march always starts outside the volume.
fn sd_scene(p: vec3<f32>) -> f32 {
    let cube = sd_box(p, CUBE_CENTER, CUBE_HALF_EXTENT);
    let ground = p.y;
    return min(cube, ground);
}

// Surface normal from the SDF, using the tetrahedron trick (4 taps).
fn scene_normal(p: vec3<f32>) -> vec3<f32> {
    let epsilon = 0.0015;
    let k0 = vec3<f32>(1.0, -1.0, -1.0);
    let k1 = vec3<f32>(-1.0, -1.0, 1.0);
    let k2 = vec3<f32>(-1.0, 1.0, -1.0);
    let k3 = vec3<f32>(1.0, 1.0, 1.0);
    let normal = k0 * sd_scene(p + k0 * epsilon)
        + k1 * sd_scene(p + k1 * epsilon)
        + k2 * sd_scene(p + k2 * epsilon)
        + k3 * sd_scene(p + k3 * epsilon);
    return normalize(normal);
}

struct SurfaceHit {
    // Distance along the ray; only meaningful when object is non-zero.
    t: f32,
    // 0.0 = miss, CUBE_ID = cube, GROUND_ID = ground plane.
    object: f32,
};

// Sphere-traces the scene SDF from the camera and classifies the hit surface.
fn trace(origin: vec3<f32>, direction: vec3<f32>) -> SurfaceHit {
    var t = 0.0;
    for (var step = 0; step < MAX_STEPS; step = step + 1) {
        let position = origin + direction * t;
        let distance = sd_scene(position);
        // The hit epsilon grows with distance so far-away hits do not sink into
        // the surface.
        if distance < HIT_EPSILON * max(1.0, t) {
            let cube_distance = sd_box(position, CUBE_CENTER, CUBE_HALF_EXTENT);
            let object = select(GROUND_ID, CUBE_ID, cube_distance < position.y);
            return SurfaceHit(t, object);
        }
        t = t + distance;
        if t > MAX_DISTANCE {
            break;
        }
    }
    return SurfaceHit(0.0, 0.0);
}

// ------------------------------------------
// Shading
// ------------------------------------------

// Dark editor-style background gradient; darker at the horizon, cooler above.
fn sky_color(direction: vec3<f32>) -> vec3<f32> {
    let horizon = vec3<f32>(0.038, 0.042, 0.052);
    let zenith = vec3<f32>(0.070, 0.090, 0.130);
    let height = clamp(direction.y * 0.5 + 0.5, 0.0, 1.0);
    return mix(horizon, zenith, smoothstep(0.35, 1.0, height));
}

// Returns (checker, line) for the ground plane; both in [0, 1]. A checker square
// is one world unit, and grid lines sit on integer world coordinates.
fn ground_pattern(xz: vec2<f32>) -> vec2<f32> {
    // World units covered by one pixel; drives the anti-aliasing below.
    let footprint = max(fwidth(xz), vec2<f32>(0.0001));
    // 1x1 checkerboard cells: parity of the cell index.
    let cell = floor(xz);
    let parity = fract((cell.x + cell.y) * 0.5) * 2.0;
    // Fade the checker to its average when a pixel covers several cells, which
    // keeps the horizon from sparkling.
    let blend = clamp(max(footprint.x, footprint.y) * 2.0, 0.0, 1.0);
    let checker = mix(parity, 0.5, blend);
    // One-unit grid lines, roughly one pixel wide.
    let distance_to_line = abs(fract(xz + 0.5) - 0.5);
    let line = 1.0 - smoothstep(vec2<f32>(0.0), footprint * 1.5, distance_to_line);
    return vec2<f32>(checker, max(line.x, line.y) * (1.0 - blend));
}

// Cheap contact shadow: a soft darkening of the ground under and beside the
// cube, so it reads as resting on the plane instead of floating above it.
//
// A real engine would render a shadow map. Here the only occluder is the cube,
// so the shadow area on the ground is the union of the cube's footprint swept
// along the light direction. Four samples along that sweep approximate the
// swept square; the result is a tight soft shadow attached to the cube base.
fn contact_shadow(xz: vec2<f32>) -> f32 {
    // Tight ambient-occlusion ring that touches the cube base on every side.
    let base_footprint =
        abs(xz) - vec2<f32>(CUBE_HALF_EXTENT.x, CUBE_HALF_EXTENT.z);
    let base_distance = length(max(base_footprint, vec2<f32>(0.0)));
    let contact = 1.0 - smoothstep(0.0, 0.22, base_distance);

    // Directional shadow: union of the footprint swept along the light.
    let sweep = LIGHT_DIRECTION.xz / LIGHT_DIRECTION.y;
    var sweep_distance = 1.0e3;
    for (var step = 0; step < 4; step = step + 1) {
        // 0.0 = at the ground, 1.0 = at the top of the cube.
        let height = f32(step) / 3.0;
        let footprint =
            abs(xz + sweep * height) - vec2<f32>(CUBE_HALF_EXTENT.x, CUBE_HALF_EXTENT.z);
        sweep_distance = min(sweep_distance, length(max(footprint, vec2<f32>(0.0))));
    }
    let shadow = 1.0 - smoothstep(0.0, 0.35, sweep_distance);

    return 1.0 - 0.28 * contact - 0.20 * shadow;
}

// Directional-ish shading for the ground plane.
fn shade_ground(position: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    let pattern = ground_pattern(position.xz);
    let dark = vec3<f32>(0.055, 0.060, 0.070);
    let light = vec3<f32>(0.105, 0.110, 0.125);
    var albedo = mix(dark, light, pattern.x);
    let line_color = vec3<f32>(0.230, 0.250, 0.290);
    albedo = mix(albedo, line_color, pattern.y * 0.7);
    let diffuse = max(dot(normal, LIGHT_DIRECTION), 0.0);
    let ambient = 0.34 + 0.12 * normal.y;
    return albedo * (ambient + 0.85 * diffuse) * contact_shadow(position.xz);
}

// Directional-ish shading for the cube, plus the selection highlight for the
// object id the engine would report when the cube is selected.
fn shade_cube(normal: vec3<f32>, view: vec3<f32>) -> vec3<f32> {
    let base = vec3<f32>(0.22, 0.25, 0.31);
    let diffuse = max(dot(normal, LIGHT_DIRECTION), 0.0);
    // A dim opposite fill keeps the shadow side readable against the dark sky.
    let fill = max(dot(normal, normalize(vec3<f32>(-0.55, 0.25, -0.60))), 0.0);
    let ambient = 0.26 + 0.18 * normal.y;
    var color = base * (ambient + 0.90 * diffuse + 0.22 * fill);
    // Cheap key-light sheen so the cube reads as solid against the dark sky.
    let halfway = normalize(LIGHT_DIRECTION + view);
    let specular = pow(max(dot(normal, halfway), 0.0), 24.0);
    color = color + vec3<f32>(0.30, 0.31, 0.33) * specular;
    // Selection highlight: warm rim light in the editor's accent colour. The
    // prototype scene only has the cube as a selectable object.
    if abs(camera.params.x - CUBE_ID) < 0.5 {
        let rim = pow(1.0 - max(dot(normal, view), 0.0), 2.0);
        let accent = vec3<f32>(0.95, 0.55, 0.16);
        color = mix(color, accent * (0.55 + 1.10 * rim), 0.20 + 0.55 * rim);
    }
    return color;
}

// Distance fog that fades geometry into the sky colour at the horizon.
fn distance_fog(distance: f32) -> f32 {
    return smoothstep(16.0, 44.0, distance);
}

// Traces and shades one primary ray; returns linear colour.
fn shade_ray(eye: vec3<f32>, direction: vec3<f32>) -> vec3<f32> {
    var color = sky_color(direction);
    let hit = trace(eye, direction);
    if hit.object > 0.0 {
        let position = eye + direction * hit.t;
        let normal = scene_normal(position);
        if hit.object == CUBE_ID {
            color = shade_cube(normal, -direction);
        } else {
            color = shade_ground(position, normal);
        }
        color = mix(color, sky_color(direction), distance_fog(hit.t));
    }
    return color;
}

// ------------------------------------------
// Fullscreen triangle
// ------------------------------------------

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Fullscreen triangle covering clip space: (-1,-1), (3,-1), (-1,3).
    // clip.xy is already the NDC the fragment shader needs for its camera ray.
    let corner = vec2<f32>(f32((vertex_index << 1u) & 2u), f32(vertex_index & 2u));
    let clip = corner * 2.0 - 1.0;
    var out: VertexOutput;
    out.clip_position = vec4<f32>(clip, 0.0, 1.0);
    out.ndc = clip;
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let eye = camera.eye_aspect.xyz;
    let aspect = camera.eye_aspect.w;
    let tan_half_fov = camera.forward_tan.w;
    // Size of one pixel in NDC, used to offset the supersampling rays.
    let ndc_per_pixel = 2.0 / camera.params.yz;

    var color = vec3<f32>(0.0);
    for (var sample_y = 0; sample_y < AA_SAMPLES; sample_y = sample_y + 1) {
        for (var sample_x = 0; sample_x < AA_SAMPLES; sample_x = sample_x + 1) {
            let jitter = (vec2<f32>(f32(sample_x), f32(sample_y)) + 0.5)
                / f32(AA_SAMPLES)
                - 0.5;
            let ndc = input.ndc + jitter * ndc_per_pixel;
            // Primary ray, reconstructed exactly like the CPU picking code does.
            let direction = normalize(
                camera.forward_tan.xyz
                    + camera.right.xyz * (ndc.x * tan_half_fov * aspect)
                    + camera.up.xyz * (ndc.y * tan_half_fov)
            );
            color = color + shade_ray(eye, direction);
        }
    }
    color = color / f32(AA_SAMPLES * AA_SAMPLES);

    // The offscreen target is Rgba8UnormSrgb: write linear light and let the
    // hardware apply the sRGB transfer function on store.
    return vec4<f32>(color, 1.0);
}