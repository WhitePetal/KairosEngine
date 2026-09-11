//! Extract-rail tests (T2 assertions 1–3 of the render/view pipeline map).
//!
//! Everything runs on a bare [`World`] — a one-stage schedule the test installs
//! its rail into, one frame being `run_schedule(TestExtract)` +
//! `clear_trackers()`. `Engine::new()` is deliberately avoided (it builds an
//! audio device), and the rails never touch wgpu, so the whole suite is
//! headless.
//!
//! Asset handles start as the default (unloaded) `Handle` — an id with no
//! resolved value. That is also the *point* of the first group — extraction must
//! not depend on the asset server.

use kairos_asset::next::Handle;
use kairos_ecs::{
    entity::Entity,
    schedule::{Schedule, ScheduleLabel, Schedules},
    world::World,
};
use kairos_math::{float3, quaternion};
use kairos_transform::LocalTransform;

use crate::{
    camera::{Camera, CameraView},
    drawer::DrawCommand,
    lod_mesh_component::LODMesh,
    material::Material,
    material_component::MaterialComponent,
    mesh::Mesh,
    view_port::{GameView, SceneView, ViewportSize},
};

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

/// The extract stage the rail is installed into. The real engine uses its
/// `kairos_editor::schedule::Extract`; a standalone crate cannot know about it,
/// so the test declares its own label — `install` takes the stage as a
/// parameter for exactly this reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct TestExtract;
impl ScheduleLabel for TestExtract {
    fn dyn_clone(&self) -> Box<dyn ScheduleLabel> {
        Box::new(*self)
    }
}

/// Creates the extract stage first, then installs the render rails that
/// register into it — the same order `Engine::new` uses.
fn boot() -> World {
    let mut world = World::new();
    world.init_resource::<Schedules>();
    world
        .resource_mut::<Schedules>()
        .insert(Schedule::new(TestExtract));
    crate::install(&mut world, TestExtract);
    world
}

/// One frame: run the extract stage, then clear the change trackers.
fn run_frame(world: &mut World) {
    world.run_schedule(TestExtract);
    world.clear_trackers();
}

/// A deliberately odd transform: position, rotation and scale all non-trivial,
/// so a matrix that ignores any of the three cannot pass by accident.
fn test_transform() -> LocalTransform {
    LocalTransform::new(
        float3::new(1.0, 2.0, 3.0),
        quaternion::from_euler(float3::new(0.3, 0.7, -0.2)),
        float3::new(2.0, 2.0, 2.0),
    )
}

/// A camera transform at the given eye, looking at the origin.
fn camera_transform(eye: float3) -> LocalTransform {
    LocalTransform::look_at(eye, float3::ZERO, float3::UP)
}

/// Spawns a mesh instance: the exact tuple the demo scene and the extract
/// query agree on.
fn spawn_mesh_entity(world: &mut World, transform: LocalTransform) -> Entity {
    world
        .spawn((
            transform,
            LODMesh::new(Handle::<Mesh>::default()),
            MaterialComponent::new(Handle::<Material>::default()),
        ))
        .id()
}

/// Spawns a camera entity and returns it.
fn spawn_camera(world: &mut World, eye: float3) -> Entity {
    world
        .spawn((camera_transform(eye), Camera::new(45.0, 0.3, 100.0)))
        .id()
}

/// Opens both windows at the same size.
fn open_both_views(world: &mut World, size: ViewportSize) {
    world.resource_mut::<SceneView>().size = size;
    world.resource_mut::<GameView>().size = size;
}

/// The two views' buffers, for assertions that must hold for every view.
fn draws_of_every_view(world: &World) -> [&[DrawCommand]; 2] {
    [
        &world.resource::<SceneView>().draws,
        &world.resource::<GameView>().draws,
    ]
}

/// The view projection the extract stage derived onto `camera`.
fn projection_of(world: &World, camera: Entity) -> Option<kairos_math::float4x4> {
    world
        .entity(camera)
        .get::<CameraView>()
        .expect("a camera entity carries the required `CameraView`")
        .view_projection
}

/// The projection/matrix `Camera` and the camera's `LocalTransform` say it
/// should be — what the extract stage is expected to derive.
fn expected_projection(world: &World, camera: Entity, aspect: f32) -> kairos_math::float4x4 {
    let transform = *world
        .entity(camera)
        .get::<LocalTransform>()
        .expect("a camera entity carries a transform");
    world
        .entity(camera)
        .get::<Camera>()
        .expect("a camera entity carries its intrinsics")
        .get_view_projection_matrix(transform, aspect)
}

// ---------------------------------------------------------------------------
// The frame buffer: rebuilt from scratch every frame
// ---------------------------------------------------------------------------

