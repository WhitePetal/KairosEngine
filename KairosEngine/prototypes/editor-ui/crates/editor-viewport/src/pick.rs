//! CPU picking for the prototype viewport.
//!
//! The real editor asks the engine's scene for the object under the cursor
//! (object ids are engine-level ids). This module is the host stand-in: it
//! rebuilds the same primary ray the viewport shader marches and intersects it
//! with the demo scene's analytic proxies.
//!
//! The objects mirror the raymarch scene in `scene.wgsl`:
//!
//! * object `1`: axis-aligned cube centred at `(0, 0.5, 0)` with half extent `0.5`;
//! * object `2`: the infinite checker ground plane at `y = 0`.
//!
//! A ray that hits both reports the nearer surface, so the cube occludes the
//! ground exactly like it does in the rendered image. Rays that leave the
//! scene (looking up at the sky) return `None`.

use crate::camera::OrbitCamera;

/// Object id of the demo cube; matches the selection highlight in the shader.
pub const CUBE_OBJECT_ID: u64 = 1;

/// Object id of the demo ground plane; matches the checker grid in the shader.
pub const GROUND_OBJECT_ID: u64 = 2;

/// World-space centre of the demo cube, shared with `scene.wgsl`.
pub const CUBE_CENTER: [f32; 3] = [0.0, 0.5, 0.0];

/// Half extent of the demo cube, shared with `scene.wgsl`.
pub const CUBE_HALF_EXTENT: [f32; 3] = [0.5, 0.5, 0.5];

/// World-space height of the demo ground plane.
pub const GROUND_Y: f32 = 0.0;

/// A ray in world space, used by [`pick_object`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    /// Ray origin in world space.
    pub origin: [f32; 3],
    /// Normalized ray direction in world space.
    pub direction: [f32; 3],
}

impl Ray {
    /// Builds a ray, normalizing `direction`.
    ///
    /// A zero or non-finite direction becomes `[0, 0, -1]` so callers never get
    /// NaNs back from the intersection helpers.
    #[must_use]
    pub fn new(origin: [f32; 3], direction: [f32; 3]) -> Self {
        let length_squared =
            direction[0] * direction[0] + direction[1] * direction[1] + direction[2] * direction[2];
        let direction = if length_squared.is_finite() && length_squared > 1.0e-20 {
            let inverse_length = 1.0 / length_squared.sqrt();
            [
                direction[0] * inverse_length,
                direction[1] * inverse_length,
                direction[2] * inverse_length,
            ]
        } else {
            [0.0, 0.0, -1.0]
        };
        Self { origin, direction }
    }

    /// Returns the point at parameter `t` along the ray.
    #[must_use]
    pub fn at(&self, t: f32) -> [f32; 3] {
        [
            self.origin[0] + self.direction[0] * t,
            self.origin[1] + self.direction[1] * t,
            self.origin[2] + self.direction[2] * t,
        ]
    }
}

/// Builds the primary ray for a normalized device coordinate.
///
/// `ndc` is in `[-1, 1]` with `x` right and `y` up (egui's image space is y-down,
/// so the app must flip y before calling this). The ray is reconstructed from
/// the camera basis, which is the same math the fragment shader uses, so a
/// click and the rendered pixel under it agree.
#[must_use]
pub fn ndc_to_ray(camera: &OrbitCamera, aspect: f32, ndc: [f32; 2]) -> Ray {
    let aspect = if aspect.is_finite() && aspect > 0.0 {
        aspect
    } else {
        1.0
    };
    let tan_half_fov = camera.tan_half_fov_y();
    let forward = camera.forward();
    let right = camera.right();
    let up = camera.up();
    let x = if ndc[0].is_finite() { ndc[0] } else { 0.0 };
    let y = if ndc[1].is_finite() { ndc[1] } else { 0.0 };
    let horizontal = x * tan_half_fov * aspect;
    let vertical = y * tan_half_fov;
    Ray::new(
        camera.eye(),
        [
            forward[0] + right[0] * horizontal + up[0] * vertical,
            forward[1] + right[1] * horizontal + up[1] * vertical,
            forward[2] + right[2] * horizontal + up[2] * vertical,
        ],
    )
}

