use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use strum::EnumIter;

mod bc;
mod srgb;
mod uncompressed;

use rayon::prelude::*;

// ============================================================
// PixelDatas — universal pixel container
// ============================================================

/// How raw encoded bytes should be interpreted per-pixel (or per-block).
///
/// Each `TextureFormat` variant maps to exactly one of these.
/// The mapping is defined in [`TextureFormat::raw_pixel_type`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawPixelType {
    /// Raw bytes are u8 — SDR uncompressed, BC/ETC/EAC/ASTC block-compressed.
    U8,
    /// Raw bytes should be reinterpreted as `half::f16` slices.
    F16,
    /// Raw bytes should be reinterpreted as `f32` slices.
    F32,
}

/// Pixel data for a single mip level.
///
/// The variant is chosen by the texture format's bit depth:
/// - `U8` for 8-bit/channel SDR formats and BC compression
/// - `F16` for half-float HDR formats (BC6h, ASTC HDR, R16F, etc.)
/// - `F32` for native 32-bit float formats (R32F, Rg32F, Rgba32F)
///
/// Never mixed within a single mip level.
#[derive(Debug, Clone)]
pub enum PixelDatas {
    /// 8-bit unsigned integer pixel data (e.g. RGBA8, BC compressed).
    U8(Vec<u8>),
    /// 16-bit half-float pixel data.
    F16(Vec<half::f16>),
    /// 32-bit full-float pixel data.
    F32(Vec<f32>),
}

impl PixelDatas {
    /// View the inner storage as raw bytes (`&[u8]`), regardless of variant.
    /// Pure — never panics.
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            PixelDatas::U8(data) => data.as_slice(),
            PixelDatas::F16(data) => bytemuck::cast_slice(data),
            PixelDatas::F32(data) => bytemuck::cast_slice(data),
        }
    }

    /// View inner storage as `&[u8]` when the variant is known to be U8.
    /// Returns `None` for non-U8 variants — pure, never panics.
    pub fn try_u8_bytes(&self) -> Option<&[u8]> {
        match self {
            PixelDatas::U8(data) => Some(data.as_slice()),
            _ => None,
        }
    }

    /// Return the byte length of the inner storage.
    pub fn byte_len(&self) -> usize {
        self.as_bytes().len()
    }
}

/// Dimensions and byte-size of a compression block.
#[derive(Debug, Clone, Copy)]
pub struct BlockLayout {
    pub w: usize,
    pub h: usize,
    pub bytes: usize,
}

impl BlockLayout {
    pub const BC: BlockLayout = BlockLayout {
        w: 4,
        h: 4,
        bytes: 0,
    };

    pub const fn new(w: usize, h: usize, bytes: usize) -> Self {
        Self { w, h, bytes }
    }
}

/// Extract a rectangular block of RGBA8 pixels from a full image.
/// Out-of-bounds pixels are filled with zeros.
pub fn extract_block(
    rgba: &[u8],
    w: usize,
    h: usize,
    x: usize,
    y: usize,
    block_w: usize,
    block_h: usize,
) -> Vec<[u8; 4]> {
    let mut block = Vec::with_capacity(block_w * block_h);
    for py in 0..block_h {
        for px in 0..block_w {
            let sx = x + px;
            let sy = y + py;
            if sx < w && sy < h {
                let i = (sy * w + sx) * 4;
                block.push([rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]);
            } else {
                block.push([0u8; 4]);
            }
        }
    }
    block
}

/// Extract a block of F16 RGBA pixels from a full image.
/// Half-float pixels are packed as `[f16; 4]` per pixel.
/// Out-of-bounds pixels are filled with zero.
pub fn extract_block_f16(
    rgba: &[half::f16],
    w: usize,
    h: usize,
    x: usize,
    y: usize,
    block_w: usize,
    block_h: usize,
) -> Vec<[half::f16; 4]> {
    let mut block = Vec::with_capacity(block_w * block_h);
    for py in 0..block_h {
        for px in 0..block_w {
            let sx = x + px;
            let sy = y + py;
            if sx < w && sy < h {
                let i = (sy * w + sx) * 4;
                block.push([rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]);
            } else {
                block.push([half::f16::ZERO; 4]);
            }
        }
    }
    block
}

/// Shared block-parallel encoding macro for compressed formats.
///
/// Destructures the input `PixelDatas` variant explicitly — each caller
/// must specify the variant (`U8`, `F16`, or `F32`). The macro selects
/// the correct `extract_block` function and output wrapping based on the
/// variant.
///
/// # Panics
/// Panics at compile time if the variant is not recognized.
#[macro_export]
macro_rules! encode_blocks {
    // U8 variant: for BC1-5, BC7, ETC2, ASTC LDR
    ($name:ident, U8, $block_w:expr, $block_h:expr, $block_size:expr, $block_fn:ident) => {
        pub fn $name(
            pixels: &$crate::graphics::texture::format::PixelDatas,
            width: usize,
            height: usize,
        ) -> $crate::graphics::texture::format::PixelDatas {
            let $crate::graphics::texture::format::PixelDatas::U8(rgba) = pixels else {
                panic!("encode_blocks! U8 called on non-U8 variant");
            };
            let bx = (width + $block_w - 1) / $block_w;
            let by = (height + $block_h - 1) / $block_h;
            let mut out = vec![0u8; bx * by * $block_size];
            out.par_chunks_mut($block_size).enumerate().for_each(
                |(i, chunk): (usize, &mut [u8])| {
                    let bx_i = i % bx;
                    let by_i = i / bx;
                    let block = $crate::graphics::texture::format::extract_block(
                        rgba,
                        width,
                        height,
                        bx_i * $block_w,
                        by_i * $block_h,
                        $block_w,
                        $block_h,
                    );
                    let encoded = $block_fn(&block);
                    chunk.copy_from_slice(&encoded);
                },
            );
            $crate::graphics::texture::format::PixelDatas::U8(out)
        }
    };
    // F16 variant: for BC6h, ASTC HDR
    ($name:ident, F16, $block_w:expr, $block_h:expr, $block_size:expr, $block_fn:ident) => {
        pub fn $name(
            pixels: &$crate::graphics::texture::format::PixelDatas,
            width: usize,
            height: usize,
        ) -> $crate::graphics::texture::format::PixelDatas {
            let $crate::graphics::texture::format::PixelDatas::F16(rgba) = pixels else {
                panic!("encode_blocks! F16 called on non-F16 variant");
            };
            let bx = (width + $block_w - 1) / $block_w;
            let by = (height + $block_h - 1) / $block_h;
            let mut out = vec![0u8; bx * by * $block_size];
            out.par_chunks_mut($block_size).enumerate().for_each(
                |(i, chunk): (usize, &mut [u8])| {
                    let bx_i = i % bx;
                    let by_i = i / bx;
                    let block = $crate::graphics::texture::format::extract_block_f16(
                        rgba,
                        width,
                        height,
                        bx_i * $block_w,
                        by_i * $block_h,
                        $block_w,
                        $block_h,
                    );
                    let encoded = $block_fn(&block);
                    chunk.copy_from_slice(&encoded);
                },
            );
            $crate::graphics::texture::format::PixelDatas::F16(out)
        }
    };
}

