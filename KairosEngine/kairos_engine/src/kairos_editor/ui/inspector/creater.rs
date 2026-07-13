use std::path::Path;

use crate::{
    asset_loader::assets::AssetsServer, kairos_editor::{
        asset_registry::AssetKind, ui::inspector::{Inspector, directory::DirectoryInspector, toml::TomlTableInspector},
    },
};

pub struct InspectorCreater {}

impl InspectorCreater {
    pub fn create_from_asseet_kind(
        asset_kind: AssetKind,
        path: &Path,
        assets_server: &mut AssetsServer,
    ) -> Box<dyn Inspector> {
        match asset_kind {
            AssetKind::Directory => Box::new(DirectoryInspector::create(path, assets_server)),
            AssetKind::Texture => todo!(),
            AssetKind::Mesh => todo!(),
            AssetKind::Material => todo!(),
            AssetKind::Audio => todo!(),
            AssetKind::Shader => todo!(),
            AssetKind::GenericAsset => todo!(),
            AssetKind::Script => todo!(),
            AssetKind::Document => todo!(),
            AssetKind::Toml => Box::new(TomlTableInspector::create(path, assets_server)),
            AssetKind::Unknown => todo!(),
        }
    }
}
