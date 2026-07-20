use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub mod format;
pub mod sampler;

use format::TextureFormat;
use sampler::SamplerConfig;

/// Power-of-two size presets for texture dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::EnumIter)]
pub enum TextureMaxSize {
    Size2 = 2,
    Size4 = 4,
    Size8 = 8,
    Size16 = 16,
    Size32 = 32,
    Size64 = 64,
    Size128 = 128,
    Size256 = 256,
    Size512 = 512,
    Size1024 = 1024,
    Size2048 = 2048,
    Size4096 = 4096,
}

impl TextureMaxSize {
    pub fn as_u32(self) -> u32 {
        self as u32
    }
}

/// Pick the smallest `TextureMaxSize` >= `max(width, height)`.
pub fn find_texture_max_size(width: u32, height: u32) -> TextureMaxSize {
    use strum::IntoEnumIterator;
    let max_side = width.max(height);
    for size in TextureMaxSize::iter() {
        if size.as_u32() >= max_side {
            return size;
        }
    }
    TextureMaxSize::Size4096
}

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
/// Contains the resolved dimensions, the pixel data loaded
/// from `.texture_bin`, and the sampler configuration.
/// `data` is a mip-chain: `data[0]` = base level, `data[1..]` = coarser levels.
#[derive(Debug, Clone)]
pub struct Texture {
    /// Texture width in pixels.
    pub width: u32,
    /// Texture height in pixels.
    pub height: u32,
    /// GPU texture format.
    pub format: TextureFormat,
    /// RGBA8 pixel data per mip level. `data[0]` = base level.
    pub data: Vec<Vec<u8>>,
    /// Sampler configuration (filter, wrap, mipmap, etc.).
    pub sampler: SamplerConfig,
}