/// A mesh entity lands in **every** view that renders this frame, as exactly
/// one entry per view, carrying the matrix the extract stage computed
/// (`compute_local_matrix`, so the play layer does no transform math).
#[test]
fn a_mesh_entity_yields_one_entry_per_renderable_view() {
    let mut world = boot();
    let transform = test_transform();
    spawn_mesh_entity(&mut world, transform);
    open_both_views(&mut world, ViewportSize::new(1280, 720));

    run_frame(&mut world);

    for draws in draws_of_every_view(&world) {
        assert_eq!(draws.len(), 1, "one mesh entity = one entry per view");
        assert_eq!(draws[0].local_to_world, transform.compute_local_matrix());
    }
}

/// The buffers are overwritten, not accumulated: despawning the entity leaves
/// them empty on the next frame.
#[test]
fn the_buffers_are_rebuilt_from_scratch_every_frame() {
    let mut world = boot();
    let entity = spawn_mesh_entity(&mut world, test_transform());
    open_both_views(&mut world, ViewportSize::new(1280, 720));

    run_frame(&mut world);
    run_frame(&mut world);
    for draws in draws_of_every_view(&world) {
        assert_eq!(draws.len(), 1, "a second frame must not accumulate");
    }

    world.despawn(entity);
    run_frame(&mut world);

    for draws in draws_of_every_view(&world) {
        assert!(draws.is_empty(), "the entry must not outlive its entity");
    }
}

/// Extraction never consults the asset server: a handle that will never
/// resolve still produces its entry, and the pipeline skips it later.
#[test]
fn entries_are_collected_while_the_handles_are_still_loading() {
    let mut world = boot();
    // `test_asset_handle` never resolves — the handle is `Loading` forever.
    spawn_mesh_entity(&mut world, test_transform());
    open_both_views(&mut world, ViewportSize::new(1280, 720));

    run_frame(&mut world);

    for draws in draws_of_every_view(&world) {
        assert_eq!(
            draws.len(),
            1,
            "an unloaded handle must not hold its entity out of the buffer"
        );
    }
}

// ---------------------------------------------------------------------------
// The view projection: derived per frame, per view, onto the camera entity
// ---------------------------------------------------------------------------

/// A view with a bound camera and a real size gets `projection × view` and
/// `width / height` derived onto that camera's `CameraView` — the single place
/// the view's projection is read from.
#[test]
fn a_bound_camera_derives_its_view_projection_and_aspect() {
    let mut world = boot();
    let camera = spawn_camera(&mut world, float3::new(0.0, 8.0, -16.0));
    world.resource_mut::<SceneView>().camera = Some(camera);
    world.resource_mut::<SceneView>().size = ViewportSize::new(1280, 720);

    run_frame(&mut world);

    let derived = *world
        .entity(camera)
        .get::<CameraView>()
        .expect("`Camera` requires `CameraView`");
    assert_eq!(derived.physical_size, (1280, 720));
    assert_eq!(derived.aspect, 1280.0 / 720.0);
    assert_eq!(
        derived.view_projection,
        Some(expected_projection(&world, camera, 1280.0 / 720.0))
    );
}

/// A zero-sized view is skipped whole: no buffer content, no projection — and
/// no division by zero on the way there.
#[test]
fn a_zero_sized_view_is_skipped_entirely() {
    let mut world = boot();
    let camera = spawn_camera(&mut world, float3::new(0.0, 8.0, -16.0));
    spawn_mesh_entity(&mut world, test_transform());
    world.resource_mut::<SceneView>().camera = Some(camera);
    // Closed window: the UI writes `size = 0`.
    world.resource_mut::<SceneView>().size = ViewportSize::new(0, 0);
    world.resource_mut::<GameView>().size = ViewportSize::new(1920, 1080);

    run_frame(&mut world);

    assert!(
        world.resource::<SceneView>().draws.is_empty(),
        "a closed window collects no draws"
    );
    assert_eq!(
        projection_of(&world, camera),
        None,
        "a closed window derives no projection"
    );

    // The sibling view is untouched by its neighbour's closed state — each
    // view is extracted on its own, not by one pass over both.
    assert_eq!(world.resource::<GameView>().draws.len(), 1);

    // …and the other way round.
    world.resource_mut::<SceneView>().size = ViewportSize::new(1920, 1080);
    world.resource_mut::<GameView>().size = ViewportSize::new(0, 0);
    run_frame(&mut world);

    assert_eq!(world.resource::<SceneView>().draws.len(), 1);
    assert!(world.resource::<GameView>().draws.is_empty());
    assert!(
        projection_of(&world, camera).is_some(),
        "reopening the scene window re-derives its projection"
    );
}

/// A projection derived on an earlier frame never survives into a frame that
/// cannot produce one — the stage starts by clearing every camera's.
#[test]
fn a_projection_is_reset_when_the_view_is_closed() {
    let mut world = boot();
    let camera = spawn_camera(&mut world, float3::new(0.0, 8.0, -16.0));
    world.resource_mut::<SceneView>().camera = Some(camera);
    world.resource_mut::<SceneView>().size = ViewportSize::new(1280, 720);

    run_frame(&mut world);
    assert!(projection_of(&world, camera).is_some());

    // Closed window (`size == 0`): the projection is dropped, not left over.
    world.resource_mut::<SceneView>().size = ViewportSize::new(0, 720);
    run_frame(&mut world);
    assert_eq!(projection_of(&world, camera), None);

    // Reopening re-derives it.
    world.resource_mut::<SceneView>().size = ViewportSize::new(1280, 720);
    run_frame(&mut world);
    assert_eq!(
        projection_of(&world, camera),
        Some(expected_projection(&world, camera, 1280.0 / 720.0))
    );
}

