use std::collections::BTreeMap;

use half::f16;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use strum::EnumIter;

mod astc;
mod bc;
mod bc6h;
mod bc7;
mod etc;
mod srgb;
mod uncompressed;

// ============================================================
// PixelDatas — universal pixel container
// ============================================================

/// Convert a [0..1] f32 to SNORM u16.
#[inline(always)]
fn f32_to_u16(v: f32) -> u16 {
    if v <= 0.0 {
        0u16
    } else if v >= 1.0 {
        65535u16
    } else {
        (v * 65535.0).round() as u16
    }
}
/// Convert a [-1..1] f16 to SNORM u16.
#[inline(always)]
fn f16_to_u16(v: f16) -> u16 {
    f32_to_u16(v.to_f32())
}
/// Convert a [0..1] f32 to UNORM u8 (0..255).
#[inline(always)]
fn f32_to_u8(v: f32) -> u8 {
    if v <= 0.0 {
        0
    } else if v >= 1.0 {
        255
    } else {
        (v * 255.0).round() as u8
    }
}
/// Convert a [0..1] f16 to UNORM u8 (0..255).
#[inline(always)]
fn f16_to_u8(v: f16) -> u8 {
    f32_to_u8(v.to_f32())
}
#[inline(always)]
fn i8_to_u8(v: i8) -> u8 {
    (v as i16 + 128) as u8
}
#[inline(always)]
fn i16_to_u8(v: i16) -> u8 {
    i8_to_u8(v as i8)
}
#[inline(always)]
fn i8_to_u16(v: i8) -> u16 {
    (v as i16 + 128) as u16
}
#[inline(always)]
fn i16_to_u16(v: i16) -> u16 {
    (v as i32 + 32768) as u16
}
#[inline(always)]
fn u16_to_i16(v: u16) -> i16 {
    (v as i32 - 32768) as i16
}
#[inline(always)]
fn u8_to_i16(v: u8) -> i16 {
    (v as i16 - 128) as i16
}
#[inline(always)]
fn f32_to_i16(v: f32) -> i16 {
    if v <= 0.0 {
        -32768
    } else if v >= 1.0 {
        32767
    } else {
        (v * 32767.0).round() as i16
    }
}
#[inline(always)]
fn f16_to_i16(v: f16) -> i16 {
    f32_to_i16(v.to_f32())
}
#[inline(always)]
fn u8_to_f32(v: u8) -> f32 {
    (v as f32) / 255.0
}
#[inline(always)]
fn u8_to_f16(v: u8) -> f16 {
    f16::from_f32(u8_to_f32(v))
}
#[inline(always)]
fn u16_to_f32(v: u16) -> f32 {
    (v as f32) / 65535.0
}
#[inline(always)]
fn u16_to_f16(v: u16) -> f16 {
    f16::from_f32(u16_to_f32(v))
}
#[inline(always)]
fn s8_to_f32(v: i8) -> f32 {
    (v as f32) / 128.0
}
#[inline(always)]
fn s8_to_f16(v: i8) -> f16 {
    f16::from_f32(s8_to_f32(v))
}
#[inline(always)]
fn s16_to_f32(v: i16) -> f32 {
    (v as f32) / 32767.0
}
#[inline(always)]
fn s16_to_f16(v: i16) -> f16 {
    f16::from_f32(s16_to_f32(v))
}

/// How raw encoded bytes should be interpreted per-pixel (or per-block).
///
/// Each `TextureFormat` variant maps to exactly one of these.
/// The mapping is defined in [`TextureFormat::raw_pixel_type`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawPixelType {
    /// Raw bytes are u8 — SDR uncompressed, BC/ETC/EAC/ASTC block-compressed.
    U8,
    S8,
    /// Raw bytes are u16 — wide integer formats (R16Uint, Rg16Uint, etc.).
    U16,
    S16,
    /// Raw bytes should be reinterpreted as `half::f16` slices.
    F16,
    /// Raw bytes should be reinterpreted as `f32` slices.
    F32,
}

/// Pixel data for a single mip level.
///
/// The variant is chosen by the texture format's bit depth:
/// - `U8` for 8-bit/channel SDR formats and BC compression
/// - `S8` for 8-bit/channel Signed formats
/// - 'U16' for 16-bit/channel formats
/// - 'I16' for 16-bit/channel Signed formats
/// - `F16` for half-float HDR formats (BC6h, ASTC HDR, R16F, etc.)
/// - `F32` for native 32-bit float formats (R32F, Rg32F, Rgba32F)
///
/// Never mixed within a single mip level.
#[derive(Debug, Clone)]
pub enum PixelDatas {
    /// 8-bit unsigned integer pixel data (e.g. RGBA8, BC compressed).
    U8(Vec<u8>),
    S8(Vec<i8>),
    /// 16-bit unsigned integer pixel data (e.g. R16Uint, Rg16Uint encoded).
    U16(Vec<u16>),
    S16(Vec<i16>),
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
            PixelDatas::U16(data) => bytemuck::cast_slice(data),
            PixelDatas::F16(data) => bytemuck::cast_slice(data),
            PixelDatas::F32(data) => bytemuck::cast_slice(data),
            PixelDatas::S8(data) => bytemuck::cast_slice(data),
            PixelDatas::S16(data) => bytemuck::cast_slice(data),
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

    /// Convert this pixel data to `U8` variant.
    pub fn convert_to_u8(&self) -> Self {
        PixelDatas::U8(self.convert_to_u8_bytes())
    }

