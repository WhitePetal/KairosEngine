use bytemuck;
use rayon::prelude::*;

use crate::graphics::texture::format::PixelDatas;
use half::f16;

/// Encode RGBA8 to single-channel R8 by extracting the R channel.
///
/// The same encoding is used for Unorm, Snorm, Uint, and Sint —
/// the byte values are identical; only the GPU sampling interpretation differs.
fn encode_r8_impl(rgba: &[u8], width: usize, height: usize) -> Vec<u8> {
    const CHUNK_SIZE: usize = 4096;
    let mut out = vec![0u8; width * height];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, data) in chunk.iter_mut().enumerate() {
                *data = rgba[(pixel_base + j) << 2];
            }
        });
    out
}

/// Decode single-channel data back to RGBA8.
///
/// `fill_g` / `fill_b` / `fill_a` control which output channels receive
/// the source value (channel 0 / R always does).
///
/// The same decoding is used for Unorm, Snorm, Uint, and Sint.
fn decode_r8_impl(
    data: &[u8],
    width: usize,
    height: usize,
    fill_g: bool,
    fill_b: bool,
    fill_a: bool,
) -> Vec<u8> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u8; pixel_count << 2];

    match (fill_g, fill_b, fill_a) {
        (true, true, true) => {
            out.par_chunks_mut(CHUNK_SIZE)
                .enumerate()
                .for_each(|(chunk_idx, chunk)| {
                    let byte_base = chunk_idx * CHUNK_SIZE;
                    for (j, rgba) in chunk.iter_mut().enumerate() {
                        let pixel_idx = (byte_base + j) >> 2;
                        *rgba = data[pixel_idx];
                    }
                });
        }
        (false, false, false) => {
            out.par_chunks_mut(CHUNK_SIZE)
                .enumerate()
                .for_each(|(chunk_idx, chunk)| {
                    let byte_base = chunk_idx * CHUNK_SIZE;
                    for (j, rgba) in chunk.iter_mut().enumerate() {
                        if (byte_base + j) % 4 == 0 {
                            *rgba = data[(byte_base + j) >> 2];
                        }
                    }
                });
        }
        _ => {
            out.par_chunks_mut(CHUNK_SIZE)
                .enumerate()
                .for_each(|(chunk_idx, chunk)| {
                    let byte_base = chunk_idx * CHUNK_SIZE;
                    for (j, rgba) in chunk.iter_mut().enumerate() {
                        let idx = byte_base + j;
                        let channel = idx % 4;
                        if channel == 0
                            || (channel == 1 && fill_g)
                            || (channel == 2 && fill_b)
                            || (channel == 3 && fill_a)
                        {
                            *rgba = data[idx >> 2];
                        }
                    }
                });
        }
    }

    out
}

// ============================================================
// Public API — single encode / decode pair for all R8 variants
// ============================================================

pub fn encode_r8(pixels: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    PixelDatas::U8(encode_r8_impl(&pixels.convert_to_u8_bytes(), w, h))
}

pub fn decode_r8(
    data: &PixelDatas,
    w: usize,
    h: usize,
    fill_g: bool,
    fill_b: bool,
    fill_a: bool,
) -> PixelDatas {
    let datas = data.convert_to_u8_bytes();
    PixelDatas::U8(decode_r8_impl(&datas, w, h, fill_g, fill_b, fill_a))
}

// ============================================================
// RG8 — two-channel 8-bit (Rg8Unorm, Rg8Snorm, Rg8Uint, Rg8Sint)
// ============================================================

/// Encode RGBA8 to two-channel RG8 by extracting R and G channels.
///
/// Output is 2 bytes per pixel: [R, G].
/// The same encoding is used for Unorm, Snorm, Uint, and Sint.
fn encode_rg8_impl(rgba: &[u8], width: usize, height: usize) -> Vec<u8> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u8; pixel_count << 1];
    out.par_chunks_mut(CHUNK_SIZE << 1)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, pair) in chunk.chunks_mut(2).enumerate() {
                let abs_pixel = pixel_base + j;
                let src_idx = abs_pixel << 2;
                pair[0] = rgba[src_idx]; // R
                pair[1] = rgba[src_idx + 1]; // G
            }
        });
    out
}

/// Decode two-channel RG8 back to RGBA8.
///
/// Output is 4 bytes per pixel: [R, G, 0, 255] (B=0, A=255).
/// The same decoding is used for all RG8 variants.
fn decode_rg8_impl(data: &[u8], width: usize, height: usize) -> Vec<u8> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u8; pixel_count << 2];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * (CHUNK_SIZE >> 2);
            for (j, rgba) in chunk.chunks_mut(4).enumerate() {
                let abs_pixel = pixel_base + j;
                let src_idx = abs_pixel << 1;
                rgba[0] = data[src_idx]; // R
                rgba[1] = data[src_idx + 1]; // G
                rgba[2] = 0; // B
                rgba[3] = 0; // A
            }
        });
    out
}

pub fn encode_rg8(pixels: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    PixelDatas::U8(encode_rg8_impl(&pixels.convert_to_u8_bytes(), w, h))
}

pub fn decode_rg8(data: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = data.convert_to_u8_bytes();
    PixelDatas::U8(decode_rg8_impl(&datas, w, h))
}

// ============================================================
// Bgra8 — R↔B swizzle (Bgra8Unorm, Bgra8UnormSrgb)
// ============================================================

/// Encode RGBA8 to BGRA8 by swapping R and B channels.
///
/// Output is 4 bytes per pixel: [B, G, R, A].
/// For Bgra8UnormSrgb, the GPU handles sRGB conversion;
/// the swizzle is identical for both variants.
fn encode_bgra8_impl(rgba: &[u8], width: usize, height: usize) -> Vec<u8> {
    const CHUNK_SIZE: usize = 1024;
    let pixel_count = width * height;
    let mut out = vec![0u8; pixel_count << 2];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * (CHUNK_SIZE >> 2);
            for (j, bgra) in chunk.chunks_mut(4).enumerate() {
                let abs_pixel = pixel_base + j;
                let src_idx = abs_pixel << 2;
                bgra[0] = rgba[src_idx + 2]; // B
                bgra[1] = rgba[src_idx + 1]; // G
                bgra[2] = rgba[src_idx]; // R
                bgra[3] = rgba[src_idx + 3]; // A
            }
        });
    out
}

