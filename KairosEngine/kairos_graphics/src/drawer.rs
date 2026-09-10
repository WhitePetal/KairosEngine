//! The drawer: what a view's frame buffer is made of, and how the world's
//! entities become entries in it.
//!
//! Two halves of one concept:
//!
//! - [`DrawCommand`] — one mesh instance to draw this frame, with everything the
//!   play layer needs (the world matrix included) already computed, so draining
//!   a buffer into a
//!   [`GraphicsCommand`](crate::graphics_graph::GraphicsCommand) is a
//!   copy with no world access and no transform math on the window side;
//! - the drawers — functions that append this frame's entries to *one* view's
//!   buffer ([`draw_meshes`] is the default one). A view's extract system calls
//!   its drawer once per frame, after clearing the buffer.
//!
//! Drawers are deliberately per view and per call: drawing into a view is the
//! view's own business, so nothing here writes two views at once (`#161` Q2).
//! Swapping what is drawn means calling a different drawer from that view's
//! extract system; adding to it means calling one more drawer there.

use std::sync::Arc;

use kairos_ecs::system::Query;
use kairos_transform::LocalTransform;

use kairos_math::float4x4;

use crate::{
    assets::{AssetHandle, MaterialAssetsSystem, MeshAssetsSystem},
    lod_mesh_component::LODMesh,
    material_component::MaterialComponent,
};

/// One mesh instance to draw this frame.
///
/// The handles are copied straight off the entity's components and may still be
/// `Loading` — a drawer never consults the asset server; the render pipeline
/// skips instances whose assets are not ready yet and picks them up on a later
/// frame.
#[derive(Debug, Clone)]
pub struct DrawCommand {
    pub mesh: Arc<AssetHandle<MeshAssetsSystem>>,
    pub material: Arc<AssetHandle<MaterialAssetsSystem>>,
    /// local → world, computed at draw-list build time.
    ///
    /// Today this is `LocalTransform::compute_local_matrix()` (every scene is a
    /// root scene); once transform propagation lands it becomes the entity's
    /// propagated world matrix and this field does not change shape.
    pub local_to_world: float4x4,
}

/// The default drawer: appends one entry per
/// `(LocalTransform, LODMesh, MaterialComponent)` entity to `draws`.
///
/// The caller — the view's extract system — has already cleared `draws` and
/// established that the view renders this frame, so this only has the world's
/// mesh entities left to answer for.
///
/// Asset readiness is deliberately not consulted: the handles may still be
/// `Loading`, and the pipeline skips unready instances until they resolve.
/// Draw-list building must not depend on the asset server (an `Engine` field,
/// not a world resource).
pub fn draw_meshes(
    draws: &mut Vec<DrawCommand>,
    meshes: &Query<(&LocalTransform, &LODMesh, &MaterialComponent)>,
) {
    for (transform, mesh, material) in meshes.iter() {
        draws.push(DrawCommand {
            mesh: mesh.lod0.clone(),
            material: material.material.clone(),
            local_to_world: transform.compute_local_matrix(),
        });
    }
}
