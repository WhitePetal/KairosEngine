//! Texture compression encoding — converts RGBA8 pixel data into
//! GPU texture formats for `.texture_bin` persistence.
//!
//! ## TODO
//! - BC6h, BC7 encoding
//! - ETC2 / EAC encoding
//! - ASTC encoding
//! - Non-RGBA uncompressed format conversion (channel swizzling)

use crate::graphics::texture_format::TextureFormat;

/// Encode RGBA8 pixel data into `format`.
///
/// Returns `None` when the format is not yet supported for encoding.
pub fn encode_rgba(rgba: &[u8], width: u32, height: u32, format: TextureFormat) -> Option<Vec<u8>> {
    let w = width as usize;
    let h = height as usize;

    match format {
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb => Some(rgba.to_vec()),

        TextureFormat::Bc1RgbaUnorm | TextureFormat::Bc1RgbaUnormSrgb => {
            Some(bc::compress_bc1(rgba, w, h))
        }
        TextureFormat::Bc2RgbaUnorm | TextureFormat::Bc2RgbaUnormSrgb => {
            Some(bc::compress_bc2(rgba, w, h))
        }
        TextureFormat::Bc3RgbaUnorm | TextureFormat::Bc3RgbaUnormSrgb => {
            Some(bc::compress_bc3(rgba, w, h))
        }
        TextureFormat::Bc4RUnorm | TextureFormat::Bc4RSnorm => Some(bc::compress_bc4(rgba, w, h)),
        TextureFormat::Bc5RgUnorm | TextureFormat::Bc5RgSnorm => Some(bc::compress_bc5(rgba, w, h)),

        _ => None,
    }
}

// ============================================================
// BCn pure-Rust encoder
// ============================================================
mod bc {
    /// BC1 (DXT1): 4×4 block, 8 bytes. RGB + optional 1-bit alpha.
    pub fn compress_bc1(rgba: &[u8], width: usize, height: usize) -> Vec<u8> {
        let blocks_x = (width + 3) / 4;
        let blocks_y = (height + 3) / 4;
        let mut out = vec![0u8; blocks_x * blocks_y * 8];

        for by in 0..blocks_y {
            for bx in 0..blocks_x {
                let block = extract_block(rgba, width, height, bx * 4, by * 4);
                let compressed = bc1_block(&block);
                let offset = (by * blocks_x + bx) * 8;
                out[offset..offset + 8].copy_from_slice(&compressed);
            }
        }
        out
    }

    /// BC2 (DXT3): 4×4 block, 16 bytes. BC1 color + 4-bit explicit alpha.
    pub fn compress_bc2(rgba: &[u8], width: usize, height: usize) -> Vec<u8> {
        let blocks_x = (width + 3) / 4;
        let blocks_y = (height + 3) / 4;
        let mut out = vec![0u8; blocks_x * blocks_y * 16];

        for by in 0..blocks_y {
            for bx in 0..blocks_x {
                let block = extract_block(rgba, width, height, bx * 4, by * 4);
                let color = bc1_block(&block);
                let alpha = bc2_alpha_block(&block);
                let offset = (by * blocks_x + bx) * 16;
                out[offset..offset + 8].copy_from_slice(&alpha);
                out[offset + 8..offset + 16].copy_from_slice(&color);
            }
        }
        out
    }

    /// BC3 (DXT5): 4×4 block, 16 bytes. BC1 color + BC4 alpha.
    pub fn compress_bc3(rgba: &[u8], width: usize, height: usize) -> Vec<u8> {
        let blocks_x = (width + 3) / 4;
        let blocks_y = (height + 3) / 4;
        let mut out = vec![0u8; blocks_x * blocks_y * 16];

        for by in 0..blocks_y {
            for bx in 0..blocks_x {
                let block = extract_block(rgba, width, height, bx * 4, by * 4);
                let color = bc1_block(&block);
                let alpha = bc4_channel(&block, 3); // alpha channel
                let offset = (by * blocks_x + bx) * 16;
                out[offset..offset + 8].copy_from_slice(&alpha);
                out[offset + 8..offset + 16].copy_from_slice(&color);
            }
        }
        out
    }