    /// Convert to u8 pixel data bytes in parallel.
    pub fn convert_to_u8_bytes(&self) -> Vec<u8> {
        const CHUNK: usize = 4096;
        match self {
            PixelDatas::U8(data) => data.clone(),
            PixelDatas::U16(data) => {
                let pixel_count = data.len();
                let mut out = vec![0u8; pixel_count];
                out.par_chunks_mut(CHUNK)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        let base = chunk_idx * CHUNK;
                        for (j, dst) in chunk.iter_mut().enumerate() {
                            *dst = data[base + j] as u8;
                        }
                    });
                out
            }
            PixelDatas::F16(data) => {
                let pixel_count = data.len();
                let mut out = vec![0u8; pixel_count];
                out.par_chunks_mut(CHUNK)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        let base = chunk_idx * CHUNK;
                        for (j, dst) in chunk.iter_mut().enumerate() {
                            *dst = f16_to_u8(data[base + j]);
                        }
                    });
                out
            }
            PixelDatas::F32(data) => {
                let pixel_count = data.len();
                let mut out = vec![0u8; pixel_count];
                out.par_chunks_mut(CHUNK)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        let base = chunk_idx * CHUNK;
                        for (j, dst) in chunk.iter_mut().enumerate() {
                            *dst = f32_to_u8(data[base + j]);
                        }
                    });
                out
            }
            PixelDatas::S8(data) => {
                let pixel_count = data.len();
                let mut out = vec![0u8; pixel_count];
                out.par_chunks_mut(CHUNK)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        let base = chunk_idx * CHUNK;
                        for (j, dst) in chunk.iter_mut().enumerate() {
                            *dst = i8_to_u8(data[base + j]);
                        }
                    });
                out
            }
            PixelDatas::S16(data) => {
                let pixel_count = data.len();
                let mut out = vec![0u8; pixel_count];
                out.par_chunks_mut(CHUNK)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        let base = chunk_idx * CHUNK;
                        for (j, dst) in chunk.iter_mut().enumerate() {
                            *dst = i16_to_u8(data[base + j]);
                        }
                    });
                out
            }
        }
    }

    /// Convert this pixel data to `S8` variant.
    pub fn convert_to_s8(&self) -> Self {
        PixelDatas::S8(self.convert_to_s8_bytes())
    }

    /// Convert to s8 pixel data bytes in parallel.
    pub fn convert_to_s8_bytes(&self) -> Vec<i8> {
        const CHUNK: usize = 4096;
        match self {
            PixelDatas::U8(data) => {
                let pixel_count = data.len();
                let mut out = vec![0i8; pixel_count];
                out.par_chunks_mut(CHUNK)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        let base = chunk_idx * CHUNK;
                        for (j, dst) in chunk.iter_mut().enumerate() {
                            *dst = data[base + j] as i8;
                        }
                    });
                out
            }
            PixelDatas::U16(data) => {
                let pixel_count = data.len();
                let mut out = vec![0i8; pixel_count];
                out.par_chunks_mut(CHUNK)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        let base = chunk_idx * CHUNK;
                        for (j, dst) in chunk.iter_mut().enumerate() {
                            *dst = data[base + j] as i8;
                        }
                    });
                out
            }
            PixelDatas::F16(data) => {
                let pixel_count = data.len();
                let mut out = vec![0i8; pixel_count];
                out.par_chunks_mut(CHUNK)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        let base = chunk_idx * CHUNK;
                        for (j, dst) in chunk.iter_mut().enumerate() {
                            *dst = f16_to_u8(data[base + j]) as i8;
                        }
                    });
                out
            }
            PixelDatas::F32(data) => {
                let pixel_count = data.len();
                let mut out = vec![0i8; pixel_count];
                out.par_chunks_mut(CHUNK)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        let base = chunk_idx * CHUNK;
                        for (j, dst) in chunk.iter_mut().enumerate() {
                            *dst = f32_to_u8(data[base + j]) as i8;
                        }
                    });
                out
            }
            PixelDatas::S8(data) => data.clone(),
            PixelDatas::S16(data) => {
                let pixel_count = data.len();
                let mut out = vec![0i8; pixel_count];
                out.par_chunks_mut(CHUNK)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        let base = chunk_idx * CHUNK;
                        for (j, dst) in chunk.iter_mut().enumerate() {
                            *dst = data[base + j] as i8;
                        }
                    });
                out
            }
        }
    }

    /// Convert this pixel data to `U16` variant.
    pub fn convert_to_u16(&self) -> Self {
        PixelDatas::U16(self.convert_to_u16_bytes())
    }

    /// Convert to u16 pixel data bytes in parallel.
    pub fn convert_to_u16_bytes(&self) -> Vec<u16> {
        const CHUNK: usize = 4096;
        match self {
            PixelDatas::U8(data) => {
                let pixel_count = data.len();
                let mut out = vec![0u16; pixel_count];
                out.par_chunks_mut(CHUNK)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        let base = chunk_idx * CHUNK;
                        for (j, dst) in chunk.iter_mut().enumerate() {
                            *dst = data[base + j] as u16;
                        }
                    });
                out
            }
            PixelDatas::U16(data) => data.clone(),
            PixelDatas::F16(data) => {
                let pixel_count = data.len();
                let mut out = vec![0u16; pixel_count];
                out.par_chunks_mut(CHUNK)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        let base = chunk_idx * CHUNK;
                        for (j, dst) in chunk.iter_mut().enumerate() {
                            *dst = f16_to_u16(data[base + j]);
                        }
                    });
                out
            }
            PixelDatas::F32(data) => {
                let pixel_count = data.len();
                let mut out = vec![0u16; pixel_count];
                out.par_chunks_mut(CHUNK)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        let base = chunk_idx * CHUNK;
                        for (j, dst) in chunk.iter_mut().enumerate() {
                            *dst = f32_to_u16(data[base + j]);
                        }
                    });
                out
            }
            PixelDatas::S8(data) => {
                let pixel_count = data.len();
                let mut out = vec![0u16; pixel_count];
                out.par_chunks_mut(CHUNK)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        let base = chunk_idx * CHUNK;
                        for (j, dst) in chunk.iter_mut().enumerate() {
                            *dst = i8_to_u16(data[base + j]);
                        }
                    });
                out
            }
            PixelDatas::S16(data) => {
                let pixel_count = data.len();
                let mut out = vec![0u16; pixel_count];
                out.par_chunks_mut(CHUNK)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        let base = chunk_idx * CHUNK;
                        for (j, dst) in chunk.iter_mut().enumerate() {
                            *dst = i16_to_u16(data[base + j]);
                        }
                    });
                out
            }
        }
    }

    /// Convert this pixel data to `S16` variant.
    pub fn convert_to_s16(&self) -> Self {
        PixelDatas::S16(self.convert_to_s16_bytes())
    }

    /// Convert to s16 pixel data bytes in parallel.
    pub fn convert_to_s16_bytes(&self) -> Vec<i16> {
        const CHUNK: usize = 4096;
        match self {
            PixelDatas::U8(data) => {
                let pixel_count = data.len();
                let mut out = vec![0i16; pixel_count];
                out.par_chunks_mut(CHUNK)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        let base = chunk_idx * CHUNK;
                        for (j, dst) in chunk.iter_mut().enumerate() {
                            *dst = u8_to_i16(data[base + j]);
                        }
                    });
                out
            }
            PixelDatas::U16(data) => {
                let pixel_count = data.len();
                let mut out = vec![0i16; pixel_count];
                out.par_chunks_mut(CHUNK)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        let base = chunk_idx * CHUNK;
                        for (j, dst) in chunk.iter_mut().enumerate() {
                            *dst = u16_to_i16(data[base + j]);
                        }
                    });
                out
            }
            PixelDatas::F16(data) => {
                let pixel_count = data.len();
                let mut out = vec![0i16; pixel_count];
                out.par_chunks_mut(CHUNK)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        let base = chunk_idx * CHUNK;
                        for (j, dst) in chunk.iter_mut().enumerate() {
                            *dst = f16_to_i16(data[base + j]);
                        }
                    });
                out
            }
            PixelDatas::F32(data) => {
                let pixel_count = data.len();
                let mut out = vec![0i16; pixel_count];
                out.par_chunks_mut(CHUNK)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        let base = chunk_idx * CHUNK;
                        for (j, dst) in chunk.iter_mut().enumerate() {
                            *dst = f32_to_i16(data[base + j]);
                        }
                    });
                out
            }
            PixelDatas::S8(data) => {
                let pixel_count = data.len();
                let mut out = vec![0i16; pixel_count];
                out.par_chunks_mut(CHUNK)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        let base = chunk_idx * CHUNK;
                        for (j, dst) in chunk.iter_mut().enumerate() {
                            *dst = data[base + j] as i16;
                        }
                    });
                out
            }
            PixelDatas::S16(data) => data.clone(),
        }
    }

    /// Convert this pixel data to `S16` variant.
    pub fn convert_to_f16(&self) -> Self {
        PixelDatas::F16(self.convert_to_f16_bytes())
    }

    /// Convert this pixel data to `F32` variant.
    pub fn convert_to_f32(&self) -> Self {
        PixelDatas::F32(self.convert_to_f32_bytes())
    }

    /// Convert to f32 pixel data in parallel.
    pub fn convert_to_f32_bytes(&self) -> Vec<f32> {
        const CHUNK: usize = 4096;
        match self {
            PixelDatas::U8(data) => {
                let pixel_count = data.len();
                let mut out = vec![0.0f32; pixel_count];
                out.par_chunks_mut(CHUNK)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        let base = chunk_idx * CHUNK;
                        for (j, dst) in chunk.iter_mut().enumerate() {
                            *dst = (data[base + j] as f32) / 255.0;
                        }
                    });
                out
            }
            PixelDatas::U16(data) => {
                let pixel_count = data.len();
                let mut out = vec![0.0f32; pixel_count];
                out.par_chunks_mut(CHUNK)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        let base = chunk_idx * CHUNK;
                        for (j, dst) in chunk.iter_mut().enumerate() {
                            *dst = (data[base + j] as f32) / 65535.0;
                        }
                    });
                out
            }
            PixelDatas::F16(data) => {
                let pixel_count = data.len();
                let mut out = vec![0.0f32; pixel_count];
                out.par_chunks_mut(CHUNK)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        let base = chunk_idx * CHUNK;
                        for (j, dst) in chunk.iter_mut().enumerate() {
                            *dst = data[base + j].to_f32();
                        }
                    });
                out
            }
            PixelDatas::F32(data) => data.clone(),
            PixelDatas::S8(data) => {
                let pixel_count = data.len();
                let mut out = vec![0.0f32; pixel_count];
                out.par_chunks_mut(CHUNK)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        let base = chunk_idx * CHUNK;
                        for (j, dst) in chunk.iter_mut().enumerate() {
                            *dst = (data[base + j] as f32) / 128.0;
                        }
                    });
                out
            }
            PixelDatas::S16(data) => {
                let pixel_count = data.len();
                let mut out = vec![0.0f32; pixel_count];
                out.par_chunks_mut(CHUNK)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        let base = chunk_idx * CHUNK;
                        for (j, dst) in chunk.iter_mut().enumerate() {
                            *dst = (data[base + j] as f32) / 32767.0;
                        }
                    });
                out
            }
        }
    }

    /// Convert to s16 pixel data bytes in parallel.
    pub fn convert_to_f16_bytes(&self) -> Vec<f16> {
        const CHUNK: usize = 4096;
        match self {
            PixelDatas::U8(data) => {
                let pixel_count = data.len();
                let mut out = vec![f16::ZERO; pixel_count];
                out.par_chunks_mut(CHUNK)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        let base = chunk_idx * CHUNK;
                        for (j, dst) in chunk.iter_mut().enumerate() {
                            *dst = u8_to_f16(data[base + j]);
                        }
                    });
                out
            }
            PixelDatas::U16(data) => {
                let pixel_count = data.len();
                let mut out = vec![f16::ZERO; pixel_count];
                out.par_chunks_mut(CHUNK)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        let base = chunk_idx * CHUNK;
                        for (j, dst) in chunk.iter_mut().enumerate() {
                            *dst = u16_to_f16(data[base + j]);
                        }
                    });
                out
            }
            PixelDatas::F16(data) => data.clone(),
            PixelDatas::F32(data) => {
                let pixel_count = data.len();
                let mut out = vec![f16::ZERO; pixel_count];
                out.par_chunks_mut(CHUNK)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        let base = chunk_idx * CHUNK;
                        for (j, dst) in chunk.iter_mut().enumerate() {
                            *dst = f16::from_f32(data[base + j]);
                        }
                    });
                out
            }
            PixelDatas::S8(data) => {
                let pixel_count = data.len();
                let mut out = vec![f16::ZERO; pixel_count];
                out.par_chunks_mut(CHUNK)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        let base = chunk_idx * CHUNK;
                        for (j, dst) in chunk.iter_mut().enumerate() {
                            *dst = s8_to_f16(data[base + j])
                        }
                    });
                out
            }
            PixelDatas::S16(data) => {
                let pixel_count = data.len();
                let mut out = vec![f16::ZERO; pixel_count];
                out.par_chunks_mut(CHUNK)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        let base = chunk_idx * CHUNK;
                        for (j, dst) in chunk.iter_mut().enumerate() {
                            *dst = s16_to_f16(data[base + j])
                        }
                    });
                out
            }
        }
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
/// Accepts any `PixelDatas` variant. Non-matching variants are converted
/// to the required pixel type before block encoding (U8 via
/// [`PixelDatas::to_rgba8_bytes`], F16 via per-channel normalization).
/// This ensures the encoder never panics on variant mismatch.
///
/// The variant name (`U8`, `F16`) selects the correct inner-pixel type
/// for `extract_block` / `extract_block_f16` and output wrapping.
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
            // Zero-alloc fast path: borrow U8 slice directly.
            // Non-U8 variants convert via to_rgba8_bytes() which allocates.
            let rgba: std::borrow::Cow<'_, [u8]> = match pixels {
                $crate::graphics::texture::format::PixelDatas::U8(data) => {
                    std::borrow::Cow::Borrowed(data.as_slice())
                }
                other => std::borrow::Cow::Owned(other.convert_to_u8_bytes()),
            };
            let bx = (width + $block_w - 1) / $block_w;
            let by = (height + $block_h - 1) / $block_h;
            let mut out = vec![0u8; bx * by * $block_size];
            out.par_chunks_mut($block_size).enumerate().for_each(
                |(i, chunk): (usize, &mut [u8])| {
                    let bx_i = i % bx;
                    let by_i = i / bx;
                    let block = $crate::graphics::texture::format::extract_block(
                        rgba.as_ref(),
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
            // Zero-alloc fast path: borrow F16 slice directly.
            // Non-F16 variants convert via per-channel normalization.
            let rgba: std::borrow::Cow<'_, [half::f16]> = match pixels {
                $crate::graphics::texture::format::PixelDatas::F16(data) => {
                    std::borrow::Cow::Borrowed(data.as_slice())
                }
                $crate::graphics::texture::format::PixelDatas::U8(data) => {
                    const CHUNK: usize = 4096;
                    let n = data.len();
                    let mut out = vec![half::f16::ZERO; n];
                    out.par_chunks_mut(CHUNK)
                        .enumerate()
                        .for_each(|(chunk_idx, chunk)| {
                            let base = chunk_idx * CHUNK;
                            for (j, dst) in chunk.iter_mut().enumerate() {
                                *dst = half::f16::from_f32(data[base + j] as f32 / 255.0);
                            }
                        });
                    std::borrow::Cow::Owned(out)
                }
                $crate::graphics::texture::format::PixelDatas::U16(data) => {
                    const CHUNK: usize = 4096;
                    let n = data.len();
                    let mut out = vec![half::f16::ZERO; n];
                    out.par_chunks_mut(CHUNK)
                        .enumerate()
                        .for_each(|(chunk_idx, chunk)| {
                            let base = chunk_idx * CHUNK;
                            for (j, dst) in chunk.iter_mut().enumerate() {
                                *dst = half::f16::from_f32(data[base + j] as f32 / 65535.0);
                            }
                        });
                    std::borrow::Cow::Owned(out)
                }
                $crate::graphics::texture::format::PixelDatas::F32(data) => {
                    const CHUNK: usize = 4096;
                    let n = data.len();
                    let mut out = vec![half::f16::ZERO; n];
                    out.par_chunks_mut(CHUNK)
                        .enumerate()
                        .for_each(|(chunk_idx, chunk)| {
                            let base = chunk_idx * CHUNK;
                            for (j, dst) in chunk.iter_mut().enumerate() {
                                *dst = half::f16::from_f32(data[base + j]);
                            }
                        });
                    std::borrow::Cow::Owned(out)
                }
            };
            let bx = (width + $block_w - 1) / $block_w;
            let by = (height + $block_h - 1) / $block_h;
            let mut out = vec![0u8; bx * by * $block_size];
            out.par_chunks_mut($block_size).enumerate().for_each(
                |(i, chunk): (usize, &mut [u8])| {
                    let bx_i = i % bx;
                    let by_i = i / bx;
                    let block = $crate::graphics::texture::format::extract_block_f16(
                        rgba.as_ref(),
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

/// Shared block-parallel decoding macro for compressed formats.
///
/// Generates a decoding function for the given variant:
///
/// | Variant | Output `PixelDatas` | Per-block buffer |
/// |---------|---------------------|------------------|
/// | `U8`    | `PixelDatas::U8`    | `[u8; 64]`       |
/// | `F16`   | `PixelDatas::F16`   | `[half::f16; 64]`|
///
/// This mirrors the design of [`encode_blocks!`] and ensures the output
/// variant matches the decoded pixel type (no panic, no assumption).
///
/// # Panics
/// Panics at compile time if the variant is not recognized.
#[macro_export]
macro_rules! decode_blocks {
    // U8 variant: for BC1-5, BC7, ETC2, ASTC LDR
    ($name:ident, U8, $layout:expr, $decode_fn:ident) => {
        pub fn $name(
            data: &$crate::graphics::texture::format::PixelDatas,
            width: usize,
            height: usize,
        ) -> $crate::graphics::texture::format::PixelDatas {
            let raw = data.convert_to_u8_bytes();
            let $crate::graphics::texture::format::BlockLayout {
                w: block_w,
                h: block_h,
                bytes: block_size,
            } = $layout;
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
                $decode_fn(&raw[off..off + block_size], &mut pixels);
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
                                std::ptr::copy_nonoverlapping(
                                    pixels[src..].as_ptr(),
                                    out_ptr.add(dst),
                                    4,
                                );
                            }
                        }
                    }
                }
            });

            $crate::graphics::texture::format::PixelDatas::U8(out)
        }
    };
    // S8 variant
    ($name:ident, S8, $layout:expr, $decode_fn:ident) => {
        pub fn $name(
            data: &$crate::graphics::texture::format::PixelDatas,
            width: usize,
            height: usize,
        ) -> $crate::graphics::texture::format::PixelDatas {
            let raw = data.convert_to_u8_bytes();
            let $crate::graphics::texture::format::BlockLayout {
                w: block_w,
                h: block_h,
                bytes: block_size,
            } = $layout;
            let bx = (width + block_w - 1) / block_w;
            let by = (height + block_h - 1) / block_h;
            let total = bx * by;
            let mut out = vec![0i8; width * height * 4];
            let out_addr = out.as_mut_ptr() as usize;

            (0..total).into_par_iter().for_each(|i| {
                let out_ptr = out_addr as *mut i8;
                let bx_i = i % bx;
                let by_i = i / bx;
                let off = i * block_size;
                let mut pixels = [0i8; 64];
                $decode_fn(&raw[off..off + block_size], &mut pixels);
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
                                std::ptr::copy_nonoverlapping(
                                    pixels[src..].as_ptr(),
                                    out_ptr.add(dst),
                                    4,
                                );
                            }
                        }
                    }
                }
            });

            $crate::graphics::texture::format::PixelDatas::S8(out)
        }
    };
    // U16 variant
    ($name:ident, U16, $layout:expr, $decode_fn:ident) => {
        pub fn $name(
            data: &$crate::graphics::texture::format::PixelDatas,
            width: usize,
            height: usize,
        ) -> $crate::graphics::texture::format::PixelDatas {
            let raw = data.convert_to_u16_bytes();
            let $crate::graphics::texture::format::BlockLayout {
                w: block_w,
                h: block_h,
                bytes: block_size,
            } = $layout;
            let bx = (width + block_w - 1) / block_w;
            let by = (height + block_h - 1) / block_h;
            let total = bx * by;
            let mut out = vec![0u16; width * height * 4];
            let out_addr = out.as_mut_ptr() as usize;

            (0..total).into_par_iter().for_each(|i| {
                let out_ptr = out_addr as *mut u8;
                let bx_i = i % bx;
                let by_i = i / bx;
                let off = i * block_size;
                let mut pixels = [0u16; 64];
                $decode_fn(&raw[off..off + block_size], &mut pixels);
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
                                std::ptr::copy_nonoverlapping(
                                    pixels[src..].as_ptr(),
                                    out_ptr.add(dst),
                                    4,
                                );
                            }
                        }
                    }
                }
            });

            $crate::graphics::texture::format::PixelDatas::U8(out)
        }
    };
    // S16 variant
    ($name:ident, S16, $layout:expr, $decode_fn:ident) => {
        pub fn $name(
            data: &$crate::graphics::texture::format::PixelDatas,
            width: usize,
            height: usize,
        ) -> $crate::graphics::texture::format::PixelDatas {
            let raw = data.convert_to_s16_bytes();
            let $crate::graphics::texture::format::BlockLayout {
                w: block_w,
                h: block_h,
                bytes: block_size,
            } = $layout;
            let bx = (width + block_w - 1) / block_w;
            let by = (height + block_h - 1) / block_h;
            let total = bx * by;
            let mut out = vec![0i16; width * height * 4];
            let out_addr = out.as_mut_ptr() as usize;

            (0..total).into_par_iter().for_each(|i| {
                let out_ptr = out_addr as *mut u8;
                let bx_i = i % bx;
                let by_i = i / bx;
                let off = i * block_size;
                let mut pixels = [0i16; 64];
                $decode_fn(&raw[off..off + block_size], &mut pixels);
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
                                std::ptr::copy_nonoverlapping(
                                    pixels[src..].as_ptr(),
                                    out_ptr.add(dst),
                                    4,
                                );
                            }
                        }
                    }
                }
            });

            $crate::graphics::texture::format::PixelDatas::U8(out)
        }
    };
    // F16 variant: for BC6h, ASTC HDR
    ($name:ident, F16, $layout:expr, $decode_fn:ident) => {
        pub fn $name(
            data: &$crate::graphics::texture::format::PixelDatas,
            width: usize,
            height: usize,
        ) -> $crate::graphics::texture::format::PixelDatas {
            let raw = data.convert_to_u16_bytes();
            let $crate::graphics::texture::format::BlockLayout {
                w: block_w,
                h: block_h,
                bytes: block_size,
            } = $layout;
            let bx = (width + block_w - 1) / block_w;
            let by = (height + block_h - 1) / block_h;
            let total = bx * by;
            let mut out = vec![half::f16::ZERO; width * height * 4];
            let out_addr = out.as_mut_ptr() as usize;

            (0..total).into_par_iter().for_each(|i| {
                let out_ptr = out_addr as *mut half::f16;
                let bx_i = i % bx;
                let by_i = i / bx;
                let off = i * block_size;
                let mut pixels = [half::f16::ZERO; 64];
                $decode_fn(&raw[off..off + block_size], &mut pixels);
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
                                std::ptr::copy_nonoverlapping(
                                    pixels[src..].as_ptr(),
                                    out_ptr.add(dst),
                                    4,
                                );
                            }
                        }
                    }
                }
            });

            $crate::graphics::texture::format::PixelDatas::F16(out)
        }
    };
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

