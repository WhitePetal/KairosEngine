//! Editor-camera controller tests (T4 assertions 4–5 of the render/view
//! pipeline map).
//!
//! Everything runs on a bare [`World`] — `schedule::install` +
//! `graphics::install` + [`install`], one frame being
//! `run_schedule(Main)` + `clear_trackers()`. `Engine::new()` is deliberately
//! avoided (it builds an audio device), and nothing here touches wgpu, so the
//! suite is headless.
//!
//! The `First`-stage wall-clock advance is swapped for a scripted one so the
//! controller's per-frame `dt` is exact, exactly as the schedule rails' fixed
//! step tests do it.

use std::time::Duration;

use kairos_ecs::{
    entity::Entity,
    resource::Resource,
    schedule::Schedule,
    system::{Res, ResMut},
    world::World,
};
use kairos_transform::LocalTransform;

use super::{EditorCameraController, OrbitState, OrbitTuning, SceneViewInput, install};
use crate::{
    graphics::{
        self,
        camera::{Camera, CameraView},
        view_port::{SceneView, ViewportSize},
    },
    kairos_editor::schedule::{self, First, Main},
    math::{float2, float3, float4x4},
    time::Time,
};

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

/// The scripted per-frame virtual delta: 16 ms, so the orbit math's `dt * 60`
/// factor is `0.96` and every expected change is exact.
const FRAME_DT: Duration = Duration::from_millis(16);

/// The per-frame raw delta [`scripted_time_system`] feeds the virtual clock.
#[derive(Resource)]
struct ScriptedFrameDelta(Duration);

/// Advances the virtual clock by the scripted frame delta instead of sampling
/// the wall clock. Replaces the `First`-stage `time_system`.
fn scripted_time_system(mut time: ResMut<Time>, scripted: Res<ScriptedFrameDelta>) {
    time.update_with_raw_delta(scripted.0);
}

/// Schedules first (they create the stages), then the render rails, then the
/// controller — the same order `Engine::new` uses — with the wall-clock advance
/// swapped for a deterministic one.
fn boot() -> World {
    let mut world = World::new();
    schedule::install(&mut world);
    graphics::install(&mut world, schedule::Extract);
    install(&mut world);

    world.insert_resource(ScriptedFrameDelta(FRAME_DT));
    // Inserting a schedule with the `First` label replaces the booted one and
    // drops its wall-clock `time_system`.
    let mut first = Schedule::new(First);
    first.add_systems(scripted_time_system);
    world.add_schedule(first);
    world
}

/// One frame, exactly as `Engine::update` drives it.
fn run_frame(world: &mut World) {
    world.run_schedule(Main);
    world.clear_trackers();
}

/// The orbit knobs the tests move by. All non-zero, so a dropped knob cannot
/// pass by accident.
fn tuning() -> OrbitTuning {
    OrbitTuning {
        orbit_speed: 0.01,
        zoom_speed: 0.1,
        fly_acce_duration: 1.0,
        fly_min_speed: 0.1,
        fly_max_speed: 1.0,
        min_distance: 1.0,
        max_distance: 100.0,
    }
}

/// Spawns an editor camera entity and binds the Scene view to it at a real size.
fn spawn_editor_camera(world: &mut World) -> Entity {
    let orbit = OrbitState::from_eye_pivot(float3::new(0.0, 0.0, -10.0), float3::ZERO);
    let entity = world
        .spawn((
            orbit.transform(),
            Camera::new(45.0, 0.3, 100.0),
            EditorCameraController {
                orbit,
                tuning: tuning(),
            },
        ))
        .id();

    world.resource_mut::<SceneView>().camera = Some(entity);
    world.resource_mut::<SceneView>().size = ViewportSize::new(1280, 720);
    entity
}

fn transform_of(world: &World, entity: Entity) -> LocalTransform {
    *world
        .entity(entity)
        .get::<LocalTransform>()
        .expect("the editor camera carries a transform")
}

fn projection_of(world: &World, entity: Entity) -> Option<float4x4> {
    world
        .entity(entity)
        .get::<CameraView>()
        .expect("`Camera` requires `CameraView`")
        .view_projection
}

