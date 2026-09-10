//! What a view's frame buffer is made of: one [`DrawCommand`] per mesh
//! instance the view renders.
//!
//! The extract stage computes everything the play layer needs up front — the
//! world matrix included — so draining a buffer into a
//! [`GraphicsCommand`](crate::graphics::graphics_graph::GraphicsCommand) is a
//! copy, with no world access and no transform math on the window side.

use std::sync::Arc;

use crate::{
    asset_loader::assets::{AssetHandle, MaterialAssetsSystem, MeshAssetsSystem},
    math::float4x4,
};

/// One mesh instance to draw this frame.
///
/// The handles are copied straight off the entity's components and may still be
/// `Loading` — extraction never consults the asset server; the render pipeline
/// skips instances whose assets are not ready yet and picks them up on a later
/// frame.
#[derive(Debug, Clone)]
pub struct DrawCommand {
    pub mesh: Arc<AssetHandle<MeshAssetsSystem>>,
    pub material: Arc<AssetHandle<MaterialAssetsSystem>>,
    /// local → world, computed at extraction time.
    ///
    /// Today this is `LocalTransform::compute_local_matrix()` (every scene is a
    /// root scene); once transform propagation lands it becomes the entity's
    /// propagated world matrix and this field does not change shape.
    pub local_to_world: float4x4,
}