impl From<TextureFormat> for wgpu::TextureSampleType {
    fn from(format: TextureFormat) -> Self {
        match format.sample_type() {
            SampleType::Float => wgpu::TextureSampleType::Float {
                filterable: format.is_filterable(),
            },
            SampleType::Uint => wgpu::TextureSampleType::Uint,
            SampleType::Sint => wgpu::TextureSampleType::Sint,
        }
    }
}

impl From<TextureFormat> for wgpu::SamplerBindingType {
    fn from(format: TextureFormat) -> Self {
        match format.sample_type() {
            SampleType::Float if format.is_filterable() => wgpu::SamplerBindingType::Filtering,
            _ => wgpu::SamplerBindingType::NonFiltering,
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
            TextureFormat::R32Uint => true,
            TextureFormat::R32Sint => true,
            TextureFormat::R32Float => true,
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
            TextureFormat::Rgb10a2Unorm => true,
            TextureFormat::Rg11b10Ufloat => true,
            TextureFormat::Rg32Uint => true,
            TextureFormat::Rg32Sint => true,
            TextureFormat::Rg32Float => true,
            TextureFormat::Rgba16Uint => true,
            TextureFormat::Rgba16Sint => true,
            TextureFormat::Rgba16Float => true,
            TextureFormat::Rgba32Uint => true,
            TextureFormat::Rgba32Sint => true,
            TextureFormat::Rgba32Float => true,
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
            TextureFormat::Bc6hRgbUfloat => true,
            TextureFormat::Bc6hRgbFloat => true,
            TextureFormat::Bc7RgbaUnorm => true,
            TextureFormat::Bc7RgbaUnormSrgb => true,
            TextureFormat::Etc2Rgb8Unorm => true,
            TextureFormat::Etc2Rgb8UnormSrgb => true,
            TextureFormat::Etc2Rgb8A1Unorm => true,
            TextureFormat::Etc2Rgb8A1UnormSrgb => true,
            TextureFormat::Etc2Rgba8Unorm => true,
            TextureFormat::Etc2Rgba8UnormSrgb => true,
            TextureFormat::EacR11Unorm => true,
            TextureFormat::EacR11Snorm => true,
            TextureFormat::EacRg11Unorm => true,
            TextureFormat::EacRg11Snorm => true,
            TextureFormat::Astc4x4Unorm => true,
            TextureFormat::Astc4x4UnormSrgb => true,
            TextureFormat::Astc4x4Hdr => true,
            TextureFormat::Astc5x4Unorm => true,
            TextureFormat::Astc5x4UnormSrgb => true,
            TextureFormat::Astc5x4Hdr => true,
            TextureFormat::Astc5x5Unorm => true,
            TextureFormat::Astc5x5UnormSrgb => true,
            TextureFormat::Astc5x5Hdr => true,
            TextureFormat::Astc6x5Unorm => true,
            TextureFormat::Astc6x5UnormSrgb => true,
            TextureFormat::Astc6x5Hdr => true,
            TextureFormat::Astc6x6Unorm => true,
            TextureFormat::Astc6x6UnormSrgb => true,
            TextureFormat::Astc6x6Hdr => true,
            TextureFormat::Astc8x5Unorm => true,
            TextureFormat::Astc8x5UnormSrgb => true,
            TextureFormat::Astc8x5Hdr => true,
            TextureFormat::Astc8x6Unorm => true,
            TextureFormat::Astc8x6UnormSrgb => true,
            TextureFormat::Astc8x6Hdr => true,
            TextureFormat::Astc8x8Unorm => true,
            TextureFormat::Astc8x8UnormSrgb => true,
            TextureFormat::Astc8x8Hdr => true,
            TextureFormat::Astc10x5Unorm => true,
            TextureFormat::Astc10x5UnormSrgb => true,
            TextureFormat::Astc10x5Hdr => true,
            TextureFormat::Astc10x6Unorm => true,
            TextureFormat::Astc10x6UnormSrgb => true,
            TextureFormat::Astc10x6Hdr => true,
            TextureFormat::Astc10x8Unorm => true,
            TextureFormat::Astc10x8UnormSrgb => true,
            TextureFormat::Astc10x8Hdr => true,
            TextureFormat::Astc10x10Unorm => true,
            TextureFormat::Astc10x10UnormSrgb => true,
            TextureFormat::Astc10x10Hdr => true,
            TextureFormat::Astc12x10Unorm => true,
            TextureFormat::Astc12x10UnormSrgb => true,
            TextureFormat::Astc12x10Hdr => true,
            TextureFormat::Astc12x12Unorm => true,
            TextureFormat::Astc12x12UnormSrgb => true,
            TextureFormat::Astc12x12Hdr => true,
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
    /// 32-bit float formats (R32Float, Rg32Float, Rgba32Float) are also
    /// not filterable per wgpu/hardware constraints.
    pub fn is_filterable(&self) -> bool {
        match self {
            Self::R32Float | Self::Rg32Float | Self::Rgba32Float => false,
            _ => self.sample_type() == SampleType::Float,
        }
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
        let mut count = 0;
        // Count from level 0 to end_level (inclusive).
        // For a 2048×2048 texture with lod_max_clamp=11, this gives 12 levels:
        //   level 0  (2048×2048) through level 11 (1×1).
        for level in 0..=end_level {
            let w = (width >> level).max(1);
            let h = (height >> level).max(1);
            if w < bw || h < bh {
                break;
            }
            count += 1;
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

            // === U8 — packed ===
            Self::Rgb10a2Unorm
            | Self::Rg11b10Ufloat => RawPixelType::U8,

            // === F32 ===
            Self::R32Float | Self::Rg32Float | Self::Rgba32Float => RawPixelType::F32,

            // === U16 — wide integer uncompressed ===
            Self::R16Uint | Self::Rg16Uint | Self::Rgba16Uint => RawPixelType::U16,
            Self::R16Sint | Self::Rg16Sint | Self::Rgba16Sint => RawPixelType::S16,

            // === U8 — uncompressed ===
            Self::R8Unorm
            | Self::R8Uint
            | Self::Rg8Unorm
            | Self::Rg8Uint
            | Self::R32Uint
            | Self::Rgba8Unorm
            | Self::Rgba8UnormSrgb
            | Self::Rgba8Uint
            | Self::Bgra8Unorm
            | Self::Bgra8UnormSrgb
            | Self::Rg32Uint
            | Self::Rgba32Uint => RawPixelType::U8,
            Self::R8Snorm
            | Self::R8Sint
            | Self::Rg8Snorm
            | Self::Rg8Sint
            | Self::R32Sint
            | Self::Rgba8Snorm
            | Self::Rgba8Sint
            | Self::Rg32Sint
            | Self::Rgba32Sint => RawPixelType::S8,

            // === U8 — BC (excluding BC6h which is F16) ===
            Self::Bc1RgbaUnorm
            | Self::Bc1RgbaUnormSrgb
            | Self::Bc2RgbaUnorm
            | Self::Bc2RgbaUnormSrgb
            | Self::Bc3RgbaUnorm
            | Self::Bc3RgbaUnormSrgb
            | Self::Bc5RgUnorm
            | Self::Bc7RgbaUnorm
            | Self::Bc4RUnorm
            | Self::Bc7RgbaUnormSrgb => RawPixelType::U8,
            Self::Bc4RSnorm | Self::Bc5RgSnorm => RawPixelType::S8,

            // === U8 — ETC2 / EAC ===
            Self::Etc2Rgb8Unorm
            | Self::Etc2Rgb8UnormSrgb
            | Self::Etc2Rgb8A1Unorm
            | Self::Etc2Rgb8A1UnormSrgb
            | Self::Etc2Rgba8Unorm
            | Self::Etc2Rgba8UnormSrgb
            | Self::EacR11Unorm
            | Self::EacRg11Unorm => RawPixelType::U8,
            Self::EacR11Snorm | Self::EacRg11Snorm => RawPixelType::S8,

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
            RawPixelType::U16 => PixelDatas::U16(bytemuck::cast_slice(raw).to_vec()),
            RawPixelType::F16 => PixelDatas::F16(bytemuck::cast_slice(raw).to_vec()),
            RawPixelType::F32 => PixelDatas::F32(bytemuck::cast_slice(raw).to_vec()),
            RawPixelType::U8 => PixelDatas::U8(raw.to_vec()),
            RawPixelType::S8 => PixelDatas::S8(bytemuck::cast_slice(raw).to_vec()),
            RawPixelType::S16 => PixelDatas::S16(bytemuck::cast_slice(raw).to_vec()),
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
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb => match pixels {
            PixelDatas::U8(datas) => PixelDatas::U8(datas.clone()),
            other => PixelDatas::U8(other.convert_to_u8_bytes()),
        },
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
        TextureFormat::Rgba8Snorm | TextureFormat::Rgba8Uint | TextureFormat::Rgba8Sint => {
            // Pass-through — same as Rgba8Unorm; GPU reinterpretation differs
            match pixels {
                PixelDatas::U8(datas) => PixelDatas::U8(datas.clone()),
                other => PixelDatas::U8(other.convert_to_u8_bytes()),
            }
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
        TextureFormat::R32Uint => uncompressed::encode_r32u(pixels, w, h),
        TextureFormat::R32Sint => uncompressed::encode_r32s(pixels, w, h),
        TextureFormat::R32Float => uncompressed::encode_r32f(pixels, w, h),
        TextureFormat::Rg32Uint => uncompressed::encode_rg32u(pixels, w, h),
        TextureFormat::Rg32Sint => uncompressed::encode_rg32s(pixels, w, h),
        TextureFormat::Rg32Float => uncompressed::encode_rg32f(pixels, w, h),
        TextureFormat::Rgba32Uint => uncompressed::encode_rgba32u(pixels, w, h),
        TextureFormat::Rgba32Sint => uncompressed::encode_rgba32s(pixels, w, h),
        TextureFormat::Rgba32Float => uncompressed::encode_rgba32f(pixels, w, h),
        TextureFormat::Rgb10a2Unorm => uncompressed::encode_rgb10a2_unorm(pixels, w, h),
        TextureFormat::Rg11b10Ufloat => uncompressed::encode_rg11b10_ufloat(pixels, w, h),
        TextureFormat::Bc6hRgbUfloat => bc6h::encode_bc6h(pixels, w, h),
        TextureFormat::Bc6hRgbFloat => bc6h::encode_bc6h_signed(pixels, w, h),
        TextureFormat::Bc7RgbaUnorm | TextureFormat::Bc7RgbaUnormSrgb => {
            bc7::encode_bc7(pixels, w, h)
        }
        TextureFormat::Etc2Rgb8Unorm | TextureFormat::Etc2Rgb8UnormSrgb => {
            etc::encode_etc2_rgb8(pixels, w, h)
        }
        TextureFormat::Etc2Rgb8A1Unorm | TextureFormat::Etc2Rgb8A1UnormSrgb => {
            etc::encode_etc2_rgb8_a1(pixels, w, h)
        }
        TextureFormat::Etc2Rgba8Unorm | TextureFormat::Etc2Rgba8UnormSrgb => {
            etc::encode_etc2_rgba8(pixels, w, h)
        }
        TextureFormat::EacR11Unorm => {
            etc::encode_eac_r11(pixels, w, h)
        }
        TextureFormat::EacR11Snorm => {
            etc::encode_eac_r11_snorm(pixels, w, h)
        }
        TextureFormat::EacRg11Unorm => {
            etc::encode_eac_rg11(pixels, w, h)
        }
        TextureFormat::EacRg11Snorm => {
            etc::encode_eac_rg11_snorm(pixels, w, h)
        }
        TextureFormat::Astc4x4Unorm | TextureFormat::Astc4x4UnormSrgb => {
            astc::encode_astc_ldr_batch(pixels, w, h, 4, 4)
        }
        TextureFormat::Astc4x4Hdr => {
            astc::encode_astc_hdr_batch(pixels, w, h, 4, 4)
        }
        TextureFormat::Astc5x4Unorm | TextureFormat::Astc5x4UnormSrgb => {
            astc::encode_astc_ldr_batch(pixels, w, h, 5, 4)
        }
        TextureFormat::Astc5x4Hdr => {
            astc::encode_astc_hdr_batch(pixels, w, h, 5, 4)
        }
        TextureFormat::Astc5x5Unorm | TextureFormat::Astc5x5UnormSrgb => {
            astc::encode_astc_ldr_batch(pixels, w, h, 5, 5)
        }
        TextureFormat::Astc5x5Hdr => {
            astc::encode_astc_hdr_batch(pixels, w, h, 5, 5)
        }
        TextureFormat::Astc6x5Unorm | TextureFormat::Astc6x5UnormSrgb => {
            astc::encode_astc_ldr_batch(pixels, w, h, 6, 5)
        }
        TextureFormat::Astc6x5Hdr => {
            astc::encode_astc_hdr_batch(pixels, w, h, 6, 5)
        }
        TextureFormat::Astc6x6Unorm | TextureFormat::Astc6x6UnormSrgb => {
            astc::encode_astc_ldr_batch(pixels, w, h, 6, 6)
        }
        TextureFormat::Astc6x6Hdr => {
            astc::encode_astc_hdr_batch(pixels, w, h, 6, 6)
        }
        TextureFormat::Astc8x5Unorm | TextureFormat::Astc8x5UnormSrgb => {
            astc::encode_astc_ldr_batch(pixels, w, h, 8, 5)
        }
        TextureFormat::Astc8x5Hdr => {
            astc::encode_astc_hdr_batch(pixels, w, h, 8, 5)
        }
        TextureFormat::Astc8x6Unorm | TextureFormat::Astc8x6UnormSrgb => {
            astc::encode_astc_ldr_batch(pixels, w, h, 8, 6)
        }
        TextureFormat::Astc8x6Hdr => {
            astc::encode_astc_hdr_batch(pixels, w, h, 8, 6)
        }
        TextureFormat::Astc8x8Unorm | TextureFormat::Astc8x8UnormSrgb => {
            astc::encode_astc_ldr_batch(pixels, w, h, 8, 8)
        }
        TextureFormat::Astc8x8Hdr => {
            astc::encode_astc_hdr_batch(pixels, w, h, 8, 8)
        }
        TextureFormat::Astc10x5Unorm | TextureFormat::Astc10x5UnormSrgb => {
            astc::encode_astc_ldr_batch(pixels, w, h, 10, 5)
        }
        TextureFormat::Astc10x5Hdr => {
            astc::encode_astc_hdr_batch(pixels, w, h, 10, 5)
        }
        TextureFormat::Astc10x6Unorm | TextureFormat::Astc10x6UnormSrgb => {
            astc::encode_astc_ldr_batch(pixels, w, h, 10, 6)
        }
        TextureFormat::Astc10x6Hdr => {
            astc::encode_astc_hdr_batch(pixels, w, h, 10, 6)
        }
        TextureFormat::Astc10x8Unorm | TextureFormat::Astc10x8UnormSrgb => {
            astc::encode_astc_ldr_batch(pixels, w, h, 10, 8)
        }
        TextureFormat::Astc10x8Hdr => {
            astc::encode_astc_hdr_batch(pixels, w, h, 10, 8)
        }
        TextureFormat::Astc10x10Unorm | TextureFormat::Astc10x10UnormSrgb => {
            astc::encode_astc_ldr_batch(pixels, w, h, 10, 10)
        }
        TextureFormat::Astc10x10Hdr => {
            astc::encode_astc_hdr_batch(pixels, w, h, 10, 10)
        }
        TextureFormat::Astc12x10Unorm | TextureFormat::Astc12x10UnormSrgb => {
            astc::encode_astc_ldr_batch(pixels, w, h, 12, 10)
        }
        TextureFormat::Astc12x10Hdr => {
            astc::encode_astc_hdr_batch(pixels, w, h, 12, 10)
        }
        TextureFormat::Astc12x12Unorm | TextureFormat::Astc12x12UnormSrgb => {
            astc::encode_astc_ldr_batch(pixels, w, h, 12, 12)
        }
        TextureFormat::Astc12x12Hdr => {
            astc::encode_astc_hdr_batch(pixels, w, h, 12, 12)
        }
        _ => todo!("encode not yet implemented for {format:?}"),
    }
}

/// Decode encoded texture data back to `PixelDatas` (RGBA).
pub fn decode(data: &PixelDatas, width: u32, height: u32, format: TextureFormat) -> PixelDatas {
    let w = width as usize;
    let h = height as usize;
    match format {
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb => data.convert_to_u8(),
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
        TextureFormat::Rgba8Snorm | TextureFormat::Rgba8Uint | TextureFormat::Rgba8Sint => {
            // Pass-through — same as Rgba8Unorm
            data.convert_to_u8()
        }
        TextureFormat::Bgra8Unorm | TextureFormat::Bgra8UnormSrgb => {
            uncompressed::decode_bgra8(data, w, h)
        }
        TextureFormat::R16Uint => uncompressed::decode_r16u(data, w, h),
        TextureFormat::R16Sint => uncompressed::decode_r16s(data, w, h),
        TextureFormat::R16Float => uncompressed::decode_r16f(data, w, h),
        TextureFormat::Rg16Uint => uncompressed::decode_rg16u(data, w, h),
        TextureFormat::Rg16Sint => uncompressed::decode_rg16s(data, w, h),
        TextureFormat::Rg16Float => uncompressed::decode_rg16f(data, w, h),
        TextureFormat::Rgba16Uint => uncompressed::decode_rgba16u(data, w, h),
        TextureFormat::Rgba16Sint => uncompressed::decode_rgba16s(data, w, h),
        TextureFormat::Rgba16Float => uncompressed::decode_rgba16f(data, w, h),
        TextureFormat::R32Uint => uncompressed::decode_r32u(data, w, h),
        TextureFormat::R32Sint => uncompressed::decode_r32s(data, w, h),
        TextureFormat::R32Float => uncompressed::decode_r32f(data, w, h),
        TextureFormat::Rg32Uint => uncompressed::decode_rg32u(data, w, h),
        TextureFormat::Rg32Sint => uncompressed::decode_rg32s(data, w, h),
        TextureFormat::Rg32Float => uncompressed::decode_rg32f(data, w, h),
        TextureFormat::Rgba32Uint => uncompressed::decode_rgba32u(data, w, h),
        TextureFormat::Rgba32Sint => uncompressed::decode_rgba32s(data, w, h),
        TextureFormat::Rgba32Float => uncompressed::decode_rgba32f(data, w, h),
        TextureFormat::Rgb10a2Unorm => uncompressed::decode_rgb10a2_unorm(data, w, h),
        TextureFormat::Rg11b10Ufloat => uncompressed::decode_rg11b10_ufloat(data, w, h),
        TextureFormat::Bc6hRgbUfloat => bc6h::decode_bc6h(data, w, h),
        TextureFormat::Bc6hRgbFloat => bc6h::decode_bc6h_signed(data, w, h),
        TextureFormat::Bc7RgbaUnorm | TextureFormat::Bc7RgbaUnormSrgb => {
            bc7::decode_bc7(data, w, h)
        }
        TextureFormat::Etc2Rgb8Unorm | TextureFormat::Etc2Rgb8UnormSrgb => {
            etc::decode_etc2_rgb8(data, w, h)
        }
        TextureFormat::Etc2Rgb8A1Unorm | TextureFormat::Etc2Rgb8A1UnormSrgb => {
            etc::decode_etc2_rgb8_a1(data, w, h)
        }
        TextureFormat::Etc2Rgba8Unorm | TextureFormat::Etc2Rgba8UnormSrgb => {
            etc::decode_etc2_rgba8(data, w, h)
        }
        TextureFormat::EacR11Unorm => {
            etc::decode_eac_r11(data, w, h)
        }
        TextureFormat::EacR11Snorm => {
            etc::decode_eac_r11_snorm(data, w, h)
        }
        TextureFormat::EacRg11Unorm => {
            etc::decode_eac_rg11(data, w, h)
        }
        TextureFormat::EacRg11Snorm => {
            etc::decode_eac_rg11_snorm(data, w, h)
        }
        TextureFormat::Astc4x4Unorm | TextureFormat::Astc4x4UnormSrgb => {
            astc::decode_astc_ldr_batch(data, w, h, 4, 4)
        }
        TextureFormat::Astc4x4Hdr => {
            astc::decode_astc_hdr_batch(data, w, h, 4, 4)
        }
        TextureFormat::Astc5x4Unorm | TextureFormat::Astc5x4UnormSrgb => {
            astc::decode_astc_ldr_batch(data, w, h, 5, 4)
        }
        TextureFormat::Astc5x4Hdr => {
            astc::decode_astc_hdr_batch(data, w, h, 5, 4)
        }
        TextureFormat::Astc5x5Unorm | TextureFormat::Astc5x5UnormSrgb => {
            astc::decode_astc_ldr_batch(data, w, h, 5, 5)
        }
        TextureFormat::Astc5x5Hdr => {
            astc::decode_astc_hdr_batch(data, w, h, 5, 5)
        }
        TextureFormat::Astc6x5Unorm | TextureFormat::Astc6x5UnormSrgb => {
            astc::decode_astc_ldr_batch(data, w, h, 6, 5)
        }
        TextureFormat::Astc6x5Hdr => {
            astc::decode_astc_hdr_batch(data, w, h, 6, 5)
        }
        TextureFormat::Astc6x6Unorm | TextureFormat::Astc6x6UnormSrgb => {
            astc::decode_astc_ldr_batch(data, w, h, 6, 6)
        }
        TextureFormat::Astc6x6Hdr => {
            astc::decode_astc_hdr_batch(data, w, h, 6, 6)
        }
        TextureFormat::Astc8x5Unorm | TextureFormat::Astc8x5UnormSrgb => {
            astc::decode_astc_ldr_batch(data, w, h, 8, 5)
        }
        TextureFormat::Astc8x5Hdr => {
            astc::decode_astc_hdr_batch(data, w, h, 8, 5)
        }
        TextureFormat::Astc8x6Unorm | TextureFormat::Astc8x6UnormSrgb => {
            astc::decode_astc_ldr_batch(data, w, h, 8, 6)
        }
        TextureFormat::Astc8x6Hdr => {
            astc::decode_astc_hdr_batch(data, w, h, 8, 6)
        }
        TextureFormat::Astc8x8Unorm | TextureFormat::Astc8x8UnormSrgb => {
            astc::decode_astc_ldr_batch(data, w, h, 8, 8)
        }
        TextureFormat::Astc8x8Hdr => {
            astc::decode_astc_hdr_batch(data, w, h, 8, 8)
        }
        TextureFormat::Astc10x5Unorm | TextureFormat::Astc10x5UnormSrgb => {
            astc::decode_astc_ldr_batch(data, w, h, 10, 5)
        }
        TextureFormat::Astc10x5Hdr => {
            astc::decode_astc_hdr_batch(data, w, h, 10, 5)
        }
        TextureFormat::Astc10x6Unorm | TextureFormat::Astc10x6UnormSrgb => {
            astc::decode_astc_ldr_batch(data, w, h, 10, 6)
        }
        TextureFormat::Astc10x6Hdr => {
            astc::decode_astc_hdr_batch(data, w, h, 10, 6)
        }
        TextureFormat::Astc10x8Unorm | TextureFormat::Astc10x8UnormSrgb => {
            astc::decode_astc_ldr_batch(data, w, h, 10, 8)
        }
        TextureFormat::Astc10x8Hdr => {
            astc::decode_astc_hdr_batch(data, w, h, 10, 8)
        }
        TextureFormat::Astc10x10Unorm | TextureFormat::Astc10x10UnormSrgb => {
            astc::decode_astc_ldr_batch(data, w, h, 10, 10)
        }
        TextureFormat::Astc10x10Hdr => {
            astc::decode_astc_hdr_batch(data, w, h, 10, 10)
        }
        TextureFormat::Astc12x10Unorm | TextureFormat::Astc12x10UnormSrgb => {
            astc::decode_astc_ldr_batch(data, w, h, 12, 10)
        }
        TextureFormat::Astc12x10Hdr => {
            astc::decode_astc_hdr_batch(data, w, h, 12, 10)
        }
        TextureFormat::Astc12x12Unorm | TextureFormat::Astc12x12UnormSrgb => {
            astc::decode_astc_ldr_batch(data, w, h, 12, 12)
        }
        TextureFormat::Astc12x12Hdr => {
            astc::decode_astc_hdr_batch(data, w, h, 12, 12)
        }
        _ => todo!("decode not yet implemented for {format:?}"),
    }
}
