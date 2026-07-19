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
        TextureFormat::Bc4RUnorm | TextureFormat::Bc4RSnorm => {
            Some(bc::compress_bc4(rgba, w, h))
        }
        TextureFormat::Bc5RgUnorm | TextureFormat::Bc5RgSnorm => {
            Some(bc::compress_bc5(rgba, w, h))
        }

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
    fn extract_block(rgba: &[u8], width: usize, height: usize, x: usize, y: usize) -> [[u8; 4]; 16] {
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

        ([c0_888[0], c0_888[1], c0_888[2]], [
            c1_888[0],
            c1_888[1],
            c1_888[2],
        ])
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
        [(r << 3) | (r >> 2), (g << 2) | (g >> 4), (b << 3) | (b >> 2)]
    }
}
