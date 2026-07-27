use std::{path::PathBuf, sync::Arc};

use serde::{Deserialize, Serialize};

use crate::{
    asset_loader::assets::{AssetHandle, ShaderAssetsSystem, TextureAssetsSystem},
    graphics::render_state::RenderState,
};

#[cfg(test)]
mod test;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedMaterial {
    pub source_path: PathBuf,
    pub shader_path: PathBuf,
    pub render_state: RenderState,
    pub texture_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct Material {
    pub shader: Option<Arc<AssetHandle<ShaderAssetsSystem>>>,
    pub render_state: RenderState,
    pub texture: Option<Arc<AssetHandle<TextureAssetsSystem>>>,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            shader: None,
            render_state: RenderState::default(),
            texture: None,
        }
    }
}
