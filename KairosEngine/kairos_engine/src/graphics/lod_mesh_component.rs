use std::sync::Arc;

use crate::{
    asset_loader::assets::{AssetHandle, MeshAssetsSystem},
    ecs::component::Component,
};

#[derive(Debug)]
pub struct LODMeshComponent {
    pub lod0: Arc<AssetHandle<MeshAssetsSystem>>,
}
impl Component for LODMeshComponent {}

impl LODMeshComponent {
    pub fn new(lod0: Arc<AssetHandle<MeshAssetsSystem>>) -> Self {
        Self { lod0 }
    }
}
