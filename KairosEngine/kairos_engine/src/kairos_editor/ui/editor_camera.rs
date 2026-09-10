use crate::math::{self, float3};
use kairos_transform::LocalTransform;

/// Orbit-camera state: the eye is derived from a pivot plus a spherical offset.
///
/// Pure data + pure math — no egui and no ECS dependency. `SceneCamera`
/// delegates to it today; the inspector previews and the editor camera entity
/// move onto it directly when `SceneCamera` retires.
#[derive(Debug, Clone, Copy)]
pub struct OrbitState {
    pub pivot: float3,
    pub distance: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub fly_timer: f32,
}

/// Speed/distance knobs the orbit math reads. Callers own their own values: the
/// editor window derives them from its style file, each inspector preview from
/// its own inspector style.
#[derive(Debug, Clone, Copy)]
pub struct OrbitTuning {
    pub orbit_speed: f32,
    pub zoom_speed: f32,
    pub fly_acce_duration: f32,
    pub fly_min_speed: f32,
    pub fly_max_speed: f32,
    pub min_distance: f32,
    pub max_distance: f32,
}

impl OrbitState {
    /// Places the eye at `eye` looking at `pivot`: solves `distance` and the
    /// yaw/pitch angles back out of the eye→pivot offset.
    pub fn from_eye_pivot(eye: float3, pivot: float3) -> Self {
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
            pivot,
            distance,
            yaw,
            pitch,
            fly_timer: 0.0,
        }
    }

    /// Drag delta in pixels → orbit around pivot.
    pub fn orbit(&mut self, dx: f32, dy: f32, tuning: &OrbitTuning, dt: f32) {
        self.yaw -= dx * tuning.orbit_speed * dt * 60.0;
        self.pitch -= dy * tuning.orbit_speed * dt * 60.0;
        let limit = std::f32::consts::FRAC_PI_2 - 0.01;
        self.pitch = self.pitch.clamp(-limit, limit);
    }

    /// Scroll delta → zoom in/out.
    pub fn zoom(&mut self, delta: f32, tuning: &OrbitTuning, dt: f32) {
        self.distance -= delta * tuning.zoom_speed * dt * 60.0;
        self.distance = self.distance.clamp(tuning.min_distance, tuning.max_distance);
    }

    /// WASD-style movement with smooth acceleration.
    /// Call each frame with `dt`; speed ramps up while keys are held.
    pub fn fly(&mut self, right_amount: f32, forward_amount: f32, tuning: &OrbitTuning, dt: f32) {
        let active = right_amount != 0.0 || forward_amount != 0.0;
        if active {
            self.fly_timer += dt;
        } else {
            self.fly_timer = 0.0;
        }
        let ramp = (self.fly_timer / tuning.fly_acce_duration).min(1.0); // smooth ramp
        let speed = self.distance * math::lerp(tuning.fly_min_speed, tuning.fly_max_speed, ramp);
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

    pub fn transform(&self) -> LocalTransform {
        LocalTransform::look_at(self.position(), self.pivot, float3::UP)
    }
}
