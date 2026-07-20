use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use strum::EnumIter;

mod bc;
mod uncompressed;

// ============================================================
// TextureCompressionConfig — data-driven feature list
// ============================================================

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, PartialOrd, Eq, Ord)]
pub enum TextureCompressFeature {
    BC,
    ETC2,
    ASTC,
}

/// Data-driven compression config.
///
/// Each entry maps a feature-family name (e.g. "BC") to an enabled flag.
/// Adding a new compression family only requires updating the TOML file
/// and the `feature_to_wgpu` lookup below.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextureCompressionConfig {
    /// Feature-family → enabled.  Example: `{ "BC": true, "ETC2": true }`.
    pub features: BTreeMap<TextureCompressFeature, bool>,
}

impl TextureCompressionConfig {
    /// Whether `feature` is enabled in this config.
    pub fn is_enabled(&self, feature: TextureCompressFeature) -> bool {
        self.features.get(&feature).copied().unwrap_or(false)
    }

    /// Build the set of `wgpu::Features` to request from the adapter.
    /// Only features enabled in this config AND supported by `available`
    /// are included. Unsupported-but-enabled features log a warning.
    pub fn adapter_features(&self, available: wgpu::Features) -> wgpu::Features {
        let mut features = wgpu::Features::empty();

        for (feature, &enabled) in &self.features {
            if !enabled {
                continue;
            }
            match feature_to_wgpu(feature) {
                Some(wgpu_feature) => {
                    if available.contains(wgpu_feature) {
                        features |= wgpu_feature;
                    } else {
                        log::warn!(
                            "Texture compression ({:?}) enabled in config but not supported by adapter",
                            feature
                        );
                    }
                }
                None => {
                    log::warn!("Unknown compression feature in config: {:?}", feature);
                }
            }
        }

        features
    }
}

/// Map a feature-family name to its `wgpu::Features` bitflag.
pub fn feature_to_wgpu(feature: &TextureCompressFeature) -> Option<wgpu::Features> {
    match feature {
        TextureCompressFeature::BC => Some(wgpu::Features::TEXTURE_COMPRESSION_BC),
        TextureCompressFeature::ETC2 => Some(wgpu::Features::TEXTURE_COMPRESSION_ETC2),
        TextureCompressFeature::ASTC => Some(
            wgpu::Features::TEXTURE_COMPRESSION_ASTC | wgpu::Features::TEXTURE_COMPRESSION_ASTC_HDR,
        ),
    }
}

// ============================================================
// TextureFormat
// ============================================================

/// Project texture format — maps to `wgpu::TextureFormat` at the GPU boundary.
///
/// Only includes formats suitable for sampled 2D textures (no depth/stencil).
/// ASTC formats are flattened into individual variants for TOML serialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, EnumIter)]
pub enum TextureFormat {
    // ---- Uncompressed ----
    R8Unorm,
    R8Snorm,
    R8Uint,
    R8Sint,
    R16Uint,
    R16Sint,
    R16Float,
    Rg8Unorm,
    Rg8Snorm,
    Rg8Uint,
    Rg8Sint,
    R32Uint,
    R32Sint,
    R32Float,
    Rg16Uint,
    Rg16Sint,
    Rg16Float,
    Rgba8Unorm,
    Rgba8UnormSrgb,
    Rgba8Snorm,
    Rgba8Uint,
    Rgba8Sint,
    Bgra8Unorm,
    Bgra8UnormSrgb,
    Rgb10a2Unorm,
    Rg11b10Ufloat,
    Rg32Uint,
    Rg32Sint,
    Rg32Float,
    Rgba16Uint,
    Rgba16Sint,
    Rgba16Float,
    Rgba32Uint,
    Rgba32Sint,
    Rgba32Float,

    // ---- BC ----
    Bc1RgbaUnorm,
    Bc1RgbaUnormSrgb,
    Bc2RgbaUnorm,
    Bc2RgbaUnormSrgb,
    Bc3RgbaUnorm,
    Bc3RgbaUnormSrgb,
    Bc4RUnorm,
    Bc4RSnorm,
    Bc5RgUnorm,
    Bc5RgSnorm,
    Bc6hRgbUfloat,
    Bc6hRgbFloat,
    Bc7RgbaUnorm,
    Bc7RgbaUnormSrgb,

    // ---- ETC2 ----
    Etc2Rgb8Unorm,
    Etc2Rgb8UnormSrgb,
    Etc2Rgb8A1Unorm,
    Etc2Rgb8A1UnormSrgb,
    Etc2Rgba8Unorm,
    Etc2Rgba8UnormSrgb,
    EacR11Unorm,
    EacR11Snorm,
    EacRg11Unorm,
    EacRg11Snorm,

    // ---- ASTC ----
    Astc4x4Unorm,
    Astc4x4UnormSrgb,
    Astc4x4Hdr,
    Astc5x4Unorm,
    Astc5x4UnormSrgb,
    Astc5x4Hdr,
    Astc5x5Unorm,
    Astc5x5UnormSrgb,
    Astc5x5Hdr,
    Astc6x5Unorm,
    Astc6x5UnormSrgb,
    Astc6x5Hdr,
    Astc6x6Unorm,
    Astc6x6UnormSrgb,
    Astc6x6Hdr,
    Astc8x5Unorm,
    Astc8x5UnormSrgb,
    Astc8x5Hdr,
    Astc8x6Unorm,
    Astc8x6UnormSrgb,
    Astc8x6Hdr,
    Astc8x8Unorm,
    Astc8x8UnormSrgb,
    Astc8x8Hdr,
    Astc10x5Unorm,
    Astc10x5UnormSrgb,
    Astc10x5Hdr,
    Astc10x6Unorm,
    Astc10x6UnormSrgb,
    Astc10x6Hdr,
    Astc10x8Unorm,
    Astc10x8UnormSrgb,
    Astc10x8Hdr,
    Astc10x10Unorm,
    Astc10x10UnormSrgb,
    Astc10x10Hdr,
    Astc12x10Unorm,
    Astc12x10UnormSrgb,
    Astc12x10Hdr,
    Astc12x12Unorm,
    Astc12x12UnormSrgb,
    Astc12x12Hdr,
}

