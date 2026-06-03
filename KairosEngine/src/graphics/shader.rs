use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta {
    source_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShaderAsset {
    meta: Meta,
    shader_string: String,
}