/// Intersects a ray with an axis-aligned box.
///
/// Returns the entry and exit parameters `(t_enter, t_exit)`; a ray whose origin
/// is inside the box returns a negative entry parameter. Returns `None` when the
/// ray misses the box or when the box is entirely behind the origin
/// (`t_exit < 0`). Directions equal to zero are handled explicitly so the slab
/// test never divides by zero.
#[must_use]
pub fn ray_aabb(ray: &Ray, min: [f32; 3], max: [f32; 3]) -> Option<(f32, f32)> {
    let mut t_enter = f32::NEG_INFINITY;
    let mut t_exit = f32::INFINITY;
    if !slab(
        ray.origin[0],
        ray.direction[0],
        min[0],
        max[0],
        &mut t_enter,
        &mut t_exit,
    ) || !slab(
        ray.origin[1],
        ray.direction[1],
        min[1],
        max[1],
        &mut t_enter,
        &mut t_exit,
    ) || !slab(
        ray.origin[2],
        ray.direction[2],
        min[2],
        max[2],
        &mut t_enter,
        &mut t_exit,
    ) {
        return None;
    }
    if t_exit < 0.0 {
        return None;
    }
    Some((t_enter, t_exit))
}

/// Intersects a ray with the horizontal plane at `plane_y`.
///
/// The plane is treated as two-sided: a ray coming from below also reports a
/// hit. Parallel rays report `None`.
#[must_use]
pub fn ray_plane_y(ray: &Ray, plane_y: f32) -> Option<f32> {
    if ray.direction[1] == 0.0 {
        return None;
    }
    let t = (plane_y - ray.origin[1]) / ray.direction[1];
    if t.is_finite() && t > 0.0 {
        Some(t)
    } else {
        None
    }
}

/// Picks the demo object under `ndc` for the given camera.
///
/// Returns [`CUBE_OBJECT_ID`], [`GROUND_OBJECT_ID`], or `None` for a miss. The
/// nearest of the two candidate surfaces wins.
#[must_use]
pub fn pick_object(camera: &OrbitCamera, aspect: f32, ndc: [f32; 2]) -> Option<u64> {
    let ray = ndc_to_ray(camera, aspect, ndc);
    let cube_min = [
        CUBE_CENTER[0] - CUBE_HALF_EXTENT[0],
        CUBE_CENTER[1] - CUBE_HALF_EXTENT[1],
        CUBE_CENTER[2] - CUBE_HALF_EXTENT[2],
    ];
    let cube_max = [
        CUBE_CENTER[0] + CUBE_HALF_EXTENT[0],
        CUBE_CENTER[1] + CUBE_HALF_EXTENT[1],
        CUBE_CENTER[2] + CUBE_HALF_EXTENT[2],
    ];
    let cube_t = ray_aabb(&ray, cube_min, cube_max).map(|(entry, _exit)| entry.max(0.0));
    let ground_t = ray_plane_y(&ray, GROUND_Y);

    match (cube_t, ground_t) {
        (Some(cube_t), Some(ground_t)) => {
            if cube_t <= ground_t {
                Some(CUBE_OBJECT_ID)
            } else {
                Some(GROUND_OBJECT_ID)
            }
        }
        (Some(_), None) => Some(CUBE_OBJECT_ID),
        (None, Some(_)) => Some(GROUND_OBJECT_ID),
        (None, None) => None,
    }
}

