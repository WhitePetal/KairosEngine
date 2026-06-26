use std::sync::Arc;

use crate::{
    asset_loader::assets::{AssetHandle, MeshAssetsSystem},
    ecs::component::Component,
};

#[derive(Debug)]
pub struct LODMesh {
    pub lod0: Arc<AssetHandle<MeshAssetsSystem>>,
}
impl Component for LODMesh {}

impl LODMesh {
    pub fn new(lod0: Arc<AssetHandle<MeshAssetsSystem>>) -> Self {
        Self { lod0 }
    }
}