// ---------------------------------------------------------------------------
// The controller: applies input once, then clears it
// ---------------------------------------------------------------------------

/// The three input channels are accumulated by the handler; these helpers stand
/// in for that accumulation so the test can feed each one in isolation.
fn feed_orbit(world: &mut World) {
    world.resource_mut::<SceneViewInput>().orbit = float2::new(12.0, 0.0);
}

fn feed_zoom(world: &mut World) {
    world.resource_mut::<SceneViewInput>().zoom = 2.0;
}

fn feed_fly(world: &mut World) {
    world.resource_mut::<SceneViewInput>().fly = float2::new(1.0, 0.0);
}

/// Every input channel moves the camera, and the controller consumes the whole
/// buffer: one frame after each feed the camera has moved and the resource is
/// back to zero — so nothing replays on the next frame.
#[test]
fn input_moves_the_camera_once_then_stops() {
    let mut world = boot();
    let camera = spawn_editor_camera(&mut world);

    // Orbit (yaw), zoom (distance) and fly (pivot) are three independent knobs;
    // exercise each one and prove the buffer is cleared after each frame.
    for (name, feed) in [
        ("orbit", feed_orbit as fn(&mut World)),
        ("zoom", feed_zoom),
        ("fly", feed_fly),
    ] {
        let before = transform_of(&world, camera);
        feed(&mut world);
        run_frame(&mut world);

        assert_ne!(
            transform_of(&world, camera),
            before,
            "`{name}` input must move the camera"
        );
        let input = world.resource::<SceneViewInput>();
        assert_eq!(input.orbit, float2::ZERO, "orbit must be consumed");
        assert_eq!(input.zoom, 0.0, "zoom must be consumed");
        assert_eq!(input.fly, float2::ZERO, "fly must be consumed");
    }

    // No new input: the controller cleared the resource, so nothing replays.
    let settled = transform_of(&world, camera);
    run_frame(&mut world);
    assert_eq!(
        transform_of(&world, camera),
        settled,
        "a consumed delta must not replay on the next frame"
    );
}

/// An idle frame does not write `LocalTransform` at all: with no input and no
/// pending fly ramp the orbit state is unchanged, so the controller leaves the
/// transform — and its change-detection marker — alone.
#[test]
fn an_idle_frame_does_not_write_the_transform() {
    let mut world = boot();
    let camera = spawn_editor_camera(&mut world);

    // Park a sentinel. A controller that wrote every frame would overwrite it
    // with the orbit-derived pose.
    let sentinel = LocalTransform::default();
    *world
        .entity_mut(camera)
        .get_mut::<LocalTransform>()
        .expect("the editor camera carries a transform") = sentinel;

    run_frame(&mut world);

    assert_eq!(
        transform_of(&world, camera),
        sentinel,
        "an idle frame must not stamp a `Changed` marker on the transform"
    );
}

// ---------------------------------------------------------------------------
// The controller runs before the extract stage within one frame
// ---------------------------------------------------------------------------

/// The same frame's extract already reflects the pose the controller wrote.
///
/// This is the behavioural proof of the scheduling boundary: the controller
/// lives in `PostUpdate` and the extract rail in `Extract`, so no `.before()`
/// links them — yet the frame that consumed the input derives its projection
/// from the new pose, not the previous one.
#[test]
fn the_extract_of_the_same_frame_sees_the_controller_write() {
    let mut world = boot();
    let camera = spawn_editor_camera(&mut world);
    let start = transform_of(&world, camera);

    world.resource_mut::<SceneViewInput>().orbit = float2::new(12.0, 0.0);
    run_frame(&mut world);

    let moved = transform_of(&world, camera);
    assert_ne!(moved, start, "the controller wrote this frame");

    let expected = {
        let intrinsics = world
            .entity(camera)
            .get::<Camera>()
            .expect("the editor camera carries its intrinsics");
        intrinsics.get_view_projection_matrix(moved, 1280.0 / 720.0)
    };
    assert_eq!(
        projection_of(&world, camera),
        Some(expected),
        "the same frame's extract must already use the new pose"
    );
}
