use crate::{
    math::{self, float3, float4x4},
    spatial::Transform,
};

/// Editor orbit camera — pure data + pure math, no egui dependency.
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
    pan_speed: f32,
    zoom_speed: f32,
    min_distance: f32,
    max_distance: f32,
}

impl SceneCamera {
    pub fn new(eye: float3, pivot: float3, fov: f32, aspect: f32, near: f32, far: f32) -> Self {
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
            aspect,
            near,
            far,
            pivot,
            distance,
            yaw,
            pitch,
            orbit_speed: 0.005,
            pan_speed: 0.01,
            zoom_speed: 0.1,
            min_distance: 0.3,
            max_distance: 100.0,
        }
    }

    /// Drag delta in pixels → orbit around pivot.
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * self.orbit_speed;
        self.pitch -= dy * self.orbit_speed;
        let limit = std::f32::consts::FRAC_PI_2 - 0.01;
        self.pitch = self.pitch.clamp(-limit, limit);
    }

    /// Drag delta in pixels → pan (move pivot in camera plane).
    pub fn pan(&mut self, dx: f32, dy: f32) {
        let right = self.right();
        let up = self.up();
        let scale = self.distance * self.pan_speed;
        self.pivot = self.pivot + right * (-dx * scale) + up * (dy * scale);
    }

    /// Scroll delta → zoom in/out.
    pub fn zoom(&mut self, delta: f32) {
        self.distance -= delta * self.zoom_speed;
        self.distance = self.distance.clamp(self.min_distance, self.max_distance);
    }

    /// WASD-style movement: `right` and `forward` in [-1, 0, 1].
    /// Moves the pivot in camera-local space, scaled by distance.
    pub fn fly(&mut self, right_amount: f32, forward_amount: f32) {
        let speed = self.distance * 2.0;
        self.pivot = self.pivot
            + self.right() * (right_amount * speed)
            + self.forward() * (forward_amount * speed);
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
    pub fn up(&self) -> float3 {
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