    /// BC4 (RGTC1): 4×4 block, 8 bytes. Single-channel (R).
    pub fn compress_bc4(rgba: &[u8], width: usize, height: usize) -> Vec<u8> {
        let blocks_x = (width + 3) / 4;
        let blocks_y = (height + 3) / 4;
        let mut out = vec![0u8; blocks_x * blocks_y * 8];

        for by in 0..blocks_y {
            for bx in 0..blocks_x {
                let block = extract_block(rgba, width, height, bx * 4, by * 4);
                let compressed = bc4_channel(&block, 0); // R channel
                let offset = (by * blocks_x + bx) * 8;
                out[offset..offset + 8].copy_from_slice(&compressed);
            }
        }
        out
    }

    /// BC5 (RGTC2): 4×4 block, 16 bytes. Two BC4 channels (R, G).
    pub fn compress_bc5(rgba: &[u8], width: usize, height: usize) -> Vec<u8> {
        let blocks_x = (width + 3) / 4;
        let blocks_y = (height + 3) / 4;
        let mut out = vec![0u8; blocks_x * blocks_y * 16];

        for by in 0..blocks_y {
            for bx in 0..blocks_x {
                let block = extract_block(rgba, width, height, bx * 4, by * 4);
                let r = bc4_channel(&block, 0);
                let g = bc4_channel(&block, 1);
                let offset = (by * blocks_x + bx) * 16;
                out[offset..offset + 8].copy_from_slice(&r);
                out[offset + 8..offset + 16].copy_from_slice(&g);
            }
        }
        out
    }

    // ---- Helpers ----

