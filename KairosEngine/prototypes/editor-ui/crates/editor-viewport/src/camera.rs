//! Orbit camera for the prototype viewport host.
//!
//! In the real editor the camera lives in the engine's camera controller and the
//! editor only forwards input. This module is the host stand-in: it is pure math
//! (no wgpu, no egui), so every interaction rule is covered by deterministic
//! unit tests.
//!
//! # Conventions
//!
//! * Right-handed world: `+x` right, `+y` up, `+z` towards the viewer. The camera
//!   looks down its local `-z` axis.
//! * Matrices are column-major `[[f32; 4]; 4]` (`m[column][row]`), which is also
//!   the memory layout WGSL expects for `mat4x4<f32>` uniforms.
//! * Clip space is wgpu-style: `z` in `[0, 1]` (not OpenGL's `[-1, 1]`).
//!
//! # Demo-scene constraints
//!
//! The prototype scene has an opaque ground plane at `y = 0` and a cube resting
//! on it. The raymarch shader assumes it starts outside the scene's signed
//! distance field, so the camera is clamped to stay above the ground: pitch is
//! clamped to [`MIN_PITCH`]`..=`[`MAX_PITCH`] and the target's `y` is clamped to
//! `>= 0`. A production editor camera would allow orbiting below the level;
//! this constraint is specific to the throwaway demo scene.

use std::f32::consts::{FRAC_PI_4, PI, TAU};

/// A 4x4 column-major matrix: `m[column][row]`, the layout WGSL expects.
pub type Mat4 = [[f32; 4]; 4];

/// Smallest pitch (radians) the camera may reach.
///
/// Kept slightly positive so the eye stays above the demo scene's ground plane.
pub const MIN_PITCH: f32 = 0.02;

/// Largest pitch (radians) the camera may reach.
///
/// Slightly below `PI / 2` so the view direction never becomes parallel to the
/// world up axis (a degenerate orbit).
pub const MAX_PITCH: f32 = 1.5;

/// Closest distance the camera may get to its target, in world units.
pub const MIN_DISTANCE: f32 = 0.75;

/// Farthest distance the camera may get from its target, in world units.
pub const MAX_DISTANCE: f32 = 30.0;

/// Default vertical field of view (45 degrees, in radians).
pub const DEFAULT_FOV_Y: f32 = FRAC_PI_4;

/// Smallest vertical field of view the camera accepts, in radians.
pub const MIN_FOV_Y: f32 = 0.2;

/// Largest vertical field of view the camera accepts, in radians.
pub const MAX_FOV_Y: f32 = 2.6;

/// Near plane distance used by [`OrbitCamera::projection`].
pub const NEAR: f32 = 0.05;

/// Far plane distance used by [`OrbitCamera::projection`].
pub const FAR: f32 = 200.0;

const DEFAULT_YAW: f32 = 0.65;
const DEFAULT_PITCH: f32 = 0.32;
const DEFAULT_DISTANCE: f32 = 5.0;
const DEFAULT_TARGET: [f32; 3] = [0.0, 0.5, 0.0];
const WORLD_UP: [f32; 3] = [0.0, 1.0, 0.0];

/// Orbiting camera that looks at [`OrbitCamera::target`] from a spherical offset.
///
/// The eye position is derived from `yaw`, `pitch` and `distance`; the target is
/// the point the camera orbits and the point that stays at the centre of the
/// screen. Fields are public because the host layer (and the app) treat the
/// camera as plain data; [`OrbitCamera::apply`] is the only place that mutates
/// it during normal interaction and it enforces the clamps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrbitCamera {
    /// Horizontal orbit angle in radians (wrapped to `[-PI, PI)`).
    pub yaw: f32,
    /// Vertical orbit angle in radians, clamped to [`MIN_PITCH`]`..=`[`MAX_PITCH`].
    pub pitch: f32,
    /// Distance from the eye to the target, clamped to [`MIN_DISTANCE`]`..=`[`MAX_DISTANCE`].
    pub distance: f32,
    /// World-space point the camera orbits and looks at.
    pub target: [f32; 3],
    /// Vertical field of view in radians, clamped to [`MIN_FOV_Y`]`..=`[`MAX_FOV_Y`].
    pub fov_y: f32,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            yaw: DEFAULT_YAW,
            pitch: DEFAULT_PITCH,
            distance: DEFAULT_DISTANCE,
            target: DEFAULT_TARGET,
            fov_y: DEFAULT_FOV_Y,
        }
    }
}

