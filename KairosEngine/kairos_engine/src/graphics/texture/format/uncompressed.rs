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
    PixelDatas::U8(decode_r8_impl(data.as_bytes(), w, h, fill_g, fill_b, fill_a))
}
