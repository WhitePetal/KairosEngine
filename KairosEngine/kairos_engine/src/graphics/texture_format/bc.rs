//! BCn (Block Compression) pure-Rust encoder/decoder.

use rayon::prelude::*;

// ============================================================
// Compressors
// ============================================================

macro_rules! compress_fn {
    ($name:ident, $block_size:expr, $block_fn:ident) => {
        pub fn $name(rgba: &[u8], width: usize, height: usize) -> Vec<u8> {
            let bx = (width + 3) / 4;
            let by = (height + 3) / 4;
            let mut out = vec![0u8; bx * by * $block_size];
            out.par_chunks_mut($block_size)
                .enumerate()
                .for_each(|(i, chunk)| {
                    let bx_i = i % bx;
                    let by_i = i / bx;
                    let block = extract_block(rgba, width, height, bx_i * 4, by_i * 4);
                    chunk.copy_from_slice(&$block_fn(&block));
                });
            out
        }
    };
}

compress_fn!(compress_bc1, 8, bc1_block);
compress_fn!(compress_bc2, 16, bc2_block);
compress_fn!(compress_bc3, 16, bc3_block);
compress_fn!(compress_bc4, 8, bc4_block);
compress_fn!(compress_bc5, 16, bc5_block);

// ============================================================
// Decompressors
// ============================================================

pub fn decompress_bc1(data: &[u8], width: usize, height: usize) -> Vec<u8> {
    decompress_blocks(data, width, height, 8, decode_bc1_block)
}
pub fn decompress_bc2(data: &[u8], width: usize, height: usize) -> Vec<u8> {
    decompress_blocks(data, width, height, 16, decode_bc2_block)
}
pub fn decompress_bc3(data: &[u8], width: usize, height: usize) -> Vec<u8> {
    decompress_blocks(data, width, height, 16, decode_bc3_block)
}
pub fn decompress_bc4(data: &[u8], width: usize, height: usize) -> Vec<u8> {
    decompress_blocks(data, width, height, 8, decode_bc4_block)
}
pub fn decompress_bc5(data: &[u8], width: usize, height: usize) -> Vec<u8> {
    decompress_blocks(data, width, height, 16, decode_bc5_block)
}