impl TextureFormat {
    pub fn supports_encoding(&self) -> bool {
        match self {
            TextureFormat::R8Unorm => true,
            TextureFormat::R8Snorm => true,
            TextureFormat::R8Uint => true,
            TextureFormat::R8Sint => true,
            TextureFormat::R16Uint => false,
            TextureFormat::R16Sint => false,
            TextureFormat::R16Float => false,
            TextureFormat::Rg8Unorm => false,
            TextureFormat::Rg8Snorm => false,
            TextureFormat::Rg8Uint => false,
            TextureFormat::Rg8Sint => false,
            TextureFormat::R32Uint => false,
            TextureFormat::R32Sint => false,
            TextureFormat::R32Float => false,
            TextureFormat::Rg16Uint => false,
            TextureFormat::Rg16Sint => false,
            TextureFormat::Rg16Float => false,
            TextureFormat::Rgba8Unorm => true,
            TextureFormat::Rgba8UnormSrgb => true,
            TextureFormat::Rgba8Snorm => false,
            TextureFormat::Rgba8Uint => false,
            TextureFormat::Rgba8Sint => false,
            TextureFormat::Bgra8Unorm => false,
            TextureFormat::Bgra8UnormSrgb => false,
            TextureFormat::Rgb10a2Unorm => false,
            TextureFormat::Rg11b10Ufloat => false,
            TextureFormat::Rg32Uint => false,
            TextureFormat::Rg32Sint => false,
            TextureFormat::Rg32Float => false,
            TextureFormat::Rgba16Uint => false,
            TextureFormat::Rgba16Sint => false,
            TextureFormat::Rgba16Float => false,
            TextureFormat::Rgba32Uint => false,
            TextureFormat::Rgba32Sint => false,
            TextureFormat::Rgba32Float => false,
            TextureFormat::Bc1RgbaUnorm => true,
            TextureFormat::Bc1RgbaUnormSrgb => true,
            TextureFormat::Bc2RgbaUnorm => true,
            TextureFormat::Bc2RgbaUnormSrgb => true,
            TextureFormat::Bc3RgbaUnorm => true,
            TextureFormat::Bc3RgbaUnormSrgb => true,
            TextureFormat::Bc4RUnorm => true,
            TextureFormat::Bc4RSnorm => true,
            TextureFormat::Bc5RgUnorm => true,
            TextureFormat::Bc5RgSnorm => true,
            TextureFormat::Bc6hRgbUfloat => false,
            TextureFormat::Bc6hRgbFloat => false,
            TextureFormat::Bc7RgbaUnorm => false,
            TextureFormat::Bc7RgbaUnormSrgb => false,
            TextureFormat::Etc2Rgb8Unorm => false,
            TextureFormat::Etc2Rgb8UnormSrgb => false,
            TextureFormat::Etc2Rgb8A1Unorm => false,
            TextureFormat::Etc2Rgb8A1UnormSrgb => false,
            TextureFormat::Etc2Rgba8Unorm => false,
            TextureFormat::Etc2Rgba8UnormSrgb => false,
            TextureFormat::EacR11Unorm => false,
            TextureFormat::EacR11Snorm => false,
            TextureFormat::EacRg11Unorm => false,
            TextureFormat::EacRg11Snorm => false,
            TextureFormat::Astc4x4Unorm => false,
            TextureFormat::Astc4x4UnormSrgb => false,
            TextureFormat::Astc4x4Hdr => false,
            TextureFormat::Astc5x4Unorm => false,
            TextureFormat::Astc5x4UnormSrgb => false,
            TextureFormat::Astc5x4Hdr => false,
            TextureFormat::Astc5x5Unorm => false,
            TextureFormat::Astc5x5UnormSrgb => false,
            TextureFormat::Astc5x5Hdr => false,
            TextureFormat::Astc6x5Unorm => false,
            TextureFormat::Astc6x5UnormSrgb => false,
            TextureFormat::Astc6x5Hdr => false,
            TextureFormat::Astc6x6Unorm => false,
            TextureFormat::Astc6x6UnormSrgb => false,
            TextureFormat::Astc6x6Hdr => false,
            TextureFormat::Astc8x5Unorm => false,
            TextureFormat::Astc8x5UnormSrgb => false,
            TextureFormat::Astc8x5Hdr => false,
            TextureFormat::Astc8x6Unorm => false,
            TextureFormat::Astc8x6UnormSrgb => false,
            TextureFormat::Astc8x6Hdr => false,
            TextureFormat::Astc8x8Unorm => false,
            TextureFormat::Astc8x8UnormSrgb => false,
            TextureFormat::Astc8x8Hdr => false,
            TextureFormat::Astc10x5Unorm => false,
            TextureFormat::Astc10x5UnormSrgb => false,
            TextureFormat::Astc10x5Hdr => false,
            TextureFormat::Astc10x6Unorm => false,
            TextureFormat::Astc10x6UnormSrgb => false,
            TextureFormat::Astc10x6Hdr => false,
            TextureFormat::Astc10x8Unorm => false,
            TextureFormat::Astc10x8UnormSrgb => false,
            TextureFormat::Astc10x8Hdr => false,
            TextureFormat::Astc10x10Unorm => false,
            TextureFormat::Astc10x10UnormSrgb => false,
            TextureFormat::Astc10x10Hdr => false,
            TextureFormat::Astc12x10Unorm => false,
            TextureFormat::Astc12x10UnormSrgb => false,
            TextureFormat::Astc12x10Hdr => false,
            TextureFormat::Astc12x12Unorm => false,
            TextureFormat::Astc12x12UnormSrgb => false,
            TextureFormat::Astc12x12Hdr => false,
        }
    }

