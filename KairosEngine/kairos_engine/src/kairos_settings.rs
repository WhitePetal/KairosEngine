// ============================================================
// EngineSettings — top-level project config
// ============================================================

use serde::{Deserialize, Serialize};

use crate::graphics::texture_format::TextureCompressionConfig;

/// Top-level engine configuration loaded from `Preferences/Engine/KairosEngine.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineSettings {
    pub texture_compression: TextureCompressionConfig,
}