/// Decode BGRA8 back to RGBA8 by swapping R and B channels.
fn decode_bgra8_impl(data: &[u8], width: usize, height: usize) -> Vec<u8> {
    const CHUNK_SIZE: usize = 1024;
    let pixel_count = width * height;
    let mut out = vec![0u8; pixel_count << 2];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * (CHUNK_SIZE >> 2);
            for (j, rgba) in chunk.chunks_mut(4).enumerate() {
                let abs_pixel = pixel_base + j;
                let src_idx = abs_pixel << 2;
                rgba[0] = data[src_idx + 2]; // R (was B)
                rgba[1] = data[src_idx + 1]; // G (was G)
                rgba[2] = data[src_idx]; // B (was R)
                rgba[3] = data[src_idx + 3]; // A (was A)
            }
        });
    out
}

pub fn encode_bgra8(pixels: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    PixelDatas::U8(encode_bgra8_impl(&pixels.convert_to_u8_bytes(), w, h))
}

pub fn decode_bgra8(data: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = data.convert_to_u8_bytes();
    PixelDatas::U8(decode_bgra8_impl(&datas, w, h))
}

// ============================================================
// R16 integer formats (R16Uint, R16Sint)
// ============================================================

/// Encode pixel to R16Uint by zero-extending the R channel.
///
/// Output is 1 u16 per pixel.
fn encode_r16u_impl(pixels: &[u16], width: usize, height: usize) -> Vec<u16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u16; pixel_count];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, dst) in chunk.iter_mut().enumerate() {
                let src_idx = (pixel_base + j) << 2;
                *dst = pixels[src_idx];
            }
        });
    out
}

/// Encode RGBA8 to R16Sint by sign-extending the R channel.
///
/// Output is 1 u16 per pixel.
fn encode_r16s_impl(pixels: &[i16], width: usize, height: usize) -> Vec<i16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0i16; pixel_count];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, dst) in chunk.iter_mut().enumerate() {
                let src_idx = (pixel_base + j) << 2;
                *dst = pixels[src_idx];
            }
        });
    out
}

/// Decode R16 Uint back to RGBA16.
///
/// Input is 1 u16 per pixel.
/// Output is 4 u16 per pixel: [R, 0, 0, 65535].
fn decode_r16_impl(data: &[u16], width: usize, height: usize) -> Vec<u16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u16; pixel_count << 2];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * (CHUNK_SIZE >> 2);
            for (j, rgba) in chunk.chunks_mut(4).enumerate() {
                let abs_pixel = pixel_base + j;
                rgba[0] = data[abs_pixel];
                rgba[1] = 0;
                rgba[2] = 0;
                rgba[3] = 0;
            }
        });
    out
}

/// Decode R16 Sint back to RGBA16.
///
/// Input is 1 u16 per pixel.
/// Output is 4 u16 per pixel: [R, 0, 0, 32767].
fn decode_r16s_impl(data: &[i16], width: usize, height: usize) -> Vec<i16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0i16; pixel_count << 2];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * (CHUNK_SIZE >> 2);
            for (j, rgba) in chunk.chunks_mut(4).enumerate() {
                let abs_pixel = pixel_base + j;
                rgba[0] = data[abs_pixel];
                rgba[1] = 0;
                rgba[2] = 0;
                rgba[3] = 0;
            }
        });
    out
}

pub fn encode_r16u(pixels: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = pixels.convert_to_u16_bytes();
    PixelDatas::U16(encode_r16u_impl(&datas, w, h))
}

pub fn encode_r16s(pixels: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = pixels.convert_to_s16_bytes();
    PixelDatas::S16(encode_r16s_impl(&datas, w, h))
}

pub fn decode_r16u(data: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = data.convert_to_u16_bytes();
    PixelDatas::U16(decode_r16_impl(&datas, w, h))
}

pub fn decode_r16s(data: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = data.convert_to_s16_bytes();
    PixelDatas::S16(decode_r16s_impl(&datas, w, h))
}

// ============================================================
// Rg16 integer formats (Rg16Uint, Rg16Sint)
// ============================================================

/// Encode RGBA8 to Rg16Uint by zero-extending the R and G channels.
///
/// Output is 2 u16 per pixel: [R, G].
fn encode_rg16u_impl(rgba: &[u16], width: usize, height: usize) -> Vec<u16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u16; pixel_count << 1];
    out.par_chunks_mut(CHUNK_SIZE << 1)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, pair) in chunk.chunks_mut(2).enumerate() {
                let src_idx = (pixel_base + j) << 2;
                pair[0] = rgba[src_idx];
                pair[1] = rgba[src_idx + 1];
            }
        });
    out
}

/// Encode RGBA8 to Rg16Sint by sign-extending the R and G channels.
///
/// Output is 2 u16 per pixel: [R, G].
fn encode_rg16s_impl(rgba: &[i16], width: usize, height: usize) -> Vec<i16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0i16; pixel_count << 1];
    out.par_chunks_mut(CHUNK_SIZE << 1)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, pair) in chunk.chunks_mut(2).enumerate() {
                let src_idx = (pixel_base + j) << 2;
                pair[0] = rgba[src_idx];
                pair[1] = rgba[src_idx + 1];
            }
        });
    out
}

/// Decode Rg16 Uint back to RGBA16.
///
/// Input is 2 u16 per pixel: [R, G].
/// Output is 4 u16 per pixel: [R, G, 0, 65535].
fn decode_rg16u_impl(data: &[u16], width: usize, height: usize) -> Vec<u16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u16; pixel_count << 2];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * (CHUNK_SIZE >> 2);
            for (j, rgba) in chunk.chunks_mut(4).enumerate() {
                let abs_pixel = pixel_base + j;
                let src_idx = abs_pixel << 1;
                rgba[0] = data[src_idx];
                rgba[1] = data[src_idx + 1];
                rgba[2] = 0;
                rgba[3] = 0;
            }
        });
    out
}

