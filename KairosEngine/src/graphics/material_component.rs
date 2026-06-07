use std::sync::Arc;

use crate::asset_loader::assets::{AssetHandle, MaterialAssetsSystem};

pub struct MaterialComponent {
    pub material: Arc<AssetHandle<MaterialAssetsSystem>>,
}
