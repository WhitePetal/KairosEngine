use rayon::prelude::*;

use crate::graphics::texture::format::PixelDatas;
use half::f16;

/// Get RGBA8 bytes from any PixelDatas variant.
/// For U8, returns bytes directly. For F16/F32, converts via to_rgba8().
fn rgba8_bytes(pixels: &PixelDatas) -> Vec<u8> {
    match pixels {
        PixelDatas::U8(_) => pixels.as_bytes().to_vec(),
        other => other.to_rgba8_bytes(),
    }
}

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
    PixelDatas::U8(encode_r8_impl(&rgba8_bytes(pixels), w, h))
}

pub fn decode_r8(
    data: &PixelDatas,
    w: usize,
    h: usize,
    fill_g: bool,
    fill_b: bool,
    fill_a: bool,
) -> PixelDatas {
    PixelDatas::U8(decode_r8_impl(
        data.as_bytes(),
        w,
        h,
        fill_g,
        fill_b,
        fill_a,
    ))
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
                pair[0] = rgba[src_idx];     // R
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
                rgba[0] = data[src_idx];         // R
                rgba[1] = data[src_idx + 1];     // G
                rgba[2] = 0;                      // B
                rgba[3] = 255;                    // A
            }
        });
    out
}

pub fn encode_rg8(pixels: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    PixelDatas::U8(encode_rg8_impl(&rgba8_bytes(pixels), w, h))
}

pub fn decode_rg8(data: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    PixelDatas::U8(decode_rg8_impl(data.as_bytes(), w, h))
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
                bgra[2] = rgba[src_idx];     // R
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
                rgba[2] = data[src_idx];     // B (was R)
                rgba[3] = data[src_idx + 3]; // A (was A)
            }
        });
    out
}

pub fn encode_bgra8(pixels: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    PixelDatas::U8(encode_bgra8_impl(&rgba8_bytes(pixels), w, h))
}

pub fn decode_bgra8(data: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    PixelDatas::U8(decode_bgra8_impl(data.as_bytes(), w, h))
}

// ============================================================
// R16 integer formats (R16Uint, R16Sint)
// ============================================================

/// Encode RGBA8 to R16Uint by zero-extending the R channel.
///
/// Output is 1 u16 per pixel.
fn encode_r16u_impl(rgba: &[u8], width: usize, height: usize) -> Vec<u16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u16; pixel_count];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, dst) in chunk.iter_mut().enumerate() {
                let src_idx = (pixel_base + j) << 2;
                *dst = rgba[src_idx] as u16;
            }
        });
    out
}

/// Encode RGBA8 to R16Sint by sign-extending the R channel.
///
/// Output is 1 u16 per pixel.
fn encode_r16s_impl(rgba: &[u8], width: usize, height: usize) -> Vec<u16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u16; pixel_count];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, dst) in chunk.iter_mut().enumerate() {
                let src_idx = (pixel_base + j) << 2;
                *dst = rgba[src_idx] as i8 as i16 as u16;
            }
        });
    out
}

/// Decode R16 (Uint or Sint) back to RGBA8.
///
/// Input is 1 u16 per pixel.
/// Output is 4 bytes per pixel: [R, 0, 0, 255].
fn decode_r16_impl(data: &[u16], width: usize, height: usize) -> Vec<u8> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u8; pixel_count << 2];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * (CHUNK_SIZE >> 2);
            for (j, rgba) in chunk.chunks_mut(4).enumerate() {
                let abs_pixel = pixel_base + j;
                rgba[0] = data[abs_pixel] as u8;
                rgba[1] = 0;
                rgba[2] = 0;
                rgba[3] = 255;
            }
        });
    out
}

fn encode_r16u_from_u16_impl(rgba: &[u16], width: usize, height: usize) -> Vec<u16> {
    use rayon::prelude::*;
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u16; pixel_count];
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

