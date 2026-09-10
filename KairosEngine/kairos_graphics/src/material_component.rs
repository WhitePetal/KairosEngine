use std::sync::Arc;

use crate::assets::{AssetHandle, MaterialAssetsSystem};
use kairos_ecs::component::Component;

#[derive(Component)]
pub struct MaterialComponent {
    pub material: Arc<AssetHandle<MaterialAssetsSystem>>,
}

impl MaterialComponent {
    pub fn new(material: Arc<AssetHandle<MaterialAssetsSystem>>) -> Self {
        Self { material }
    }
}