fn decompress_blocks(
    data: &[u8],
    width: usize,
    height: usize,
    block_size: usize,
    decode: impl Fn(&[u8], &mut [u8; 64]) + Sync,
) -> Vec<u8> {
    let bx = (width + 3) / 4;
    let by = (height + 3) / 4;
    let total = bx * by;
    let mut out = vec![0u8; width * height * 4];
    let out_addr = out.as_mut_ptr() as usize;

    (0..total).into_par_iter().for_each(|i| {
        let out_ptr = out_addr as *mut u8;
        let bx_i = i % bx;
        let by_i = i / bx;
        let off = i * block_size;
        let mut pixels = [0u8; 64];
        decode(&data[off..off + block_size], &mut pixels);
        for py in 0..4 {
            for px in 0..4 {
                let sx = bx_i * 4 + px;
                let sy = by_i * 4 + py;
                if sx < width && sy < height {
                    let dst = (sy * width + sx) * 4;
                    let src = (py * 4 + px) * 4;
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

    out
}

// ============================================================
// Block encode helpers
// ============================================================

fn bc1_block(block: &[[u8; 4]; 16]) -> [u8; 8] { encode_bc1_color(block) }
fn bc2_block(block: &[[u8; 4]; 16]) -> [u8; 16] { let mut o = [0u8; 16]; o[..8].copy_from_slice(&bc2_alpha_block(block)); o[8..].copy_from_slice(&bc1_block(block)); o }
fn bc3_block(block: &[[u8; 4]; 16]) -> [u8; 16] { let mut o = [0u8; 16]; o[..8].copy_from_slice(&bc4_channel(block, 3)); o[8..].copy_from_slice(&bc1_block(block)); o }
fn bc4_block(block: &[[u8; 4]; 16]) -> [u8; 8] { bc4_channel(block, 0) }
fn bc5_block(block: &[[u8; 4]; 16]) -> [u8; 16] { let mut o = [0u8; 16]; o[..8].copy_from_slice(&bc4_channel(block, 0)); o[8..].copy_from_slice(&bc4_channel(block, 1)); o }

// ============================================================
// Block decode helpers
// ============================================================

fn decode_bc1_block(blk: &[u8], out: &mut [u8; 64]) {
    let c0 = u16::from_le_bytes([blk[0], blk[1]]);
    let c1 = u16::from_le_bytes([blk[2], blk[3]]);
    let p = bc1_palette(c0, c1);
    for i in 0..16 {
        let b = blk[4 + i / 4];
        let idx = ((b >> (2 * (i % 4))) & 3) as usize;
        out[i * 4] = p[idx][0];
        out[i * 4 + 1] = p[idx][1];
        out[i * 4 + 2] = p[idx][2];
        out[i * 4 + 3] = 255;
    }
}

fn decode_bc2_block(blk: &[u8], out: &mut [u8; 64]) {
    for i in 0..16 {
        let b = blk[i / 2];
        let a = if i % 2 == 0 { b & 0xF } else { b >> 4 };
        out[i * 4 + 3] = (a as u16 * 255 / 15) as u8;
    }
    let c0 = u16::from_le_bytes([blk[8], blk[9]]);
    let c1 = u16::from_le_bytes([blk[10], blk[11]]);
    let p = bc1_palette(c0, c1);
    for i in 0..16 {
        let b = blk[12 + i / 4];
        let idx = ((b >> (2 * (i % 4))) & 3) as usize;
        out[i * 4] = p[idx][0];
        out[i * 4 + 1] = p[idx][1];
        out[i * 4 + 2] = p[idx][2];
    }
}

fn decode_bc3_block(blk: &[u8], out: &mut [u8; 64]) {
    let a = bc4_decode8(&blk[0..8]);
    let c0 = u16::from_le_bytes([blk[8], blk[9]]);
    let c1 = u16::from_le_bytes([blk[10], blk[11]]);
    let p = bc1_palette(c0, c1);
    for i in 0..16 {
        let b = blk[12 + i / 4];
        let idx = ((b >> (2 * (i % 4))) & 3) as usize;
        out[i * 4] = p[idx][0];
        out[i * 4 + 1] = p[idx][1];
        out[i * 4 + 2] = p[idx][2];
        out[i * 4 + 3] = a[i];
    }
}

fn decode_bc4_block(blk: &[u8], out: &mut [u8; 64]) {
    let r = bc4_decode8(blk);
    for i in 0..16 {
        out[i * 4] = r[i];
        out[i * 4 + 1] = r[i];
        out[i * 4 + 2] = r[i];
        out[i * 4 + 3] = 255;
    }
}

fn decode_bc5_block(blk: &[u8], out: &mut [u8; 64]) {
    let r = bc4_decode8(&blk[0..8]);
    let g = bc4_decode8(&blk[8..16]);
    for i in 0..16 {
        out[i * 4] = r[i];
        out[i * 4 + 1] = g[i];
        out[i * 4 + 2] = 0;
        out[i * 4 + 3] = 255;
    }
}

// ============================================================
// Low-level helpers
// ============================================================

fn extract_block(rgba: &[u8], w: usize, h: usize, x: usize, y: usize) -> [[u8; 4]; 16] {
    let mut block = [[0u8; 4]; 16];
    for py in 0..4 {
        for px in 0..4 {
            let sx = x + px;
            let sy = y + py;
            if sx < w && sy < h {
                let i = (sy * w + sx) * 4;
                block[py * 4 + px] = [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]];
            }
        }
    }
    block
}

fn encode_bc1_color(block: &[[u8; 4]; 16]) -> [u8; 8] {
    let (c0, c1) = optimal_endpoints(block);
    let indices = compute_indices(block, c0, c1);
    let mut out = [0u8; 8];
    let c0_565 = rgb_to_565(c0);
    let c1_565 = rgb_to_565(c1);
    let (c0_565, c1_565, indices) = if c0_565 > c1_565 {
        (c0_565, c1_565, indices)
    } else {
        let mut flipped = [0u8; 16];
        for (i, &idx) in indices.iter().enumerate() {
            flipped[i] = if idx == 0 { 1 } else if idx == 1 { 0 } else { idx };
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

fn bc2_alpha_block(block: &[[u8; 4]; 16]) -> [u8; 8] {
    let mut out = [0u8; 8];
    for (i, p) in block.iter().enumerate() {
        let a4 = ((p[3] as u16 * 15 + 128) / 255) as u8;
        if i % 2 == 0 { out[i / 2] = a4; }
        else { out[i / 2] |= a4 << 4; }
    }
    out
}

fn bc4_channel(block: &[[u8; 4]; 16], ch: usize) -> [u8; 8] {
    let vals: Vec<u8> = block.iter().map(|p| p[ch]).collect();
    let min_val = *vals.iter().min().unwrap_or(&0);
    let max_val = *vals.iter().max().unwrap_or(&255);
    let mut out = [0u8; 8];
    out[0] = max_val;
    out[1] = min_val;
    if max_val > min_val {
        let interp = |i: u8| -> u8 {
            match i {
                0 => max_val,
                1 => min_val,
                _ => (((8 - i) as u32 * max_val as u32 + (i - 1) as u32 * min_val as u32 + 3) / 7) as u8,
            }
        };
        let mut indices: u64 = 0;
        for (i, p) in block.iter().enumerate() {
            let v = p[ch];
            let mut best = 0u8;
            let mut best_err = u16::MAX;
            for idx in 0..8u8 {
                let err = (v as i16 - interp(idx) as i16).unsigned_abs();
                if err < best_err { best_err = err; best = idx; }
            }
            indices |= (best as u64) << (i * 3);
        }
        out[2..8].copy_from_slice(&indices.to_le_bytes()[..6]);
    }
    out
}

fn optimal_endpoints(block: &[[u8; 4]; 16]) -> ([u8; 3], [u8; 3]) {
    let (mut r_min, mut r_max) = (255u8, 0u8);
    let (mut g_min, mut g_max) = (255u8, 0u8);
    let (mut b_min, mut b_max) = (255u8, 0u8);
    for p in block {
        r_min = r_min.min(p[0]); r_max = r_max.max(p[0]);
        g_min = g_min.min(p[1]); g_max = g_max.max(p[1]);
        b_min = b_min.min(p[2]); b_max = b_max.max(p[2]);
    }
    let c0_565 = rgb_to_565([r_max, g_max, b_max]);
    let c1_565 = rgb_to_565([r_min, g_min, b_min]);
    (rgb565_to_888(c0_565), rgb565_to_888(c1_565))
}

fn compute_indices(block: &[[u8; 4]; 16], c0: [u8; 3], c1: [u8; 3]) -> [u8; 16] {
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
    let p = [c0, c1, c2, c3];
    let mut indices = [0u8; 16];
    for (i, px) in block.iter().enumerate() {
        let mut best = 0u8;
        let mut best_err = u32::MAX;
        for (idx, &pal) in p.iter().enumerate() {
            let dr = px[0] as i32 - pal[0] as i32;
            let dg = px[1] as i32 - pal[1] as i32;
            let db = px[2] as i32 - pal[2] as i32;
            let err = (dr * dr + dg * dg + db * db) as u32;
            if err < best_err { best_err = err; best = idx as u8; }
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

fn bc1_palette(c0: u16, c1: u16) -> [[u8; 3]; 4] {
    let c0_888 = rgb565_to_888(c0);
    let c1_888 = rgb565_to_888(c1);
    if c0 > c1 {
        [c0_888, c1_888,
            [((2 * c0_888[0] as u16 + c1_888[0] as u16) / 3) as u8,
             ((2 * c0_888[1] as u16 + c1_888[1] as u16) / 3) as u8,
             ((2 * c0_888[2] as u16 + c1_888[2] as u16) / 3) as u8],
            [((c0_888[0] as u16 + 2 * c1_888[0] as u16) / 3) as u8,
             ((c0_888[1] as u16 + 2 * c1_888[1] as u16) / 3) as u8,
             ((c0_888[2] as u16 + 2 * c1_888[2] as u16) / 3) as u8]]
    } else {
        [c0_888, c1_888,
            [((c0_888[0] as u16 + c1_888[0] as u16) / 2) as u8,
             ((c0_888[1] as u16 + c1_888[1] as u16) / 2) as u8,
             ((c0_888[2] as u16 + c1_888[2] as u16) / 2) as u8],
            [0, 0, 0]]
    }
}

fn bc4_decode8(blk: &[u8]) -> [u8; 16] {
    let a0 = blk[0] as u32;
    let a1 = blk[1] as u32;
    let mut vals = [0u8; 16];
    if a0 > a1 {
        for i in 0..16 {
            let b = blk[2 + (3 * i) / 8];
            let idx = ((b >> ((3 * i) % 8)) & 7) as u32;
            vals[i] = match idx {
                0 => a0 as u8, 1 => a1 as u8,
                idx => (((8 - idx) * a0 + (idx - 1) * a1 + 3) / 7) as u8,
            };
        }
    } else {
        for i in 0..16 {
            let b = blk[2 + (3 * i) / 8];
            let idx = ((b >> ((3 * i) % 8)) & 7) as u32;
            vals[i] = match idx {
                0 => a0 as u8, 1 => a1 as u8,
                2..=5 => (((6 - idx) * a0 + (idx - 1) * a1 + 2) / 5) as u8,
                6 => 0, _ => 255,
            };
        }
    }
    vals
}
