use std::sync::Arc;

use crate::asset_loader::assets::{AssetHandle, MeshAssetsSystem};

pub struct LODMeshComponent {
    pub lod0: Arc<AssetHandle<MeshAssetsSystem>>,
}
