use std::sync::Arc;

use crate::asset_loader::assets::{AssetHandle, MeshAssetsSystem};
use kairos_ecs::component::Component;

#[derive(Component, Debug)]
pub struct LODMesh {
    pub lod0: Arc<AssetHandle<MeshAssetsSystem>>,
}

impl LODMesh {
    pub fn new(lod0: Arc<AssetHandle<MeshAssetsSystem>>) -> Self {
        Self { lod0 }
    }
}