fn encode_r16u_from_f16_impl(rgba: &[f16], width: usize, height: usize) -> Vec<u16> {
    use rayon::prelude::*;
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u16; pixel_count];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, dst) in chunk.iter_mut().enumerate() {
                let f = rgba[(pixel_base + j) << 2].to_f32();
                *dst = if f <= 0.0 { 0 }
                       else if f >= 1.0 { 65535 }
                       else { (f * 65535.0).round() as u16 };
            }
        });
    out
}

fn encode_r16u_from_f32_impl(rgba: &[f32], width: usize, height: usize) -> Vec<u16> {
    use rayon::prelude::*;
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u16; pixel_count];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, dst) in chunk.iter_mut().enumerate() {
                let f = rgba[(pixel_base + j) << 2];
                *dst = if f <= 0.0 { 0 }
                       else if f >= 1.0 { 65535 }
                       else { (f * 65535.0).round() as u16 };
            }
        });
    out
}

fn encode_r16s_from_u16_impl(rgba: &[u16], width: usize, height: usize) -> Vec<u16> {
    use rayon::prelude::*;
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u16; pixel_count];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, dst) in chunk.iter_mut().enumerate() {
                *dst = rgba[(pixel_base + j) << 2] as i16 as u16;
            }
        });
    out
}

fn encode_r16s_from_f16_impl(rgba: &[f16], width: usize, height: usize) -> Vec<u16> {
    use rayon::prelude::*;
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u16; pixel_count];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, dst) in chunk.iter_mut().enumerate() {
                let f = rgba[(pixel_base + j) << 2].to_f32();
                let clamped = f.max(-1.0).min(1.0);
                *dst = (clamped * 32767.0).round() as i16 as u16;
            }
        });
    out
}

fn encode_r16s_from_f32_impl(rgba: &[f32], width: usize, height: usize) -> Vec<u16> {
    use rayon::prelude::*;
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u16; pixel_count];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, dst) in chunk.iter_mut().enumerate() {
                let f = rgba[(pixel_base + j) << 2];
                let clamped = f.max(-1.0).min(1.0);
                *dst = (clamped * 32767.0).round() as i16 as u16;
            }
        });
    out
}

pub fn encode_r16u(pixels: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    match pixels {
        PixelDatas::U8(rgba) => PixelDatas::U16(encode_r16u_impl(rgba, w, h)),
        PixelDatas::U16(rgba) => PixelDatas::U16(encode_r16u_from_u16_impl(rgba, w, h)),
        PixelDatas::F16(rgba) => PixelDatas::U16(encode_r16u_from_f16_impl(rgba, w, h)),
        PixelDatas::F32(rgba) => PixelDatas::U16(encode_r16u_from_f32_impl(rgba, w, h)),
    }
}

pub fn encode_r16s(pixels: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    match pixels {
        PixelDatas::U8(rgba) => PixelDatas::U16(encode_r16s_impl(rgba, w, h)),
        PixelDatas::U16(rgba) => PixelDatas::U16(encode_r16s_from_u16_impl(rgba, w, h)),
        PixelDatas::F16(rgba) => PixelDatas::U16(encode_r16s_from_f16_impl(rgba, w, h)),
        PixelDatas::F32(rgba) => PixelDatas::U16(encode_r16s_from_f32_impl(rgba, w, h)),
    }
}

pub fn decode_r16(data: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    match data {
        PixelDatas::U16(d) => PixelDatas::U8(decode_r16_impl(d, w, h)),
        other => {
            let src: &[u16] = bytemuck::cast_slice(other.as_bytes());
            PixelDatas::U8(decode_r16_impl(src, w, h))
        }
    }
}

// ============================================================
// Rg16 integer formats (Rg16Uint, Rg16Sint)
// ============================================================

/// Encode RGBA8 to Rg16Uint by zero-extending the R and G channels.
///
/// Output is 2 u16 per pixel: [R, G].
fn encode_rg16u_impl(rgba: &[u8], width: usize, height: usize) -> Vec<u16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u16; pixel_count << 1];
    out.par_chunks_mut(CHUNK_SIZE << 1)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, pair) in chunk.chunks_mut(2).enumerate() {
                let src_idx = (pixel_base + j) << 2;
                pair[0] = rgba[src_idx] as u16;
                pair[1] = rgba[src_idx + 1] as u16;
            }
        });
    out
}

