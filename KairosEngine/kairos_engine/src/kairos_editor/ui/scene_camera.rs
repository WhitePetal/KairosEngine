use crate::math::{float3, float4x4};
use kairos_transform::LocalTransform;

use super::editor_camera::{OrbitState, OrbitTuning};

/// Editor orbit camera — pure data + pure math, no egui dependency.
#[derive(Debug, Clone, Copy)]
pub struct SceneCamera {
    pub fov: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,

    // orbit state
    orbit: OrbitState,

    // sensitivity
    tuning: OrbitTuning,
}

impl SceneCamera {
    pub fn new(
        eye: float3,
        pivot: float3,
        fov: f32,
        near: f32,
        far: f32,
        orbit_speed: f32,
        zoom_speed: f32,
        fly_acce_duration: f32,
        fly_min_speed: f32,
        fly_max_speed: f32,
        min_distance: f32,
        max_distance: f32,
    ) -> Self {
        Self {
            fov,
            aspect: 1.0,
            near,
            far,
            orbit: OrbitState::from_eye_pivot(eye, pivot),
            tuning: OrbitTuning {
                orbit_speed,
                zoom_speed,
                fly_acce_duration,
                fly_min_speed,
                fly_max_speed,
                min_distance,
                max_distance,
            },
        }
    }

    /// Drag delta in pixels → orbit around pivot.
    pub fn orbit(&mut self, dx: f32, dy: f32, dt: f32) {
        self.orbit.orbit(dx, dy, &self.tuning, dt);
    }

    /// Scroll delta → zoom in/out.
    pub fn zoom(&mut self, delta: f32, dt: f32) {
        self.orbit.zoom(delta, &self.tuning, dt);
    }

    /// WASD-style movement with smooth acceleration.
    /// Call each frame with `dt`; speed ramps up while keys are held.
    pub fn fly(&mut self, right_amount: f32, forward_amount: f32, dt: f32) {
        self.orbit
            .fly(right_amount, forward_amount, &self.tuning, dt);
    }

    /// World-space position derived from orbit state.
    pub fn position(&self) -> float3 {
        self.orbit.position()
    }

    /// Camera forward direction (toward pivot).
    pub fn forward(&self) -> float3 {
        self.orbit.forward()
    }

    /// Camera right direction.
    pub fn right(&self) -> float3 {
        self.orbit.right()
    }

    /// Camera up direction.
    pub fn _up(&self) -> float3 {
        self.orbit._up()
    }

    pub fn transform(&self) -> LocalTransform {
        self.orbit.transform()
    }

    pub fn view_projection(&self) -> float4x4 {
        let t = self.transform();
        let camera = crate::graphics::camera::Camera::new(self.fov, self.near, self.far);
        camera.get_view_projection_matrix(t, self.aspect)
    }
}