    pub fn compression_feature(&self) -> Option<TextureCompressFeature> {
        match self {
            Self::Rgba8Unorm
            | Self::Rgba8UnormSrgb
            | Self::R8Unorm
            | Self::R8Snorm
            | Self::R8Uint
            | Self::R8Sint
            | Self::R16Uint
            | Self::R16Sint
            | Self::R16Float
            | Self::Rg8Unorm
            | Self::Rg8Snorm
            | Self::Rg8Uint
            | Self::Rg8Sint
            | Self::R32Sint
            | Self::R32Float
            | Self::Rg16Uint
            | Self::Rg16Sint
            | Self::Rg16Float
            | Self::Rgba8Snorm
            | Self::Rgba8Uint
            | Self::Rgba8Sint
            | Self::Bgra8Unorm
            | Self::Bgra8UnormSrgb
            | Self::Rgb10a2Unorm
            | Self::Rg11b10Ufloat
            | Self::Rg32Uint
            | Self::Rg32Sint
            | Self::Rg32Float
            | Self::Rgba16Uint
            | Self::Rgba16Sint
            | Self::Rgba16Float
            | Self::Rgba32Uint
            | Self::Rgba32Sint
            | Self::Rgba32Float
            | Self::R32Uint => None,
            Self::Bc1RgbaUnorm
            | Self::Bc1RgbaUnormSrgb
            | Self::Bc2RgbaUnorm
            | Self::Bc2RgbaUnormSrgb
            | Self::Bc3RgbaUnorm
            | Self::Bc3RgbaUnormSrgb
            | Self::Bc4RUnorm
            | Self::Bc4RSnorm
            | Self::Bc5RgUnorm
            | Self::Bc5RgSnorm
            | Self::Bc6hRgbUfloat
            | Self::Bc6hRgbFloat
            | Self::Bc7RgbaUnorm
            | Self::Bc7RgbaUnormSrgb => Some(TextureCompressFeature::BC),
            Self::Etc2Rgb8Unorm
            | Self::Etc2Rgb8UnormSrgb
            | Self::Etc2Rgb8A1Unorm
            | Self::Etc2Rgb8A1UnormSrgb
            | Self::Etc2Rgba8Unorm
            | Self::Etc2Rgba8UnormSrgb
            | Self::EacR11Unorm
            | Self::EacR11Snorm
            | Self::EacRg11Unorm
            | Self::EacRg11Snorm => Some(TextureCompressFeature::ETC2),
            Self::Astc4x4Unorm
            | Self::Astc4x4UnormSrgb
            | Self::Astc4x4Hdr
            | Self::Astc5x4Unorm
            | Self::Astc5x4UnormSrgb
            | Self::Astc5x4Hdr
            | Self::Astc5x5Unorm
            | Self::Astc5x5UnormSrgb
            | Self::Astc5x5Hdr
            | Self::Astc6x5Unorm
            | Self::Astc6x5UnormSrgb
            | Self::Astc6x5Hdr
            | Self::Astc6x6Unorm
            | Self::Astc6x6UnormSrgb
            | Self::Astc6x6Hdr
            | Self::Astc8x5Unorm
            | Self::Astc8x5UnormSrgb
            | Self::Astc8x5Hdr
            | Self::Astc8x6Unorm
            | Self::Astc8x6UnormSrgb
            | Self::Astc8x6Hdr
            | Self::Astc8x8Unorm
            | Self::Astc8x8UnormSrgb
            | Self::Astc8x8Hdr
            | Self::Astc10x5Unorm
            | Self::Astc10x5UnormSrgb
            | Self::Astc10x5Hdr
            | Self::Astc10x6Unorm
            | Self::Astc10x6UnormSrgb
            | Self::Astc10x6Hdr
            | Self::Astc10x8Unorm
            | Self::Astc10x8UnormSrgb
            | Self::Astc10x8Hdr
            | Self::Astc10x10Unorm
            | Self::Astc10x10UnormSrgb
            | Self::Astc10x10Hdr
            | Self::Astc12x10Unorm
            | Self::Astc12x10UnormSrgb
            | Self::Astc12x10Hdr
            | Self::Astc12x12Unorm
            | Self::Astc12x12UnormSrgb
            | Self::Astc12x12Hdr => Some(TextureCompressFeature::ASTC),
        }
    }

    pub fn is_available(&self, config: &TextureCompressionConfig) -> bool {
        if !self.supports_encoding() {
            return false;
        }
        match self.compression_feature() {
            Some(feature) => config.is_enabled(feature),
            None => true,
        }
    }