/// Decode Rg16 Sint back to RGBA16.
///
/// Input is 2 u16 per pixel: [R, G].
/// Output is 4 u16 per pixel: [R, G, 0, 32767].
fn decode_rg16s_impl(data: &[i16], width: usize, height: usize) -> Vec<i16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0i16; pixel_count << 2];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * (CHUNK_SIZE >> 2);
            for (j, rgba) in chunk.chunks_mut(4).enumerate() {
                let abs_pixel = pixel_base + j;
                let src_idx = abs_pixel << 1;
                rgba[0] = data[src_idx];
                rgba[1] = data[src_idx + 1];
                rgba[2] = 0;
                rgba[3] = 0;
            }
        });
    out
}

pub fn encode_rg16u(pixels: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = pixels.convert_to_u16_bytes();
    PixelDatas::U16(encode_rg16u_impl(&datas, w, h))
}

pub fn encode_rg16s(pixels: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = pixels.convert_to_s16_bytes();
    PixelDatas::S16(encode_rg16s_impl(&datas, w, h))
}

pub fn decode_rg16u(data: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = data.convert_to_u16_bytes();
    PixelDatas::U16(decode_rg16u_impl(&datas, w, h))
}

pub fn decode_rg16s(data: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = data.convert_to_s16_bytes();
    PixelDatas::S16(decode_rg16s_impl(&datas, w, h))
}

// ============================================================
// Rgba16 integer formats (Rgba16Uint, Rgba16Sint)
// ============================================================

/// Encode RGBA8 to Rgba16Uint by zero-extending all channels.
///
/// Output is 4 u16 per pixel: [R, G, B, A].
fn encode_rgba16u_impl(rgba: &[u16], width: usize, height: usize) -> Vec<u16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u16; pixel_count << 2];
    out.par_chunks_mut(CHUNK_SIZE << 2)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, quad) in chunk.chunks_mut(4).enumerate() {
                let src_idx = (pixel_base + j) << 2;
                quad[0] = rgba[src_idx];
                quad[1] = rgba[src_idx + 1];
                quad[2] = rgba[src_idx + 2];
                quad[3] = rgba[src_idx + 3];
            }
        });
    out
}

/// Encode RGBA8 to Rgba16Sint by sign-extending all channels.
///
/// Output is 4 u16 per pixel: [R, G, B, A].
fn encode_rgba16s_impl(rgba: &[i16], width: usize, height: usize) -> Vec<i16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0i16; pixel_count << 2];
    out.par_chunks_mut(CHUNK_SIZE << 2)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, quad) in chunk.chunks_mut(4).enumerate() {
                let src_idx = (pixel_base + j) << 2;
                quad[0] = rgba[src_idx];
                quad[1] = rgba[src_idx + 1];
                quad[2] = rgba[src_idx + 2];
                quad[3] = rgba[src_idx + 3];
            }
        });
    out
}

/// Decode Rgba16 Uint back to RGBA16.
///
/// Input is 4 u16 per pixel: [R, G, B, A].
/// Output is 4 u16 per pixel: [R, G, B, A].
fn decode_rgba16u_impl(data: &[u16], width: usize, height: usize) -> Vec<u16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u16; pixel_count << 2];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * (CHUNK_SIZE >> 2);
            for (j, rgba) in chunk.chunks_mut(4).enumerate() {
                let abs_pixel = pixel_base + j;
                let src_idx = abs_pixel << 2;
                rgba.copy_from_slice(&data[src_idx..src_idx + 4]);
            }
        });
    out
}

/// Decode Rgba16 Sint back to RGBA16.
///
/// Input is 4 u16 per pixel: [R, G, B, A].
/// Output is 4 u16 per pixel: [R, G, B, A].
fn decode_rgba16s_impl(data: &[i16], width: usize, height: usize) -> Vec<i16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0i16; pixel_count << 2];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * (CHUNK_SIZE >> 2);
            for (j, rgba) in chunk.chunks_mut(4).enumerate() {
                let abs_pixel = pixel_base + j;
                let src_idx = abs_pixel << 2;
                rgba.copy_from_slice(&data[src_idx..src_idx + 4]);
            }
        });
    out
}

pub fn encode_rgba16u(pixels: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = pixels.convert_to_u16_bytes();
    PixelDatas::U16(encode_rgba16u_impl(&datas, w, h))
}

pub fn encode_rgba16s(pixels: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = pixels.convert_to_s16_bytes();
    PixelDatas::S16(encode_rgba16s_impl(&datas, w, h))
}

pub fn decode_rgba16u(data: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = data.convert_to_u16_bytes();
    PixelDatas::U16(decode_rgba16u_impl(&datas, w, h))
}

pub fn decode_rgba16s(data: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = data.convert_to_s16_bytes();
    PixelDatas::S16(decode_rgba16s_impl(&datas, w, h))
}

// ============================================================
// R16Float — single-channel half-float
// ============================================================

/// Encode RGBA half-float to R16Float by extracting the R channel.
///
/// Output is 1 f16 per pixel.
fn encode_r16f_impl(rgba: &[f16], width: usize, height: usize) -> Vec<f16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![f16::ZERO; pixel_count];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, dst) in chunk.iter_mut().enumerate() {
                *dst = rgba[(pixel_base + j) << 2];
            }
        });
    out
}

/// Decode R16Float back to RGBA half-float.
///
/// Input is 1 f16 per pixel.
/// Output is 4 f16 per pixel: [R, 0, 0, 1.0].
fn decode_r16f_impl(data: &[f16], width: usize, height: usize) -> Vec<f16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![f16::ZERO; pixel_count << 2];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * (CHUNK_SIZE >> 2);
            for (j, rgba) in chunk.chunks_mut(4).enumerate() {
                let abs_pixel = pixel_base + j;
                rgba[0] = data[abs_pixel];
                rgba[1] = f16::ZERO;
                rgba[2] = f16::ZERO;
                rgba[3] = f16::from_f32(0.0);
            }
        });
    out
}

pub fn encode_r16f(pixels: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = pixels.convert_to_f16_bytes();
    PixelDatas::F16(encode_r16f_impl(&datas, w, h))
}

