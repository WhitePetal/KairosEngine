use std::{path::PathBuf, sync::Arc};

use serde::{Deserialize, Serialize};

use crate::asset_loader::assets::{AssetHandle, ShaderAssetsSystem, TextureAssetsSystem};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta {
    pub source_path: PathBuf,
    pub shader_path: PathBuf,
    pub texture_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct Material {
    pub texture: Option<Arc<AssetHandle<TextureAssetsSystem>>>,
    pub shader: Option<Arc<AssetHandle<ShaderAssetsSystem>>>,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            texture: None,
            shader: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MaterialAsset {
    pub meta: Meta,
    #[serde(skip)]
    pub material: Material,
}