/// Shared block-parallel decoding function for compressed formats.
///
/// Processes blocks in parallel, calls `decode` per block (which writes
/// RGBA8 pixels), and returns the result as `PixelDatas::U8`.
pub fn decode_blocks(
    data: &PixelDatas,
    width: usize,
    height: usize,
    layout: BlockLayout,
    decode: impl Fn(&[u8], &mut [u8; 64]) + Sync,
) -> PixelDatas {
    let raw = data.as_bytes();
    let BlockLayout {
        w: block_w,
        h: block_h,
        bytes: block_size,
    } = layout;
    let bx = (width + block_w - 1) / block_w;
    let by = (height + block_h - 1) / block_h;
    let total = bx * by;
    let mut out = vec![0u8; width * height * 4];
    let out_addr = out.as_mut_ptr() as usize;

    (0..total).into_par_iter().for_each(|i| {
        let out_ptr = out_addr as *mut u8;
        let bx_i = i % bx;
        let by_i = i / bx;
        let off = i * block_size;
        let mut pixels = [0u8; 64];
        decode(&raw[off..off + block_size], &mut pixels);
        for py in 0..block_h {
            for px in 0..block_w {
                let sx = bx_i * block_w + px;
                let sy = by_i * block_h + py;
                if sx < width && sy < height {
                    let dst = (sy * width + sx) * 4;
                    let src = (py * block_w + px) * 4;
                    // SAFETY: each (sx, sy) pair is unique across all blocks,
                    // so no two threads write to the same output location.
                    unsafe {
                        std::ptr::copy_nonoverlapping(pixels[src..].as_ptr(), out_ptr.add(dst), 4);
                    }
                }
            }
        }
    });

    PixelDatas::U8(out)
}

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

/// Texture sample type for pipeline/bind-group compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SampleType {
    Float,
    Uint,
    Sint,
}

impl From<SampleType> for wgpu::TextureSampleType {
    fn from(value: SampleType) -> Self {
        match value {
            SampleType::Float => wgpu::TextureSampleType::Float { filterable: true },
            SampleType::Uint => wgpu::TextureSampleType::Uint,
            SampleType::Sint => wgpu::TextureSampleType::Sint,
        }
    }
}