impl OrbitCamera {
    /// Returns the world-space eye position implied by the orbit parameters.
    #[must_use]
    pub fn eye(&self) -> [f32; 3] {
        let (yaw_sin, yaw_cos) = self.yaw.sin_cos();
        let (pitch_sin, pitch_cos) = self.pitch.sin_cos();
        let offset = [pitch_cos * yaw_sin, pitch_sin, pitch_cos * yaw_cos];
        [
            self.target[0] + offset[0] * self.distance,
            self.target[1] + offset[1] * self.distance,
            self.target[2] + offset[2] * self.distance,
        ]
    }

    /// Returns the normalized direction the camera looks along (eye to target).
    #[must_use]
    pub fn forward(&self) -> [f32; 3] {
        let eye = self.eye();
        normalize([
            self.target[0] - eye[0],
            self.target[1] - eye[1],
            self.target[2] - eye[2],
        ])
    }

    /// Returns the normalized camera-right axis in world space.
    #[must_use]
    pub fn right(&self) -> [f32; 3] {
        normalize(cross(self.forward(), WORLD_UP))
    }

    /// Returns the normalized camera-up axis in world space.
    #[must_use]
    pub fn up(&self) -> [f32; 3] {
        cross(self.right(), self.forward())
    }

    /// Returns the view matrix (world space to camera space).
    #[must_use]
    pub fn view(&self) -> Mat4 {
        let eye = self.eye();
        let forward = self.forward();
        let right = self.right();
        let up = self.up();
        [
            [right[0], up[0], -forward[0], 0.0],
            [right[1], up[1], -forward[1], 0.0],
            [right[2], up[2], -forward[2], 0.0],
            [-dot(right, eye), -dot(up, eye), dot(forward, eye), 1.0],
        ]
    }

    /// Returns the perspective projection matrix for `aspect = width / height`.
    ///
    /// Non-finite or non-positive aspect ratios fall back to `1.0`, and a
    /// non-finite or out-of-range [`OrbitCamera::fov_y`] is clamped first, so
    /// this never produces NaN for a broken camera field.
    #[must_use]
    pub fn projection(&self, aspect: f32) -> Mat4 {
        let aspect = safe_aspect(aspect);
        let focal = 1.0 / self.tan_half_fov_y();
        // wgpu clip space maps the near plane to z = 0 and the far plane to
        // z = 1; the demo scene and picking math assume exactly that.
        [
            [focal / aspect, 0.0, 0.0, 0.0],
            [0.0, focal, 0.0, 0.0],
            [0.0, 0.0, FAR / (NEAR - FAR), -1.0],
            [0.0, 0.0, NEAR * FAR / (NEAR - FAR), 0.0],
        ]
    }

    /// Returns `projection(aspect) * view()`, ready to upload as a WGSL uniform.
    #[must_use]
    pub fn view_projection(&self, aspect: f32) -> Mat4 {
        mat4_mul(self.projection(aspect), self.view())
    }

    /// Applies one frame of user input and re-applies all camera clamps.
    ///
    /// Non-finite input components are ignored instead of poisoning the camera
    /// state. The operation is idempotent for idle input: applying
    /// [`CameraInput::default`] never changes the camera.
    pub fn apply(&mut self, input: CameraInput) {
        if input.orbit[0].is_finite() {
            self.yaw = wrap_angle(self.yaw + input.orbit[0]);
        }
        if input.orbit[1].is_finite() {
            self.pitch += input.orbit[1];
        }
        if input.zoom.is_finite() {
            // Multiplicative zoom so the zoom speed feels constant at any
            // distance; the factor is clamped so one huge event cannot flip the
            // camera inside out.
            self.distance *= (1.0 - input.zoom).clamp(0.05, 20.0);
        }
        let pan = [finite_or_zero(input.pan[0]), finite_or_zero(input.pan[1])];
        if pan != [0.0, 0.0] {
            // Convert normalized pan into world units at the target plane. The
            // viewport is `2 * tan(fov/2)` world units tall per unit distance.
            let scale = 2.0 * self.distance * self.tan_half_fov_y();
            let right = self.right();
            let up = self.up();
            self.target = [
                self.target[0] + scale * (pan[1] * up[0] - pan[0] * right[0]),
                self.target[1] + scale * (pan[1] * up[1] - pan[0] * right[1]),
                self.target[2] + scale * (pan[1] * up[2] - pan[0] * right[2]),
            ];
        }
        self.clamp_to_limits();
    }