    /// Extract a 4×4 pixel block. Pixels outside the image are black transparent.
    fn extract_block(
        rgba: &[u8],
        width: usize,
        height: usize,
        x: usize,
        y: usize,
    ) -> [[u8; 4]; 16] {
        let mut block = [[0u8; 4]; 16];
        for py in 0..4 {
            for px in 0..4 {
                let sx = x + px;
                let sy = y + py;
                if sx < width && sy < height {
                    let i = (sy * width + sx) * 4;
                    block[py * 4 + px] = [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]];
                }
            }
        }
        block
    }

    /// Compress a single 4×4 block to BC1 (8 bytes).
    fn bc1_block(block: &[[u8; 4]; 16]) -> [u8; 8] {
        let (c0, c1) = optimal_endpoints(block, false);
        let indices = compute_indices(block, c0, c1, false);
        let mut out = [0u8; 8];

        let c0_565 = rgb_to_565(c0);
        let c1_565 = rgb_to_565(c1);

        // Ensure c0 > c1 for 4-color mode
        let (c0_565, c1_565, indices) = if c0_565 > c1_565 {
            (c0_565, c1_565, indices)
        } else {
            // Swap endpoints and flip indices
            let mut flipped = [0u8; 16];
            for (i, &idx) in indices.iter().enumerate() {
                flipped[i] = if idx == 0 {
                    1
                } else if idx == 1 {
                    0
                } else {
                    idx
                };
            }
            (c1_565, c0_565, flipped)
        };

        out[0..2].copy_from_slice(&c0_565.to_le_bytes());
        out[2..4].copy_from_slice(&c1_565.to_le_bytes());
        for i in 0..4 {
            out[4 + i] = (indices[i * 4] as u8)
                | ((indices[i * 4 + 1] as u8) << 2)
                | ((indices[i * 4 + 2] as u8) << 4)
                | ((indices[i * 4 + 3] as u8) << 6);
        }
        out
    }

    /// Compress the alpha channel of a 4×4 block to BC2 alpha (8 bytes).
    fn bc2_alpha_block(block: &[[u8; 4]; 16]) -> [u8; 8] {
        let mut out = [0u8; 8];
        for (i, p) in block.iter().enumerate() {
            let a4 = ((p[3] as u16 * 15 + 128) / 255) as u8; // 8→4 bit
            if i % 2 == 0 {
                out[i / 2] = a4;
            } else {
                out[i / 2] |= a4 << 4;
            }
        }
        out
    }

    /// Compress a single channel to BC4 format (8 bytes).
    fn bc4_channel(block: &[[u8; 4]; 16], channel: usize) -> [u8; 8] {
        // Find min/max of the channel
        let values: Vec<u8> = block.iter().map(|p| p[channel]).collect();
        let min_val = *values.iter().min().unwrap_or(&0);
        let max_val = *values.iter().max().unwrap_or(&255);

        let mut out = [0u8; 8];
        out[0] = max_val;
        out[1] = min_val;

        if max_val > min_val {
            // 8 interpolated values
            let interpolated = |i: u8| -> u8 {
                if i == 0 {
                    max_val
                } else if i == 1 {
                    min_val
                } else if max_val > min_val {
                    let w = (8 - i) as u32 * max_val as u32 + (i - 1) as u32 * min_val as u32;
                    ((w + 3) / 7) as u8
                } else {
                    min_val
                }
            };

            let mut indices: u64 = 0;
            for (i, p) in block.iter().enumerate() {
                let v = p[channel];
                let mut best = 0u8;
                let mut best_err = u16::MAX;
                for idx in 0..8u8 {
                    let err = (v as i16 - interpolated(idx) as i16).unsigned_abs();
                    if err < best_err {
                        best_err = err;
                        best = idx;
                    }
                }
                indices |= (best as u64) << (i * 3);
            }

            out[2..8].copy_from_slice(&indices.to_le_bytes()[..6]);
        }

        out
    }

    /// Find optimal color endpoints for a BC1 block using a simple
    /// min/max approach (fast, good enough for real-time).
    fn optimal_endpoints(block: &[[u8; 4]; 16], _use_alpha: bool) -> ([u8; 3], [u8; 3]) {
        let (mut r_min, mut r_max) = (255u8, 0u8);
        let (mut g_min, mut g_max) = (255u8, 0u8);
        let (mut b_min, mut b_max) = (255u8, 0u8);

        for p in block {
            r_min = r_min.min(p[0]);
            r_max = r_max.max(p[0]);
            g_min = g_min.min(p[1]);
            g_max = g_max.max(p[1]);
            b_min = b_min.min(p[2]);
            b_max = b_max.max(p[2]);
        }

        let c0 = [r_max, g_max, b_max];
        let c1 = [r_min, g_min, b_min];

        // Inset the endpoints slightly to account for the 565 quantization
        let c0_565 = rgb_to_565(c0);
        let c1_565 = rgb_to_565(c1);
        let c0_888 = rgb565_to_888(c0_565);
        let c1_888 = rgb565_to_888(c1_565);

        (
            [c0_888[0], c0_888[1], c0_888[2]],
            [c1_888[0], c1_888[1], c1_888[2]],
        )
    }

    /// Compute 2-bit indices per pixel (0-3) mapping to the palette.
    fn compute_indices(
        block: &[[u8; 4]; 16],
        c0: [u8; 3],
        c1: [u8; 3],
        _use_alpha: bool,
    ) -> [u8; 16] {
        // Palette: c0, c1, c2=(2c0+c1)/3, c3=(c0+2c1)/3
        let c2 = [
            ((2u16 * c0[0] as u16 + c1[0] as u16) / 3) as u8,
            ((2u16 * c0[1] as u16 + c1[1] as u16) / 3) as u8,
            ((2u16 * c0[2] as u16 + c1[2] as u16) / 3) as u8,
        ];
        let c3 = [
            ((c0[0] as u16 + 2u16 * c1[0] as u16) / 3) as u8,
            ((c0[1] as u16 + 2u16 * c1[1] as u16) / 3) as u8,
            ((c0[2] as u16 + 2u16 * c1[2] as u16) / 3) as u8,
        ];
        let palette = [c0, c1, c2, c3];

        let mut indices = [0u8; 16];
        for (i, p) in block.iter().enumerate() {
            let mut best = 0u8;
            let mut best_err = u32::MAX;
            for (idx, &pal) in palette.iter().enumerate() {
                let dr = p[0] as i32 - pal[0] as i32;
                let dg = p[1] as i32 - pal[1] as i32;
                let db = p[2] as i32 - pal[2] as i32;
                let err = (dr * dr + dg * dg + db * db) as u32;
                if err < best_err {
                    best_err = err;
                    best = idx as u8;
                }
            }
            indices[i] = best;
        }
        indices
    }

    fn rgb_to_565(c: [u8; 3]) -> u16 {
        ((c[0] as u16 & 0xF8) << 8) | ((c[1] as u16 & 0xFC) << 3) | (c[2] as u16 >> 3)
    }

    fn rgb565_to_888(c: u16) -> [u8; 3] {
        let r = ((c >> 11) & 0x1F) as u8;
        let g = ((c >> 5) & 0x3F) as u8;
        let b = (c & 0x1F) as u8;
        [
            (r << 3) | (r >> 2),
            (g << 2) | (g >> 4),
            (b << 3) | (b >> 2),
        ]
    }

    // ---- Decompressors (for inspector preview) ----

    pub fn decompress_bc1(data: &[u8], width: usize, height: usize) -> Vec<u8> {
        decompress_blocks(data, width, height, 8, |blk, out| {
            let c0 = u16::from_le_bytes([blk[0], blk[1]]);
            let c1 = u16::from_le_bytes([blk[2], blk[3]]);
            let palette = bc1_palette(c0, c1);
            for i in 0..16 {
                let byte = blk[4 + i / 4];
                let idx = ((byte >> (2 * (i % 4))) & 3) as usize;
                out[i * 4] = palette[idx][0];
                out[i * 4 + 1] = palette[idx][1];
                out[i * 4 + 2] = palette[idx][2];
                out[i * 4 + 3] = 255;
            }
        })
    }

    pub fn decompress_bc2(data: &[u8], width: usize, height: usize) -> Vec<u8> {
        decompress_blocks(data, width, height, 16, |blk, out| {
            // Alpha block (bytes 0-7)
            for i in 0..16 {
                let byte = blk[i / 2];
                let a = if i % 2 == 0 { byte & 0xF } else { byte >> 4 };
                out[i * 4 + 3] = (a as u16 * 255 / 15) as u8;
            }
            // Color block (bytes 8-15)
            let c0 = u16::from_le_bytes([blk[8], blk[9]]);
            let c1 = u16::from_le_bytes([blk[10], blk[11]]);
            let palette = bc1_palette(c0, c1);
            for i in 0..16 {
                let byte = blk[12 + i / 4];
                let idx = ((byte >> (2 * (i % 4))) & 3) as usize;
                out[i * 4] = palette[idx][0];
                out[i * 4 + 1] = palette[idx][1];
                out[i * 4 + 2] = palette[idx][2];
            }
        })
    }

    pub fn decompress_bc3(data: &[u8], width: usize, height: usize) -> Vec<u8> {
        decompress_blocks(data, width, height, 16, |blk, out| {
            // Alpha BC4 block (bytes 0-7)
            let alpha = bc4_decode8(&blk[0..8]);
            // Color BC1 block (bytes 8-15)
            let c0 = u16::from_le_bytes([blk[8], blk[9]]);
            let c1 = u16::from_le_bytes([blk[10], blk[11]]);
            let palette = bc1_palette(c0, c1);
            for i in 0..16 {
                let byte = blk[12 + i / 4];
                let idx = ((byte >> (2 * (i % 4))) & 3) as usize;
                out[i * 4] = palette[idx][0];
                out[i * 4 + 1] = palette[idx][1];
                out[i * 4 + 2] = palette[idx][2];
                out[i * 4 + 3] = alpha[i];
            }
        })
    }

    pub fn decompress_bc4(data: &[u8], width: usize, height: usize) -> Vec<u8> {
        decompress_blocks(data, width, height, 8, |blk, out| {
            let r = bc4_decode8(blk);
            for i in 0..16 {
                out[i * 4] = r[i];
                out[i * 4 + 1] = r[i];
                out[i * 4 + 2] = r[i];
                out[i * 4 + 3] = 255;
            }
        })
    }

    pub fn decompress_bc5(data: &[u8], width: usize, height: usize) -> Vec<u8> {
        decompress_blocks(data, width, height, 16, |blk, out| {
            let r = bc4_decode8(&blk[0..8]);
            let g = bc4_decode8(&blk[8..16]);
            for i in 0..16 {
                out[i * 4] = r[i];
                out[i * 4 + 1] = g[i];
                out[i * 4 + 2] = 0;
                out[i * 4 + 3] = 255;
            }
        })
    }

    fn bc1_palette(c0: u16, c1: u16) -> [[u8; 3]; 4] {
        let c0_888 = rgb565_to_888(c0);
        let c1_888 = rgb565_to_888(c1);
        if c0 > c1 {
            [
                c0_888,
                c1_888,
                [
                    ((2 * c0_888[0] as u16 + c1_888[0] as u16) / 3) as u8,
                    ((2 * c0_888[1] as u16 + c1_888[1] as u16) / 3) as u8,
                    ((2 * c0_888[2] as u16 + c1_888[2] as u16) / 3) as u8,
                ],
                [
                    ((c0_888[0] as u16 + 2 * c1_888[0] as u16) / 3) as u8,
                    ((c0_888[1] as u16 + 2 * c1_888[1] as u16) / 3) as u8,
                    ((c0_888[2] as u16 + 2 * c1_888[2] as u16) / 3) as u8,
                ],
            ]
        } else {
            [
                c0_888,
                c1_888,
                [
                    ((c0_888[0] as u16 + c1_888[0] as u16) / 2) as u8,
                    ((c0_888[1] as u16 + c1_888[1] as u16) / 2) as u8,
                    ((c0_888[2] as u16 + c1_888[2] as u16) / 2) as u8,
                ],
                [0, 0, 0], // transparent black
            ]
        }
    }

    fn bc4_decode8(blk: &[u8]) -> [u8; 16] {
        let a0 = blk[0] as u32;
        let a1 = blk[1] as u32;
        let mut vals = [0u8; 16];
        if a0 > a1 {
            for i in 0..16 {
                let byte = blk[2 + (3 * i) / 8];
                let idx = ((byte >> ((3 * i) % 8)) & 7) as u32;
                vals[i] = match idx {
                    0 => a0 as u8,
                    1 => a1 as u8,
                    idx => (((8 - idx) * a0 + (idx - 1) * a1 + 3) / 7) as u8,
                };
            }
        } else {
            for i in 0..16 {
                let byte = blk[2 + (3 * i) / 8];
                let idx = ((byte >> ((3 * i) % 8)) & 7) as u32;
                vals[i] = match idx {
                    0 => a0 as u8,
                    1 => a1 as u8,
                    2..=5 => (((6 - idx) * a0 + (idx - 1) * a1 + 2) / 5) as u8,
                    6 => 0,
                    _ => 255,
                };
            }
        }
        vals
    }

    fn decompress_blocks(
        data: &[u8],
        width: usize,
        height: usize,
        block_size: usize,
        decode: impl Fn(&[u8], &mut [u8; 64]),
    ) -> Vec<u8> {
        let blocks_x = (width + 3) / 4;
        let blocks_y = (height + 3) / 4;
        let mut out = vec![0u8; width * height * 4];

        for by in 0..blocks_y {
            for bx in 0..blocks_x {
                let offset = (by * blocks_x + bx) * block_size;
                let mut pixels = [0u8; 64];
                decode(&data[offset..offset + block_size], &mut pixels);
                for py in 0..4 {
                    for px in 0..4 {
                        let sx = bx * 4 + px;
                        let sy = by * 4 + py;
                        if sx < width && sy < height {
                            let dst = (sy * width + sx) * 4;
                            let src = (py * 4 + px) * 4;
                            out[dst..dst + 4].copy_from_slice(&pixels[src..src + 4]);
                        }
                    }
                }
            }
        }
        out
    }
} // mod bc