pub fn decode_r16f(data: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = data.convert_to_f16_bytes();
    PixelDatas::F16(decode_r16f_impl(&datas, w, h))
}

// ============================================================
// Rg16Float — two-channel half-float
// ============================================================

/// Encode RGBA half-float to Rg16Float by extracting the R and G channels.
///
/// Output is 2 f16 per pixel: [R, G].
fn encode_rg16f_impl(rgba: &[f16], width: usize, height: usize) -> Vec<f16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![f16::ZERO; pixel_count << 1];
    out.par_chunks_mut(CHUNK_SIZE << 1)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, pair) in chunk.chunks_mut(2).enumerate() {
                let src_idx = (pixel_base + j) << 2;
                pair[0] = rgba[src_idx]; // R
                pair[1] = rgba[src_idx + 1]; // G
            }
        });
    out
}

/// Decode Rg16Float back to RGBA half-float.
///
/// Input is 2 f16 per pixel: [R, G].
/// Output is 4 f16 per pixel: [R, G, 0, 1.0].
fn decode_rg16f_impl(data: &[f16], width: usize, height: usize) -> Vec<f16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![f16::ZERO; pixel_count << 2];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * (CHUNK_SIZE >> 2);
            for (j, rgba) in chunk.chunks_mut(4).enumerate() {
                let abs_pixel = pixel_base + j;
                let src_idx = abs_pixel << 1;
                rgba[0] = data[src_idx]; // R
                rgba[1] = data[src_idx + 1]; // G
                rgba[2] = f16::ZERO;
                rgba[3] = f16::from_f32(0.0);
            }
        });
    out
}

pub fn encode_rg16f(pixels: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = pixels.convert_to_f16_bytes();
    PixelDatas::F16(encode_rg16f_impl(&datas, w, h))
}

pub fn decode_rg16f(data: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = data.convert_to_f16_bytes();
    PixelDatas::F16(decode_rg16f_impl(&datas, w, h))
}

// ============================================================
// Rgba16Float — four-channel half-float
// ============================================================

/// Encode RGBA half-float to Rgba16Float (passthrough).
///
/// Output is 4 f16 per pixel: [R, G, B, A].
fn encode_rgba16f_from_f16_impl(rgba: &[f16], width: usize, height: usize) -> Vec<f16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![f16::ZERO; pixel_count << 2];
    out.par_chunks_mut(CHUNK_SIZE << 2)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, quad) in chunk.chunks_mut(4).enumerate() {
                let src_idx = (pixel_base + j) << 2;
                quad.copy_from_slice(&rgba[src_idx..src_idx + 4]);
            }
        });
    out
}

/// Decode Rgba16Float back to RGBA half-float (passthrough).
///
/// Input is 4 f16 per pixel: [R, G, B, A].
/// Output is 4 f16 per pixel: [R, G, B, A].
fn decode_rgba16f_impl(data: &[f16], width: usize, height: usize) -> Vec<f16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![f16::ZERO; pixel_count << 2];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * (CHUNK_SIZE >> 2);
            for (j, rgba) in chunk.chunks_mut(4).enumerate() {
                let abs_pixel = pixel_base + j;
                let src_idx = abs_pixel << 2;
                rgba.copy_from_slice(&data[src_idx..src_idx + 4]);
            }
        });
    out
}

pub fn encode_rgba16f(pixels: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = pixels.convert_to_f16_bytes();
    PixelDatas::F16(encode_rgba16f_from_f16_impl(&datas, w, h))
}

pub fn decode_rgba16f(data: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = data.convert_to_f16_bytes();
    PixelDatas::F16(decode_rgba16f_impl(&datas, w, h))
}

// ============================================================
// R32Uint — single-channel 32-bit unsigned integer
// Input: RGBA8 (u8), output: 1 u32 per pixel stored as LE bytes in U8
// Decode: 1 u32 → [R as f32, 0, 0, 0] in F32
// ============================================================

fn encode_r32u_impl(rgba: &[u8], width: usize, height: usize) -> Vec<u8> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u8; pixel_count * 4];
    out.par_chunks_mut(CHUNK_SIZE * 4)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, word) in chunk.chunks_mut(4).enumerate() {
                let src_idx = (pixel_base + j) << 2;
                let r = rgba[src_idx] as u32;
                word.copy_from_slice(&r.to_le_bytes());
            }
        });
    out
}

fn decode_r32u_impl(data: &[u8], width: usize, height: usize) -> Vec<f32> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0.0f32; pixel_count << 2];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * (CHUNK_SIZE >> 2);
            for (j, rgba) in chunk.chunks_mut(4).enumerate() {
                let abs_pixel = pixel_base + j;
                let src_idx = abs_pixel * 4;
                let r = u32::from_le_bytes([data[src_idx], data[src_idx + 1], data[src_idx + 2], data[src_idx + 3]]);
                rgba[0] = r as f32;
                rgba[1] = 0.0;
                rgba[2] = 0.0;
                rgba[3] = 0.0;
            }
        });
    out
}

pub fn encode_r32u(pixels: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = pixels.convert_to_u8_bytes();
    PixelDatas::U8(encode_r32u_impl(&datas, w, h))
}

pub fn decode_r32u(data: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = data.convert_to_u8_bytes();
    PixelDatas::F32(decode_r32u_impl(&datas, w, h))
}

// ============================================================
// R32Sint — single-channel 32-bit signed integer
// Input: RGBA8 signed (s8), output: 1 i32 per pixel stored as LE bytes in S8
// Decode: 1 i32 → [R as f32, 0, 0, 0] in F32
// ============================================================

fn encode_r32s_impl(rgba: &[i16], width: usize, height: usize) -> Vec<i8> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0i8; pixel_count * 4];
    out.par_chunks_mut(CHUNK_SIZE * 4)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, word) in chunk.chunks_mut(4).enumerate() {
                let src_idx = (pixel_base + j) << 2;
                let r = rgba[src_idx] as i32;
                let le = r.to_le_bytes();
                word[0] = le[0] as i8;
                word[1] = le[1] as i8;
                word[2] = le[2] as i8;
                word[3] = le[3] as i8;
            }
        });
    out
}