    /// Block dimensions (width, height) for this format.
    /// Uncompressed → (1, 1), BC/ETC → (4, 4), ASTC → varies.
    pub fn block_dimensions(&self) -> (u32, u32) {
        match self {
            Self::Astc4x4Unorm | Self::Astc4x4UnormSrgb | Self::Astc4x4Hdr => (4, 4),
            Self::Astc5x4Unorm | Self::Astc5x4UnormSrgb | Self::Astc5x4Hdr => (5, 4),
            Self::Astc5x5Unorm | Self::Astc5x5UnormSrgb | Self::Astc5x5Hdr => (5, 5),
            Self::Astc6x5Unorm | Self::Astc6x5UnormSrgb | Self::Astc6x5Hdr => (6, 5),
            Self::Astc6x6Unorm | Self::Astc6x6UnormSrgb | Self::Astc6x6Hdr => (6, 6),
            Self::Astc8x5Unorm | Self::Astc8x5UnormSrgb | Self::Astc8x5Hdr => (8, 5),
            Self::Astc8x6Unorm | Self::Astc8x6UnormSrgb | Self::Astc8x6Hdr => (8, 6),
            Self::Astc8x8Unorm | Self::Astc8x8UnormSrgb | Self::Astc8x8Hdr => (8, 8),
            Self::Astc10x5Unorm | Self::Astc10x5UnormSrgb | Self::Astc10x5Hdr => (10, 5),
            Self::Astc10x6Unorm | Self::Astc10x6UnormSrgb | Self::Astc10x6Hdr => (10, 6),
            Self::Astc10x8Unorm | Self::Astc10x8UnormSrgb | Self::Astc10x8Hdr => (10, 8),
            Self::Astc10x10Unorm | Self::Astc10x10UnormSrgb | Self::Astc10x10Hdr => (10, 10),
            Self::Astc12x10Unorm | Self::Astc12x10UnormSrgb | Self::Astc12x10Hdr => (12, 10),
            Self::Astc12x12Unorm | Self::Astc12x12UnormSrgb | Self::Astc12x12Hdr => (12, 12),

            Self::R8Unorm | Self::R8Snorm | Self::R8Uint
            | Self::R8Sint | Self::R16Uint | Self::R16Sint
            | Self::R16Float | Self::Rg8Unorm | Self::Rg8Snorm
            | Self::Rg8Uint | Self::Rg8Sint | Self::R32Uint
            | Self::R32Sint | Self::R32Float | Self::Rg16Uint
            | Self::Rg16Sint | Self::Rg16Float | Self::Rgba8Unorm
            | Self::Rgba8UnormSrgb | Self::Rgba8Snorm | Self::Rgba8Uint
            | Self::Rgba8Sint | Self::Bgra8Unorm | Self::Bgra8UnormSrgb
            | Self::Rgb10a2Unorm | Self::Rg11b10Ufloat | Self::Rg32Uint
            | Self::Rg32Sint | Self::Rg32Float | Self::Rgba16Uint
            | Self::Rgba16Sint | Self::Rgba16Float | Self::Rgba32Uint
            | Self::Rgba32Sint | Self::Rgba32Float => (1, 1),

            Self::Bc1RgbaUnorm | Self::Bc1RgbaUnormSrgb | Self::Bc2RgbaUnorm
            | Self::Bc2RgbaUnormSrgb | Self::Bc3RgbaUnorm | Self::Bc3RgbaUnormSrgb
            | Self::Bc4RUnorm | Self::Bc4RSnorm | Self::Bc5RgUnorm
            | Self::Bc5RgSnorm | Self::Bc6hRgbUfloat | Self::Bc6hRgbFloat
            | Self::Bc7RgbaUnorm | Self::Bc7RgbaUnormSrgb => (4, 4),

            Self::Etc2Rgb8Unorm | Self::Etc2Rgb8UnormSrgb | Self::Etc2Rgb8A1Unorm
            | Self::Etc2Rgb8A1UnormSrgb | Self::Etc2Rgba8Unorm | Self::Etc2Rgba8UnormSrgb
            | Self::EacR11Unorm | Self::EacR11Snorm
            | Self::EacRg11Unorm | Self::EacRg11Snorm => (4, 4),
        }
    }

    /// The `wgpu::TextureSampleType` for bind group compatibility.
    pub fn wgpu_sample_type(&self) -> wgpu::TextureSampleType {
        match self {
            Self::R8Uint | Self::R16Uint | Self::Rg8Uint | Self::Rg16Uint
            | Self::R32Uint | Self::Rg32Uint | Self::Rgba8Uint | Self::Rgba16Uint
            | Self::Rgba32Uint => {
                wgpu::TextureSampleType::Uint
            }
            Self::R8Sint | Self::R16Sint | Self::Rg8Sint | Self::Rg16Sint
            | Self::R32Sint | Self::Rg32Sint | Self::Rgba8Sint | Self::Rgba16Sint
            | Self::Rgba32Sint => wgpu::TextureSampleType::Sint,
            _ => wgpu::TextureSampleType::Float { filterable: true },
        }
    }

    /// Whether this format supports hardware texture filtering (Linear).
    /// Uint/Sint formats do not — they require Nearest filtering.
    pub fn is_filterable(&self) -> bool {
        matches!(self.wgpu_sample_type(), wgpu::TextureSampleType::Float { .. })
    }
}

