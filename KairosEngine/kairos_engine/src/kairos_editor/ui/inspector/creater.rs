use std::path::Path;

use crate::{
    asset_loader::assets::AssetsServer,
    kairos_editor::{
        asset_registry::AssetKind,
        ui::inspector::{
            Inspector, audio::AudioInspector, code::CodeInspector, directory::DirectoryInspector,
            document::DocumentInspector, font::FontInspector, mesh::MeshInspector,
            shader::ShaderInspector, texture::TextureInspector, toml::TomlTableInspector,
            unknown::UnknownInspector,
        },
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
            AssetKind::Texture => Ok(Box::new(TextureInspector::create(path, assets_server)?)),
            AssetKind::Mesh => Ok(Box::new(MeshInspector::create(path, assets_server)?)),
            AssetKind::Material => todo!(),
            AssetKind::Audio => Ok(Box::new(AudioInspector::create(path, assets_server)?)),
            AssetKind::Shader => Ok(Box::new(ShaderInspector::create(path, assets_server)?)),
            AssetKind::Script => Ok(Box::new(CodeInspector::create(path, assets_server)?)),
            AssetKind::Document => Ok(Box::new(DocumentInspector::create(path, assets_server)?)),
            AssetKind::Toml => Ok(Box::new(TomlTableInspector::create(path, assets_server)?)),
            AssetKind::Font => Ok(Box::new(FontInspector::create(path, assets_server)?)),
            AssetKind::Unknown => Ok(Box::new(UnknownInspector::create(path, assets_server)?)),
        }
    }
}