fn decode_r32s_impl(data: &[i8], width: usize, height: usize) -> Vec<f32> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0.0f32; pixel_count << 2];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * (CHUNK_SIZE >> 2);
            for (j, rgba) in chunk.chunks_mut(4).enumerate() {
                let abs_pixel = pixel_base + j;
                let src_idx = abs_pixel * 4;
                let bytes: [u8; 4] = [
                    data[src_idx] as u8,
                    data[src_idx + 1] as u8,
                    data[src_idx + 2] as u8,
                    data[src_idx + 3] as u8,
                ];
                let r = i32::from_le_bytes(bytes);
                rgba[0] = r as f32;
                rgba[1] = 0.0;
                rgba[2] = 0.0;
                rgba[3] = 0.0;
            }
        });
    out
}

pub fn encode_r32s(pixels: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = pixels.convert_to_s16_bytes();
    PixelDatas::S8(encode_r32s_impl(&datas, w, h))
}

pub fn decode_r32s(data: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let raw: Vec<i8> = bytemuck::cast_slice(data.as_bytes()).to_vec();
    PixelDatas::F32(decode_r32s_impl(&raw, w, h))
}

// ============================================================
// R32Float — single-channel 32-bit float
// Input: RGBA f32, output: 1 f32 per pixel
// Decode: 1 f32 → [R, 0, 0, 1.0] in F32
// ============================================================

fn encode_r32f_impl(rgba: &[f32], width: usize, height: usize) -> Vec<f32> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0.0f32; pixel_count];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, dst) in chunk.iter_mut().enumerate() {
                *dst = rgba[(pixel_base + j) << 2];
            }
        });
    out
}

fn decode_r32f_impl(data: &[f32], width: usize, height: usize) -> Vec<f32> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0.0f32; pixel_count << 2];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * (CHUNK_SIZE >> 2);
            for (j, rgba) in chunk.chunks_mut(4).enumerate() {
                let abs_pixel = pixel_base + j;
                rgba[0] = data[abs_pixel];
                rgba[1] = 0.0;
                rgba[2] = 0.0;
                rgba[3] = 1.0;
            }
        });
    out
}

pub fn encode_r32f(pixels: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = pixels.convert_to_f32_bytes();
    PixelDatas::F32(encode_r32f_impl(&datas, w, h))
}

pub fn decode_r32f(data: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = data.convert_to_f32_bytes();
    PixelDatas::F32(decode_r32f_impl(&datas, w, h))
}

// ============================================================
// Rg32Uint — two-channel 32-bit unsigned integer
// Input: RGBA8 (u8), output: 2 u32 per pixel stored as LE bytes in U8
// Decode: 2 u32 → [R as f32, G as f32, 0, 0] in F32
// ============================================================

fn encode_rg32u_impl(rgba: &[u8], width: usize, height: usize) -> Vec<u8> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u8; pixel_count * 8];
    out.par_chunks_mut(CHUNK_SIZE * 8)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, pair) in chunk.chunks_mut(8).enumerate() {
                let src_idx = (pixel_base + j) << 2;
                let r = rgba[src_idx] as u32;
                let g = rgba[src_idx + 1] as u32;
                pair[..4].copy_from_slice(&r.to_le_bytes());
                pair[4..].copy_from_slice(&g.to_le_bytes());
            }
        });
    out
}

fn decode_rg32u_impl(data: &[u8], width: usize, height: usize) -> Vec<f32> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0.0f32; pixel_count << 2];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * (CHUNK_SIZE >> 2);
            for (j, rgba) in chunk.chunks_mut(4).enumerate() {
                let abs_pixel = pixel_base + j;
                let src_idx = abs_pixel * 8;
                let r = u32::from_le_bytes([data[src_idx], data[src_idx + 1], data[src_idx + 2], data[src_idx + 3]]);
                let g = u32::from_le_bytes([data[src_idx + 4], data[src_idx + 5], data[src_idx + 6], data[src_idx + 7]]);
                rgba[0] = r as f32;
                rgba[1] = g as f32;
                rgba[2] = 0.0;
                rgba[3] = 0.0;
            }
        });
    out
}

pub fn encode_rg32u(pixels: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = pixels.convert_to_u8_bytes();
    PixelDatas::U8(encode_rg32u_impl(&datas, w, h))
}

pub fn decode_rg32u(data: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = data.convert_to_u8_bytes();
    PixelDatas::F32(decode_rg32u_impl(&datas, w, h))
}

// ============================================================
// Rg32Sint — two-channel 32-bit signed integer
// Input: RGBA signed (s8), output: 2 i32 per pixel stored as LE bytes in S8
// Decode: 2 i32 → [R as f32, G as f32, 0, 0] in F32
// ============================================================

fn encode_rg32s_impl(rgba: &[i16], width: usize, height: usize) -> Vec<i8> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0i8; pixel_count * 8];
    out.par_chunks_mut(CHUNK_SIZE * 8)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, pair) in chunk.chunks_mut(8).enumerate() {
                let src_idx = (pixel_base + j) << 2;
                let r = rgba[src_idx] as i32;
                let g = rgba[src_idx + 1] as i32;
                let r_le = r.to_le_bytes();
                let g_le = g.to_le_bytes();
                pair[0] = r_le[0] as i8;
                pair[1] = r_le[1] as i8;
                pair[2] = r_le[2] as i8;
                pair[3] = r_le[3] as i8;
                pair[4] = g_le[0] as i8;
                pair[5] = g_le[1] as i8;
                pair[6] = g_le[2] as i8;
                pair[7] = g_le[3] as i8;
            }
        });
    out
}

fn decode_rg32s_impl(data: &[i8], width: usize, height: usize) -> Vec<f32> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0.0f32; pixel_count << 2];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * (CHUNK_SIZE >> 2);
            for (j, rgba) in chunk.chunks_mut(4).enumerate() {
                let abs_pixel = pixel_base + j;
                let src_idx = abs_pixel * 8;
                let r = i32::from_le_bytes([data[src_idx] as u8, data[src_idx + 1] as u8, data[src_idx + 2] as u8, data[src_idx + 3] as u8]);
                let g = i32::from_le_bytes([data[src_idx + 4] as u8, data[src_idx + 5] as u8, data[src_idx + 6] as u8, data[src_idx + 7] as u8]);
                rgba[0] = r as f32;
                rgba[1] = g as f32;
                rgba[2] = 0.0;
                rgba[3] = 0.0;
            }
        });
    out
}

