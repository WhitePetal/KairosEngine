//! The editor camera: the orbit state and tuning it moves by, the controller
//! component an editor camera entity carries, and the per-frame system that
//! consumes the view's input and writes the resulting transform.
//!
//! The camera is a world entity like any other: [`EditorCameraController`]
//! carries the orbit state and tuning, a `LocalTransform` the derived pose and a
//! [`Camera`](crate::graphics::camera::Camera) the intrinsics. The Scene
//! window's `OpenSceneTab` handler spawns it on first open and never despawns it,
//! so it lives as long as the world and the view survives a close/reopen.
//!
//! The UI writes input into [`SceneViewInput`] (the window layer has no world
//! access while drawing); this module's controller consumes it in `PostUpdate`
//! and clears it, and the extract stage — a later schedule boundary — reads the
//! transform the controller just wrote. That ordering is a property of the
//! schedule, so no `.before(extract)` is needed here.

use kairos_ecs::{
    component::Component,
    resource::Resource,
    schedule::Schedules,
    system::{Query, Res, ResMut},
    world::World,
};
use kairos_transform::LocalTransform;

use crate::{
    kairos_editor::schedule::PostUpdate,
    math::{self, float2, float3},
    time::Time,
};

/// Orbit-camera state: the eye is derived from a pivot plus a spherical offset.
///
/// Pure data + pure math — no egui and no ECS dependency.
#[derive(Debug, Clone, Copy, PartialEq)]
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

    pub fn transform(&self) -> LocalTransform {
        LocalTransform::look_at(self.position(), self.pivot, float3::UP)
    }
}

/// The editor camera's orbit state and tuning, carried by the editor camera
/// entity.
///
/// This is the entity's identity as "the camera the Scene window drives": the
/// controller query matches on it (no marker needed — the game camera never
/// carries one), while `SceneView.camera` remains the authority on which entity
/// the view renders from.
#[derive(Component)]
pub struct EditorCameraController {
    pub orbit: OrbitState,
    pub tuning: OrbitTuning,
}

/// The camera input the Scene window accumulates this frame.
///
/// The window layer draws with `&Engine` and physically cannot write a world
/// resource, so it sends messages instead; the message handler accumulates the
/// deltas here and the controller consumes them. Accumulating (rather than
/// overwriting) matters: the window only sends while hovered, but this resource
/// outlives that, so the controller must clear it every frame or a stale delta
/// would replay as a runaway orbit.
#[derive(Resource)]
pub struct SceneViewInput {
    /// Accumulated drag delta, in pixels.
    pub orbit: float2,
    /// Accumulated scroll delta.
    pub zoom: f32,
    /// Accumulated `(right, forward)` movement, each in `{-1, 0, 1}`.
    pub fly: float2,
}

impl Default for SceneViewInput {
    fn default() -> Self {
        Self {
            orbit: float2::ZERO,
            zoom: 0.0,
            fly: float2::ZERO,
        }
    }
}

/// Installs the editor camera controller: the input resource it consumes and
/// the system that consumes it.
///
/// The system registers into [`PostUpdate`], before the extract stage. It needs
/// no `.before(extract)` because `Extract` is a separate schedule that runs
/// after `PostUpdate` — "the controller has run by the time the frame is read"
/// is a scheduling boundary, not a per-system promise.
///
/// # Panics
///
/// If the schedule rails are not installed yet: registering the controller into
/// a stage that does not exist is a bootstrap order bug.
pub fn install(world: &mut World) {
    log::debug!("installing the editor camera controller");

    world.init_resource::<SceneViewInput>();

    let mut schedules = world.get_resource_or_init::<Schedules>();
    schedules
        .get_mut(PostUpdate)
        .expect("the `PostUpdate` schedule must exist: install the schedule rails first")
        .add_systems(editor_camera_controller_system);
}

/// Applies this frame's accumulated input to every editor camera, then clears
/// the input.
///
/// The delta comes from [`Time`] (the schedule's per-frame virtual clock), not
/// from the messages: they carry only the increment, so a frame that arrives
/// between a hover and its handler cannot stretch it.
///
/// The input is cleared unconditionally — see [`SceneViewInput`] for why that is
/// correctness, not style. `LocalTransform` is written only when the orbit state
/// actually changed, so an idle frame does not stamp a `Changed` marker on the
/// transform the extract stage reads.
fn editor_camera_controller_system(
    mut input: ResMut<SceneViewInput>,
    time: Res<Time>,
    mut cameras: Query<(&mut EditorCameraController, &mut LocalTransform)>,
) {
    let dt = time.delta_time().as_secs_f32();
    let (orbit_delta, zoom_delta, fly_delta) = (input.orbit, input.zoom, input.fly);
    input.orbit = float2::ZERO;
    input.zoom = 0.0;
    input.fly = float2::ZERO;

    for (mut controller, mut transform) in cameras.iter_mut() {
        let previous = controller.orbit;
        let tuning = controller.tuning;
        controller
            .orbit
            .orbit(orbit_delta.x(), orbit_delta.y(), &tuning, dt);
        controller.orbit.zoom(zoom_delta, &tuning, dt);
        controller
            .orbit
            .fly(fly_delta.x(), fly_delta.y(), &tuning, dt);

        if controller.orbit != previous {
            *transform = controller.orbit.transform();
        }
    }
}

#[cfg(test)]
mod test;
