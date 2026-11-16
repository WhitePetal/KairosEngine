// ============================================================
// EngineSettings — top-level project config
// ============================================================

use serde::{Deserialize, Serialize};

use crate::graphics::texture::format::TextureCompressionConfig;

/// Top-level engine configuration loaded from `Preferences/Engine/KairosEngine.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineSettings {
    pub texture_compression: TextureCompressionConfig,
}

impl EngineSettings {
    /// Reads the project's settings file.
    ///
    /// Lives with the type, not in a host: every host reads the same file the
    /// same way, and the path constant is already the engine's
    /// ([`crate::kairos_paths::PATH_KAIROS_SETTINGS`]).
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let bytes = std::fs::read(crate::kairos_paths::PATH_KAIROS_SETTINGS)?;
        Ok(toml::from_slice(&bytes)?)
    }
}
