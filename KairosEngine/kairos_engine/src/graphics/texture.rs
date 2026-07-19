use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::graphics::texture_format::TextureFormat;

/// TOML-serializable form stored in `.texture` files.
///
/// Contains the source image path and the texture dimensions.
/// The pixel data is stored separately in the companion `.texture_bin` file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedTexture {
    /// Path to the source image file (e.g. PNG).
    pub source_path: PathBuf,
    /// Output width in pixels.
    pub width: u32,
    /// Output height in pixels.
    pub height: u32,
    /// GPU texture format.
    pub format: TextureFormat,
}

/// Runtime form held by `TextureAssetsSystem`.
///
/// Contains the resolved dimensions and the RGBA8 pixel data loaded
/// from `.texture_bin`.
#[derive(Debug, Clone)]
pub struct Texture {
    /// Texture width in pixels.
    pub width: u32,
    /// Texture height in pixels.
    pub height: u32,
    /// GPU texture format.
    pub format: TextureFormat,
    /// RGBA8 pixel data.
    pub data: Vec<u8>,
}