pub fn encode_rg32s(pixels: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = pixels.convert_to_s16_bytes();
    PixelDatas::S8(encode_rg32s_impl(&datas, w, h))
}

pub fn decode_rg32s(data: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let raw: Vec<i8> = bytemuck::cast_slice(data.as_bytes()).to_vec();
    PixelDatas::F32(decode_rg32s_impl(&raw, w, h))
}

// ============================================================
// Rg32Float — two-channel 32-bit float
// Input: RGBA f32, output: 2 f32 per pixel [R, G]
// Decode: 2 f32 → [R, G, 0, 1.0] in F32
// ============================================================

fn encode_rg32f_impl(rgba: &[f32], width: usize, height: usize) -> Vec<f32> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0.0f32; pixel_count << 1];
    out.par_chunks_mut(CHUNK_SIZE << 1)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, pair) in chunk.chunks_mut(2).enumerate() {
                let src_idx = (pixel_base + j) << 2;
                pair[0] = rgba[src_idx];
                pair[1] = rgba[src_idx + 1];
            }
        });
    out
}

fn decode_rg32f_impl(data: &[f32], width: usize, height: usize) -> Vec<f32> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0.0f32; pixel_count << 2];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * (CHUNK_SIZE >> 2);
            for (j, rgba) in chunk.chunks_mut(4).enumerate() {
                let abs_pixel = pixel_base + j;
                let src_idx = abs_pixel << 1;
                rgba[0] = data[src_idx];
                rgba[1] = data[src_idx + 1];
                rgba[2] = 0.0;
                rgba[3] = 1.0;
            }
        });
    out
}

pub fn encode_rg32f(pixels: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = pixels.convert_to_f32_bytes();
    PixelDatas::F32(encode_rg32f_impl(&datas, w, h))
}

pub fn decode_rg32f(data: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = data.convert_to_f32_bytes();
    PixelDatas::F32(decode_rg32f_impl(&datas, w, h))
}

// ============================================================
// Rgba32Uint — four-channel 32-bit unsigned integer
// Input: RGBA8 (u8), output: 4 u32 per pixel stored as LE bytes in U8
// Decode: 4 u32 → [R as f32, G as f32, B as f32, A as f32] in F32
// ============================================================

fn encode_rgba32u_impl(rgba: &[u8], width: usize, height: usize) -> Vec<u8> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u8; pixel_count * 16];
    out.par_chunks_mut(CHUNK_SIZE * 16)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, quad) in chunk.chunks_mut(16).enumerate() {
                let src_idx = (pixel_base + j) << 2;
                let r = rgba[src_idx] as u32;
                let g = rgba[src_idx + 1] as u32;
                let b = rgba[src_idx + 2] as u32;
                let a = rgba[src_idx + 3] as u32;
                quad[..4].copy_from_slice(&r.to_le_bytes());
                quad[4..8].copy_from_slice(&g.to_le_bytes());
                quad[8..12].copy_from_slice(&b.to_le_bytes());
                quad[12..].copy_from_slice(&a.to_le_bytes());
            }
        });
    out
}

fn decode_rgba32u_impl(data: &[u8], width: usize, height: usize) -> Vec<f32> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0.0f32; pixel_count << 2];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * (CHUNK_SIZE >> 2);
            for (j, rgba) in chunk.chunks_mut(4).enumerate() {
                let abs_pixel = pixel_base + j;
                let src_idx = abs_pixel * 16;
                let r = u32::from_le_bytes([data[src_idx], data[src_idx + 1], data[src_idx + 2], data[src_idx + 3]]);
                let g = u32::from_le_bytes([data[src_idx + 4], data[src_idx + 5], data[src_idx + 6], data[src_idx + 7]]);
                let b = u32::from_le_bytes([data[src_idx + 8], data[src_idx + 9], data[src_idx + 10], data[src_idx + 11]]);
                let a = u32::from_le_bytes([data[src_idx + 12], data[src_idx + 13], data[src_idx + 14], data[src_idx + 15]]);
                rgba[0] = r as f32;
                rgba[1] = g as f32;
                rgba[2] = b as f32;
                rgba[3] = a as f32;
            }
        });
    out
}

pub fn encode_rgba32u(pixels: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = pixels.convert_to_u8_bytes();
    PixelDatas::U8(encode_rgba32u_impl(&datas, w, h))
}

pub fn decode_rgba32u(data: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = data.convert_to_u8_bytes();
    PixelDatas::F32(decode_rgba32u_impl(&datas, w, h))
}

// ============================================================
// Rgba32Sint — four-channel 32-bit signed integer
// Input: RGBA signed (s8), output: 4 i32 per pixel stored as LE bytes in S8
// Decode: 4 i32 → [R as f32, G as f32, B as f32, A as f32] in F32
// ============================================================

fn encode_rgba32s_impl(rgba: &[i16], width: usize, height: usize) -> Vec<i8> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0i8; pixel_count * 16];
    out.par_chunks_mut(CHUNK_SIZE * 16)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, quad) in chunk.chunks_mut(16).enumerate() {
                let src_idx = (pixel_base + j) << 2;
                let r = rgba[src_idx] as i32;
                let g = rgba[src_idx + 1] as i32;
                let b = rgba[src_idx + 2] as i32;
                let a = rgba[src_idx + 3] as i32;
                let r_le = r.to_le_bytes();
                let g_le = g.to_le_bytes();
                let b_le = b.to_le_bytes();
                let a_le = a.to_le_bytes();
                quad[0] = r_le[0] as i8;
                quad[1] = r_le[1] as i8;
                quad[2] = r_le[2] as i8;
                quad[3] = r_le[3] as i8;
                quad[4] = g_le[0] as i8;
                quad[5] = g_le[1] as i8;
                quad[6] = g_le[2] as i8;
                quad[7] = g_le[3] as i8;
                quad[8] = b_le[0] as i8;
                quad[9] = b_le[1] as i8;
                quad[10] = b_le[2] as i8;
                quad[11] = b_le[3] as i8;
                quad[12] = a_le[0] as i8;
                quad[13] = a_le[1] as i8;
                quad[14] = a_le[2] as i8;
                quad[15] = a_le[3] as i8;
            }
        });
    out
}