/// One axis of the slab test for [`ray_aabb`].
fn slab(
    origin: f32,
    direction: f32,
    min: f32,
    max: f32,
    t_enter: &mut f32,
    t_exit: &mut f32,
) -> bool {
    if direction == 0.0 {
        // Parallel to this slab: the origin must already be inside it.
        return origin >= min && origin <= max;
    }
    let inverse_direction = 1.0 / direction;
    let mut near = (min - origin) * inverse_direction;
    let mut far = (max - origin) * inverse_direction;
    if near > far {
        core::mem::swap(&mut near, &mut far);
    }
    *t_enter = t_enter.max(near);
    *t_exit = t_exit.min(far);
    *t_enter <= *t_exit
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f32, expected: f32, tolerance: f32) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected}, got {actual} (tolerance {tolerance})"
        );
    }

    fn camera(yaw: f32, pitch: f32, distance: f32, target: [f32; 3]) -> OrbitCamera {
        OrbitCamera {
            yaw,
            pitch,
            distance,
            target,
            fov_y: crate::camera::DEFAULT_FOV_Y,
        }
    }

    #[test]
    fn center_ray_hits_the_cube() {
        let camera = OrbitCamera::default();
        // The default camera targets the cube centre, so the centre pixel must
        // hit object 1 for any aspect ratio the app might use.
        for aspect in [1.0, 16.0 / 9.0, 0.5] {
            assert_eq!(
                pick_object(&camera, aspect, [0.0, 0.0]),
                Some(CUBE_OBJECT_ID)
            );
        }
    }

    #[test]
    fn sky_ray_misses_everything() {
        let camera = OrbitCamera::default();
        assert_eq!(pick_object(&camera, 16.0 / 9.0, [0.0, 1.0]), None);
        assert_eq!(pick_object(&camera, 16.0 / 9.0, [0.9, 0.95]), None);
    }

    #[test]
    fn downward_ray_misses_cube_but_hits_the_ground() {
        // Eye at (0, 2.4, 4.4) looking at the origin; the ray below the cube
        // reaches the ground plane several units in front of the cube.
        let camera = camera(0.0, 0.5, 5.0, [0.0, 0.0, 0.0]);
        assert_eq!(
            pick_object(&camera, 16.0 / 9.0, [0.0, -0.9]),
            Some(GROUND_OBJECT_ID)
        );
    }

    #[test]
    fn top_down_ray_hits_the_cube_top() {
        let camera = camera(0.0, 1.5, 3.0, [0.0, 0.0, 0.0]);
        assert_eq!(pick_object(&camera, 1.0, [0.0, 0.0]), Some(CUBE_OBJECT_ID));
    }

    #[test]
    fn ray_aabb_reports_entry_and_exit() {
        let ray = Ray::new([0.0, 0.5, -3.0], [0.0, 0.0, 1.0]);
        let (entry, exit) = ray_aabb(&ray, [-0.5, 0.0, -0.5], [0.5, 1.0, 0.5]).unwrap();
        assert_close(entry, 2.5, 1.0e-5);
        assert_close(exit, 3.5, 1.0e-5);
    }

    #[test]
    fn ray_aabb_misses_when_offset_sideways() {
        let ray = Ray::new([2.0, 0.5, -3.0], [0.0, 0.0, 1.0]);
        assert_eq!(ray_aabb(&ray, [-0.5, 0.0, -0.5], [0.5, 1.0, 0.5]), None);
    }

    #[test]
    fn ray_aabb_reports_negative_entry_when_origin_is_inside() {
        let ray = Ray::new([0.0, 0.5, 0.0], [0.0, 1.0, 0.0]);
        let (entry, exit) = ray_aabb(&ray, [-0.5, 0.0, -0.5], [0.5, 1.0, 0.5]).unwrap();
        assert_close(entry, -0.5, 1.0e-5);
        assert_close(exit, 0.5, 1.0e-5);
    }

    #[test]
    fn ray_aabb_misses_when_box_is_behind_the_origin() {
        let ray = Ray::new([0.0, 0.5, -5.0], [0.0, 0.0, -1.0]);
        assert_eq!(ray_aabb(&ray, [-0.5, 0.0, -0.5], [0.5, 1.0, 0.5]), None);
    }

    #[test]
    fn ray_plane_returns_forward_hit_distance() {
        let ray = Ray::new([0.0, 2.0, 0.0], [0.0, -1.0, 0.0]);
        assert_eq!(ray_plane_y(&ray, GROUND_Y), Some(2.0));
    }

    #[test]
    fn ray_plane_ignores_parallel_and_backwards_rays() {
        let parallel = Ray::new([0.0, 2.0, 0.0], [1.0, 0.0, 0.0]);
        assert_eq!(ray_plane_y(&parallel, GROUND_Y), None);
        let upwards = Ray::new([0.0, 2.0, 0.0], [0.0, 1.0, 0.0]);
        assert_eq!(ray_plane_y(&upwards, GROUND_Y), None);
    }

    #[test]
    fn ray_plane_is_two_sided() {
        let from_below = Ray::new([0.0, -1.0, 0.0], [0.0, 1.0, 0.0]);
        assert_eq!(ray_plane_y(&from_below, GROUND_Y), Some(1.0));
    }

    #[test]
    fn ray_new_normalizes_direction_and_guards_degenerate_input() {
        let normalized = Ray::new([0.0, 0.0, 0.0], [0.0, 3.0, 0.0]);
        assert_eq!(normalized.direction, [0.0, 1.0, 0.0]);

        let zero = Ray::new([1.0, 2.0, 3.0], [0.0, 0.0, 0.0]);
        assert_eq!(zero.direction, [0.0, 0.0, -1.0]);

        let nan = Ray::new([0.0, 0.0, 0.0], [f32::NAN, 0.0, 0.0]);
        assert_eq!(nan.direction, [0.0, 0.0, -1.0]);
    }

    #[test]
    fn ray_at_walks_along_the_direction() {
        let ray = Ray::new([1.0, 2.0, 3.0], [0.0, 0.0, -2.0]);
        let point = ray.at(4.0);
        assert_close(point[0], 1.0, 1.0e-6);
        assert_close(point[1], 2.0, 1.0e-6);
        assert_close(point[2], -1.0, 1.0e-6);
    }

    #[test]
    fn ndc_to_ray_center_matches_camera_forward() {
        let camera = OrbitCamera::default();
        let ray = ndc_to_ray(&camera, 1.5, [0.0, 0.0]);
        let forward = camera.forward();
        for (actual, expected) in ray.direction.iter().zip(forward.iter()) {
            assert_close(*actual, *expected, 1.0e-5);
        }
    }

    #[test]
    fn ndc_to_ray_x_follows_right_and_y_follows_up() {
        let camera = OrbitCamera::default();
        let right_ray = ndc_to_ray(&camera, 1.5, [1.0, 0.0]);
        let up_ray = ndc_to_ray(&camera, 1.5, [0.0, 1.0]);
        let right = camera.right();
        let up = camera.up();
        let right_dot: f32 = (0..3)
            .map(|axis| right_ray.direction[axis] * right[axis])
            .sum();
        let up_dot: f32 = (0..3).map(|axis| up_ray.direction[axis] * up[axis]).sum();
        assert!(right_dot > 0.0, "ndc +x must aim to camera right");
        assert!(up_dot > 0.0, "ndc +y must aim to camera up");

        // Corners must stay within the frustum's half-angle for both axes.
        let camera_forward = camera.forward();
        let forward_dot: f32 = (0..3)
            .map(|axis| right_ray.direction[axis] * camera_forward[axis])
            .sum();
        assert!(forward_dot > 0.0);
    }

    #[test]
    fn ndc_to_ray_horizontal_spread_scales_with_aspect() {
        let camera = OrbitCamera::default();
        let forward = camera.forward();
        let right = camera.right();
        let tan_half = camera.tan_half_fov_y();
        for aspect in [1.0_f32, 2.0, 0.75] {
            let ray = ndc_to_ray(&camera, aspect, [1.0, 0.0]);
            let horizontal: f32 = (0..3).map(|axis| ray.direction[axis] * right[axis]).sum();
            let depth: f32 = (0..3).map(|axis| ray.direction[axis] * forward[axis]).sum();
            assert_close(horizontal / depth, tan_half * aspect, 1.0e-5);
        }
    }

    #[test]
    fn ndc_to_ray_survives_broken_aspect_and_ndc() {
        let camera = OrbitCamera::default();
        for ndc in [[f32::NAN, 0.0], [0.0, f32::INFINITY], [0.0, 0.0]] {
            let ray = ndc_to_ray(&camera, f32::NAN, ndc);
            assert!(ray.direction.iter().all(|value| value.is_finite()));
            assert!(ray.origin.iter().all(|value| value.is_finite()));
        }
    }

    #[test]
    fn vertical_scan_hits_cube_then_ground() {
        // Straight ahead is the cube; far enough down the ray passes under it
        // and lands on the ground plane.
        let camera = camera(0.0, 0.5, 5.0, [0.0, 0.0, 0.0]);
        assert_eq!(
            pick_object(&camera, 1.5, [0.0, 0.6]),
            Some(GROUND_OBJECT_ID)
        );
        assert_eq!(
            pick_object(&camera, 1.5, [0.0, -0.9]),
            Some(GROUND_OBJECT_ID)
        );
    }
}