/// Encode RGBA8 to Rg16Sint by sign-extending the R and G channels.
///
/// Output is 2 u16 per pixel: [R, G].
fn encode_rg16s_impl(rgba: &[u8], width: usize, height: usize) -> Vec<u16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u16; pixel_count << 1];
    out.par_chunks_mut(CHUNK_SIZE << 1)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, pair) in chunk.chunks_mut(2).enumerate() {
                let src_idx = (pixel_base + j) << 2;
                pair[0] = rgba[src_idx] as i8 as i16 as u16;
                pair[1] = rgba[src_idx + 1] as i8 as i16 as u16;
            }
        });
    out
}

/// Decode Rg16 (Uint or Sint) back to RGBA8.
///
/// Input is 2 u16 per pixel: [R, G].
/// Output is 4 bytes per pixel: [R, G, 0, 255].
fn decode_rg16_impl(data: &[u16], width: usize, height: usize) -> Vec<u8> {
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
                rgba[0] = data[src_idx] as u8;
                rgba[1] = data[src_idx + 1] as u8;
                rgba[2] = 0;
                rgba[3] = 255;
            }
        });
    out
}

fn encode_rg16u_from_u16_impl(rgba: &[u16], width: usize, height: usize) -> Vec<u16> {
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

fn encode_rg16u_from_f16_impl(rgba: &[f16], width: usize, height: usize) -> Vec<u16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u16; pixel_count << 1];
    out.par_chunks_mut(CHUNK_SIZE << 1)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, pair) in chunk.chunks_mut(2).enumerate() {
                let src_idx = (pixel_base + j) << 2;
                let fr = rgba[src_idx].to_f32();
                let fg = rgba[src_idx + 1].to_f32();
                pair[0] = if fr <= 0.0 { 0 } else if fr >= 1.0 { 65535 } else { (fr * 65535.0).round() as u16 };
                pair[1] = if fg <= 0.0 { 0 } else if fg >= 1.0 { 65535 } else { (fg * 65535.0).round() as u16 };
            }
        });
    out
}

fn encode_rg16u_from_f32_impl(rgba: &[f32], width: usize, height: usize) -> Vec<u16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u16; pixel_count << 1];
    out.par_chunks_mut(CHUNK_SIZE << 1)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, pair) in chunk.chunks_mut(2).enumerate() {
                let src_idx = (pixel_base + j) << 2;
                let fr = rgba[src_idx];
                let fg = rgba[src_idx + 1];
                pair[0] = if fr <= 0.0 { 0 } else if fr >= 1.0 { 65535 } else { (fr * 65535.0).round() as u16 };
                pair[1] = if fg <= 0.0 { 0 } else if fg >= 1.0 { 65535 } else { (fg * 65535.0).round() as u16 };
            }
        });
    out
}

fn encode_rg16s_from_u16_impl(rgba: &[u16], width: usize, height: usize) -> Vec<u16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u16; pixel_count << 1];
    out.par_chunks_mut(CHUNK_SIZE << 1)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, pair) in chunk.chunks_mut(2).enumerate() {
                let src_idx = (pixel_base + j) << 2;
                pair[0] = rgba[src_idx] as i16 as u16;
                pair[1] = rgba[src_idx + 1] as i16 as u16;
            }
        });
    out
}

fn encode_rg16s_from_f16_impl(rgba: &[f16], width: usize, height: usize) -> Vec<u16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u16; pixel_count << 1];
    out.par_chunks_mut(CHUNK_SIZE << 1)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, pair) in chunk.chunks_mut(2).enumerate() {
                let src_idx = (pixel_base + j) << 2;
                let fr = rgba[src_idx].to_f32().max(-1.0).min(1.0);
                let fg = rgba[src_idx + 1].to_f32().max(-1.0).min(1.0);
                pair[0] = (fr * 32767.0).round() as i16 as u16;
                pair[1] = (fg * 32767.0).round() as i16 as u16;
            }
        });
    out
}

