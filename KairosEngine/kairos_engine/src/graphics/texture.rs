use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub mod format;
pub mod sampler;

use format::TextureFormat;
use sampler::SamplerConfig;

/// TOML-serializable form stored in `.texture` files.
///
/// Contains the source image path, texture dimensions, format,
/// and sampler configuration.
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
    /// Sampler configuration (filter, wrap, mipmap, etc.).
    pub sampler: SamplerConfig,
}

/// Runtime form held by `TextureAssetsSystem`.
///
/// Contains the resolved dimensions, the RGBA8 pixel data loaded
/// from `.texture_bin`, and the sampler configuration.
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
    /// Sampler configuration (filter, wrap, mipmap, etc.).
    pub sampler: SamplerConfig,
}
