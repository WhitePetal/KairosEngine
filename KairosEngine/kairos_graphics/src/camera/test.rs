use kairos_asset::next::Handle;
use kairos_ecs::{component::Component, world::World};
use kairos_transform::LocalTransform;

use super::{Camera, CameraView};
use crate::{
    lod_mesh_component::LODMesh, material::Material, material_component::MaterialComponent,
    mesh::Mesh,
};

/// Compile-time proof that the render trio is spawnable as `kairos_ecs`
/// components (and thus `Send + Sync + 'static`). This stops compiling if any
/// `#[derive(Component)]` on these types is dropped.
#[test]
fn render_types_implement_component() {
    fn assert_component<T: Component>() {}

    assert_component::<Camera>();
    assert_component::<CameraView>();
    assert_component::<LODMesh>();
    assert_component::<MaterialComponent>();
}

/// `Camera` declares `#[require(CameraView)]`, so a `(LocalTransform, Camera)`
/// tuple is enough to spawn a complete camera: the derived view component is
/// inserted automatically and starts out as "no view size known yet".
#[test]
fn spawning_a_camera_requires_a_camera_view() {
    let mut world = World::new();

    let entity = world
        .spawn((
            LocalTransform::default(),
            Camera::new(45.0, 0.3, 100.0),
        ))
        .id();

    let view = world
        .entity(entity)
        .get::<CameraView>()
        .expect("Camera requires CameraView");

    assert_eq!(view.physical_size, (0, 0));
    assert_eq!(view.aspect, 1.0);
    assert!(view.view_projection.is_none());
}

/// The mesh/material pair is spawnable alongside a transform — the shape the
/// demo scene and the extract query both rely on.
#[test]
fn a_mesh_and_material_pair_can_be_spawned_together() {
    let mut world = World::new();

    let entity = world
        .spawn((
            LocalTransform::default(),
            LODMesh::new(Handle::<Mesh>::default()),
            MaterialComponent::new(Handle::<Material>::default()),
        ))
        .id();

    assert!(world.entity(entity).get::<LODMesh>().is_some());
    assert!(world.entity(entity).get::<MaterialComponent>().is_some());
}

/// The view's aspect is an input to the projection, not a cached field:
/// doubling it halves the horizontal scale and leaves the vertical one alone.
#[test]
fn the_projection_honours_the_aspect_it_is_given() {
    let camera = Camera::new(45.0, 0.3, 100.0);

    let square = camera.get_projection_matrix(1.0);
    let wide = camera.get_projection_matrix(2.0);

    assert_eq!(wide.c0().x(), square.c0().x() / 2.0);
    assert_eq!(wide.c1().y(), square.c1().y());
}
