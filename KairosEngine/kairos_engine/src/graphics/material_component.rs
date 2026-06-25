use std::sync::Arc;

use crate::{
    asset_loader::assets::{AssetHandle, MaterialAssetsSystem},
    ecs::component::Component,
};

pub struct Material {
    pub material: Arc<AssetHandle<MaterialAssetsSystem>>,
}
impl Component for Material {}

impl Material {
    pub fn new(material: Arc<AssetHandle<MaterialAssetsSystem>>) -> Self {
        Self { material }
    }
}