fn encode_rg16s_from_f32_impl(rgba: &[f32], width: usize, height: usize) -> Vec<u16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u16; pixel_count << 1];
    out.par_chunks_mut(CHUNK_SIZE << 1)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, pair) in chunk.chunks_mut(2).enumerate() {
                let src_idx = (pixel_base + j) << 2;
                let fr = rgba[src_idx].max(-1.0).min(1.0);
                let fg = rgba[src_idx + 1].max(-1.0).min(1.0);
                pair[0] = (fr * 32767.0).round() as i16 as u16;
                pair[1] = (fg * 32767.0).round() as i16 as u16;
            }
        });
    out
}

pub fn encode_rg16u(pixels: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    match pixels {
        PixelDatas::U8(rgba) => PixelDatas::U16(encode_rg16u_impl(rgba, w, h)),
        PixelDatas::U16(rgba) => PixelDatas::U16(encode_rg16u_from_u16_impl(rgba, w, h)),
        PixelDatas::F16(rgba) => PixelDatas::U16(encode_rg16u_from_f16_impl(rgba, w, h)),
        PixelDatas::F32(rgba) => PixelDatas::U16(encode_rg16u_from_f32_impl(rgba, w, h)),
    }
}

pub fn encode_rg16s(pixels: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    match pixels {
        PixelDatas::U8(rgba) => PixelDatas::U16(encode_rg16s_impl(rgba, w, h)),
        PixelDatas::U16(rgba) => PixelDatas::U16(encode_rg16s_from_u16_impl(rgba, w, h)),
        PixelDatas::F16(rgba) => PixelDatas::U16(encode_rg16s_from_f16_impl(rgba, w, h)),
        PixelDatas::F32(rgba) => PixelDatas::U16(encode_rg16s_from_f32_impl(rgba, w, h)),
    }
}

pub fn decode_rg16(data: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    match data {
        PixelDatas::U16(d) => PixelDatas::U8(decode_rg16_impl(d, w, h)),
        other => {
            let src: &[u16] = bytemuck::cast_slice(other.as_bytes());
            PixelDatas::U8(decode_rg16_impl(src, w, h))
        }
    }
}

// ============================================================
// Rgba16 integer formats (Rgba16Uint, Rgba16Sint)
// ============================================================

/// Encode RGBA8 to Rgba16Uint by zero-extending all channels.
///
/// Output is 4 u16 per pixel: [R, G, B, A].
fn encode_rgba16u_impl(rgba: &[u8], width: usize, height: usize) -> Vec<u16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u16; pixel_count << 2];
    out.par_chunks_mut(CHUNK_SIZE << 2)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, quad) in chunk.chunks_mut(4).enumerate() {
                let src_idx = (pixel_base + j) << 2;
                quad[0] = rgba[src_idx] as u16;
                quad[1] = rgba[src_idx + 1] as u16;
                quad[2] = rgba[src_idx + 2] as u16;
                quad[3] = rgba[src_idx + 3] as u16;
            }
        });
    out
}

/// Encode RGBA8 to Rgba16Sint by sign-extending all channels.
///
/// Output is 4 u16 per pixel: [R, G, B, A].
fn encode_rgba16s_impl(rgba: &[u8], width: usize, height: usize) -> Vec<u16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u16; pixel_count << 2];
    out.par_chunks_mut(CHUNK_SIZE << 2)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, quad) in chunk.chunks_mut(4).enumerate() {
                let src_idx = (pixel_base + j) << 2;
                quad[0] = rgba[src_idx] as i8 as i16 as u16;
                quad[1] = rgba[src_idx + 1] as i8 as i16 as u16;
                quad[2] = rgba[src_idx + 2] as i8 as i16 as u16;
                quad[3] = rgba[src_idx + 3] as i8 as i16 as u16;
            }
        });
    out
}