/// Decode compressed texture data back to RGBA8 for preview.
pub fn decode_to_rgba8(
    data: &[u8],
    width: u32,
    height: u32,
    format: TextureFormat,
) -> Option<Vec<u8>> {
    let w = width as usize;
    let h = height as usize;
    match format {
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb => Some(data.to_vec()),
        TextureFormat::Bc1RgbaUnorm | TextureFormat::Bc1RgbaUnormSrgb => {
            Some(bc::decompress_bc1(data, w, h))
        }
        TextureFormat::Bc2RgbaUnorm | TextureFormat::Bc2RgbaUnormSrgb => {
            Some(bc::decompress_bc2(data, w, h))
        }
        TextureFormat::Bc3RgbaUnorm | TextureFormat::Bc3RgbaUnormSrgb => {
            Some(bc::decompress_bc3(data, w, h))
        }
        TextureFormat::Bc4RUnorm | TextureFormat::Bc4RSnorm => Some(bc::decompress_bc4(data, w, h)),
        TextureFormat::Bc5RgUnorm | TextureFormat::Bc5RgSnorm => {
            Some(bc::decompress_bc5(data, w, h))
        }
        TextureFormat::R8Unorm => todo!(),
        TextureFormat::R8Snorm => todo!(),
        TextureFormat::R8Uint => todo!(),
        TextureFormat::R8Sint => todo!(),
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
