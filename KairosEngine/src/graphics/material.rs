use std::{path::PathBuf, sync::Arc};

use serde::{Deserialize, Serialize};

use crate::asset_loader::assets::{AssetHandle, TextureAssetsSystem};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta {
    source_path: PathBuf,
    texture_path: PathBuf,
    shader_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Material {
    texture: Option<Arc<AssetHandle<TextureAssetsSystem>>>,
    // shader: Arc<AssetHandle<S>>,
}

impl Default for Material {
    fn default() -> Self {
        Self { texture: None }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MaterialAsset {
    meta: Meta,
    #[serde(skip)]
    material: Material,
}