impl From<TextureFormat> for wgpu::TextureFormat {
    fn from(value: TextureFormat) -> Self {
        use wgpu::TextureFormat as Wgpu;
        match value {
            TextureFormat::R8Unorm => Wgpu::R8Unorm,
            TextureFormat::R8Snorm => Wgpu::R8Snorm,
            TextureFormat::R8Uint => Wgpu::R8Uint,
            TextureFormat::R8Sint => Wgpu::R8Sint,
            TextureFormat::R16Uint => Wgpu::R16Uint,
            TextureFormat::R16Sint => Wgpu::R16Sint,
            TextureFormat::R16Float => Wgpu::R16Float,
            TextureFormat::Rg8Unorm => Wgpu::Rg8Unorm,
            TextureFormat::Rg8Snorm => Wgpu::Rg8Snorm,
            TextureFormat::Rg8Uint => Wgpu::Rg8Uint,
            TextureFormat::Rg8Sint => Wgpu::Rg8Sint,
            TextureFormat::R32Uint => Wgpu::R32Uint,
            TextureFormat::R32Sint => Wgpu::R32Sint,
            TextureFormat::R32Float => Wgpu::R32Float,
            TextureFormat::Rg16Uint => Wgpu::Rg16Uint,
            TextureFormat::Rg16Sint => Wgpu::Rg16Sint,
            TextureFormat::Rg16Float => Wgpu::Rg16Float,
            TextureFormat::Rgba8Unorm => Wgpu::Rgba8Unorm,
            TextureFormat::Rgba8UnormSrgb => Wgpu::Rgba8UnormSrgb,
            TextureFormat::Rgba8Snorm => Wgpu::Rgba8Snorm,
            TextureFormat::Rgba8Uint => Wgpu::Rgba8Uint,
            TextureFormat::Rgba8Sint => Wgpu::Rgba8Sint,
            TextureFormat::Bgra8Unorm => Wgpu::Bgra8Unorm,
            TextureFormat::Bgra8UnormSrgb => Wgpu::Bgra8UnormSrgb,
            TextureFormat::Rgb10a2Unorm => Wgpu::Rgb10a2Unorm,
            TextureFormat::Rg11b10Ufloat => Wgpu::Rg11b10Ufloat,
            TextureFormat::Rg32Uint => Wgpu::Rg32Uint,
            TextureFormat::Rg32Sint => Wgpu::Rg32Sint,
            TextureFormat::Rg32Float => Wgpu::Rg32Float,
            TextureFormat::Rgba16Uint => Wgpu::Rgba16Uint,
            TextureFormat::Rgba16Sint => Wgpu::Rgba16Sint,
            TextureFormat::Rgba16Float => Wgpu::Rgba16Float,
            TextureFormat::Rgba32Uint => Wgpu::Rgba32Uint,
            TextureFormat::Rgba32Sint => Wgpu::Rgba32Sint,
            TextureFormat::Rgba32Float => Wgpu::Rgba32Float,
            TextureFormat::Bc1RgbaUnorm => Wgpu::Bc1RgbaUnorm,
            TextureFormat::Bc1RgbaUnormSrgb => Wgpu::Bc1RgbaUnormSrgb,
            TextureFormat::Bc2RgbaUnorm => Wgpu::Bc2RgbaUnorm,
            TextureFormat::Bc2RgbaUnormSrgb => Wgpu::Bc2RgbaUnormSrgb,
            TextureFormat::Bc3RgbaUnorm => Wgpu::Bc3RgbaUnorm,
            TextureFormat::Bc3RgbaUnormSrgb => Wgpu::Bc3RgbaUnormSrgb,
            TextureFormat::Bc4RUnorm => Wgpu::Bc4RUnorm,
            TextureFormat::Bc4RSnorm => Wgpu::Bc4RSnorm,
            TextureFormat::Bc5RgUnorm => Wgpu::Bc5RgUnorm,
            TextureFormat::Bc5RgSnorm => Wgpu::Bc5RgSnorm,
            TextureFormat::Bc6hRgbUfloat => Wgpu::Bc6hRgbUfloat,
            TextureFormat::Bc6hRgbFloat => Wgpu::Bc6hRgbFloat,
            TextureFormat::Bc7RgbaUnorm => Wgpu::Bc7RgbaUnorm,
            TextureFormat::Bc7RgbaUnormSrgb => Wgpu::Bc7RgbaUnormSrgb,
            TextureFormat::Etc2Rgb8Unorm => Wgpu::Etc2Rgb8Unorm,
            TextureFormat::Etc2Rgb8UnormSrgb => Wgpu::Etc2Rgb8UnormSrgb,
            TextureFormat::Etc2Rgb8A1Unorm => Wgpu::Etc2Rgb8A1Unorm,
            TextureFormat::Etc2Rgb8A1UnormSrgb => Wgpu::Etc2Rgb8A1UnormSrgb,
            TextureFormat::Etc2Rgba8Unorm => Wgpu::Etc2Rgba8Unorm,
            TextureFormat::Etc2Rgba8UnormSrgb => Wgpu::Etc2Rgba8UnormSrgb,
            TextureFormat::EacR11Unorm => Wgpu::EacR11Unorm,
            TextureFormat::EacR11Snorm => Wgpu::EacR11Snorm,
            TextureFormat::EacRg11Unorm => Wgpu::EacRg11Unorm,
            TextureFormat::EacRg11Snorm => Wgpu::EacRg11Snorm,
            TextureFormat::Astc4x4Unorm => Wgpu::Astc {
                block: wgpu::AstcBlock::B4x4,
                channel: wgpu::AstcChannel::Unorm,
            },
            TextureFormat::Astc4x4UnormSrgb => Wgpu::Astc {
                block: wgpu::AstcBlock::B4x4,
                channel: wgpu::AstcChannel::UnormSrgb,
            },
            TextureFormat::Astc4x4Hdr => Wgpu::Astc {
                block: wgpu::AstcBlock::B4x4,
                channel: wgpu::AstcChannel::Hdr,
            },
            TextureFormat::Astc5x4Unorm => Wgpu::Astc {
                block: wgpu::AstcBlock::B5x4,
                channel: wgpu::AstcChannel::Unorm,
            },
            TextureFormat::Astc5x4UnormSrgb => Wgpu::Astc {
                block: wgpu::AstcBlock::B5x4,
                channel: wgpu::AstcChannel::UnormSrgb,
            },
            TextureFormat::Astc5x4Hdr => Wgpu::Astc {
                block: wgpu::AstcBlock::B5x4,
                channel: wgpu::AstcChannel::Hdr,
            },
            TextureFormat::Astc5x5Unorm => Wgpu::Astc {
                block: wgpu::AstcBlock::B5x5,
                channel: wgpu::AstcChannel::Unorm,
            },
            TextureFormat::Astc5x5UnormSrgb => Wgpu::Astc {
                block: wgpu::AstcBlock::B5x5,
                channel: wgpu::AstcChannel::UnormSrgb,
            },
            TextureFormat::Astc5x5Hdr => Wgpu::Astc {
                block: wgpu::AstcBlock::B5x5,
                channel: wgpu::AstcChannel::Hdr,
            },
            TextureFormat::Astc6x5Unorm => Wgpu::Astc {
                block: wgpu::AstcBlock::B6x5,
                channel: wgpu::AstcChannel::Unorm,
            },
            TextureFormat::Astc6x5UnormSrgb => Wgpu::Astc {
                block: wgpu::AstcBlock::B6x5,
                channel: wgpu::AstcChannel::UnormSrgb,
            },
            TextureFormat::Astc6x5Hdr => Wgpu::Astc {
                block: wgpu::AstcBlock::B6x5,
                channel: wgpu::AstcChannel::Hdr,
            },
            TextureFormat::Astc6x6Unorm => Wgpu::Astc {
                block: wgpu::AstcBlock::B6x6,
                channel: wgpu::AstcChannel::Unorm,
            },
            TextureFormat::Astc6x6UnormSrgb => Wgpu::Astc {
                block: wgpu::AstcBlock::B6x6,
                channel: wgpu::AstcChannel::UnormSrgb,
            },
            TextureFormat::Astc6x6Hdr => Wgpu::Astc {
                block: wgpu::AstcBlock::B6x6,
                channel: wgpu::AstcChannel::Hdr,
            },
            TextureFormat::Astc8x5Unorm => Wgpu::Astc {
                block: wgpu::AstcBlock::B8x5,
                channel: wgpu::AstcChannel::Unorm,
            },
            TextureFormat::Astc8x5UnormSrgb => Wgpu::Astc {
                block: wgpu::AstcBlock::B8x5,
                channel: wgpu::AstcChannel::UnormSrgb,
            },
            TextureFormat::Astc8x5Hdr => Wgpu::Astc {
                block: wgpu::AstcBlock::B8x5,
                channel: wgpu::AstcChannel::Hdr,
            },
            TextureFormat::Astc8x6Unorm => Wgpu::Astc {
                block: wgpu::AstcBlock::B8x6,
                channel: wgpu::AstcChannel::Unorm,
            },
            TextureFormat::Astc8x6UnormSrgb => Wgpu::Astc {
                block: wgpu::AstcBlock::B8x6,
                channel: wgpu::AstcChannel::UnormSrgb,
            },
            TextureFormat::Astc8x6Hdr => Wgpu::Astc {
                block: wgpu::AstcBlock::B8x6,
                channel: wgpu::AstcChannel::Hdr,
            },
            TextureFormat::Astc8x8Unorm => Wgpu::Astc {
                block: wgpu::AstcBlock::B8x8,
                channel: wgpu::AstcChannel::Unorm,
            },
            TextureFormat::Astc8x8UnormSrgb => Wgpu::Astc {
                block: wgpu::AstcBlock::B8x8,
                channel: wgpu::AstcChannel::UnormSrgb,
            },
            TextureFormat::Astc8x8Hdr => Wgpu::Astc {
                block: wgpu::AstcBlock::B8x8,
                channel: wgpu::AstcChannel::Hdr,
            },
            TextureFormat::Astc10x5Unorm => Wgpu::Astc {
                block: wgpu::AstcBlock::B10x5,
                channel: wgpu::AstcChannel::Unorm,
            },
            TextureFormat::Astc10x5UnormSrgb => Wgpu::Astc {
                block: wgpu::AstcBlock::B10x5,
                channel: wgpu::AstcChannel::UnormSrgb,
            },
            TextureFormat::Astc10x5Hdr => Wgpu::Astc {
                block: wgpu::AstcBlock::B10x5,
                channel: wgpu::AstcChannel::Hdr,
            },
            TextureFormat::Astc10x6Unorm => Wgpu::Astc {
                block: wgpu::AstcBlock::B10x6,
                channel: wgpu::AstcChannel::Unorm,
            },
            TextureFormat::Astc10x6UnormSrgb => Wgpu::Astc {
                block: wgpu::AstcBlock::B10x6,
                channel: wgpu::AstcChannel::UnormSrgb,
            },
            TextureFormat::Astc10x6Hdr => Wgpu::Astc {
                block: wgpu::AstcBlock::B10x6,
                channel: wgpu::AstcChannel::Hdr,
            },
            TextureFormat::Astc10x8Unorm => Wgpu::Astc {
                block: wgpu::AstcBlock::B10x8,
                channel: wgpu::AstcChannel::Unorm,
            },
            TextureFormat::Astc10x8UnormSrgb => Wgpu::Astc {
                block: wgpu::AstcBlock::B10x8,
                channel: wgpu::AstcChannel::UnormSrgb,
            },
            TextureFormat::Astc10x8Hdr => Wgpu::Astc {
                block: wgpu::AstcBlock::B10x8,
                channel: wgpu::AstcChannel::Hdr,
            },
            TextureFormat::Astc10x10Unorm => Wgpu::Astc {
                block: wgpu::AstcBlock::B10x10,
                channel: wgpu::AstcChannel::Unorm,
            },
            TextureFormat::Astc10x10UnormSrgb => Wgpu::Astc {
                block: wgpu::AstcBlock::B10x10,
                channel: wgpu::AstcChannel::UnormSrgb,
            },
            TextureFormat::Astc10x10Hdr => Wgpu::Astc {
                block: wgpu::AstcBlock::B10x10,
                channel: wgpu::AstcChannel::Hdr,
            },
            TextureFormat::Astc12x10Unorm => Wgpu::Astc {
                block: wgpu::AstcBlock::B12x10,
                channel: wgpu::AstcChannel::Unorm,
            },
            TextureFormat::Astc12x10UnormSrgb => Wgpu::Astc {
                block: wgpu::AstcBlock::B12x10,
                channel: wgpu::AstcChannel::UnormSrgb,
            },
            TextureFormat::Astc12x10Hdr => Wgpu::Astc {
                block: wgpu::AstcBlock::B12x10,
                channel: wgpu::AstcChannel::Hdr,
            },
            TextureFormat::Astc12x12Unorm => Wgpu::Astc {
                block: wgpu::AstcBlock::B12x12,
                channel: wgpu::AstcChannel::Unorm,
            },
            TextureFormat::Astc12x12UnormSrgb => Wgpu::Astc {
                block: wgpu::AstcBlock::B12x12,
                channel: wgpu::AstcChannel::UnormSrgb,
            },
            TextureFormat::Astc12x12Hdr => Wgpu::Astc {
                block: wgpu::AstcBlock::B12x12,
                channel: wgpu::AstcChannel::Hdr,
            },
        }
    }
}