/// Decode Rgba16 (Uint or Sint) back to RGBA8.
///
/// Input is 4 u16 per pixel: [R, G, B, A].
/// Output is 4 bytes per pixel: [R, G, B, A].
fn decode_rgba16_impl(data: &[u16], width: usize, height: usize) -> Vec<u8> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u8; pixel_count << 2];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * (CHUNK_SIZE >> 2);
            for (j, rgba) in chunk.chunks_mut(4).enumerate() {
                let abs_pixel = pixel_base + j;
                let src_idx = abs_pixel << 2;
                rgba[0] = data[src_idx] as u8;
                rgba[1] = data[src_idx + 1] as u8;
                rgba[2] = data[src_idx + 2] as u8;
                rgba[3] = data[src_idx + 3] as u8;
            }
        });
    out
}

fn encode_rgba16u_from_u16_impl(rgba: &[u16], _width: usize, _height: usize) -> Vec<u16> {
    rgba.to_vec()
}

fn encode_rgba16u_from_f16_impl(rgba: &[f16], width: usize, height: usize) -> Vec<u16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u16; pixel_count << 2];
    out.par_chunks_mut(CHUNK_SIZE << 2)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, quad) in chunk.chunks_mut(4).enumerate() {
                let src_idx = (pixel_base + j) << 2;
                for k in 0..4 {
                    let f = rgba[src_idx + k].to_f32();
                    quad[k] = if f <= 0.0 { 0 } else if f >= 1.0 { 65535 } else { (f * 65535.0).round() as u16 };
                }
            }
        });
    out
}

fn encode_rgba16u_from_f32_impl(rgba: &[f32], width: usize, height: usize) -> Vec<u16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u16; pixel_count << 2];
    out.par_chunks_mut(CHUNK_SIZE << 2)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, quad) in chunk.chunks_mut(4).enumerate() {
                let src_idx = (pixel_base + j) << 2;
                for k in 0..4 {
                    let f = rgba[src_idx + k];
                    quad[k] = if f <= 0.0 { 0 } else if f >= 1.0 { 65535 } else { (f * 65535.0).round() as u16 };
                }
            }
        });
    out
}

fn encode_rgba16s_from_u16_impl(rgba: &[u16], _width: usize, _height: usize) -> Vec<u16> {
    rgba.to_vec()
}

fn encode_rgba16s_from_f16_impl(rgba: &[f16], width: usize, height: usize) -> Vec<u16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u16; pixel_count << 2];
    out.par_chunks_mut(CHUNK_SIZE << 2)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, quad) in chunk.chunks_mut(4).enumerate() {
                let src_idx = (pixel_base + j) << 2;
                for k in 0..4 {
                    let f = rgba[src_idx + k].to_f32().max(-1.0).min(1.0);
                    quad[k] = (f * 32767.0).round() as i16 as u16;
                }
            }
        });
    out
}

fn encode_rgba16s_from_f32_impl(rgba: &[f32], width: usize, height: usize) -> Vec<u16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![0u16; pixel_count << 2];
    out.par_chunks_mut(CHUNK_SIZE << 2)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, quad) in chunk.chunks_mut(4).enumerate() {
                let src_idx = (pixel_base + j) << 2;
                for k in 0..4 {
                    let f = rgba[src_idx + k].max(-1.0).min(1.0);
                    quad[k] = (f * 32767.0).round() as i16 as u16;
                }
            }
        });
    out
}

pub fn encode_rgba16u(pixels: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    match pixels {
        PixelDatas::U8(rgba) => PixelDatas::U16(encode_rgba16u_impl(rgba, w, h)),
        PixelDatas::U16(rgba) => PixelDatas::U16(encode_rgba16u_from_u16_impl(rgba, w, h)),
        PixelDatas::F16(rgba) => PixelDatas::U16(encode_rgba16u_from_f16_impl(rgba, w, h)),
        PixelDatas::F32(rgba) => PixelDatas::U16(encode_rgba16u_from_f32_impl(rgba, w, h)),
    }
}

pub fn encode_rgba16s(pixels: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    match pixels {
        PixelDatas::U8(rgba) => PixelDatas::U16(encode_rgba16s_impl(rgba, w, h)),
        PixelDatas::U16(rgba) => PixelDatas::U16(encode_rgba16s_from_u16_impl(rgba, w, h)),
        PixelDatas::F16(rgba) => PixelDatas::U16(encode_rgba16s_from_f16_impl(rgba, w, h)),
        PixelDatas::F32(rgba) => PixelDatas::U16(encode_rgba16s_from_f32_impl(rgba, w, h)),
    }
}

