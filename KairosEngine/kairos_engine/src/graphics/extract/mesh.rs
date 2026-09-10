//! The default mesh render service: the draw producer that renders every
//! `(LocalTransform, LODMesh, MaterialComponent)` entity into every view.
//!
//! This is the kairos equivalent of bevy's `extract_meshes` — one ordinary
//! system on the extract rail, not a trait or a resource indirection.
//! Replacing it means registering a different system in `graphics::install`;
//! extending it means registering one more system that writes into the view
//! buffers after [`ResetViewBuffers`](super::ResetViewBuffers).

use kairos_ecs::system::{Query, ResMut};
use kairos_transform::LocalTransform;

use crate::graphics::{
    drawer::DrawCommand,
    lod_mesh_component::LODMesh,
    material_component::MaterialComponent,
    view_port::{GameView, SceneView, ViewResource},
};

/// Writes one draw entry per mesh entity into each view that renders this
/// frame.
///
/// Asset readiness is deliberately not consulted: the handles may still be
/// `Loading`, and the render pipeline skips unready instances until they
/// resolve. Extraction must not depend on the asset server (which is an
/// `Engine` field, not a world resource).
pub fn extract_meshes(
    meshes: Query<(&LocalTransform, &LODMesh, &MaterialComponent)>,
    mut scene_view: ResMut<SceneView>,
    mut game_view: ResMut<GameView>,
) {
    collect_into(&mut *scene_view, &meshes);
    collect_into(&mut *game_view, &meshes);
}

/// Appends this view's share of the mesh entities to its buffer.
///
/// The buffer is gated on the view's size alone — asset readiness is the
/// pipeline's business, not extraction's.
fn collect_into(
    view: &mut impl ViewResource,
    meshes: &Query<(&LocalTransform, &LODMesh, &MaterialComponent)>,
) {
    if !view.size().is_renderable() {
        return;
    }

    let draws = view.draws_mut();
    for (transform, mesh, material) in meshes.iter() {
        draws.push(DrawCommand {
            mesh: mesh.lod0.clone(),
            material: material.material.clone(),
            local_to_world: transform.compute_local_matrix(),
        });
    }
}