/// Encode RGBA8 pixel data into the given format.
/// Returns `None` when the format is not yet supported for encoding.
pub fn encode_rgba(rgba: &[u8], width: u32, height: u32, format: TextureFormat) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    match format {
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb => rgba.to_vec(),
        TextureFormat::Bc1RgbaUnorm | TextureFormat::Bc1RgbaUnormSrgb => bc::encode_bc1(rgba, w, h),
        TextureFormat::Bc2RgbaUnorm | TextureFormat::Bc2RgbaUnormSrgb => bc::encode_bc2(rgba, w, h),
        TextureFormat::Bc3RgbaUnorm | TextureFormat::Bc3RgbaUnormSrgb => bc::encode_bc3(rgba, w, h),
        TextureFormat::Bc4RUnorm | TextureFormat::Bc4RSnorm => bc::encode_bc4(rgba, w, h),
        TextureFormat::Bc5RgUnorm | TextureFormat::Bc5RgSnorm => bc::encode_bc5(rgba, w, h),
        TextureFormat::R8Unorm => uncompressed::encode_r8u(rgba, w, h),
        TextureFormat::R8Snorm => uncompressed::encode_r8s(rgba, w, h),
        TextureFormat::R8Uint => uncompressed::encode_r8ui(rgba, w, h),
        TextureFormat::R8Sint => uncompressed::encode_r8si(rgba, w, h),
        TextureFormat::R16Uint => todo!(),
        TextureFormat::R16Sint => todo!(),
        TextureFormat::R16Float => todo!(),
        TextureFormat::Rg8Unorm => todo!(),
        TextureFormat::Rg8Snorm => todo!(),
        TextureFormat::Rg8Uint => todo!(),
        TextureFormat::Rg8Sint => todo!(),
        TextureFormat::R32Uint => todo!(),
        TextureFormat::R32Sint => todo!(),
        TextureFormat::R32Float => todo!(),
        TextureFormat::Rg16Uint => todo!(),
        TextureFormat::Rg16Sint => todo!(),
        TextureFormat::Rg16Float => todo!(),
        TextureFormat::Rgba8Snorm => todo!(),
        TextureFormat::Rgba8Uint => todo!(),
        TextureFormat::Rgba8Sint => todo!(),
        TextureFormat::Bgra8Unorm => todo!(),
        TextureFormat::Bgra8UnormSrgb => todo!(),
        TextureFormat::Rgb10a2Unorm => todo!(),
        TextureFormat::Rg11b10Ufloat => todo!(),
        TextureFormat::Rg32Uint => todo!(),
        TextureFormat::Rg32Sint => todo!(),
        TextureFormat::Rg32Float => todo!(),
        TextureFormat::Rgba16Uint => todo!(),
        TextureFormat::Rgba16Sint => todo!(),
        TextureFormat::Rgba16Float => todo!(),
        TextureFormat::Rgba32Uint => todo!(),
        TextureFormat::Rgba32Sint => todo!(),
        TextureFormat::Rgba32Float => todo!(),
        TextureFormat::Bc6hRgbUfloat => todo!(),
        TextureFormat::Bc6hRgbFloat => todo!(),
        TextureFormat::Bc7RgbaUnorm => todo!(),
        TextureFormat::Bc7RgbaUnormSrgb => todo!(),
        TextureFormat::Etc2Rgb8Unorm => todo!(),
        TextureFormat::Etc2Rgb8UnormSrgb => todo!(),
        TextureFormat::Etc2Rgb8A1Unorm => todo!(),
        TextureFormat::Etc2Rgb8A1UnormSrgb => todo!(),
        TextureFormat::Etc2Rgba8Unorm => todo!(),
        TextureFormat::Etc2Rgba8UnormSrgb => todo!(),
        TextureFormat::EacR11Unorm => todo!(),
        TextureFormat::EacR11Snorm => todo!(),
        TextureFormat::EacRg11Unorm => todo!(),
        TextureFormat::EacRg11Snorm => todo!(),
        TextureFormat::Astc4x4Unorm => todo!(),
        TextureFormat::Astc4x4UnormSrgb => todo!(),
        TextureFormat::Astc4x4Hdr => todo!(),
        TextureFormat::Astc5x4Unorm => todo!(),
        TextureFormat::Astc5x4UnormSrgb => todo!(),
        TextureFormat::Astc5x4Hdr => todo!(),
        TextureFormat::Astc5x5Unorm => todo!(),
        TextureFormat::Astc5x5UnormSrgb => todo!(),
        TextureFormat::Astc5x5Hdr => todo!(),
        TextureFormat::Astc6x5Unorm => todo!(),
        TextureFormat::Astc6x5UnormSrgb => todo!(),
        TextureFormat::Astc6x5Hdr => todo!(),
        TextureFormat::Astc6x6Unorm => todo!(),
        TextureFormat::Astc6x6UnormSrgb => todo!(),
        TextureFormat::Astc6x6Hdr => todo!(),
        TextureFormat::Astc8x5Unorm => todo!(),
        TextureFormat::Astc8x5UnormSrgb => todo!(),
        TextureFormat::Astc8x5Hdr => todo!(),
        TextureFormat::Astc8x6Unorm => todo!(),
        TextureFormat::Astc8x6UnormSrgb => todo!(),
        TextureFormat::Astc8x6Hdr => todo!(),
        TextureFormat::Astc8x8Unorm => todo!(),
        TextureFormat::Astc8x8UnormSrgb => todo!(),
        TextureFormat::Astc8x8Hdr => todo!(),
        TextureFormat::Astc10x5Unorm => todo!(),
        TextureFormat::Astc10x5UnormSrgb => todo!(),
        TextureFormat::Astc10x5Hdr => todo!(),
        TextureFormat::Astc10x6Unorm => todo!(),
        TextureFormat::Astc10x6UnormSrgb => todo!(),
        TextureFormat::Astc10x6Hdr => todo!(),
        TextureFormat::Astc10x8Unorm => todo!(),
        TextureFormat::Astc10x8UnormSrgb => todo!(),
        TextureFormat::Astc10x8Hdr => todo!(),
        TextureFormat::Astc10x10Unorm => todo!(),
        TextureFormat::Astc10x10UnormSrgb => todo!(),
        TextureFormat::Astc10x10Hdr => todo!(),
        TextureFormat::Astc12x10Unorm => todo!(),
        TextureFormat::Astc12x10UnormSrgb => todo!(),
        TextureFormat::Astc12x10Hdr => todo!(),
        TextureFormat::Astc12x12Unorm => todo!(),
        TextureFormat::Astc12x12UnormSrgb => todo!(),
        TextureFormat::Astc12x12Hdr => todo!(),
    }
}