    /// Returns `tan(fov_y / 2)` after clamping the field of view.
    ///
    /// The shader receives this value and uses it to reconstruct the primary
    /// ray for each pixel; picking uses the same value so the CPU ray and the
    /// rendered ray agree exactly.
    pub(crate) fn tan_half_fov_y(&self) -> f32 {
        (effective_fov_y(self.fov_y) * 0.5).tan()
    }

    fn clamp_to_limits(&mut self) {
        if !self.yaw.is_finite() {
            self.yaw = DEFAULT_YAW;
        }
        self.yaw = wrap_angle(self.yaw);
        if !self.pitch.is_finite() {
            self.pitch = DEFAULT_PITCH;
        }
        if !self.distance.is_finite() {
            self.distance = DEFAULT_DISTANCE;
        }
        self.pitch = self.pitch.clamp(MIN_PITCH, MAX_PITCH);
        self.distance = self.distance.clamp(MIN_DISTANCE, MAX_DISTANCE);
        self.fov_y = effective_fov_y(self.fov_y);
        for (slot, value) in self.target.iter_mut().enumerate() {
            if !value.is_finite() {
                *value = DEFAULT_TARGET[slot];
            }
        }
        // Keep the eye above the demo scene's ground plane; see module docs.
        self.target[1] = self.target[1].max(0.0);
    }
}

/// One frame of orbit-camera input, in the units the app's input handler
/// produces.
///
/// * [`CameraInput::orbit`] is a delta in radians: `x` adds to
///   [`OrbitCamera::yaw`], `y` adds to [`OrbitCamera::pitch`] (positive raises
///   the eye above the target).
/// * [`CameraInput::pan`] is a delta in normalized viewport units (fraction of
///   the viewport height; `y` positive is screen-down). Positive `x` drags the
///   scene right, positive `y` drags it down, matching a grab-and-drag
///   interaction.
/// * [`CameraInput::zoom`] is a multiplicative factor: positive zooms in
///   (`distance *= 1 - zoom`), negative zooms out.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CameraInput {
    /// Orbit delta in radians, as `[yaw, pitch]`.
    pub orbit: [f32; 2],
    /// Pan delta in normalized viewport units, as `[x, y]`.
    pub pan: [f32; 2],
    /// Zoom delta; positive zooms in, negative zooms out.
    pub zoom: f32,
}

impl CameraInput {
    /// Input that leaves the camera unchanged.
    pub const NONE: Self = Self {
        orbit: [0.0; 2],
        pan: [0.0; 2],
        zoom: 0.0,
    };

    /// Returns `true` when this input cannot change the camera.
    #[must_use]
    pub fn is_idle(&self) -> bool {
        *self == Self::NONE
    }
}

/// Clamps a field of view into the range the camera supports.
fn effective_fov_y(fov_y: f32) -> f32 {
    if fov_y.is_finite() {
        fov_y.clamp(MIN_FOV_Y, MAX_FOV_Y)
    } else {
        DEFAULT_FOV_Y
    }
}

/// Sanitizes an aspect ratio for the projection matrix.
fn safe_aspect(aspect: f32) -> f32 {
    if aspect.is_finite() && aspect > 0.0 {
        aspect
    } else {
        1.0
    }
}

/// Wraps an angle into `[-PI, PI)` so yaw never grows without bound.
///
/// Angles that are already in range are returned untouched, which keeps
/// [`OrbitCamera::apply`] exactly idempotent for idle input.
fn wrap_angle(angle: f32) -> f32 {
    if (-PI..PI).contains(&angle) {
        angle
    } else {
        (angle + PI).rem_euclid(TAU) - PI
    }
}