/// Project texture format — maps to `wgpu::TextureFormat` at the GPU boundary.

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
            TextureFormat::R16Uint => true,
            TextureFormat::R16Sint => true,
            TextureFormat::R16Float => true,
            TextureFormat::Rg8Unorm => true,
            TextureFormat::Rg8Snorm => true,
            TextureFormat::Rg8Uint => true,
            TextureFormat::Rg8Sint => true,
            TextureFormat::R32Uint => false,
            TextureFormat::R32Sint => false,
            TextureFormat::R32Float => false,
            TextureFormat::Rg16Uint => true,
            TextureFormat::Rg16Sint => true,
            TextureFormat::Rg16Float => true,
            TextureFormat::Rgba8Unorm => true,
            TextureFormat::Rgba8UnormSrgb => true,
            TextureFormat::Rgba8Snorm => true,
            TextureFormat::Rgba8Uint => true,
            TextureFormat::Rgba8Sint => true,
            TextureFormat::Bgra8Unorm => true,
            TextureFormat::Bgra8UnormSrgb => true,
            TextureFormat::Rgb10a2Unorm => false,
            TextureFormat::Rg11b10Ufloat => false,
            TextureFormat::Rg32Uint => false,
            TextureFormat::Rg32Sint => false,
            TextureFormat::Rg32Float => false,
            TextureFormat::Rgba16Uint => true,
            TextureFormat::Rgba16Sint => true,
            TextureFormat::Rgba16Float => true,
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

            Self::R8Unorm
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
            | Self::R32Uint
            | Self::R32Sint
            | Self::R32Float
            | Self::Rg16Uint
            | Self::Rg16Sint
            | Self::Rg16Float
            | Self::Rgba8Unorm
            | Self::Rgba8UnormSrgb
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
            | Self::Rgba32Float => (1, 1),

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
            | Self::Bc7RgbaUnormSrgb => (4, 4),

            Self::Etc2Rgb8Unorm
            | Self::Etc2Rgb8UnormSrgb
            | Self::Etc2Rgb8A1Unorm
            | Self::Etc2Rgb8A1UnormSrgb
            | Self::Etc2Rgba8Unorm
            | Self::Etc2Rgba8UnormSrgb
            | Self::EacR11Unorm
            | Self::EacR11Snorm
            | Self::EacRg11Unorm
            | Self::EacRg11Snorm => (4, 4),
        }
    }

    /// The texture sample type for pipeline/bind-group compatibility.
    pub fn sample_type(&self) -> SampleType {
        match self {
            Self::R8Uint
            | Self::R16Uint
            | Self::Rg8Uint
            | Self::Rg16Uint
            | Self::R32Uint
            | Self::Rg32Uint
            | Self::Rgba8Uint
            | Self::Rgba16Uint
            | Self::Rgba32Uint => SampleType::Uint,
            Self::R8Sint
            | Self::R16Sint
            | Self::Rg8Sint
            | Self::Rg16Sint
            | Self::R32Sint
            | Self::Rg32Sint
            | Self::Rgba8Sint
            | Self::Rgba16Sint
            | Self::Rgba32Sint => SampleType::Sint,
            _ => SampleType::Float,
        }
    }

    /// Whether this format supports hardware texture filtering (Linear).
    /// Uint/Sint formats do not — they require Nearest filtering.
    pub fn is_filterable(&self) -> bool {
        self.sample_type() == SampleType::Float
    }

    /// Byte count per block (or per pixel for uncompressed formats).
    ///
    /// For block-compressed formats:
    ///   - BC1, BC4: 8 bytes per 4×4 block
    ///   - BC2, BC3, BC5, BC6H, BC7: 16 bytes per 4×4 block
    ///   - ETC2 RGB8, ETC2 RGB8A1, EAC R11: 8 bytes per 4×4 block
    ///   - ETC2 RGBA8, EAC RG11: 16 bytes per 4×4 block
    ///   - ASTC: 16 bytes per block (block dimensions vary)
    /// For uncompressed formats: bytes per pixel.
    pub fn block_byte_size(&self) -> u32 {
        match self {
            Self::Bc1RgbaUnorm | Self::Bc1RgbaUnormSrgb | Self::Bc4RUnorm | Self::Bc4RSnorm => 8,

            Self::Etc2Rgb8Unorm
            | Self::Etc2Rgb8UnormSrgb
            | Self::Etc2Rgb8A1Unorm
            | Self::Etc2Rgb8A1UnormSrgb
            | Self::EacR11Unorm
            | Self::EacR11Snorm => 8,

            Self::Bc2RgbaUnorm
            | Self::Bc2RgbaUnormSrgb
            | Self::Bc3RgbaUnorm
            | Self::Bc3RgbaUnormSrgb
            | Self::Bc5RgUnorm
            | Self::Bc5RgSnorm
            | Self::Bc6hRgbUfloat
            | Self::Bc6hRgbFloat
            | Self::Bc7RgbaUnorm
            | Self::Bc7RgbaUnormSrgb
            | Self::Etc2Rgba8Unorm
            | Self::Etc2Rgba8UnormSrgb
            | Self::EacRg11Unorm
            | Self::EacRg11Snorm => 16,

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
            | Self::Astc12x12Hdr => 16,

            // Uncompressed (block dims = 1×1 → bytes per pixel)
            Self::R8Unorm | Self::R8Snorm | Self::R8Uint | Self::R8Sint => 1,
            Self::R16Uint
            | Self::R16Sint
            | Self::R16Float
            | Self::Rg8Unorm
            | Self::Rg8Snorm
            | Self::Rg8Uint
            | Self::Rg8Sint => 2,
            Self::R32Uint
            | Self::R32Sint
            | Self::R32Float
            | Self::Rg16Uint
            | Self::Rg16Sint
            | Self::Rg16Float
            | Self::Rgba8Unorm
            | Self::Rgba8UnormSrgb
            | Self::Rgba8Snorm
            | Self::Rgba8Uint
            | Self::Rgba8Sint
            | Self::Bgra8Unorm
            | Self::Bgra8UnormSrgb
            | Self::Rgb10a2Unorm
            | Self::Rg11b10Ufloat => 4,
            Self::Rg32Uint
            | Self::Rg32Sint
            | Self::Rg32Float
            | Self::Rgba16Uint
            | Self::Rgba16Sint
            | Self::Rgba16Float => 8,
            Self::Rgba32Uint | Self::Rgba32Sint | Self::Rgba32Float => 16,
        }
    }

    /// Number of mip levels actually stored for the given base dimensions,
    /// respecting the block-size constraint and `lod_max_clamp`.
    ///
    /// This replicates the same counting logic as the save-side loop in
    /// `inspector/texture.rs` — levels where either dimension drops below
    /// the block size are omitted from the binary, even if `lod_max_clamp`
    /// suggests a higher count.
    pub fn stored_mip_count(&self, width: u32, height: u32, lod_max_clamp: f32) -> usize {
        let max_possible = (width.max(height) as f32).log2().floor() as u32;
        let end_level = (lod_max_clamp.floor() as u32).min(max_possible);
        let (bw, bh) = self.block_dimensions();
        let mut w = width;
        let mut h = height;
        let mut count = 0;
        for _ in 0..end_level {
            if w < bw || h < bh {
                break;
            }
            count += 1;
            w = (w / 2).max(1);
            h = (h / 2).max(1);
        }
        count
    }

    /// Compute the exact byte count of a single mip level's encoded data.
    ///
    /// For uncompressed formats: `ceil(w)·ceil(h)·bytes_per_pixel`
    /// For block-compressed formats: `num_blocks·block_bytes`
    pub fn mip_level_byte_count(&self, width: u32, height: u32, level: u32) -> usize {
        let w = (width >> level).max(1);
        let h = (height >> level).max(1);
        let (bw, bh) = self.block_dimensions();
        let blocks_x = (w + bw - 1) / bw;
        let blocks_y = (h + bh - 1) / bh;
        (blocks_x * blocks_y * self.block_byte_size()) as usize
    }

    /// Map each format variant to its raw pixel storage type.
    ///
    /// Every variant is listed explicitly — adding a new `TextureFormat`
    /// forces a compile error here, guaranteeing the mapping stays in sync.
    pub fn raw_pixel_type(&self) -> RawPixelType {
        match self {
            // === F16 ===
            Self::R16Float
            | Self::Rg16Float
            | Self::Rgba16Float
            | Self::Bc6hRgbUfloat
            | Self::Bc6hRgbFloat => RawPixelType::F16,

            // === F32 ===
            Self::R32Float | Self::Rg32Float | Self::Rgba32Float => RawPixelType::F32,

            // === U8 — uncompressed ===
            Self::R8Unorm
            | Self::R8Snorm
            | Self::R8Uint
            | Self::R8Sint
            | Self::R16Uint
            | Self::R16Sint
            | Self::Rg8Unorm
            | Self::Rg8Snorm
            | Self::Rg8Uint
            | Self::Rg8Sint
            | Self::R32Uint
            | Self::R32Sint
            | Self::Rg16Uint
            | Self::Rg16Sint
            | Self::Rgba8Unorm
            | Self::Rgba8UnormSrgb
            | Self::Rgba8Snorm
            | Self::Rgba8Uint
            | Self::Rgba8Sint
            | Self::Bgra8Unorm
            | Self::Bgra8UnormSrgb
            | Self::Rgb10a2Unorm
            | Self::Rg11b10Ufloat
            | Self::Rg32Uint
            | Self::Rg32Sint
            | Self::Rgba16Uint
            | Self::Rgba16Sint
            | Self::Rgba32Uint
            | Self::Rgba32Sint => RawPixelType::U8,

            // === U8 — BC (excluding BC6h which is F16) ===
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
            | Self::Bc7RgbaUnorm
            | Self::Bc7RgbaUnormSrgb => RawPixelType::U8,

            // === U8 — ETC2 / EAC ===
            Self::Etc2Rgb8Unorm
            | Self::Etc2Rgb8UnormSrgb
            | Self::Etc2Rgb8A1Unorm
            | Self::Etc2Rgb8A1UnormSrgb
            | Self::Etc2Rgba8Unorm
            | Self::Etc2Rgba8UnormSrgb
            | Self::EacR11Unorm
            | Self::EacR11Snorm
            | Self::EacRg11Unorm
            | Self::EacRg11Snorm => RawPixelType::U8,

            // === U8 — ASTC (LDR *and* HDR; block-compressed bytes) ===
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
            | Self::Astc12x12Hdr => RawPixelType::U8,
        }
    }

    /// Construct a `PixelDatas` from raw encoded bytes.
    ///
    /// Delegates to [`raw_pixel_type`](Self::raw_pixel_type) to determine
    /// whether the bytes should be wrapped as-is (`U8`), reinterpreted as
    /// half-floats (`F16`), or reinterpreted as full-floats (`F32`).
    ///
    /// This method itself needs no update when new format variants are added
    /// — only [`raw_pixel_type`](Self::raw_pixel_type) does.
    pub fn pixel_datas_from_raw(&self, raw: &[u8]) -> PixelDatas {
        match self.raw_pixel_type() {
            RawPixelType::F16 => PixelDatas::F16(bytemuck::cast_slice(raw).to_vec()),
            RawPixelType::F32 => PixelDatas::F32(bytemuck::cast_slice(raw).to_vec()),
            RawPixelType::U8 => PixelDatas::U8(raw.to_vec()),
        }
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

/// Encode pixel data into the given format's native GPU layout.
///
/// Accepts and returns `PixelDatas`. For the existing 10 SDR formats,
/// both input and output are `PixelDatas::U8`. Future HDR formats will
/// use `F16` / `F32` variants.
///
/// # Panics
/// Panics if the format is not yet supported for encoding, or if the
/// `PixelDatas` variant does not match what the format expects.
pub fn encode(pixels: &PixelDatas, width: u32, height: u32, format: TextureFormat) -> PixelDatas {
    let w = width as usize;
    let h = height as usize;
    match format {
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb => {
            PixelDatas::U8(pixels.as_bytes().to_vec())
        }
        TextureFormat::Bc1RgbaUnorm | TextureFormat::Bc1RgbaUnormSrgb => {
            bc::encode_bc1(pixels, w, h)
        }
        TextureFormat::Bc2RgbaUnorm | TextureFormat::Bc2RgbaUnormSrgb => {
            bc::encode_bc2(pixels, w, h)
        }
        TextureFormat::Bc3RgbaUnorm | TextureFormat::Bc3RgbaUnormSrgb => {
            bc::encode_bc3(pixels, w, h)
        }
        TextureFormat::Bc4RUnorm | TextureFormat::Bc4RSnorm => bc::encode_bc4(pixels, w, h),
        TextureFormat::Bc5RgUnorm | TextureFormat::Bc5RgSnorm => bc::encode_bc5(pixels, w, h),
        TextureFormat::R8Unorm => uncompressed::encode_r8(pixels, w, h),
        TextureFormat::R8Snorm => uncompressed::encode_r8(pixels, w, h),
        TextureFormat::R8Uint => uncompressed::encode_r8(pixels, w, h),
        TextureFormat::R8Sint => uncompressed::encode_r8(pixels, w, h),
        TextureFormat::Rg8Unorm
        | TextureFormat::Rg8Snorm
        | TextureFormat::Rg8Uint
        | TextureFormat::Rg8Sint => uncompressed::encode_rg8(pixels, w, h),
        TextureFormat::Rgba8Snorm
        | TextureFormat::Rgba8Uint
        | TextureFormat::Rgba8Sint => {
            // Pass-through — same as Rgba8Unorm; GPU reinterpretation differs
            PixelDatas::U8(pixels.as_bytes().to_vec())
        }
        TextureFormat::Bgra8Unorm | TextureFormat::Bgra8UnormSrgb => {
            uncompressed::encode_bgra8(pixels, w, h)
        }
        TextureFormat::R16Uint => uncompressed::encode_r16u(pixels, w, h),
        TextureFormat::R16Sint => uncompressed::encode_r16s(pixels, w, h),
        TextureFormat::R16Float => uncompressed::encode_r16f(pixels, w, h),
        TextureFormat::Rg16Uint => uncompressed::encode_rg16u(pixels, w, h),
        TextureFormat::Rg16Sint => uncompressed::encode_rg16s(pixels, w, h),
        TextureFormat::Rg16Float => uncompressed::encode_rg16f(pixels, w, h),
        TextureFormat::Rgba16Uint => uncompressed::encode_rgba16u(pixels, w, h),
        TextureFormat::Rgba16Sint => uncompressed::encode_rgba16s(pixels, w, h),
        TextureFormat::Rgba16Float => uncompressed::encode_rgba16f(pixels, w, h),
        _ => todo!("encode not yet implemented for {format:?}"),
    }
}

/// Decode encoded texture data back to `PixelDatas` (RGBA8).
///
/// For SDR formats this returns `PixelDatas::U8`. Future HDR formats
/// will return `F16` or `F32`.
pub fn decode(data: &PixelDatas, width: u32, height: u32, format: TextureFormat) -> PixelDatas {
    let raw = data.as_bytes();
    let w = width as usize;
    let h = height as usize;
    match format {
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb => PixelDatas::U8(raw.to_vec()),
        TextureFormat::Bc1RgbaUnorm | TextureFormat::Bc1RgbaUnormSrgb => bc::decode_bc1(data, w, h),
        TextureFormat::Bc2RgbaUnorm | TextureFormat::Bc2RgbaUnormSrgb => bc::decode_bc2(data, w, h),
        TextureFormat::Bc3RgbaUnorm | TextureFormat::Bc3RgbaUnormSrgb => bc::decode_bc3(data, w, h),
        TextureFormat::Bc4RUnorm | TextureFormat::Bc4RSnorm => bc::decode_bc4(data, w, h),
        TextureFormat::Bc5RgUnorm | TextureFormat::Bc5RgSnorm => bc::decode_bc5(data, w, h),
        TextureFormat::R8Unorm => uncompressed::decode_r8(data, w, h, true, true, true),
        TextureFormat::R8Snorm => uncompressed::decode_r8(data, w, h, true, true, true),
        TextureFormat::R8Uint => uncompressed::decode_r8(data, w, h, true, true, true),
        TextureFormat::R8Sint => uncompressed::decode_r8(data, w, h, true, true, true),
        TextureFormat::Rg8Unorm
        | TextureFormat::Rg8Snorm
        | TextureFormat::Rg8Uint
        | TextureFormat::Rg8Sint => uncompressed::decode_rg8(data, w, h),
        TextureFormat::Rgba8Snorm
        | TextureFormat::Rgba8Uint
        | TextureFormat::Rgba8Sint => {
            // Pass-through — same as Rgba8Unorm
            PixelDatas::U8(raw.to_vec())
        }
        TextureFormat::Bgra8Unorm | TextureFormat::Bgra8UnormSrgb => {
            uncompressed::decode_bgra8(data, w, h)
        }
        TextureFormat::R16Uint | TextureFormat::R16Sint => uncompressed::decode_r16(data, w, h),
        TextureFormat::R16Float => uncompressed::decode_r16f(data, w, h),
        TextureFormat::Rg16Uint | TextureFormat::Rg16Sint => uncompressed::decode_rg16(data, w, h),
        TextureFormat::Rg16Float => uncompressed::decode_rg16f(data, w, h),
        TextureFormat::Rgba16Uint | TextureFormat::Rgba16Sint => uncompressed::decode_rgba16(data, w, h),
        TextureFormat::Rgba16Float => uncompressed::decode_rgba16f(data, w, h),
        _ => todo!("decode not yet implemented for {format:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a simple 4×4 RGBA8 gradient test image.
    fn make_test_rgba(w: usize, h: usize) -> Vec<u8> {
        let mut rgba = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 4;
                rgba[i] = (x * 255 / w.max(1)) as u8;
                rgba[i + 1] = (y * 255 / h.max(1)) as u8;
                rgba[i + 2] = 128;
                rgba[i + 3] = 255;
            }
        }
        rgba
    }

    #[test]
    fn bc1_roundtrip_4x4() {
        let w = 4;
        let h = 4;
        let rgba = make_test_rgba(w, h);
        let input = PixelDatas::U8(rgba.clone());
        let encoded = encode(&input, w as u32, h as u32, TextureFormat::Bc1RgbaUnorm);
        let encoded_bytes = match &encoded {
            PixelDatas::U8(b) => b,
            _ => panic!("expected U8"),
        };
        // 4×4 → one 4×4 block → 8 bytes
        assert_eq!(encoded_bytes.len(), 8, "BC1 4x4 should produce 8 bytes");
        let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Bc1RgbaUnorm);
        let decoded_bytes = match &decoded {
            PixelDatas::U8(b) => b.as_slice(),
            _ => panic!("expected U8"),
        };
        assert_eq!(decoded_bytes.len(), rgba.len());
    }

    #[test]
    fn bc3_roundtrip_8x8() {
        let w = 8;
        let h = 8;
        let rgba = make_test_rgba(w, h);
        let input = PixelDatas::U8(rgba.clone());
        let encoded = encode(&input, w as u32, h as u32, TextureFormat::Bc3RgbaUnorm);
        // 8x8 → 2×2 = 4 blocks, 16 bytes each → 64 bytes
        assert_eq!(encoded.as_bytes().len(), 64);
        let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Bc3RgbaUnorm);
        assert_eq!(decoded.as_bytes().len(), rgba.len());
    }

    #[test]
    fn r8_encode_decode() {
        let rgba = make_test_rgba(16, 16);
        let input = PixelDatas::U8(rgba.clone());
        let encoded = encode(&input, 16, 16, TextureFormat::R8Unorm);
        assert_eq!(encoded.as_bytes().len(), 256); // 16×16 = 256 bytes for R8
        let decoded = decode(&encoded, 16, 16, TextureFormat::R8Unorm);
        assert_eq!(decoded.as_bytes().len(), 1024); // 16×16×4 = 1024 bytes decoded
    }

    #[test]
    fn rgba8_pass_through() {
        let rgba = make_test_rgba(8, 8);
        let input = PixelDatas::U8(rgba.clone());
        let encoded = encode(&input, 8, 8, TextureFormat::Rgba8Unorm);
        // RGBA8 pass-through preserves bytes
        assert_eq!(encoded.as_bytes(), rgba.as_slice());
    }

    #[test]
    fn bc_encode_preserves_variant() {
        // BC encoding should return U8 variant.
        let rgba = make_test_rgba(4, 4);
        let input = PixelDatas::U8(rgba);
        let encoded = encode(&input, 4, 4, TextureFormat::Bc1RgbaUnorm);
        assert!(matches!(encoded, PixelDatas::U8(_)));
    }

    // ============================================================
    // Group A: Uncompressed SDR format tests
    // ============================================================

    #[test]
    fn rgba8_snorm_pass_through() {
        let rgba = make_test_rgba(8, 8);
        let input = PixelDatas::U8(rgba.clone());
        let encoded = encode(&input, 8, 8, TextureFormat::Rgba8Snorm);
        assert_eq!(encoded.as_bytes(), rgba.as_slice());
        let decoded = decode(&encoded, 8, 8, TextureFormat::Rgba8Snorm);
        assert_eq!(decoded.as_bytes(), rgba.as_slice());
    }

    #[test]
    fn rgba8_uint_pass_through() {
        let rgba = make_test_rgba(8, 8);
        let input = PixelDatas::U8(rgba.clone());
        let encoded = encode(&input, 8, 8, TextureFormat::Rgba8Uint);
        assert_eq!(encoded.as_bytes(), rgba.as_slice());
        let decoded = decode(&encoded, 8, 8, TextureFormat::Rgba8Uint);
        assert_eq!(decoded.as_bytes(), rgba.as_slice());
    }

    #[test]
    fn rgba8_sint_pass_through() {
        let rgba = make_test_rgba(8, 8);
        let input = PixelDatas::U8(rgba.clone());
        let encoded = encode(&input, 8, 8, TextureFormat::Rgba8Sint);
        assert_eq!(encoded.as_bytes(), rgba.as_slice());
        let decoded = decode(&encoded, 8, 8, TextureFormat::Rgba8Sint);
        assert_eq!(decoded.as_bytes(), rgba.as_slice());
    }

    #[test]
    fn rg8_encode_decode() {
        let rgba = make_test_rgba(16, 16);
        let input = PixelDatas::U8(rgba.clone());
        let encoded = encode(&input, 16, 16, TextureFormat::Rg8Unorm);
        // 16×16 → 2 bytes per pixel = 512 bytes
        assert_eq!(encoded.as_bytes().len(), 512);
        // Verify R and G are preserved, B/A dropped
        let enc_bytes = encoded.as_bytes();
        for y in 0..16 {
            for x in 0..16 {
                let src_idx = (y * 16 + x) * 4;
                let enc_idx = (y * 16 + x) * 2;
                assert_eq!(enc_bytes[enc_idx], rgba[src_idx], "R at ({},{})", x, y);
                assert_eq!(enc_bytes[enc_idx + 1], rgba[src_idx + 1], "G at ({},{})", x, y);
            }
        }
        // Decode back
        let decoded = decode(&encoded, 16, 16, TextureFormat::Rg8Unorm);
        assert_eq!(decoded.as_bytes().len(), 1024);
        let dec_bytes = decoded.as_bytes();
        for y in 0..16 {
            for x in 0..16 {
                let idx = (y * 16 + x) * 4;
                let src_idx = (y * 16 + x) * 4;
                assert_eq!(dec_bytes[idx], rgba[src_idx], "decoded R at ({},{})", x, y);
                assert_eq!(dec_bytes[idx + 1], rgba[src_idx + 1], "decoded G at ({},{})", x, y);
                assert_eq!(dec_bytes[idx + 2], 0, "decoded B at ({},{}) should be 0", x, y);
                assert_eq!(dec_bytes[idx + 3], 255, "decoded A at ({},{}) should be 255", x, y);
            }
        }
    }

    #[test]
    fn rg8_snorm_encode_decode() {
        let rgba = make_test_rgba(4, 4);
        let input = PixelDatas::U8(rgba);
        let encoded = encode(&input, 4, 4, TextureFormat::Rg8Snorm);
        assert_eq!(encoded.as_bytes().len(), 32); // 4×4×2
        let decoded = decode(&encoded, 4, 4, TextureFormat::Rg8Snorm);
        assert_eq!(decoded.as_bytes().len(), 64); // 4×4×4
    }

    #[test]
    fn rg8_uint_encode_decode() {
        let rgba = make_test_rgba(4, 4);
        let input = PixelDatas::U8(rgba);
        let encoded = encode(&input, 4, 4, TextureFormat::Rg8Uint);
        assert_eq!(encoded.as_bytes().len(), 32);
        let decoded = decode(&encoded, 4, 4, TextureFormat::Rg8Uint);
        assert_eq!(decoded.as_bytes().len(), 64);
    }

    #[test]
    fn rg8_sint_encode_decode() {
        let rgba = make_test_rgba(4, 4);
        let input = PixelDatas::U8(rgba);
        let encoded = encode(&input, 4, 4, TextureFormat::Rg8Sint);
        assert_eq!(encoded.as_bytes().len(), 32);
        let decoded = decode(&encoded, 4, 4, TextureFormat::Rg8Sint);
        assert_eq!(decoded.as_bytes().len(), 64);
    }

    #[test]
    fn bgra8_encode_decode() {
        let rgba = make_test_rgba(8, 8);
        let input = PixelDatas::U8(rgba.clone());
        let encoded = encode(&input, 8, 8, TextureFormat::Bgra8Unorm);
        assert_eq!(encoded.as_bytes().len(), 256); // 8×8×4
        // Verify R↔B swap
        let enc_bytes = encoded.as_bytes();
        for y in 0..8 {
            for x in 0..8 {
                let src_idx = (y * 8 + x) * 4;
                let enc_idx = (y * 8 + x) * 4;
                assert_eq!(enc_bytes[enc_idx], rgba[src_idx + 2], "B (was R) at ({},{})", x, y);
                assert_eq!(enc_bytes[enc_idx + 1], rgba[src_idx + 1], "G at ({},{})", x, y);
                assert_eq!(enc_bytes[enc_idx + 2], rgba[src_idx], "R (was B) at ({},{})", x, y);
                assert_eq!(enc_bytes[enc_idx + 3], rgba[src_idx + 3], "A at ({},{})", x, y);
            }
        }
        // Decode back should restore original
        let decoded = decode(&encoded, 8, 8, TextureFormat::Bgra8Unorm);
        assert_eq!(decoded.as_bytes(), rgba.as_slice());
    }

    #[test]
    fn bgra8_srgb_encode_decode() {
        let rgba = make_test_rgba(8, 8);
        let input = PixelDatas::U8(rgba.clone());
        let encoded = encode(&input, 8, 8, TextureFormat::Bgra8UnormSrgb);
        assert_eq!(encoded.as_bytes().len(), 256);
        // Decode back should restore original
        let decoded = decode(&encoded, 8, 8, TextureFormat::Bgra8UnormSrgb);
        assert_eq!(decoded.as_bytes(), rgba.as_slice());
    }

    #[test]
    fn all_group_a_supports_encoding() {
        let formats = [
            TextureFormat::Rg8Unorm,
            TextureFormat::Rg8Snorm,
            TextureFormat::Rg8Uint,
            TextureFormat::Rg8Sint,
            TextureFormat::Rgba8Snorm,
            TextureFormat::Rgba8Uint,
            TextureFormat::Rgba8Sint,
            TextureFormat::Bgra8Unorm,
            TextureFormat::Bgra8UnormSrgb,
        ];
        for fmt in &formats {
            assert!(
                fmt.supports_encoding(),
                "{fmt:?} should support encoding"
            );
        }
    }

    /// Roundtrip test: random RGBA8 pixels → encode → decode → original
    #[test]
    fn rg8_roundtrip_random() {
        // Simple LCG RNG
        let mut state: u32 = 42;
        let mut next_rand = || -> u8 {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            (state >> 24) as u8
        };
        let w = 16;
        let h = 16;
        let mut rgba = vec![0u8; w * h * 4];
        for px in rgba.chunks_mut(4) {
            px[0] = next_rand();
            px[1] = next_rand();
            px[2] = next_rand();
            px[3] = next_rand();
        }
        let input = PixelDatas::U8(rgba.clone());
        let encoded = encode(&input, w as u32, h as u32, TextureFormat::Rg8Unorm);
        let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Rg8Unorm);
        let dec = decoded.as_bytes();
        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) * 4;
                assert_eq!(dec[idx], rgba[idx], "R at ({},{})", x, y);
                assert_eq!(dec[idx + 1], rgba[idx + 1], "G at ({},{})", x, y);
                assert_eq!(dec[idx + 2], 0, "B at ({},{})", x, y);
                assert_eq!(dec[idx + 3], 255, "A at ({},{})", x, y);
            }
        }
    }

    #[test]
    fn bgra8_roundtrip_random() {
        let mut state: u32 = 99;
        let mut next_rand = || -> u8 {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            (state >> 24) as u8
        };
        let w = 16;
        let h = 16;
        let mut rgba = vec![0u8; w * h * 4];
        for px in rgba.chunks_mut(4) {
            px[0] = next_rand();
            px[1] = next_rand();
            px[2] = next_rand();
            px[3] = 255;
        }
        let input = PixelDatas::U8(rgba.clone());
        let encoded = encode(&input, w as u32, h as u32, TextureFormat::Bgra8Unorm);
        let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Bgra8Unorm);
        assert_eq!(decoded.as_bytes(), rgba.as_slice(), "BGRA8 roundtrip");
    }

    #[test]
    fn rgba8_snorm_roundtrip_random() {
        let mut state: u32 = 77;
        let mut next_rand = || -> u8 {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            (state >> 24) as u8
        };
        let w = 8;
        let h = 8;
        let mut rgba = vec![0u8; w * h * 4];
        for px in rgba.chunks_mut(4) {
            // Snorm range: 0..=255 maps to -1..=1, but bytes are stored as-is
            px[0] = next_rand();
            px[1] = next_rand();
            px[2] = next_rand();
            px[3] = next_rand();
        }
        let input = PixelDatas::U8(rgba.clone());
        let encoded = encode(&input, w as u32, h as u32, TextureFormat::Rgba8Snorm);
        // Pass-through: encode should return identical bytes
        assert_eq!(encoded.as_bytes(), rgba.as_slice());
        let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Rgba8Snorm);
        assert_eq!(decoded.as_bytes(), rgba.as_slice());
    }

    #[test]
    fn all_group_a_encode_decode_sizes() {
        let formats = [
            (TextureFormat::Rg8Unorm, 2usize),
            (TextureFormat::Rg8Snorm, 2),
            (TextureFormat::Rg8Uint, 2),
            (TextureFormat::Rg8Sint, 2),
            (TextureFormat::Rgba8Snorm, 4),
            (TextureFormat::Rgba8Uint, 4),
            (TextureFormat::Rgba8Sint, 4),
            (TextureFormat::Bgra8Unorm, 4),
            (TextureFormat::Bgra8UnormSrgb, 4),
        ];
        for (fmt, bpp) in &formats {
            let rgba = make_test_rgba(4, 4);
            let input = PixelDatas::U8(rgba);
            let encoded = encode(&input, 4, 4, *fmt);
            assert_eq!(
                encoded.as_bytes().len(),
                4 * 4 * bpp,
                "{fmt:?} encoded size mismatch"
            );
            let decoded = decode(&encoded, 4, 4, *fmt);
            assert_eq!(
                decoded.as_bytes().len(),
                4 * 4 * 4,
                "{fmt:?} decoded size should be RGBA8"
            );
        }
    }

    // ============================================================
    // Group B: Wide format tests (R16, Rg16, Rgba16 Uint/Sint/Float)
    // ============================================================

    #[test]
    fn all_group_b_supports_encoding() {
        let formats = [
            TextureFormat::R16Uint,
            TextureFormat::R16Sint,
            TextureFormat::R16Float,
            TextureFormat::Rg16Uint,
            TextureFormat::Rg16Sint,
            TextureFormat::Rg16Float,
            TextureFormat::Rgba16Uint,
            TextureFormat::Rgba16Sint,
            TextureFormat::Rgba16Float,
        ];
        for fmt in &formats {
            assert!(
                fmt.supports_encoding(),
                "{fmt:?} should support encoding"
            );
        }
    }

    #[test]
    fn r16_uint_roundtrip() {
        let w = 8;
        let h = 8;
        let mut state: u32 = 42;
        let mut next_rand = || -> u8 {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            (state >> 24) as u8
        };
        let mut rgba = vec![0u8; w * h * 4];
        for px in rgba.chunks_mut(4) {
            px[0] = next_rand();
            px[1] = next_rand();
            px[2] = next_rand();
            px[3] = next_rand();
        }
        let input = PixelDatas::U8(rgba.clone());
        let encoded = encode(&input, w as u32, h as u32, TextureFormat::R16Uint);
        assert_eq!(encoded.as_bytes().len(), w * h * 2);
        let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::R16Uint);
        assert_eq!(decoded.as_bytes().len(), w * h * 4);
        let dec = decoded.as_bytes();
        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) * 4;
                assert_eq!(dec[idx], rgba[idx], "R at ({},{})", x, y);
                assert_eq!(dec[idx + 1], 0, "G at ({},{})", x, y);
                assert_eq!(dec[idx + 2], 0, "B at ({},{})", x, y);
                assert_eq!(dec[idx + 3], 255, "A at ({},{})", x, y);
            }
        }
    }

    #[test]
    fn r16_sint_roundtrip() {
        let w = 8;
        let h = 8;
        let mut state: u32 = 43;
        let mut next_rand = || -> u8 {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            (state >> 24) as u8
        };
        let mut rgba = vec![0u8; w * h * 4];
        for px in rgba.chunks_mut(4) {
            px[0] = next_rand();
            px[1] = next_rand();
            px[2] = next_rand();
            px[3] = next_rand();
        }
        let input = PixelDatas::U8(rgba.clone());
        let encoded = encode(&input, w as u32, h as u32, TextureFormat::R16Sint);
        assert_eq!(encoded.as_bytes().len(), w * h * 2);
        let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::R16Sint);
        assert_eq!(decoded.as_bytes().len(), w * h * 4);
        let dec = decoded.as_bytes();
        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) * 4;
                assert_eq!(dec[idx], rgba[idx], "R at ({},{})", x, y);
                assert_eq!(dec[idx + 1], 0, "G at ({},{})", x, y);
                assert_eq!(dec[idx + 2], 0, "B at ({},{})", x, y);
                assert_eq!(dec[idx + 3], 255, "A at ({},{})", x, y);
            }
        }
    }

    #[test]
    fn r16_float_roundtrip() {
        let w = 8;
        let h = 8;
        let pixel_count = w * h;
        let mut rgba_f16 = vec![half::f16::ZERO; pixel_count * 4];
        for i in 0..pixel_count {
            let idx = i * 4;
            rgba_f16[idx] = half::f16::from_f32(0.5);
            rgba_f16[idx + 1] = half::f16::from_f32(0.25);
            rgba_f16[idx + 2] = half::f16::from_f32(0.75);
            rgba_f16[idx + 3] = half::f16::from_f32(1.0);
        }
        let input = PixelDatas::F16(rgba_f16.clone());
        let encoded = encode(&input, w as u32, h as u32, TextureFormat::R16Float);
        // R16Float: 1 f16 per pixel
        assert_eq!(encoded.as_bytes().len(), pixel_count * 2);
        let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::R16Float);
        assert_eq!(decoded.as_bytes().len(), pixel_count * 8); // 4 f16 per pixel
        let dec: &[half::f16] = bytemuck::cast_slice(decoded.as_bytes());
        for i in 0..pixel_count {
            let idx = i * 4;
            let half_one = half::f16::from_f32(1.0);
            assert_eq!(dec[idx], rgba_f16[idx], "R at pixel {}", i);
            assert_eq!(dec[idx + 1], half::f16::ZERO, "G at pixel {}", i);
            assert_eq!(dec[idx + 2], half::f16::ZERO, "B at pixel {}", i);
            assert_eq!(dec[idx + 3], half_one, "A at pixel {}", i);
        }
    }

    #[test]
    fn rg16_uint_roundtrip() {
        let w = 8;
        let h = 8;
        let mut state: u32 = 44;
        let mut next_rand = || -> u8 {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            (state >> 24) as u8
        };
        let mut rgba = vec![0u8; w * h * 4];
        for px in rgba.chunks_mut(4) {
            px[0] = next_rand();
            px[1] = next_rand();
            px[2] = next_rand();
            px[3] = next_rand();
        }
        let input = PixelDatas::U8(rgba.clone());
        let encoded = encode(&input, w as u32, h as u32, TextureFormat::Rg16Uint);
        assert_eq!(encoded.as_bytes().len(), w * h * 4);
        // Verify encoding preserves R and G
        let enc = encoded.as_bytes();
        for y in 0..h {
            for x in 0..w {
                let src_idx = (y * w + x) * 4;
                let enc_idx = (y * w + x) * 4;
                let r_enc = u16::from_le_bytes([enc[enc_idx], enc[enc_idx + 1]]);
                let g_enc = u16::from_le_bytes([enc[enc_idx + 2], enc[enc_idx + 3]]);
                assert_eq!(r_enc as u8, rgba[src_idx], "R at ({},{})", x, y);
                assert_eq!(g_enc as u8, rgba[src_idx + 1], "G at ({},{})", x, y);
            }
        }
        let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Rg16Uint);
        assert_eq!(decoded.as_bytes().len(), w * h * 4);
        let dec = decoded.as_bytes();
        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) * 4;
                assert_eq!(dec[idx], rgba[idx], "R at ({},{})", x, y);
                assert_eq!(dec[idx + 1], rgba[idx + 1], "G at ({},{})", x, y);
                assert_eq!(dec[idx + 2], 0, "B at ({},{})", x, y);
                assert_eq!(dec[idx + 3], 255, "A at ({},{})", x, y);
            }
        }
    }

    #[test]
    fn rg16_sint_roundtrip() {
        let w = 8;
        let h = 8;
        let mut state: u32 = 45;
        let mut next_rand = || -> u8 {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            (state >> 24) as u8
        };
        let mut rgba = vec![0u8; w * h * 4];
        for px in rgba.chunks_mut(4) {
            px[0] = next_rand();
            px[1] = next_rand();
            px[2] = next_rand();
            px[3] = next_rand();
        }
        let input = PixelDatas::U8(rgba.clone());
        let encoded = encode(&input, w as u32, h as u32, TextureFormat::Rg16Sint);
        assert_eq!(encoded.as_bytes().len(), w * h * 4);
        let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Rg16Sint);
        assert_eq!(decoded.as_bytes().len(), w * h * 4);
        let dec = decoded.as_bytes();
        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) * 4;
                assert_eq!(dec[idx], rgba[idx], "R at ({},{})", x, y);
                assert_eq!(dec[idx + 1], rgba[idx + 1], "G at ({},{})", x, y);
                assert_eq!(dec[idx + 2], 0, "B at ({},{})", x, y);
                assert_eq!(dec[idx + 3], 255, "A at ({},{})", x, y);
            }
        }
    }

    #[test]
    fn rg16_float_roundtrip() {
        let w = 8;
        let h = 8;
        let pixel_count = w * h;
        let mut rgba_f16 = vec![half::f16::ZERO; pixel_count * 4];
        let half_one = half::f16::from_f32(1.0);
        for i in 0..pixel_count {
            let idx = i * 4;
            rgba_f16[idx] = half::f16::from_f32(0.3);
            rgba_f16[idx + 1] = half::f16::from_f32(0.6);
            rgba_f16[idx + 2] = half::f16::from_f32(0.9);
            rgba_f16[idx + 3] = half_one;
        }
        let input = PixelDatas::F16(rgba_f16.clone());
        let encoded = encode(&input, w as u32, h as u32, TextureFormat::Rg16Float);
        // Rg16Float: 2 f16 per pixel
        assert_eq!(encoded.as_bytes().len(), pixel_count * 4);
        let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Rg16Float);
        assert_eq!(decoded.as_bytes().len(), pixel_count * 8);
        let dec: &[half::f16] = bytemuck::cast_slice(decoded.as_bytes());
        for i in 0..pixel_count {
            let idx = i * 4;
            assert_eq!(dec[idx], rgba_f16[idx], "R at pixel {}", i);
            assert_eq!(dec[idx + 1], rgba_f16[idx + 1], "G at pixel {}", i);
            assert_eq!(dec[idx + 2], half::f16::ZERO, "B at pixel {}", i);
            assert_eq!(dec[idx + 3], half_one, "A at pixel {}", i);
        }
    }

    #[test]
    fn rgba16_uint_roundtrip() {
        let w = 8;
        let h = 8;
        let mut state: u32 = 46;
        let mut next_rand = || -> u8 {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            (state >> 24) as u8
        };
        let mut rgba = vec![0u8; w * h * 4];
        for px in rgba.chunks_mut(4) {
            px[0] = next_rand();
            px[1] = next_rand();
            px[2] = next_rand();
            px[3] = next_rand();
        }
        let input = PixelDatas::U8(rgba.clone());
        let encoded = encode(&input, w as u32, h as u32, TextureFormat::Rgba16Uint);
        assert_eq!(encoded.as_bytes().len(), w * h * 8);
        let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Rgba16Uint);
        assert_eq!(decoded.as_bytes().len(), w * h * 4);
        assert_eq!(decoded.as_bytes(), rgba.as_slice(), "Rgba16Uint roundtrip");
    }

    #[test]
    fn rgba16_sint_roundtrip() {
        let w = 8;
        let h = 8;
        let mut state: u32 = 47;
        let mut next_rand = || -> u8 {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            (state >> 24) as u8
        };
        let mut rgba = vec![0u8; w * h * 4];
        for px in rgba.chunks_mut(4) {
            px[0] = next_rand();
            px[1] = next_rand();
            px[2] = next_rand();
            px[3] = next_rand();
        }
        let input = PixelDatas::U8(rgba.clone());
        let encoded = encode(&input, w as u32, h as u32, TextureFormat::Rgba16Sint);
        assert_eq!(encoded.as_bytes().len(), w * h * 8);
        let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Rgba16Sint);
        assert_eq!(decoded.as_bytes().len(), w * h * 4);
        assert_eq!(decoded.as_bytes(), rgba.as_slice(), "Rgba16Sint roundtrip");
    }

    #[test]
    fn rgba16_float_roundtrip() {
        let w = 8;
        let h = 8;
        let pixel_count = w * h;
        let mut rgba_f16 = vec![half::f16::ZERO; pixel_count * 4];
        for i in 0..pixel_count {
            let idx = i * 4;
            rgba_f16[idx] = half::f16::from_f32(0.1);
            rgba_f16[idx + 1] = half::f16::from_f32(0.2);
            rgba_f16[idx + 2] = half::f16::from_f32(0.3);
            rgba_f16[idx + 3] = half::f16::from_f32(0.4);
        }
        let input = PixelDatas::F16(rgba_f16.clone());
        let encoded = encode(&input, w as u32, h as u32, TextureFormat::Rgba16Float);
        // Rgba16Float: passthrough, 4 f16 per pixel
        assert_eq!(encoded.as_bytes().len(), pixel_count * 8);
        // Verify passthrough
        let enc: &[half::f16] = bytemuck::cast_slice(encoded.as_bytes());
        assert_eq!(enc, rgba_f16.as_slice(), "Rgba16Float passthrough");
        let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Rgba16Float);
        assert_eq!(decoded.as_bytes().len(), pixel_count * 8);
        let dec: &[half::f16] = bytemuck::cast_slice(decoded.as_bytes());
        assert_eq!(dec, rgba_f16.as_slice(), "Rgba16Float decode");
    }

    #[test]
    fn all_group_b_encode_decode_sizes() {
        let formats = [
            (TextureFormat::R16Uint, 2usize),
            (TextureFormat::R16Sint, 2),
            (TextureFormat::R16Float, 2),  // 1 f16 = 2 bytes
            (TextureFormat::Rg16Uint, 4),
            (TextureFormat::Rg16Sint, 4),
            (TextureFormat::Rg16Float, 4), // 2 f16 = 4 bytes
            (TextureFormat::Rgba16Uint, 8),
            (TextureFormat::Rgba16Sint, 8),
            (TextureFormat::Rgba16Float, 8), // 4 f16 = 8 bytes
        ];
        for (fmt, bpp) in &formats {
            let rgba = make_test_rgba(4, 4);
            let input = match fmt.raw_pixel_type() {
                RawPixelType::U8 => PixelDatas::U8(rgba),
                RawPixelType::F16 => {
                    let pixel_count = 4 * 4;
                    let mut f16_data = vec![half::f16::ZERO; pixel_count * 4];
                    for i in 0..pixel_count {
                        let idx = i * 4;
                        f16_data[idx] = half::f16::from_f32(0.5);
                        f16_data[idx + 1] = half::f16::from_f32(0.5);
                        f16_data[idx + 2] = half::f16::from_f32(0.5);
                        f16_data[idx + 3] = half::f16::from_f32(1.0);
                    }
                    PixelDatas::F16(f16_data)
                }
                _ => unreachable!(),
            };
            let encoded = encode(&input, 4, 4, *fmt);
            assert_eq!(
                encoded.as_bytes().len(),
                4 * 4 * bpp,
                "{fmt:?} encoded size mismatch"
            );
            let decoded = decode(&encoded, 4, 4, *fmt);
            let expected_dec_size = match fmt.raw_pixel_type() {
                RawPixelType::U8 => 4 * 4 * 4,       // RGBA8
                RawPixelType::F16 => 4 * 4 * 8,       // RGBA f16 = 4 * 4 * 4 * 2
                _ => unreachable!(),
            };
            assert_eq!(
                decoded.as_bytes().len(),
                expected_dec_size,
                "{fmt:?} decoded size mismatch"
            );
        }
    }

    #[test]
    fn r16_sint_sign_extension() {
        // Verify that values >= 128 are sign-extended for Sint encoding
        let w = 1usize;
        let h = 1usize;
        // R=200, G=100, B=50, A=255
        let rgba = vec![200u8, 100, 50, 255];
        let input = PixelDatas::U8(rgba);
        let encoded = encode(&input, w as u32, h as u32, TextureFormat::R16Sint);
        let enc = encoded.as_bytes();
        let r = u16::from_le_bytes([enc[0], enc[1]]);
        // 200 as i8 = -56, as i16 = -56 = 0xFFC8 = 65480
        assert_eq!(r, (-56i16) as u16, "R should be sign-extended");
        // Decode back should truncate to original
        let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::R16Sint);
        let dec = decoded.as_bytes();
        assert_eq!(dec[0], 200, "R should be 200 after truncation");
        assert_eq!(dec[1], 0, "G");
        assert_eq!(dec[2], 0, "B");
        assert_eq!(dec[3], 255, "A");
    }

    #[test]
    fn r16_uint_zero_extension() {
        // Verify Uint encoding uses zero-extension (not sign-extension)
        let w = 1usize;
        let h = 1usize;
        let rgba = vec![200u8, 100, 50, 255];
        let input = PixelDatas::U8(rgba);
        let encoded = encode(&input, w as u32, h as u32, TextureFormat::R16Uint);
        let enc = encoded.as_bytes();
        let r = u16::from_le_bytes([enc[0], enc[1]]);
        // 200 as u16 = 200 (zero-extended, not sign-extended)
        assert_eq!(r, 200u16, "R should be zero-extended");
        // Decode back
        let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::R16Uint);
        let dec = decoded.as_bytes();
        assert_eq!(dec[0], 200, "R should be 200");
    }

    // ============================================================
    // Golden data tests — known inputs produce known byte sequences
    // ============================================================

    #[test]
    fn r16_uint_golden() {
        // 2×1 image: R=[128, 255], G/B/A=[0,0,0]
        let rgba = vec![128u8, 0, 0, 0, 255u8, 0, 0, 0];
        let input = PixelDatas::U8(rgba);
        let encoded = encode(&input, 2, 1, TextureFormat::R16Uint);
        // R16Uint: 2 bytes per pixel = 4 bytes total
        assert_eq!(encoded.as_bytes().len(), 4);
        let enc = encoded.as_bytes();
        // Pixel 0: R=128 → u16=128 → LE=[0x80, 0x00]
        assert_eq!(enc[0], 0x80, "R lo byte pixel 0");
        assert_eq!(enc[1], 0x00, "R hi byte pixel 0");
        // Pixel 1: R=255 → u16=255 → LE=[0xFF, 0x00]
        assert_eq!(enc[2], 0xFF, "R lo byte pixel 1");
        assert_eq!(enc[3], 0x00, "R hi byte pixel 1");
    }

    #[test]
    fn r16_sint_golden() {
        // 2×1 image: R=[0, 255], G/B/A=[0,0,0]
        let rgba = vec![0u8, 0, 0, 0, 255u8, 0, 0, 0];
        let input = PixelDatas::U8(rgba);
        let encoded = encode(&input, 2, 1, TextureFormat::R16Sint);
        assert_eq!(encoded.as_bytes().len(), 4);
        let enc = encoded.as_bytes();
        // Pixel 0: R=0 as i8=0, sign-extend → u16=0 → LE=[0x00, 0x00]
        assert_eq!(enc[0], 0x00);
        assert_eq!(enc[1], 0x00);
        // Pixel 1: R=255 as i8=-1, sign-extend → i16=-1 → u16=0xFFFF → LE=[0xFF, 0xFF]
        assert_eq!(enc[2], 0xFF);
        assert_eq!(enc[3], 0xFF);
    }

    #[test]
    fn r16_float_golden() {
        // 1×1 image with known f16 values
        let rgba_f16 = vec![
            half::f16::from_f32(1.0),
            half::f16::from_f32(0.5),
            half::f16::from_f32(0.25),
            half::f16::from_f32(2.0),
        ];
        let input = PixelDatas::F16(rgba_f16);
        let encoded = encode(&input, 1, 1, TextureFormat::R16Float);
        // R16Float: 1 f16 per pixel = 2 bytes
        assert_eq!(encoded.as_bytes().len(), 2);
        let enc: &[half::f16] = bytemuck::cast_slice(encoded.as_bytes());
        assert_eq!(enc[0], half::f16::from_f32(1.0), "R channel should be 1.0");
    }

    #[test]
    fn rgba16_uint_golden() {
        // 1×1 image with all channels set
        let rgba = vec![0x12u8, 0x34, 0xAB, 0xCD];
        let input = PixelDatas::U8(rgba);
        let encoded = encode(&input, 1, 1, TextureFormat::Rgba16Uint);
        assert_eq!(encoded.as_bytes().len(), 8);
        let enc = encoded.as_bytes();
        // R=0x12 → u16=0x12 → LE=[0x12, 0x00]
        assert_eq!(enc[0], 0x12);
        assert_eq!(enc[1], 0x00);
        // G=0x34 → u16=0x34 → LE=[0x34, 0x00]
        assert_eq!(enc[2], 0x34);
        assert_eq!(enc[3], 0x00);
        // B=0xAB → u16=0xAB → LE=[0xAB, 0x00]
        assert_eq!(enc[4], 0xAB);
        assert_eq!(enc[5], 0x00);
        // A=0xCD → u16=0xCD → LE=[0xCD, 0x00]
        assert_eq!(enc[6], 0xCD);
        assert_eq!(enc[7], 0x00);
    }

    #[test]
    fn rgba16_float_golden() {
        // 1×1 image: passthrough should preserve exact f16 values
        let rgba_f16 = vec![
            half::f16::from_f32(0.1),
            half::f16::from_f32(0.2),
            half::f16::from_f32(0.3),
            half::f16::from_f32(0.4),
        ];
        let input = PixelDatas::F16(rgba_f16.clone());
        let encoded = encode(&input, 1, 1, TextureFormat::Rgba16Float);
        assert_eq!(encoded.as_bytes().len(), 8);
        let enc: &[half::f16] = bytemuck::cast_slice(encoded.as_bytes());
        assert_eq!(enc[0], half::f16::from_f32(0.1), "R");
        assert_eq!(enc[1], half::f16::from_f32(0.2), "G");
        assert_eq!(enc[2], half::f16::from_f32(0.3), "B");
        assert_eq!(enc[3], half::f16::from_f32(0.4), "A");
    }
}