pub fn decode_rgba16(data: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    match data {
        PixelDatas::U16(d) => PixelDatas::U8(decode_rgba16_impl(d, w, h)),
        other => {
            let src: &[u16] = bytemuck::cast_slice(other.as_bytes());
            PixelDatas::U8(decode_rgba16_impl(src, w, h))
        }
    }
}

// ============================================================
// R16Float — single-channel half-float
// ============================================================

/// Encode RGBA half-float to R16Float by extracting the R channel.
///
/// Output is 1 f16 per pixel.
fn encode_r16f_from_f16_impl(rgba: &[f16], width: usize, height: usize) -> Vec<f16> {
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

/// Encode RGBA8 to R16Float by normalizing the R channel.
///
/// Output is 1 f16 per pixel.
fn encode_r16f_from_u8_impl(rgba: &[u8], width: usize, height: usize) -> Vec<f16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![f16::ZERO; pixel_count];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, dst) in chunk.iter_mut().enumerate() {
                let src_idx = (pixel_base + j) << 2;
                *dst = f16::from_f32(rgba[src_idx] as f32 / 255.0);
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
                rgba[3] = f16::from_f32(1.0);
            }
        });
    out
}

pub fn encode_r16f(pixels: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    match pixels {
        PixelDatas::F16(rgba) => {
            PixelDatas::F16(encode_r16f_from_f16_impl(rgba, w, h))
        }
        PixelDatas::U8(rgba) => {
            PixelDatas::F16(encode_r16f_from_u8_impl(rgba, w, h))
        }
        PixelDatas::U16(_) => {
            let u8 = pixels.to_rgba8_bytes();
            PixelDatas::F16(encode_r16f_from_u8_impl(&u8, w, h))
        }
        PixelDatas::F32(rgba) => {
            let f16_vec: Vec<f16> = rgba.iter().map(|&v| f16::from_f32(v)).collect();
            PixelDatas::F16(encode_r16f_from_f16_impl(&f16_vec, w, h))
        }
    }
}

pub fn decode_r16f(data: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let src: &[f16] = match data {
        PixelDatas::F16(d) => d.as_slice(),
        PixelDatas::U16(d) => bytemuck::cast_slice(d),
        PixelDatas::U8(d) => bytemuck::cast_slice(d),
        PixelDatas::F32(d) => {
            let f16_vec: Vec<f16> = d.iter().map(|&v| f16::from_f32(v)).collect();
            return PixelDatas::F16(decode_r16f_impl(&f16_vec, w, h));
        }
    };
    PixelDatas::F16(decode_r16f_impl(src, w, h))
}

// ============================================================
// Rg16Float — two-channel half-float
// ============================================================

/// Encode RGBA half-float to Rg16Float by extracting the R and G channels.
///
/// Output is 2 f16 per pixel: [R, G].
fn encode_rg16f_from_f16_impl(rgba: &[f16], width: usize, height: usize) -> Vec<f16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![f16::ZERO; pixel_count << 1];
    out.par_chunks_mut(CHUNK_SIZE << 1)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, pair) in chunk.chunks_mut(2).enumerate() {
                let src_idx = (pixel_base + j) << 2;
                pair[0] = rgba[src_idx];     // R
                pair[1] = rgba[src_idx + 1]; // G
            }
        });
    out
}

/// Encode RGBA8 to Rg16Float by normalizing the R and G channels.
///
/// Output is 2 f16 per pixel: [R, G].
fn encode_rg16f_from_u8_impl(rgba: &[u8], width: usize, height: usize) -> Vec<f16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![f16::ZERO; pixel_count << 1];
    out.par_chunks_mut(CHUNK_SIZE << 1)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, pair) in chunk.chunks_mut(2).enumerate() {
                let src_idx = (pixel_base + j) << 2;
                pair[0] = f16::from_f32(rgba[src_idx] as f32 / 255.0);
                pair[1] = f16::from_f32(rgba[src_idx + 1] as f32 / 255.0);
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
                rgba[0] = data[src_idx];     // R
                rgba[1] = data[src_idx + 1]; // G
                rgba[2] = f16::ZERO;
                rgba[3] = f16::from_f32(1.0);
            }
        });
    out
}