fn finite_or_zero(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize(vector: [f32; 3]) -> [f32; 3] {
    let length_squared = dot(vector, vector);
    if length_squared.is_finite() && length_squared > 1.0e-20 {
        let inverse_length = 1.0 / length_squared.sqrt();
        [
            vector[0] * inverse_length,
            vector[1] * inverse_length,
            vector[2] * inverse_length,
        ]
    } else {
        [0.0, 0.0, -1.0]
    }
}

/// Multiplies two column-major matrices (`a * b`).
fn mat4_mul(a: Mat4, b: Mat4) -> Mat4 {
    let mut result = [[0.0_f32; 4]; 4];
    for (column, out_column) in result.iter_mut().enumerate() {
        for (row, out_value) in out_column.iter_mut().enumerate() {
            *out_value = a[0][row] * b[column][0]
                + a[1][row] * b[column][1]
                + a[2][row] * b[column][2]
                + a[3][row] * b[column][3];
        }
    }
    result
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

    fn assert_vector_close(actual: [f32; 3], expected: [f32; 3], tolerance: f32) {
        for axis in 0..3 {
            assert_close(actual[axis], expected[axis], tolerance);
        }
    }

    fn length(vector: [f32; 3]) -> f32 {
        dot(vector, vector).sqrt()
    }

    /// Applies a column-major matrix to a point, returning homogeneous clip space.
    fn transform_point(matrix: Mat4, point: [f32; 3]) -> [f32; 4] {
        let row_0 = matrix[0][0] * point[0]
            + matrix[1][0] * point[1]
            + matrix[2][0] * point[2]
            + matrix[3][0];
        let row_1 = matrix[0][1] * point[0]
            + matrix[1][1] * point[1]
            + matrix[2][1] * point[2]
            + matrix[3][1];
        let row_2 = matrix[0][2] * point[0]
            + matrix[1][2] * point[1]
            + matrix[2][2] * point[2]
            + matrix[3][2];
        let row_3 = matrix[0][3] * point[0]
            + matrix[1][3] * point[1]
            + matrix[2][3] * point[2]
            + matrix[3][3];
        [row_0, row_1, row_2, row_3]
    }

    #[test]
    fn default_camera_is_framed_and_clamped() {
        let camera = OrbitCamera::default();
        assert!(camera.pitch >= MIN_PITCH && camera.pitch <= MAX_PITCH);
        assert!(camera.distance >= MIN_DISTANCE && camera.distance <= MAX_DISTANCE);
        assert!(camera.fov_y >= MIN_FOV_Y && camera.fov_y <= MAX_FOV_Y);

        let to_eye = [
            camera.eye()[0] - camera.target[0],
            camera.eye()[1] - camera.target[1],
            camera.eye()[2] - camera.target[2],
        ];
        assert_close(length(to_eye), camera.distance, 1.0e-4);
        assert!(
            camera.eye()[1] > 0.0,
            "default eye must be above the ground plane"
        );
    }

    #[test]
    fn view_matrix_places_eye_at_origin_and_target_down_negative_z() {
        let camera = OrbitCamera::default();
        let view = camera.view();

        let eye_in_view = transform_point(view, camera.eye());
        assert_vector_close(
            [eye_in_view[0], eye_in_view[1], eye_in_view[2]],
            [0.0; 3],
            1.0e-4,
        );
        assert_close(eye_in_view[3], 1.0, 1.0e-5);

        // The camera looks down its local -z axis, so the target sits at
        // (0, 0, -distance) in view space.
        let target_in_view = transform_point(view, camera.target);
        assert_close(target_in_view[0], 0.0, 1.0e-4);
        assert_close(target_in_view[1], 0.0, 1.0e-4);
        assert_close(target_in_view[2], -camera.distance, 1.0e-4);
    }

    #[test]
    fn view_projection_puts_target_at_ndc_center_with_depth_in_unit_range() {
        let camera = OrbitCamera::default();
        let clip = transform_point(camera.view_projection(16.0 / 9.0), camera.target);
        assert!(clip[3] > 0.0, "target must be in front of the camera");
        let ndc = [clip[0] / clip[3], clip[1] / clip[3], clip[2] / clip[3]];
        assert_close(ndc[0], 0.0, 1.0e-4);
        assert_close(ndc[1], 0.0, 1.0e-4);
        assert!(
            (0.0..=1.0).contains(&ndc[2]),
            "ndc depth {0} must be in [0, 1]",
            ndc[2]
        );
    }

    #[test]
    fn view_projection_maps_near_and_far_planes_to_unit_depth() {
        let camera = OrbitCamera::default();
        let view_projection = camera.view_projection(1.5);
        let eye = camera.eye();
        let forward = camera.forward();

        let near_point = [
            eye[0] + forward[0] * NEAR,
            eye[1] + forward[1] * NEAR,
            eye[2] + forward[2] * NEAR,
        ];
        let near_clip = transform_point(view_projection, near_point);
        assert_close(near_clip[2] / near_clip[3], 0.0, 1.0e-3);

        let far_point = [
            eye[0] + forward[0] * FAR,
            eye[1] + forward[1] * FAR,
            eye[2] + forward[2] * FAR,
        ];
        let far_clip = transform_point(view_projection, far_point);
        assert_close(far_clip[2] / far_clip[3], 1.0, 1.0e-3);
    }

    #[test]
    fn aspect_ratio_only_scales_the_horizontal_projection_term() {
        let camera = OrbitCamera::default();
        let wide = camera.projection(2.0);
        let square = camera.projection(1.0);
        assert_close(wide[0][0], square[0][0] * 0.5, 1.0e-6);
        assert_close(wide[1][1], square[1][1], 1.0e-6);
        assert_close(wide[2][2], square[2][2], 1.0e-6);
    }

    #[test]
    fn broken_aspect_and_fov_never_produce_nan() {
        let mut camera = OrbitCamera::default();
        for fov in [0.0, -1.0, 10.0, f32::NAN] {
            camera.fov_y = fov;
            for aspect in [0.0, -3.0, f32::NAN, f32::INFINITY] {
                let view_projection = camera.view_projection(aspect);
                for column in view_projection {
                    for value in column {
                        assert!(
                            value.is_finite(),
                            "non-finite projection entry for fov {fov}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn camera_basis_is_orthonormal_and_right_handed() {
        let camera = OrbitCamera::default();
        let right = camera.right();
        let up = camera.up();
        let forward = camera.forward();
        assert_close(length(right), 1.0, 1.0e-5);
        assert_close(length(up), 1.0, 1.0e-5);
        assert_close(length(forward), 1.0, 1.0e-5);
        assert_close(dot(right, up), 0.0, 1.0e-5);
        assert_close(dot(right, forward), 0.0, 1.0e-5);
        assert_close(dot(up, forward), 0.0, 1.0e-5);
        // right-handed: right x up == -forward? For a look-at camera the
        // convention is right x up = -forward (forward is -z of the basis).
        let cross_product = cross(right, up);
        assert_vector_close(
            cross_product,
            [-forward[0], -forward[1], -forward[2]],
            1.0e-5,
        );
    }

    #[test]
    fn pitch_is_clamped_at_both_ends() {
        let mut camera = OrbitCamera::default();
        camera.apply(CameraInput {
            orbit: [0.0, 100.0],
            ..CameraInput::default()
        });
        assert_close(camera.pitch, MAX_PITCH, 1.0e-6);

        let mut camera = OrbitCamera::default();
        camera.apply(CameraInput {
            orbit: [0.0, -100.0],
            ..CameraInput::default()
        });
        assert_close(camera.pitch, MIN_PITCH, 1.0e-6);
    }

    #[test]
    fn distance_is_clamped_at_both_ends() {
        let mut camera = OrbitCamera::default();
        camera.apply(CameraInput {
            zoom: 100.0,
            ..CameraInput::default()
        });
        assert_close(camera.distance, MIN_DISTANCE, 1.0e-4);

        let mut camera = OrbitCamera::default();
        camera.apply(CameraInput {
            zoom: -100.0,
            ..CameraInput::default()
        });
        assert_close(camera.distance, MAX_DISTANCE, 1.0e-4);
    }

    #[test]
    fn zoom_is_multiplicative_and_moves_the_eye_closer() {
        let mut camera = OrbitCamera {
            distance: 4.0,
            ..OrbitCamera::default()
        };
        camera.apply(CameraInput {
            zoom: 0.25,
            ..CameraInput::default()
        });
        assert_close(camera.distance, 3.0, 1.0e-5);

        let to_eye = [
            camera.eye()[0] - camera.target[0],
            camera.eye()[1] - camera.target[1],
            camera.eye()[2] - camera.target[2],
        ];
        assert_close(length(to_eye), 3.0, 1.0e-4);
    }

    #[test]
    fn yaw_wraps_into_pi_range_and_stays_bounded() {
        let mut camera = OrbitCamera::default();
        camera.apply(CameraInput {
            orbit: [100.0, 0.0],
            ..CameraInput::default()
        });
        assert!(
            (-PI..PI).contains(&camera.yaw),
            "yaw {} must be wrapped to [-PI, PI)",
            camera.yaw
        );
        camera.apply(CameraInput {
            orbit: [-250.0, 0.0],
            ..CameraInput::default()
        });
        assert!((-PI..PI).contains(&camera.yaw));
    }

    #[test]
    fn pan_moves_the_target_in_the_camera_plane() {
        let mut camera = OrbitCamera {
            yaw: 0.0,
            pitch: 0.5,
            distance: 4.0,
            target: [0.0, 0.5, 0.0],
            fov_y: DEFAULT_FOV_Y,
        };
        let right = camera.right();
        let up = camera.up();
        let scale = 2.0 * camera.distance * camera.tan_half_fov_y();
        let pan = [0.25_f32, 0.5_f32];

        camera.apply(CameraInput {
            pan,
            ..CameraInput::default()
        });

        let expected = [
            -pan[0] * scale * right[0] + pan[1] * scale * up[0],
            -pan[0] * scale * right[1] + pan[1] * scale * up[1],
            -pan[0] * scale * right[2] + pan[1] * scale * up[2],
        ];
        assert_vector_close(
            [
                camera.target[0] - 0.0,
                camera.target[1] - 0.5,
                camera.target[2] - 0.0,
            ],
            expected,
            1.0e-4,
        );
        // At yaw 0, right is +x: dragging the scene right must move the target
        // left, and dragging down must move the target up.
        assert!(camera.target[0] < 0.0);
        assert!(camera.target[1] > 0.5);
    }

    #[test]
    fn target_is_kept_above_the_ground_plane() {
        let mut camera = OrbitCamera::default();
        camera.apply(CameraInput {
            pan: [0.0, -100.0],
            ..CameraInput::default()
        });
        assert!(camera.target[1] >= 0.0);
        assert!(camera.eye()[1] >= 0.0);
    }

    #[test]
    fn non_finite_input_is_ignored() {
        let mut camera = OrbitCamera {
            yaw: 0.3,
            pitch: 0.4,
            distance: 4.0,
            target: [0.0, 0.5, 0.0],
            fov_y: DEFAULT_FOV_Y,
        };
        let before = camera;
        camera.apply(CameraInput {
            orbit: [f32::NAN, f32::INFINITY],
            pan: [f32::NEG_INFINITY, f32::NAN],
            zoom: f32::NAN,
        });
        assert_eq!(camera, before);
    }

    #[test]
    fn idle_input_never_changes_the_camera() {
        let mut camera = OrbitCamera::default();
        let before = camera;
        camera.apply(CameraInput::default());
        assert_eq!(camera, before);
        assert!(CameraInput::default().is_idle());
        assert!(CameraInput::NONE.is_idle());
        assert!(
            !CameraInput {
                zoom: 1.0,
                ..CameraInput::default()
            }
            .is_idle()
        );
    }

    #[test]
    fn orbit_is_cumulative() {
        let mut stepwise = OrbitCamera::default();
        stepwise.apply(CameraInput {
            orbit: [0.1, 0.05],
            ..CameraInput::default()
        });
        stepwise.apply(CameraInput {
            orbit: [0.1, 0.05],
            ..CameraInput::default()
        });

        let mut combined = OrbitCamera::default();
        combined.apply(CameraInput {
            orbit: [0.2, 0.1],
            ..CameraInput::default()
        });

        assert_close(stepwise.yaw, combined.yaw, 1.0e-5);
        assert_close(stepwise.pitch, combined.pitch, 1.0e-5);
    }

    #[test]
    fn every_clamped_state_keeps_the_eye_outside_the_ground() {
        let mut camera = OrbitCamera::default();
        for pitch in [MIN_PITCH, 0.5, 1.0, MAX_PITCH] {
            for distance in [MIN_DISTANCE, 5.0, MAX_DISTANCE] {
                for target_y in [0.0, 0.5, 3.0] {
                    camera.pitch = pitch;
                    camera.distance = distance;
                    camera.target = [0.0, target_y, 0.0];
                    assert!(
                        camera.eye()[1] >= 0.0,
                        "eye below ground for pitch {pitch}, distance {distance}, target {target_y}"
                    );
                }
            }
        }
    }
}