/// Unbinding a camera drops the derived state it was carrying: a camera no view
/// renders from cannot leave an earlier frame's projection behind for whoever
/// still holds the entity.
#[test]
fn a_projection_is_reset_when_its_view_unbinds_the_camera() {
    let mut world = boot();
    let camera = spawn_camera(&mut world, float3::new(0.0, 8.0, -16.0));
    world.resource_mut::<SceneView>().camera = Some(camera);
    world.resource_mut::<SceneView>().size = ViewportSize::new(1280, 720);

    run_frame(&mut world);
    assert!(projection_of(&world, camera).is_some());

    world.resource_mut::<SceneView>().camera = None;
    run_frame(&mut world);

    assert_eq!(
        projection_of(&world, camera),
        None,
        "a view that no longer renders this camera must not leave its projection behind"
    );
    assert_eq!(
        world.entity(camera).get::<CameraView>().expect("CameraView").aspect,
        1.0,
        "the camera's derived state is back to `CameraView::default`"
    );
}

/// A view with no camera bound (and a binding that outlives its entity) is
/// tolerated: the frame still runs, the buffer is still rebuilt.
#[test]
fn a_view_without_a_camera_does_not_panic() {
    let mut world = boot();
    spawn_mesh_entity(&mut world, test_transform());
    // Neither view binds a camera.
    open_both_views(&mut world, ViewportSize::new(1280, 720));

    run_frame(&mut world);
    for draws in draws_of_every_view(&world) {
        assert_eq!(draws.len(), 1, "draws are gated on view size, not on a camera");
    }

    // A binding that outlives its entity must not panic either.
    let camera = spawn_camera(&mut world, float3::new(0.0, 8.0, -16.0));
    world.resource_mut::<SceneView>().camera = Some(camera);
    run_frame(&mut world);
    assert!(projection_of(&world, camera).is_some());

    world.despawn(camera);
    run_frame(&mut world);

    // The frame survived the dangling binding, and the sibling view is
    // unaffected by it.
    assert_eq!(world.resource::<GameView>().draws.len(), 1);
}

/// The two views are independent containers: each derives its projection from
/// its own camera, with its own aspect, and neither reads the other's.
#[test]
fn each_view_derives_its_own_projection() {
    let mut world = boot();
    let scene_camera = spawn_camera(&mut world, float3::new(0.0, 8.0, -16.0));
    let game_camera = spawn_camera(&mut world, float3::new(12.0, 3.0, 4.0));

    world.resource_mut::<SceneView>().camera = Some(scene_camera);
    world.resource_mut::<SceneView>().size = ViewportSize::new(1280, 720);
    world.resource_mut::<GameView>().camera = Some(game_camera);
    world.resource_mut::<GameView>().size = ViewportSize::new(720, 1280);

    run_frame(&mut world);

    let scene = projection_of(&world, scene_camera).expect("scene view derives a projection");
    let game = projection_of(&world, game_camera).expect("game view derives a projection");

    assert_eq!(scene, expected_projection(&world, scene_camera, 1280.0 / 720.0));
    assert_eq!(game, expected_projection(&world, game_camera, 720.0 / 1280.0));
    assert_ne!(
        scene, game,
        "two cameras with different poses and aspects must not share a projection"
    );
}

/// Extraction writes the camera a view is *bound to* and nothing else: the
/// camera query is an index, not a sweep, so a camera in the world that no view
/// renders from keeps its cleared default instead of being derived "on the
/// side" by whichever view happens to be extracted.
#[test]
fn extraction_leaves_cameras_no_view_is_bound_to_alone() {
    let mut world = boot();
    let scene_camera = spawn_camera(&mut world, float3::new(0.0, 8.0, -16.0));
    let unbound_camera = spawn_camera(&mut world, float3::new(12.0, 3.0, 4.0));

    world.resource_mut::<SceneView>().camera = Some(scene_camera);
    world.resource_mut::<SceneView>().size = ViewportSize::new(1280, 720);
    // The Game view renders this frame but binds no camera at all.
    world.resource_mut::<GameView>().size = ViewportSize::new(1280, 720);

    run_frame(&mut world);

    assert_eq!(
        projection_of(&world, scene_camera),
        Some(expected_projection(&world, scene_camera, 1280.0 / 720.0)),
        "the bound camera is derived"
    );
    assert_eq!(
        projection_of(&world, unbound_camera),
        None,
        "an unbound camera must not be derived by someone else's view"
    );
    assert_eq!(
        world
            .entity(unbound_camera)
            .get::<CameraView>()
            .expect("CameraView")
            .physical_size,
        (0, 0),
        "an unbound camera stays at its cleared default"
    );
}