pub fn encode_rg16f(pixels: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    match pixels {
        PixelDatas::F16(rgba) => {
            PixelDatas::F16(encode_rg16f_from_f16_impl(rgba, w, h))
        }
        PixelDatas::U8(rgba) => {
            PixelDatas::F16(encode_rg16f_from_u8_impl(rgba, w, h))
        }
        PixelDatas::U16(_) => {
            let u8 = pixels.to_rgba8_bytes();
            PixelDatas::F16(encode_rg16f_from_u8_impl(&u8, w, h))
        }
        PixelDatas::F32(rgba) => {
            let f16_vec: Vec<f16> = rgba.iter().map(|&v| f16::from_f32(v)).collect();
            PixelDatas::F16(encode_rg16f_from_f16_impl(&f16_vec, w, h))
        }
    }
}

pub fn decode_rg16f(data: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let src: &[f16] = match data {
        PixelDatas::F16(d) => d.as_slice(),
        PixelDatas::U16(d) => bytemuck::cast_slice(d),
        PixelDatas::U8(d) => bytemuck::cast_slice(d),
        PixelDatas::F32(d) => {
            let f16_vec: Vec<f16> = d.iter().map(|&v| f16::from_f32(v)).collect();
            return PixelDatas::F16(decode_rg16f_impl(&f16_vec, w, h));
        }
    };
    PixelDatas::F16(decode_rg16f_impl(src, w, h))
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

/// Encode RGBA8 to Rgba16Float by normalizing all channels.
///
/// Output is 4 f16 per pixel: [R, G, B, A].
fn encode_rgba16f_from_u8_impl(rgba: &[u8], width: usize, height: usize) -> Vec<f16> {
    const CHUNK_SIZE: usize = 4096;
    let pixel_count = width * height;
    let mut out = vec![f16::ZERO; pixel_count << 2];
    out.par_chunks_mut(CHUNK_SIZE << 2)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, quad) in chunk.chunks_mut(4).enumerate() {
                let src_idx = (pixel_base + j) << 2;
                quad[0] = f16::from_f32(rgba[src_idx] as f32 / 255.0);
                quad[1] = f16::from_f32(rgba[src_idx + 1] as f32 / 255.0);
                quad[2] = f16::from_f32(rgba[src_idx + 2] as f32 / 255.0);
                quad[3] = f16::from_f32(rgba[src_idx + 3] as f32 / 255.0);
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
    match pixels {
        PixelDatas::F16(rgba) => {
            PixelDatas::F16(encode_rgba16f_from_f16_impl(rgba, w, h))
        }
        PixelDatas::U8(rgba) => {
            PixelDatas::F16(encode_rgba16f_from_u8_impl(rgba, w, h))
        }
        PixelDatas::U16(_) => {
            let u8 = pixels.to_rgba8_bytes();
            PixelDatas::F16(encode_rgba16f_from_u8_impl(&u8, w, h))
        }
        PixelDatas::F32(rgba) => {
            let f16_vec: Vec<f16> = rgba.iter().map(|&v| f16::from_f32(v)).collect();
            PixelDatas::F16(encode_rgba16f_from_f16_impl(&f16_vec, w, h))
        }
    }
}

pub fn decode_rgba16f(data: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    let src: &[f16] = match data {
        PixelDatas::F16(d) => d.as_slice(),
        PixelDatas::U16(d) => bytemuck::cast_slice(d),
        PixelDatas::U8(d) => bytemuck::cast_slice(d),
        PixelDatas::F32(d) => {
            let f16_vec: Vec<f16> = d.iter().map(|&v| f16::from_f32(v)).collect();
            return PixelDatas::F16(decode_rgba16f_impl(&f16_vec, w, h));
        }
    };
    PixelDatas::F16(decode_rgba16f_impl(src, w, h))
}
