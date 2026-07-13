use std::path::Path;

use crate::{
    asset_loader::assets::AssetsServer,
    kairos_editor::{
        asset_registry::AssetKind,
        ui::inspector::{Inspector, directory::DirectoryInspector, toml::TomlTableInspector},
    },
};

pub struct InspectorCreater {}

impl InspectorCreater {
    pub fn create_from_asseet_kind(
        asset_kind: AssetKind,
        path: &Path,
        assets_server: &mut AssetsServer,
    ) -> Result<Box<dyn Inspector>, Box<dyn std::error::Error>> {
        match asset_kind {
            AssetKind::Directory => Ok(Box::new(DirectoryInspector::create(path, assets_server)?)),
            AssetKind::Texture => todo!(),
            AssetKind::Mesh => todo!(),
            AssetKind::Material => todo!(),
            AssetKind::Audio => todo!(),
            AssetKind::Shader => todo!(),
            AssetKind::GenericAsset => todo!(),
            AssetKind::Script => todo!(),
            AssetKind::Document => todo!(),
            AssetKind::Toml => Ok(Box::new(TomlTableInspector::create(path, assets_server)?)),
            AssetKind::Unknown => todo!(),
        }
    }
}