fn decode_rgba32s_impl(data: &[i8], width: usize, height: usize) -> Vec<f32> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0.0f32; pixel_count << 2];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * (CHUNK_SIZE >> 2);
            for (j, rgba) in chunk.chunks_mut(4).enumerate() {
                let abs_pixel = pixel_base + j;
                let src_idx = abs_pixel * 16;
                let r = i32::from_le_bytes([data[src_idx] as u8, data[src_idx + 1] as u8, data[src_idx + 2] as u8, data[src_idx + 3] as u8]);
                let g = i32::from_le_bytes([data[src_idx + 4] as u8, data[src_idx + 5] as u8, data[src_idx + 6] as u8, data[src_idx + 7] as u8]);
                let b = i32::from_le_bytes([data[src_idx + 8] as u8, data[src_idx + 9] as u8, data[src_idx + 10] as u8, data[src_idx + 11] as u8]);
                let a = i32::from_le_bytes([data[src_idx + 12] as u8, data[src_idx + 13] as u8, data[src_idx + 14] as u8, data[src_idx + 15] as u8]);
                rgba[0] = r as f32;
                rgba[1] = g as f32;
                rgba[2] = b as f32;
                rgba[3] = a as f32;
            }
        });
    out
}

pub fn encode_rgba32s(pixels: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = pixels.convert_to_s16_bytes();
    PixelDatas::S8(encode_rgba32s_impl(&datas, w, h))
}

pub fn decode_rgba32s(data: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let raw: Vec<i8> = bytemuck::cast_slice(data.as_bytes()).to_vec();
    PixelDatas::F32(decode_rgba32s_impl(&raw, w, h))
}

// ============================================================
// Rgba32Float — four-channel 32-bit float (passthrough)
// Input: RGBA f32, output: 4 f32 per pixel [R, G, B, A]
// Decode: 4 f32 → [R, G, B, A] in F32
// ============================================================

fn encode_rgba32f_impl(rgba: &[f32], width: usize, height: usize) -> Vec<f32> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0.0f32; pixel_count << 2];
    out.par_chunks_mut(CHUNK_SIZE << 2)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, quad) in chunk.chunks_mut(4).enumerate() {
                let src_idx = (pixel_base + j) << 2;
                quad.copy_from_slice(&rgba[src_idx..src_idx + 4]);
            }
        });
    out
}

fn decode_rgba32f_impl(data: &[f32], width: usize, height: usize) -> Vec<f32> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0.0f32; pixel_count << 2];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * (CHUNK_SIZE >> 2);
            for (j, rgba) in chunk.chunks_mut(4).enumerate() {
                let abs_pixel = pixel_base + j;
                let src_idx = abs_pixel << 2;
                rgba.copy_from_slice(&data[src_idx..src_idx + 4]);
            }
        });
    out
}

pub fn encode_rgba32f(pixels: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = pixels.convert_to_f32_bytes();
    PixelDatas::F32(encode_rgba32f_impl(&datas, w, h))
}

pub fn decode_rgba32f(data: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = data.convert_to_f32_bytes();
    PixelDatas::F32(decode_rgba32f_impl(&datas, w, h))
}

// ============================================================
// Rgb10a2Unorm — packed 10-10-10-2 unsigned normalized
// Input: RGBA f32, output: 1 u32 per pixel packed as 10-10-10-2
// Decode: 1 u32 → [R, G, B, A] in F32 (unpacked normalized)
// ============================================================

/// Pack 4 f32 values into a single u32 (R:10, G:10, B:10, A:2).
#[inline(always)]
fn pack_rgb10a2(r: f32, g: f32, b: f32, a: f32) -> u32 {
    let r = (r.clamp(0.0, 1.0) * 1023.0).round() as u32;
    let g = (g.clamp(0.0, 1.0) * 1023.0).round() as u32;
    let b = (b.clamp(0.0, 1.0) * 1023.0).round() as u32;
    let a = (a.clamp(0.0, 1.0) * 3.0).round() as u32;
    r | (g << 10) | (b << 20) | (a << 30)
}

/// Unpack a u32 into 4 f32 values (R:10, G:10, B:10, A:2).
#[inline(always)]
fn unpack_rgb10a2(packed: u32) -> (f32, f32, f32, f32) {
    let r = (packed & 0x3FF) as f32 / 1023.0;
    let g = ((packed >> 10) & 0x3FF) as f32 / 1023.0;
    let b = ((packed >> 20) & 0x3FF) as f32 / 1023.0;
    let a = ((packed >> 30) & 0x3) as f32 / 3.0;
    (r, g, b, a)
}

fn encode_rgb10a2_unorm_impl(rgba: &[f32], width: usize, height: usize) -> Vec<u8> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u8; pixel_count * 4];
    out.par_chunks_mut(CHUNK_SIZE * 4)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, word) in chunk.chunks_mut(4).enumerate() {
                let src_idx = (pixel_base + j) << 2;
                let packed = pack_rgb10a2(rgba[src_idx], rgba[src_idx + 1], rgba[src_idx + 2], rgba[src_idx + 3]);
                word.copy_from_slice(&packed.to_le_bytes());
            }
        });
    out
}

fn decode_rgb10a2_unorm_impl(data: &[u8], width: usize, height: usize) -> Vec<f32> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0.0f32; pixel_count << 2];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * (CHUNK_SIZE >> 2);
            for (j, rgba) in chunk.chunks_mut(4).enumerate() {
                let abs_pixel = pixel_base + j;
                let src_idx = abs_pixel * 4;
                let packed = u32::from_le_bytes([data[src_idx], data[src_idx + 1], data[src_idx + 2], data[src_idx + 3]]);
                let (r, g, b, a) = unpack_rgb10a2(packed);
                rgba[0] = r;
                rgba[1] = g;
                rgba[2] = b;
                rgba[3] = a;
            }
        });
    out
}