/// Decode compressed texture data back to RGBA8 for preview.
pub fn decode_to_rgba8(data: &[u8], width: u32, height: u32, format: TextureFormat) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    match format {
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb => data.to_vec(),
        TextureFormat::Bc1RgbaUnorm | TextureFormat::Bc1RgbaUnormSrgb => bc::decode_bc1(data, w, h),
        TextureFormat::Bc2RgbaUnorm | TextureFormat::Bc2RgbaUnormSrgb => bc::decode_bc2(data, w, h),
        TextureFormat::Bc3RgbaUnorm | TextureFormat::Bc3RgbaUnormSrgb => bc::decode_bc3(data, w, h),
        TextureFormat::Bc4RUnorm | TextureFormat::Bc4RSnorm => bc::decode_bc4(data, w, h),
        TextureFormat::Bc5RgUnorm | TextureFormat::Bc5RgSnorm => bc::decode_bc5(data, w, h),
        TextureFormat::R8Unorm => uncompressed::decode_r8u(data, w, h, true, true, true),
        TextureFormat::R8Snorm => uncompressed::decode_r8s(data, w, h, true, true, true),
        TextureFormat::R8Uint => uncompressed::decode_r8ui(data, w, h, true, true, true),
        TextureFormat::R8Sint => uncompressed::decode_r8si(data, w, h, true, true, true),
        TextureFormat::R16Uint => todo!(),
        TextureFormat::R16Sint => todo!(),
        TextureFormat::R16Float => todo!(),
        TextureFormat::Rg8Unorm => todo!(),
        TextureFormat::Rg8Snorm => todo!(),
        TextureFormat::Rg8Uint => todo!(),
        TextureFormat::Rg8Sint => todo!(),
        TextureFormat::R32Uint => todo!(),
        TextureFormat::R32Sint => todo!(),
        TextureFormat::R32Float => todo!(),
        TextureFormat::Rg16Uint => todo!(),
        TextureFormat::Rg16Sint => todo!(),
        TextureFormat::Rg16Float => todo!(),
        TextureFormat::Rgba8Snorm => todo!(),
        TextureFormat::Rgba8Uint => todo!(),
        TextureFormat::Rgba8Sint => todo!(),
        TextureFormat::Bgra8Unorm => todo!(),
        TextureFormat::Bgra8UnormSrgb => todo!(),
        TextureFormat::Rgb10a2Unorm => todo!(),
        TextureFormat::Rg11b10Ufloat => todo!(),
        TextureFormat::Rg32Uint => todo!(),
        TextureFormat::Rg32Sint => todo!(),
        TextureFormat::Rg32Float => todo!(),
        TextureFormat::Rgba16Uint => todo!(),
        TextureFormat::Rgba16Sint => todo!(),
        TextureFormat::Rgba16Float => todo!(),
        TextureFormat::Rgba32Uint => todo!(),
        TextureFormat::Rgba32Sint => todo!(),
        TextureFormat::Rgba32Float => todo!(),
        TextureFormat::Bc6hRgbUfloat => todo!(),
        TextureFormat::Bc6hRgbFloat => todo!(),
        TextureFormat::Bc7RgbaUnorm => todo!(),
        TextureFormat::Bc7RgbaUnormSrgb => todo!(),
        TextureFormat::Etc2Rgb8Unorm => todo!(),
        TextureFormat::Etc2Rgb8UnormSrgb => todo!(),
        TextureFormat::Etc2Rgb8A1Unorm => todo!(),
        TextureFormat::Etc2Rgb8A1UnormSrgb => todo!(),
        TextureFormat::Etc2Rgba8Unorm => todo!(),
        TextureFormat::Etc2Rgba8UnormSrgb => todo!(),
        TextureFormat::EacR11Unorm => todo!(),
        TextureFormat::EacR11Snorm => todo!(),
        TextureFormat::EacRg11Unorm => todo!(),
        TextureFormat::EacRg11Snorm => todo!(),
        TextureFormat::Astc4x4Unorm => todo!(),
        TextureFormat::Astc4x4UnormSrgb => todo!(),
        TextureFormat::Astc4x4Hdr => todo!(),
        TextureFormat::Astc5x4Unorm => todo!(),
        TextureFormat::Astc5x4UnormSrgb => todo!(),
        TextureFormat::Astc5x4Hdr => todo!(),
        TextureFormat::Astc5x5Unorm => todo!(),
        TextureFormat::Astc5x5UnormSrgb => todo!(),
        TextureFormat::Astc5x5Hdr => todo!(),
        TextureFormat::Astc6x5Unorm => todo!(),
        TextureFormat::Astc6x5UnormSrgb => todo!(),
        TextureFormat::Astc6x5Hdr => todo!(),
        TextureFormat::Astc6x6Unorm => todo!(),
        TextureFormat::Astc6x6UnormSrgb => todo!(),
        TextureFormat::Astc6x6Hdr => todo!(),
        TextureFormat::Astc8x5Unorm => todo!(),
        TextureFormat::Astc8x5UnormSrgb => todo!(),
        TextureFormat::Astc8x5Hdr => todo!(),
        TextureFormat::Astc8x6Unorm => todo!(),
        TextureFormat::Astc8x6UnormSrgb => todo!(),
        TextureFormat::Astc8x6Hdr => todo!(),
        TextureFormat::Astc8x8Unorm => todo!(),
        TextureFormat::Astc8x8UnormSrgb => todo!(),
        TextureFormat::Astc8x8Hdr => todo!(),
        TextureFormat::Astc10x5Unorm => todo!(),
        TextureFormat::Astc10x5UnormSrgb => todo!(),
        TextureFormat::Astc10x5Hdr => todo!(),
        TextureFormat::Astc10x6Unorm => todo!(),
        TextureFormat::Astc10x6UnormSrgb => todo!(),
        TextureFormat::Astc10x6Hdr => todo!(),
        TextureFormat::Astc10x8Unorm => todo!(),
        TextureFormat::Astc10x8UnormSrgb => todo!(),
        TextureFormat::Astc10x8Hdr => todo!(),
        TextureFormat::Astc10x10Unorm => todo!(),
        TextureFormat::Astc10x10UnormSrgb => todo!(),
        TextureFormat::Astc10x10Hdr => todo!(),
        TextureFormat::Astc12x10Unorm => todo!(),
        TextureFormat::Astc12x10UnormSrgb => todo!(),
        TextureFormat::Astc12x10Hdr => todo!(),
        TextureFormat::Astc12x12Unorm => todo!(),
        TextureFormat::Astc12x12UnormSrgb => todo!(),
        TextureFormat::Astc12x12Hdr => todo!(),
    }
}
