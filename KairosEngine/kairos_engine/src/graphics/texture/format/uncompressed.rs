use rayon::prelude::*;

use crate::graphics::texture::format::PixelDatas;

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
                *data = rgba[(pixel_base + j) * 4];
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
    let mut out = vec![0u8; pixel_count * 4];

    match (fill_g, fill_b, fill_a) {
        (true, true, true) => {
            out.par_chunks_mut(CHUNK_SIZE)
                .enumerate()
                .for_each(|(chunk_idx, chunk)| {
                    let byte_base = chunk_idx * CHUNK_SIZE;
                    for (j, rgba) in chunk.iter_mut().enumerate() {
                        let pixel_idx = (byte_base + j) / 4;
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
                            *rgba = data[(byte_base + j) / 4];
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
                            *rgba = data[idx / 4];
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
    PixelDatas::U8(encode_r8_impl(pixels.as_bytes(), w, h))
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
    let mut out = vec![0u8; pixel_count * 2];
    out.par_chunks_mut(CHUNK_SIZE * 2)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * CHUNK_SIZE;
            for (j, pair) in chunk.chunks_mut(2).enumerate() {
                let abs_pixel = pixel_base + j;
                let src_idx = abs_pixel * 4;
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
    let mut out = vec![0u8; pixel_count * 4];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * (CHUNK_SIZE / 4);
            for (j, rgba) in chunk.chunks_mut(4).enumerate() {
                let abs_pixel = pixel_base + j;
                let src_idx = abs_pixel * 2;
                rgba[0] = data[src_idx];         // R
                rgba[1] = data[src_idx + 1];     // G
                rgba[2] = 0;                      // B
                rgba[3] = 255;                    // A
            }
        });
    out
}

pub fn encode_rg8(pixels: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    PixelDatas::U8(encode_rg8_impl(pixels.as_bytes(), w, h))
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
    let mut out = vec![0u8; pixel_count * 4];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * (CHUNK_SIZE / 4);
            for (j, bgra) in chunk.chunks_mut(4).enumerate() {
                let abs_pixel = pixel_base + j;
                let src_idx = abs_pixel * 4;
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
    let mut out = vec![0u8; pixel_count * 4];
    out.par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let pixel_base = chunk_idx * (CHUNK_SIZE / 4);
            for (j, rgba) in chunk.chunks_mut(4).enumerate() {
                let abs_pixel = pixel_base + j;
                let src_idx = abs_pixel * 4;
                rgba[0] = data[src_idx + 2]; // R (was B)
                rgba[1] = data[src_idx + 1]; // G (was G)
                rgba[2] = data[src_idx];     // B (was R)
                rgba[3] = data[src_idx + 3]; // A (was A)
            }
        });
    out
}

pub fn encode_bgra8(pixels: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    PixelDatas::U8(encode_bgra8_impl(pixels.as_bytes(), w, h))
}

pub fn decode_bgra8(data: &PixelDatas, w: usize, h: usize) -> PixelDatas {
    PixelDatas::U8(decode_bgra8_impl(data.as_bytes(), w, h))
}