pub fn encode_rgb10a2_unorm(pixels: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = pixels.convert_to_f32_bytes();
    PixelDatas::U8(encode_rgb10a2_unorm_impl(&datas, w, h))
}

pub fn decode_rgb10a2_unorm(data: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = data.convert_to_u8_bytes();
    PixelDatas::F32(decode_rgb10a2_unorm_impl(&datas, w, h))
}

// ============================================================
// Rg11b10Ufloat — packed 11-11-10 unsigned float
// Input: RGBA f32, output: 1 u32 per pixel packed as 11-11-10 ufloat
// Decode: 1 u32 → [R, G, B, 1.0] in F32
// ============================================================

/// Convert f32 to 11-bit unsigned float (5 exponent, 6 mantissa, bias 15).
#[inline(always)]
fn f32_to_uf11(v: f32) -> u16 {
    if v <= 0.0 {
        return 0;
    }
    if !v.is_finite() {
        return 0x7C0; // Exponent all 1s, mantissa 0 → INF pattern
    }
    // Route through f16 which shares the same exponent bias (15)
    let f = f16::from_f32(v);
    let bits = f.to_bits();
    if bits & 0x8000 != 0 {
        return 0; // Signed negative → clamp to 0
    }
    let exp = (bits >> 10) & 0x1F;
    let mant = bits & 0x3FF;
    let mant_6 = mant >> 4; // Truncate 10-bit f16 mantissa to 6 bits
    (exp << 6) | mant_6
}

/// Convert f32 to 10-bit unsigned float (5 exponent, 5 mantissa, bias 15).
#[inline(always)]
fn f32_to_uf10(v: f32) -> u16 {
    if v <= 0.0 {
        return 0;
    }
    if !v.is_finite() {
        return 0x7C0; // Exponent all 1s, mantissa 0 → INF pattern
    }
    let f = f16::from_f32(v);
    let bits = f.to_bits();
    if bits & 0x8000 != 0 {
        return 0;
    }
    let exp = (bits >> 10) & 0x1F;
    let mant = bits & 0x3FF;
    let mant_5 = mant >> 5; // Truncate 10-bit f16 mantissa to 5 bits
    (exp << 5) | mant_5
}

/// Convert 11-bit unsigned float to f32.
#[inline(always)]
fn uf11_to_f32(val: u16) -> f32 {
    let exp = (val >> 6) & 0x1F;
    let mant = val & 0x3F;
    let f16_bits = if exp == 0 && mant == 0 {
        0u16
    } else if exp == 0x1F {
        0x7C00u16 // F16 INF
    } else {
        // Reconstruct as f16: shift mantissa left by 4 to fill 10-bit f16 mantissa
        (exp << 10) | (mant << 4)
    };
    f16::from_bits(f16_bits).to_f32()
}

/// Convert 10-bit unsigned float to f32.
#[inline(always)]
fn uf10_to_f32(val: u16) -> f32 {
    let exp = (val >> 5) & 0x1F;
    let mant = val & 0x1F;
    let f16_bits = if exp == 0 && mant == 0 {
        0u16
    } else if exp == 0x1F {
        0x7C00u16
    } else {
        (exp << 10) | (mant << 5)
    };
    f16::from_bits(f16_bits).to_f32()
}

/// Pack R (11-bit ufloat), G (11-bit ufloat), B (10-bit ufloat) into a u32.
/// Layout: bits 0-10 = R, bits 11-21 = G, bits 22-31 = B
#[inline(always)]
fn pack_rg11b10_ufloat(r: f32, g: f32, b: f32) -> u32 {
    let r_bits = f32_to_uf11(r) as u32;
    let g_bits = f32_to_uf11(g) as u32;
    let b_bits = f32_to_uf10(b) as u32;
    r_bits | (g_bits << 11) | (b_bits << 22)
}

/// Unpack a u32 into R (f32), G (f32), B (f32).
#[inline(always)]
fn unpack_rg11b10_ufloat(packed: u32) -> (f32, f32, f32) {
    let r = uf11_to_f32((packed & 0x7FF) as u16);
    let g = uf11_to_f32(((packed >> 11) & 0x7FF) as u16);
    let b = uf10_to_f32(((packed >> 22) & 0x3FF) as u16);
    (r, g, b)
}

fn encode_rg11b10_ufloat_impl(rgba: &[f32], width: usize, height: usize) -> Vec<u8> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u8; pixel_count * 4];
    out.par_chunks_mut(CHUNK_SIZE * 4)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, word) in chunk.chunks_mut(4).enumerate() {
                let src_idx = (pixel_base + j) << 2;
                let packed = pack_rg11b10_ufloat(rgba[src_idx], rgba[src_idx + 1], rgba[src_idx + 2]);
                word.copy_from_slice(&packed.to_le_bytes());
            }
        });
    out
}

fn decode_rg11b10_ufloat_impl(data: &[u8], width: usize, height: usize) -> Vec<f32> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0.0f32; pixel_count << 2];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * (CHUNK_SIZE >> 2);
            for (j, rgba) in chunk.chunks_mut(4).enumerate() {
                let abs_pixel = pixel_base + j;
                let src_idx = abs_pixel * 4;
                let packed = u32::from_le_bytes([data[src_idx], data[src_idx + 1], data[src_idx + 2], data[src_idx + 3]]);
                let (r, g, b) = unpack_rg11b10_ufloat(packed);
                rgba[0] = r;
                rgba[1] = g;
                rgba[2] = b;
                rgba[3] = 1.0;
            }
        });
    out
}

pub fn encode_rg11b10_ufloat(pixels: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = pixels.convert_to_f32_bytes();
    PixelDatas::U8(encode_rg11b10_ufloat_impl(&datas, w, h))
}

pub fn decode_rg11b10_ufloat(data: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let datas = data.convert_to_u8_bytes();
    PixelDatas::F32(decode_rg11b10_ufloat_impl(&datas, w, h))
}
