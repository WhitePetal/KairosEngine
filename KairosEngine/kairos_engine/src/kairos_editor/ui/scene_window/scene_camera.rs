use serde::{Deserialize, Serialize};

use crate::{
    math::{self, float3, float4x4},
    spatial::Transform,
};

/// Editor orbit camera — pure data + pure math, no egui dependency.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SceneCamera {
    pub fov: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,

    // orbit state
    pivot: float3,
    distance: f32,
    yaw: f32,
    pitch: f32,

    // sensitivity
    orbit_speed: f32,
    zoom_speed: f32,
    fly_acce_duration: f32,
    fly_min_speed: f32,
    fly_max_speed: f32,
    min_distance: f32,
    max_distance: f32,

    fly_timer: f32,
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
        let offset = eye - pivot;
        let distance = math::length(&offset);
        let forward = if distance > 0.001 {
            offset / distance
        } else {
            float3::new(0.0, 0.0, -1.0)
        };
        // forward = (cos(pitch)*sin(yaw), sin(pitch), -cos(pitch)*cos(yaw))
        let yaw = f32::atan2(forward.x(), -forward.z());
        let pitch = f32::asin(forward.y());

        Self {
            fov,
            aspect: 1.0,
            near,
            far,
            pivot,
            distance,
            yaw,
            pitch,
            orbit_speed,
            zoom_speed,
            fly_acce_duration,
            fly_min_speed,
            fly_max_speed,
            min_distance,
            max_distance,
            fly_timer: 0.0,
        }
    }

    /// Drag delta in pixels → orbit around pivot.
    pub fn orbit(&mut self, dx: f32, dy: f32, dt: f32) {
        self.yaw -= dx * self.orbit_speed * dt * 60.0;
        self.pitch -= dy * self.orbit_speed * dt * 60.0;
        let limit = std::f32::consts::FRAC_PI_2 - 0.01;
        self.pitch = self.pitch.clamp(-limit, limit);
    }

    /// Scroll delta → zoom in/out.
    pub fn zoom(&mut self, delta: f32, dt: f32) {
        self.distance -= delta * self.zoom_speed * dt * 60.0;
        self.distance = self.distance.clamp(self.min_distance, self.max_distance);
    }

    /// WASD-style movement with smooth acceleration.
    /// Call each frame with `dt`; speed ramps up while keys are held.
    pub fn fly(&mut self, right_amount: f32, forward_amount: f32, dt: f32) {
        let active = right_amount != 0.0 || forward_amount != 0.0;
        if active {
            self.fly_timer += dt;
        } else {
            self.fly_timer = 0.0;
        }
        let ramp = (self.fly_timer / self.fly_acce_duration).min(1.0); // smooth ramp
        let speed = self.distance * math::lerp(self.fly_min_speed, self.fly_max_speed, ramp);
        self.pivot = self.pivot
            + self.right() * (right_amount * speed * dt)
            + self.forward() * (forward_amount * speed * dt);
    }

    /// World-space position derived from orbit state.
    pub fn position(&self) -> float3 {
        let cp = self.pitch.cos();
        let sp = self.pitch.sin();
        let cy = self.yaw.cos();
        let sy = self.yaw.sin();
        self.pivot + float3::new(cp * sy, sp, -cp * cy) * self.distance
    }

    /// Camera forward direction (toward pivot).
    pub fn forward(&self) -> float3 {
        math::normalize(self.pivot - self.position())
    }

    /// Camera right direction.
    pub fn right(&self) -> float3 {
        math::normalize(math::cross(self.forward(), float3::UP))
    }

    /// Camera up direction.
    pub fn _up(&self) -> float3 {
        math::cross(self.right(), self.forward())
    }

    pub fn transform(&self) -> Transform {
        Transform::look_at(self.position(), self.pivot, float3::UP)
    }

    pub fn view_projection(&self) -> float4x4 {
        let t = self.transform();
        let camera =
            crate::graphics::camera::Camera::new(self.fov, self.aspect, self.near, self.far);
        camera.get_view_projection_matrix(t)
    }
}
