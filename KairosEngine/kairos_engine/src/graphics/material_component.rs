use std::sync::Arc;

use crate::{
    asset_loader::assets::{AssetHandle, MaterialAssetsSystem},
    ecs::component::Component,
};

pub struct MaterialComponent {
    pub material: Arc<AssetHandle<MaterialAssetsSystem>>,
}
// TODO!
// impl Component for MaterialComponent {}

impl MaterialComponent {
    pub fn new(material: Arc<AssetHandle<MaterialAssetsSystem>>) -> Self {
        Self { material }
    }
}
