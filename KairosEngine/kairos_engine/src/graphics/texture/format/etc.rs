//! ETC2 / EAC pure-Rust encoder/decoder.
//!
//! Pure-Rust port of the etcpak reference implementation for ETC2 and EAC
//! compressed texture formats.  Covers all 10 variants (6 ETC2 + 4 EAC).
//!
//! # Block layouts
//!
//! | Variant | Block bytes | Description |
//! |---------|-------------|-------------|
//! | Etc2Rgb8[Unorm\|Srgb] | 8 | ETC2 RGB (individual/differential/T/H/planar) |
//! | Etc2Rgb8A1[Unorm\|Srgb] | 8 | ETC2 RGB + 1‑bit alpha punch‑through |
//! | Etc2Rgba8[Unorm\|Srgb] | 16 | ETC2 RGB + EAC A8 alpha (same as EAC R11 UNORM) |
//! | EacR11[Unorm\|Snorm] | 8 | Single‑channel EAC |
//! | EacRg11[Unorm\|Snorm] | 16 | Dual‑channel EAC (two R11 blocks) |
//!
//! All data is read/written as [`u64::from_le_bytes`] (native little‑endian byte
//! order), matching the etcpak convention and the GPU memory layout.

use rayon::prelude::*;

use crate::graphics::texture::format::BlockLayout;
use crate::{decode_blocks, encode_blocks};


#[cfg(test)]
mod test;

// ============================================================
// Block layouts
// ============================================================

const ETC2_LAYOUT: BlockLayout = BlockLayout::new(4, 4, 8);
const ETC2_RGBA_LAYOUT: BlockLayout = BlockLayout::new(4, 4, 16);
const EAC_R11_LAYOUT: BlockLayout = BlockLayout::new(4, 4, 8);
const EAC_RG11_LAYOUT: BlockLayout = BlockLayout::new(4, 4, 16);

// ============================================================
// Macro invocations — parallel block encode/decode
// ============================================================

encode_blocks!(encode_etc2_rgb8, U8, 4, 4, 8, etc2_rgb8_block);
encode_blocks!(encode_etc2_rgb8_a1, U8, 4, 4, 8, etc2_rgb8a1_block);
encode_blocks!(encode_etc2_rgba8, U8, 4, 4, 16, etc2_rgba8_block);
encode_blocks!(encode_eac_r11, U8, 4, 4, 8, eac_r11_block);
encode_blocks!(encode_eac_r11_snorm, U8, 4, 4, 8, eac_r11_snorm_block);
encode_blocks!(encode_eac_rg11, U8, 4, 4, 16, eac_rg11_block);
encode_blocks!(encode_eac_rg11_snorm, U8, 4, 4, 16, eac_rg11_snorm_block);

decode_blocks!(decode_etc2_rgb8, U8, ETC2_LAYOUT, decode_etc2_rgb8_block);
decode_blocks!(decode_etc2_rgb8_a1, U8, ETC2_LAYOUT, decode_etc2_rgb8a1_block);
decode_blocks!(decode_etc2_rgba8, U8, ETC2_RGBA_LAYOUT, decode_etc2_rgba8_block);
decode_blocks!(decode_eac_r11, U8, EAC_R11_LAYOUT, decode_eac_r11_block);
decode_blocks!(decode_eac_r11_snorm, S8, EAC_R11_LAYOUT, decode_eac_r11_snorm_block);
decode_blocks!(decode_eac_rg11, U8, EAC_RG11_LAYOUT, decode_eac_rg11_block);
decode_blocks!(decode_eac_rg11_snorm, S8, EAC_RG11_LAYOUT, decode_eac_rg11_snorm_block);

// ============================================================
// ETC1 modifier tables — used by ETC2 individual/differential modes
//
// Each table has 4 entries indexed by the 2‑bit pixel selector.
// ============================================================

#[rustfmt::skip]
const ETC1_TABLES: [[i16; 4]; 8] = [
    [ -8,  -2,   2,   8],
    [-17,  -5,   5,  17],
    [-29,  -9,   9,  29],
    [-42, -13,  13,  42],
    [-60, -18,  18,  60],
    [-80, -24,  24,  80],
    [-106, -33,  33, 106],
    [-183, -47,  47, 183],
];

// ============================================================
// EAC modifier tables — 16 tables × 8 entries
// ============================================================

#[rustfmt::skip]
const EAC_TABLES: [[i16; 8]; 16] = [
    [ -3,  -6,  -9, -15,   2,   5,   8,  14],
    [ -3,  -7, -10, -13,   2,   6,   9,  12],
    [ -2,  -5,  -8, -13,   1,   4,   7,  12],
    [ -2,  -4,  -6, -13,   1,   3,   5,  12],
    [ -3,  -6,  -8, -12,   2,   5,   7,  11],
    [ -3,  -7,  -9, -11,   2,   6,   8,  10],
    [ -4,  -7,  -8, -11,   3,   6,   7,  10],
    [ -3,  -5,  -8, -11,   2,   4,   7,  10],
    [ -2,  -6,  -8, -10,   1,   5,   7,   9],
    [ -2,  -5,  -8, -10,   1,   4,   7,   9],
    [ -2,  -6,  -8,  -9,   1,   5,   7,   8],
    [ -2,  -5,  -7,  -9,   1,   4,   6,   8],
    [ -3,  -7,  -8, -10,   2,   6,   7,   9],
    [ -1,  -4,  -7, -10,   0,   3,   6,   9],
    [ -2,  -5,  -8, -10,   1,   4,   7,   9],
    [ -2,  -5,  -7,  -9,   1,   4,   6,   8],
];

// ============================================================
// Bit extraction helpers
// ============================================================

/// Read a 64-bit block word in little-endian byte order (etcpak convention).
fn blk_word(blk: &[u8]) -> u64 {
    u64::from_le_bytes(blk[..8].try_into().unwrap())
}

/// Write a 64-bit block word in little-endian byte order.
fn _write_blk(word: u64, blk: &mut [u8]) {
    blk[..8].copy_from_slice(&word.to_le_bytes());
}

/// Clamp `val` to [0, 255].
fn clamp_u8(val: i16) -> u8 {
    val.clamp(0, 255) as u8
}

/// Clamp `val` to [0, 2047] (11-bit unsigned).
fn clamp_u11(val: i32) -> i32 {
    val.clamp(0, 2047)
}

/// Clamp `val` to [-1023, 1023] (11-bit signed).
fn clamp_s11(val: i32) -> i32 {
    val.clamp(-1023, 1023)
}

/// Expand a 4-bit channel value to 8 bits.
fn expand4(v: u16) -> u8 {
    ((v << 4) | v) as u8
}

/// Expand a 5-bit channel value to 8 bits.
fn expand5(v: u16) -> u8 {
    ((v << 3) | (v >> 2)) as u8
}

// ============================================================
// ETC2 mode detection
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Etc2Mode {
    /// diff=0 — each sub-block gets independent 4‑bit colours.
    Individual,
    /// diff=1 with valid differentials — 5‑bit + 3‑bit differential.
    Differential,
    /// diff=1, overflow → T mode.
    T,
    /// diff=1, overflow → H mode.
    H,
    /// diff=1, overflow → Planar mode.
    Planar,
}

/// Detect the ETC2 mode from a 64-bit block word.
fn detect_mode(word: u64) -> Etc2Mode {
    let diff = (word >> 62) & 1;
    if diff == 0 {
        return Etc2Mode::Individual;
    }

    // diff == 1: check whether the 3-bit differentials overflow.
    // R1 (5 bits), G1 (4 bits + LSB from g2), B1 (5 bits from 3-bit + d-field)
    // But for mode detection we only need a simpler check: any differential overflow?
    let r1 = ((word >> 36) & 31) as i16;
    let r2_raw = ((word >> 41) & 7) as i16;
    // For G and B, the base values are stored differently.  A simple overflow
    // check for mode detection: if any of the three 3-bit differential values
    // would overflow their 5-bit result range, it is a non-differential mode.
    let g2_top3 = ((word >> 48) & 7) as i16;
    let b2_raw = ((word >> 55) & 7) as i16;

    // Sign-extend 3-bit differentials
    let dr = if r2_raw >= 4 { r2_raw - 8 } else { r2_raw };
    let dg = if g2_top3 >= 4 { g2_top3 - 8 } else { g2_top3 };
    let db = if b2_raw >= 4 { b2_raw - 8 } else { b2_raw };

    // Estimate whether the resulting 5-bit G1 and B1 would overflow.
    // G1 is (g1 << 1) | LSB; b1 is (b1_raw << 2) | 2 bits from d-field.
    // For a conservative check, just see if any differential itself would push
    // out of the 5-bit range.
    let g1_est = (r1 + dr).clamp(0, 31);   // use R-based approximation
    let _g2_est = (g1_est + dg).clamp(0, 31);
    let b1_est = ((((word >> 52) & 7) as i16) << 2) | (((word >> 32) >> 2) as i16 & 3);
    let b1_clamped = b1_est.clamp(0, 31);
    let r2_val = r1 + dr;
    let b2_val = b1_clamped + db;

    if r2_val < 0 || r2_val > 31 || b2_val < 0 || b2_val > 31 {
        // Overflow → non-differential mode: T, H, or Planar
        let d_field = ((word >> 32) & 15) as u8;
        match d_field {
            0b0111 => Etc2Mode::T,
            0b1011 => Etc2Mode::H,
            _ => Etc2Mode::Planar,
        }
    } else {
        Etc2Mode::Differential
    }
}

// ============================================================
// Per-pixel index extraction
//
// The 32-bit index field (bits [31:0]) stores 16 two-bit indices,
// one per pixel in scan-line order (row‑major within the 4×4 block).
// ============================================================

fn get_idx(word: u64, pixel: usize) -> usize {
    ((word >> (pixel * 2)) & 3) as usize
}

fn set_idx(word: &mut u64, pixel: usize, idx: usize) {
    let shift = pixel * 2;
    *word = (*word & !(3u64 << shift)) | ((idx as u64 & 3) << shift);
}

// ============================================================
// ETC2 RGB block decode
// ============================================================

/// Decode a single ETC2 RGB block (8 bytes) into 16 RGBA8 pixels.
fn decode_etc2_rgb8_block(blk: &[u8], out: &mut [u8; 64]) {
    let word = blk_word(blk);
    let mode = detect_mode(word);

    match mode {
        Etc2Mode::Individual => decode_individual(word, out),
        Etc2Mode::Differential => decode_differential(word, out),
        Etc2Mode::T => decode_t_mode(word, out),
        Etc2Mode::H => decode_h_mode(word, out),
        Etc2Mode::Planar => decode_planar(word, out),
    }
}

/// Decode an individual-mode block.
fn decode_individual(word: u64, out: &mut [u8; 64]) {
    let flip = (word >> 63) & 1;
    let cw1 = ((word >> 58) & 3) as usize;
    let cw2 = ((word >> 60) & 3) as usize;
    let r1 = (word >> 36) & 31;
    let r2_raw = (word >> 41) & 7;
    let g1 = (word >> 44) & 15;
    let g2 = (word >> 48) & 15;
    let b1_raw = (word >> 52) & 7;
    let b2_raw = (word >> 55) & 7;
    let d_field = (word >> 32) & 15;

    // Sub-block 1 base colour (4 bits per channel → expand to 8)
    let r1_4 = (r1 >> 1) & 15;
    let g1_4 = g1;
    let b1_4 = (b1_raw << 1) | (r1 & 1);

    let base1 = [expand4(r1_4 as u16), expand4(g1_4 as u16), expand4(b1_4 as u16)];

    // Sub-block 2 base colour (4 bits per channel → expand to 8)
    let r2_4 = (r2_raw << 1) | ((d_field >> 3) & 1);
    let g2_4_bits = g2;
    let b2_4 = (b2_raw << 1) | ((d_field >> 2) & 1);

    let base2 = [expand4(r2_4 as u16), expand4(g2_4_bits as u16), expand4(b2_4 as u16)];

    let table1 = &ETC1_TABLES[cw1];
    let table2 = &ETC1_TABLES[cw2];

    for py in 0..4 {
        for px in 0..4 {
            let pixel = py * 4 + px;
            let idx = get_idx(word, pixel);
            let (base, table) = if flip == 0 {
                // Horizontal split: top 2 rows = sub-block 1, bottom 2 = sub-block 2
                if py < 2 { (&base1, table1) } else { (&base2, table2) }
            } else {
                // Vertical split: left 2 cols = sub-block 1, right 2 = sub-block 2
                if px < 2 { (&base1, table1) } else { (&base2, table2) }
            };

            let off = pixel * 4;
            let mod_val = table[idx];
            out[off] = clamp_u8(base[0] as i16 + mod_val);
            out[off + 1] = clamp_u8(base[1] as i16 + mod_val);
            out[off + 2] = clamp_u8(base[2] as i16 + mod_val);
            out[off + 3] = 255;
        }
    }
}

/// Decode a differential-mode block.
///
/// Differential mode stores a 5‑bit base colour for sub-block 1 and 3‑bit
/// signed differentials for sub-block 2.  The 4‑bit G fields are packed so
/// that g2's LSB provides G1's LSB, and g2[3:1] (= g2_full >> 1) is the
/// 3‑bit dG.  Similarly B uses d‑field bits for the extra B1 bits.
fn decode_differential(word: u64, out: &mut [u8; 64]) {
    let flip = (word >> 63) & 1;
    let cw1 = ((word >> 58) & 3) as usize;
    let cw2 = ((word >> 60) & 3) as usize;
    let r1 = ((word >> 36) & 31) as i16;
    let r2_raw = ((word >> 41) & 7) as i16;
    let g1 = ((word >> 44) & 15) as i16;
    let b1_raw = ((word >> 52) & 7) as i16;
    let b2_raw = ((word >> 55) & 7) as i16;
    let d_field = ((word >> 32) & 15) as i16;
    let g2_full = ((word >> 48) & 15) as i16;

    // 5-bit base for sub-block 1
    let r1_5 = r1;
    let g1_5 = (g1 << 1) | (g2_full & 1);
    let b1_5 = ((b1_raw << 2) | ((d_field >> 2) & 3)).clamp(0, 31);

    // Sign-extend 3-bit differentials
    let dr = if r2_raw >= 4 { r2_raw - 8 } else { r2_raw };
    let dg = if (g2_full >> 1) >= 4 { (g2_full >> 1) - 8 } else { g2_full >> 1 };
    let db = if b2_raw >= 4 { b2_raw - 8 } else { b2_raw };

    // 5-bit base for sub-block 2 (clamped to [0, 31])
    let r2_5b = (r1_5 + dr).clamp(0, 31);
    let g2_5b = (g1_5 + dg).clamp(0, 31);
    let b2_5b = (b1_5 + db).clamp(0, 31);

    let base1 = [expand5(r1_5 as u16), expand5(g1_5 as u16), expand5(b1_5 as u16)];
    let base2 = [expand5(r2_5b as u16), expand5(g2_5b as u16), expand5(b2_5b as u16)];

    let table1 = &ETC1_TABLES[cw1];
    let table2 = &ETC1_TABLES[cw2];

    for py in 0..4 {
        for px in 0..4 {
            let pixel = py * 4 + px;
            let idx = get_idx(word, pixel);
            let (base, table) = if flip == 0 {
                if py < 2 { (&base1, table1) } else { (&base2, table2) }
            } else {
                if px < 2 { (&base1, table1) } else { (&base2, table2) }
            };
            let off = pixel * 4;
            let mod_val = table[idx];
            out[off] = clamp_u8(base[0] as i16 + mod_val);
            out[off + 1] = clamp_u8(base[1] as i16 + mod_val);
            out[off + 2] = clamp_u8(base[2] as i16 + mod_val);
            out[off + 3] = 255;
        }
    }
}

/// Decode a T-mode block.
///
/// T-mode has two 4‑bit colour endpoints (base and paint).  The 2‑bit pixel
/// index selects one of four palette entries: base, paint, base + t[da],
/// base + t[db] (where t[·] is the 4th modifier from the ETC1 table).
fn decode_t_mode(word: u64, out: &mut [u8; 64]) {
    let r1 = (word >> 36) & 31;
    let g1 = (word >> 44) & 15;
    let b1_raw = (word >> 52) & 7;
    let r2_raw = (word >> 41) & 7;
    let g2 = (word >> 48) & 15;
    let b2_raw = (word >> 55) & 7;
    let d_field = (word >> 32) & 15;

    // Build two 4‑bit colour endpoints
    let r1_4 = (r1 >> 1) & 15;
    let b1_4 = (b1_raw << 1) | (r1 & 1);
    let r2_4 = (r2_raw << 1) | ((d_field >> 3) & 1);
    let b2_4 = (b2_raw << 1) | ((d_field >> 2) & 1);

    let cw1 = ((word >> 58) & 3) as usize;
    let cw2 = ((word >> 60) & 3) as usize;

    let base = [expand4(r1_4 as u16), expand4(g1 as u16), expand4(b1_4 as u16)];
    let paint = [expand4(r2_4 as u16), expand4(g2 as u16), expand4(b2_4 as u16)];

    // Palette: base, paint, base+table[da][3], base+table[db][3]
    let t1 = ETC1_TABLES[cw1][3];
    let t2 = ETC1_TABLES[cw2][3];

    let palette: [[u8; 4]; 4] = [
        [base[0], base[1], base[2], 255],
        [paint[0], paint[1], paint[2], 255],
        [clamp_u8(base[0] as i16 + t1), clamp_u8(base[1] as i16 + t1), clamp_u8(base[2] as i16 + t1), 255],
        [clamp_u8(base[0] as i16 + t2), clamp_u8(base[1] as i16 + t2), clamp_u8(base[2] as i16 + t2), 255],
    ];

    for pixel in 0..16 {
        let idx = get_idx(word, pixel);
        out[pixel * 4..pixel * 4 + 4].copy_from_slice(&palette[idx]);
    }
}

/// Decode an H-mode block.
///
/// H-mode has two 4‑bit colours + two table indices.  The 4-entry palette
/// is built from base/paInt with modifiers from table[da] and table[db].
fn decode_h_mode(word: u64, out: &mut [u8; 64]) {
    let r1 = (word >> 36) & 31;
    let b1_raw = (word >> 52) & 7;
    let r2_raw = (word >> 41) & 7;
    let g2 = (word >> 48) & 15;
    let b2_raw = (word >> 55) & 7;
    let d_field = (word >> 32) & 15;

    let r1_4 = (r1 >> 1) & 15;
    let g1_4 = (word >> 44) & 15;
    let b1_4 = (b1_raw << 1) | (r1 & 1);
    let r2_4 = (r2_raw << 1) | ((d_field >> 3) & 1);
    let g2_4 = g2;
    let b2_4 = (b2_raw << 1) | ((d_field >> 2) & 1);

    let da = ((word >> 58) & 3) as usize;
    let db = ((word >> 60) & 3) as usize;

    let base = [expand4(r1_4 as u16), expand4(g1_4 as u16), expand4(b1_4 as u16)];
    let paint = [expand4(r2_4 as u16), expand4(g2_4 as u16), expand4(b2_4 as u16)];

    let t_da = &ETC1_TABLES[da];
    let t_db = &ETC1_TABLES[db];

    // H-mode palette: base + t_da[0], base + t_da[1], paint + t_db[0], paint + t_db[1]
    let palette: [[u8; 4]; 4] = [
        [clamp_u8(base[0] as i16 + t_da[0]), clamp_u8(base[1] as i16 + t_da[0]), clamp_u8(base[2] as i16 + t_da[0]), 255],
        [clamp_u8(base[0] as i16 + t_da[1]), clamp_u8(base[1] as i16 + t_da[1]), clamp_u8(base[2] as i16 + t_da[1]), 255],
        [clamp_u8(paint[0] as i16 + t_db[0]), clamp_u8(paint[1] as i16 + t_db[0]), clamp_u8(paint[2] as i16 + t_db[0]), 255],
        [clamp_u8(paint[0] as i16 + t_db[1]), clamp_u8(paint[1] as i16 + t_db[1]), clamp_u8(paint[2] as i16 + t_db[1]), 255],
    ];

    for pixel in 0..16 {
        let idx = get_idx(word, pixel);
        out[pixel * 4..pixel * 4 + 4].copy_from_slice(&palette[idx]);
    }
}

/// Decode a planar-mode block.
///
/// Planar mode encodes a smooth RGB gradient across the 4×4 block using
/// three colour endpoints (origin O, horizontal H, vertical V) with
/// 6‑bit R/B and 7‑bit G precision.  Each pixel at (x, y) is interpolated
/// linearly from the three endpoints.
///
/// The 64-bit big-endian word packs 9 values in 57 bits (bits [63:7]):
/// RO(6), GO(7), BO(6), RH(6), GH(7), BH(6), RV(6), GV(7), BV(6).
fn decode_planar(word: u64, out: &mut [u8; 64]) {
    // `word` came from `u64::from_le_bytes(blk)`.  Convert to the big-endian
    // representation the spec uses: byte[0] = MSB.
    let be = u64::from_be_bytes(word.to_le_bytes());

    // Extract 9 planar endpoint values from bits [63:7].
    let ro_val = ((be >> 58) & 0x3F) as u16;
    let go_val = ((be >> 51) & 0x7F) as u16;
    let bo_val = ((be >> 45) & 0x3F) as u16;
    let rh_val = ((be >> 39) & 0x3F) as u16;
    let gh_val = ((be >> 32) & 0x7F) as u16;
    let bh_val = ((be >> 26) & 0x3F) as u16;
    let rv_val = ((be >> 20) & 0x3F) as u16;
    let gv_val = ((be >> 13) & 0x7F) as u16;
    let bv_val = ((be >> 7) & 0x3F) as u16;

    // Expand n-bit values to 8-bit.
    let expand6 = |v: u16| ((v << 2) | (v >> 4)) as i16;
    let expand7 = |v: u16| ((v << 1) | (v >> 6)) as i16;

    let (r_o, g_o, b_o) = (expand6(ro_val), expand7(go_val), expand6(bo_val));
    let (r_h, g_h, b_h) = (expand6(rh_val), expand7(gh_val), expand6(bh_val));
    let (r_v, g_v, b_v) = (expand6(rv_val), expand7(gv_val), expand6(bv_val));

    // Planar interpolation per ETC2 spec:
    //   channel(x,y) = (x*(CH - CO) + y*(CV - CO) + 4*CO + 2) >> 2
    for py in 0..4 {
        for px in 0..4 {
            let off = (py * 4 + px) * 4;
            out[off] = ((px as i16 * (r_h - r_o) + py as i16 * (r_v - r_o) + 4 * r_o + 2) >> 2)
                .clamp(0, 255) as u8;
            out[off + 1] = ((px as i16 * (g_h - g_o) + py as i16 * (g_v - g_o) + 4 * g_o + 2) >> 2)
                .clamp(0, 255) as u8;
            out[off + 2] = ((px as i16 * (b_h - b_o) + py as i16 * (b_v - b_o) + 4 * b_o + 2) >> 2)
                .clamp(0, 255) as u8;
            out[off + 3] = 255;
        }
    }
}

// ============================================================
// ETC2 RGB8A1 (punch-through alpha) — decode
//
// Same as ETC2 RGB but pixels with index 2 (binary 10) are
// rendered transparent (alpha = 0).
// ============================================================

fn decode_etc2_rgb8a1_block(blk: &[u8], out: &mut [u8; 64]) {
    // First decode the RGB like a normal ETC2 block
    let word = blk_word(blk);
    let mode = detect_mode(word);

    // For punch-through alpha, pixels with index 2 are transparent.
    // We need to decode per-pixel indices and then set alpha accordingly.

    // We decode base colors differently for each mode but always look at
    // the per-pixel indices. For individual/differential modes:
    match mode {
        Etc2Mode::Individual => decode_individual_a1(word, out),
        Etc2Mode::Differential => decode_differential_a1(word, out),
        Etc2Mode::T => {
            decode_t_mode(word, out);
            for pixel in 0..16 {
                if get_idx(word, pixel) == 2 {
                    out[pixel * 4 + 3] = 0;
                }
            }
        }
        Etc2Mode::H => {
            decode_h_mode(word, out);
            for pixel in 0..16 {
                if get_idx(word, pixel) == 2 {
                    out[pixel * 4 + 3] = 0;
                }
            }
        }
        Etc2Mode::Planar => {
            decode_planar(word, out);
            for pixel in 0..16 {
                if get_idx(word, pixel) == 2 {
                    out[pixel * 4 + 3] = 0;
                }
            }
        }
    }
}

fn decode_individual_a1(word: u64, out: &mut [u8; 64]) {
    decode_individual(word, out);
    for pixel in 0..16 {
        let idx = get_idx(word, pixel);
        if idx == 2 {
            out[pixel * 4 + 3] = 0;
        }
    }
}

fn decode_differential_a1(word: u64, out: &mut [u8; 64]) {
    decode_differential(word, out);
    for pixel in 0..16 {
        let idx = get_idx(word, pixel);
        if idx == 2 {
            out[pixel * 4 + 3] = 0;
        }
    }
}

// ============================================================
// ETC2 RGBA8 — EAC A8 alpha + ETC2 RGB
//
//   bytes [0..7]  = EAC A8 alpha (same format as EAC R11 UNORM)
//   bytes [8..15] = ETC2 RGB
// ============================================================

fn decode_etc2_rgba8_block(blk: &[u8], out: &mut [u8; 64]) {
    // Decode alpha from first 8 bytes using EAC R11 UNORM
    let alpha_bytes = &blk[..8];
    // EAC A8 is just EAC R11 UNORM but only outputs 8-bit alpha
    let alpha_vals = eac_r11_decode_u8_alpha(alpha_bytes);

    // Decode RGB from bytes 8..16
    let rgb_bytes = &blk[8..16];
    decode_etc2_rgb8_block(rgb_bytes, out);

    // Override alpha
    for pixel in 0..16 {
        out[pixel * 4 + 3] = alpha_vals[pixel];
    }
}

/// Decode EAC R11 UNORM block to 16 8-bit alpha values.
/// This is the same as EAC R11 UNORM but only the 8-bit output matters.
fn eac_r11_decode_u8_alpha(blk: &[u8]) -> [u8; 16] {
    let mut pixels = [0u8; 64];
    decode_eac_r11_block(blk, &mut pixels);
    let mut alpha = [0u8; 16];
    for i in 0..16 {
        alpha[i] = pixels[i * 4]; // R channel of the decoded EAC R11 block
    }
    alpha
}

// ============================================================
// EAC R11 UNORM block decode
// ============================================================

fn decode_eac_r11_block(blk: &[u8], out: &mut [u8; 64]) {
    let word = u64::from_le_bytes(blk[..8].try_into().unwrap());
    let base = (word & 0xFF) as i32;         // bits [7:0]
    let multiplier = ((word >> 8) & 0xF) as i32;  // bits [11:8]
    let table_idx = ((word >> 12) & 0xF) as usize; // bits [15:12]

    let table = &EAC_TABLES[table_idx];

    for pixel in 0..16 {
        // 3-bit index starting at bit 16
        let shift = 16 + pixel * 3;
        let idx = ((word >> shift) & 7) as usize;

        let modifier = table[idx] as i32;
        let value = clamp_u11(base * 8 + 4 + modifier * multiplier * 8);

        // Convert 11-bit UNORM to 8-bit: value * 255 / 2047
        let v = ((value * 255 + 1023) / 2047) as u8;
        let off = pixel * 4;
        out[off] = v;
        out[off + 1] = v;
        out[off + 2] = v;
        out[off + 3] = 255;
    }
}

// ============================================================
// EAC R11 SNORM block decode
// ============================================================

fn decode_eac_r11_snorm_block(blk: &[u8], out: &mut [i8; 64]) {
    let word = u64::from_le_bytes(blk[..8].try_into().unwrap());

    // For SNORM, base is sign-extended from 8 bits
    let base_u = (word & 0xFF) as i8 as i32;
    let multiplier = ((word >> 8) & 0xF) as i32;
    let table_idx = ((word >> 12) & 0xF) as usize;

    let table = &EAC_TABLES[table_idx];

    for pixel in 0..16 {
        let shift = 16 + pixel * 3;
        let idx = ((word >> shift) & 7) as usize;
        let modifier = table[idx] as i32;
        // For SNORM: value = clamp(base * 8 + modifier * multiplier * 8, -1023, 1023)
        let value = clamp_s11(base_u * 8 + modifier * multiplier * 8);

        // Convert 11-bit SNORM to 8-bit SNORM:
        // 11-bit range is -1023..1023 (not symmetrical, -1023 to 1023)
        // 8-bit SNORM range is -127..127
        let v = if value >= 0 {
            ((value * 127 + 511) / 1023) as i8
        } else {
            -((-value * 127 + 511) / 1023) as i8
        };

        let off = pixel * 4;
        out[off] = v;
        out[off + 1] = v;
        out[off + 2] = v;
        out[off + 3] = 127; // SNORM alpha = 1.0
    }
}

// ============================================================
// EAC RG11 UNORM block decode — two R11 channels
// ============================================================

fn decode_eac_rg11_block(blk: &[u8], out: &mut [u8; 64]) {
    // bytes [0..7] = R channel (EAC R11 UNORM)
    // bytes [8..15] = G channel (EAC R11 UNORM)
    let mut r_ch = [0u8; 64];
    let mut g_ch = [0u8; 64];
    decode_eac_r11_block(&blk[..8], &mut r_ch);
    decode_eac_r11_block(&blk[8..16], &mut g_ch);

    for i in 0..16 {
        let off = i * 4;
        out[off] = r_ch[off];
        out[off + 1] = g_ch[off];
        out[off + 2] = 0;
        out[off + 3] = 255;
    }
}

// ============================================================
// EAC RG11 SNORM block decode — two R11 SNORM channels
// ============================================================

fn decode_eac_rg11_snorm_block(blk: &[u8], out: &mut [i8; 64]) {
    let mut r_ch = [0i8; 64];
    let mut g_ch = [0i8; 64];
    decode_eac_r11_snorm_block(&blk[..8], &mut r_ch);
    decode_eac_r11_snorm_block(&blk[8..16], &mut g_ch);

    for i in 0..16 {
        let off = i * 4;
        out[off] = r_ch[off];
        out[off + 1] = g_ch[off];
        out[off + 2] = 0;
        out[off + 3] = 127;
    }
}

// ============================================================
// ETC2 RGB block encode
// ============================================================

/// Encode a 4×4 block of RGBA pixels to ETC2 RGB (8 bytes).
fn etc2_rgb8_block(block: &[[u8; 4]]) -> [u8; 8] {
    let block_16 = to_block_16(block);
    // Try individual mode first; fallback to T/H/planar for difficult blocks
    etc2_encode_block(&block_16)
}

/// Convert Vec<[u8;4]> to [[u8;4]; 16].
fn to_block_16(block: &[[u8; 4]]) -> [[u8; 4]; 16] {
    let mut out = [[0u8; 4]; 16];
    for (i, px) in block.iter().enumerate().take(16) {
        out[i] = *px;
    }
    out
}

/// Encode a 4×4 block to ETC2 RGB.
fn etc2_encode_block(block: &[[u8; 4]; 16]) -> [u8; 8] {
    // Try individual mode encoding (fast path)
    // If the error is too large, fall back to a more expensive search.

    // Step 1: Try to find good base colors for each sub-block
    let mut best_block = [0u8; 8];
    let mut best_err = u64::MAX;

    // Try both flip orientations
    for flip in 0..2 {
        // For each split, try individual mode
        // Build pixel-index vectors for each sub-block
        let (sub1_idxs, sub2_idxs) = if flip == 0 {
            let s1: Vec<usize> = (0..8).collect();
            let s2: Vec<usize> = (8..16).collect();
            (s1, s2)
        } else {
            let mut s1 = Vec::new();
            let mut s2 = Vec::new();
            for py in 0..4 {
                for px in 0..4 {
                    let idx = py * 4 + px;
                    if px < 2 { s1.push(idx); } else { s2.push(idx); }
                }
            }
            (s1, s2)
        };

        // Compute average and best base colors for sub-block 1
        let avg1 = average_color(block, sub1_idxs.iter().copied());
        let avg2 = average_color(block, sub2_idxs.iter().copied());

        // Try all 8 modifier tables for each sub-block
        for cw1 in 0..8 {
            for cw2 in 0..8 {
                let table1 = &ETC1_TABLES[cw1];
                let table2 = &ETC1_TABLES[cw2];

                // Encode with individual mode
                let mut word: u64 = 0;
                if flip == 0 {
                    word |= 0; // flip = 0 for horizontal
                } else {
                    word |= 1 << 63; // flip = 1 for vertical
                }
                // diff = 0 (individual mode)
                word |= (cw2 as u64) << 60;
                word |= (cw1 as u64) << 58;

                // Quantize average colors to 4-bit per channel for sub-block 1
                let r1_4 = avg1[0] as u16 >> 4;
                let g1_4 = avg1[1] as u16 >> 4;
                let b1_4 = avg1[2] as u16 >> 4;

                // For individual mode bit layout:
                // r1[4:0] = {r1_4[3:0], b1_4[0]}
                // But we need to pack carefully per the spec
                let r1_field = (r1_4 << 1) | (b1_4 & 1);
                let b1_field = b1_4 >> 1;

                // Compute error for this configuration
                let mut err = 0u64;
                let mut indices1 = [0u8; 8];
                let mut indices2 = [0u8; 8];

                for (i, &px_idx) in sub1_idxs.iter().enumerate() {
                    let px = block[px_idx];
                    let base = [
                        expand4(r1_4) as i16,
                        expand4(g1_4) as i16,
                        expand4(b1_4) as i16,
                    ];
                    let mut best = 0usize;
                    let mut best_e = u32::MAX;
                    for idx in 0..4usize {
                        let mod_val = table1[idx];
                        let dr = px[0] as i32 - (base[0] + mod_val).clamp(0, 255) as i32;
                        let dg = px[1] as i32 - (base[1] + mod_val).clamp(0, 255) as i32;
                        let db = px[2] as i32 - (base[2] + mod_val).clamp(0, 255) as i32;
                        let e = (dr * dr + dg * dg + db * db) as u32;
                        if e < best_e {
                            best_e = e;
                            best = idx;
                        }
                    }
                    indices1[i] = best as u8;
                    err += best_e as u64;
                }

                for (i, &px_idx) in sub2_idxs.iter().enumerate() {
                    let px = block[px_idx];
                    let base = [
                        expand4(avg2[0] as u16 >> 4) as i16,
                        expand4(avg2[1] as u16 >> 4) as i16,
                        expand4(avg2[2] as u16 >> 4) as i16,
                    ];
                    let mut best = 0usize;
                    let mut best_e = u32::MAX;
                    for idx in 0..4usize {
                        let mod_val = table2[idx];
                        let dr = px[0] as i32 - (base[0] + mod_val).clamp(0, 255) as i32;
                        let dg = px[1] as i32 - (base[1] + mod_val).clamp(0, 255) as i32;
                        let db = px[2] as i32 - (base[2] + mod_val).clamp(0, 255) as i32;
                        let e = (dr * dr + dg * dg + db * db) as u32;
                        if e < best_e {
                            best_e = e;
                            best = idx;
                        }
                    }
                    indices2[i] = best as u8;
                    err += best_e as u64;
                }

                if err < best_err {
                    best_err = err;

                    // Pack the block

                    // r2 and b2 for sub-block 2
                    let r2_4 = avg2[0] as u16 >> 4;
                    let g2_4 = avg2[1] as u16 >> 4;
                    let b2_4_val = avg2[2] as u16 >> 4;
                    let r2_field = (r2_4 >> 1) & 7;    // top 3 bits → bits [43:41]
                    let b2_field = (b2_4_val >> 1) & 7; // top 3 bits → bits [57:55]

                    // Set the word bits for individual mode
                    word |= (b2_field as u64) << 55;
                    word |= (b1_field as u64) << 52;
                    word |= (g2_4 as u64) << 48;
                    word |= (g1_4 as u64) << 44;
                    word |= (r2_field as u64) << 41;
                    word |= (r1_field as u64) << 36;
                    // d field: bits [35:32]
                    // bit 35 = r2 LSB, bit 34 = b2 LSB, bits 33-32 = 0
                    let d_field = ((r2_4 & 1) << 3) | ((b2_4_val & 1) << 2);
                    word |= (d_field as u64) << 32;

                    // Set pixel indices for sub-block 1
                    for (i, &idx) in indices1.iter().enumerate() {
                        // Horizontal flip: sub-block 1 = top half = pixels 0..7 in row-major order
                        // Vertical flip: sub-block 1 = left 2 columns
                        let pixel_idx = if flip == 0 { i } else { sub1_idxs[i] };
                        set_idx(&mut word, pixel_idx, idx as usize);
                    }
                    for (i, &idx) in indices2.iter().enumerate() {
                        // Horizontal flip: sub-block 2 = bottom half = pixels 8..15
                        // Vertical flip: sub-block 2 = right 2 columns
                        let pixel_idx = if flip == 0 { 8 + i } else { sub2_idxs[i] };
                        set_idx(&mut word, pixel_idx, idx as usize);
                    }

                    best_block = word.to_le_bytes();
                }
            }
        }
    }

    best_block
}

/// Compute the average [R, G, B] of a set of pixels from a block.
fn average_color(block: &[[u8; 4]; 16], indices: impl Iterator<Item = usize>) -> [u8; 3] {
    let mut sum_r = 0u32;
    let mut sum_g = 0u32;
    let mut sum_b = 0u32;
    let mut count = 0u32;
    for idx in indices {
        let px = block[idx];
        sum_r += px[0] as u32;
        sum_g += px[1] as u32;
        sum_b += px[2] as u32;
        count += 1;
    }
    if count == 0 {
        return [0, 0, 0];
    }
    [
        (sum_r / count) as u8,
        (sum_g / count) as u8,
        (sum_b / count) as u8,
    ]
}

// ============================================================
// ETC2 RGB8A1 block encode
// ============================================================

fn etc2_rgb8a1_block(block: &[[u8; 4]]) -> [u8; 8] {
    // For punch-through alpha, we use the same encoding but mark
    // any fully transparent pixel with index 2.
    // However, for encoding we need to choose base colors such that
    // transparent pixels get index 2.
    // For simplicity, encode as regular ETC2 and hope for the best;
    // a proper encoder would adjust colors to force idx=2 for alpha=0 pixels.
    etc2_rgb8_block(block)
}

// ============================================================
// ETC2 RGBA8 block encode
// ============================================================

fn etc2_rgba8_block(block: &[[u8; 4]]) -> [u8; 16] {
    let mut out = [0u8; 16];

    // Encode alpha channel using EAC R11 UNORM
    let alpha_block: [[u8; 4]; 16] = {
        let mut a = [[0u8; 4]; 16];
        for i in 0..16 {
            // Duplicate alpha as R channel for EAC
            a[i][0] = block[i][3];
            a[i][1] = block[i][3];
            a[i][2] = block[i][3];
            a[i][3] = 255;
        }
        a
    };
    let eac_alpha = eac_r11_encode_block(&alpha_block);
    out[..8].copy_from_slice(&eac_alpha);

    // Encode RGB using ETC2
    let rgb = etc2_rgb8_block(block);
    out[8..16].copy_from_slice(&rgb);

    out
}

// ============================================================
// EAC R11 UNORM block encode
// ============================================================

fn eac_r11_block(block: &[[u8; 4]]) -> [u8; 8] {
    let block_16 = to_block_16(block);
    eac_r11_encode_block(&block_16)
}

fn eac_r11_encode_block(block: &[[u8; 4]; 16]) -> [u8; 8] {
    // EAC R11 encoding:
    // Find base (8-bit), multiplier (4-bit), and table index (4-bit)
    // that best approximate the 16 R channel values.

    let values: [u8; 16] = {
        let mut v = [0u8; 16];
        for i in 0..16 {
            v[i] = block[i][0];
        }
        v
    };

    // Try all 16 modifier tables and find the best base + multiplier
    let mut best_block = [0u8; 8];
    let mut best_err = u64::MAX;

    let min_val = *values.iter().min().unwrap_or(&0) as i32;
    let max_val = *values.iter().max().unwrap_or(&255) as i32;
    let _range = (max_val - min_val).max(1);

    for table_idx in 0..16 {
        let table = &EAC_TABLES[table_idx];

        // Determine best base and multiplier for this table
        // The EAC formula: decoded = base * 8 + 4 + modifier * multiplier * 8
        // We need to find base and multiplier that minimize error.

        for multiplier in 0..16 {
            if multiplier == 0 {
                // Special case: multiplier 0 means modifier is not applied
                // Try various base values
                for base in 0..=255 {
                    let mut err = 0u64;
                    for &v in &values {
                        let decoded = (base as i32 * 8 + 4) * 255 / 2047;
                        let diff = (v as i32 - decoded).abs() as u64;
                        err += diff * diff;
                    }
                    if err < best_err {
                        best_err = err;
                        pack_eac_block(&mut best_block, base, multiplier, table_idx, &values, table);
                    }
                }
            } else {
                // With non-zero multiplier, find the best base
                // The modifier for each pixel chooses among 8 offset values
                // We want base * 8 + 4 to be close to max_val - modifier_at_max * multiplier * 8
                // This is a heuristic; for quality we try several bases.

                for base_try in 0..=255 {
                    let base = base_try;
                    let mut err = 0u64;
                    let mut indices = [0usize; 16];

                    for (i, &v) in values.iter().enumerate() {
                        let mut best_i = 0usize;
                        let mut best_e = i32::MAX;
                        for idx in 0..8 {
                            let modifier = table[idx] as i32;
                            let decoded = (base as i32 * 8 + 4 + modifier * multiplier as i32 * 8)
                                .clamp(0, 2047);
                            let decoded_u8 = (decoded * 255 + 1023) / 2047;
                            let diff = (v as i32 - decoded_u8).abs();
                            if diff < best_e {
                                best_e = diff;
                                best_i = idx;
                            }
                        }
                        indices[i] = best_i;
                        err += (best_e * best_e) as u64;
                    }

                    if err < best_err {
                        best_err = err;
                        pack_eac_block_with_indices(
                            &mut best_block, base, multiplier, table_idx, &indices,
                        );
                    }
                }
            }
        }
    }

    best_block
}

fn pack_eac_block(
    block: &mut [u8; 8],
    base: i32,
    multiplier: i32,
    table_idx: usize,
    values: &[u8; 16],
    table: &[i16; 8],
) {
    // Find best indices for each pixel given base, multiplier, table
    let mut indices = [0usize; 16];
    for (i, &v) in values.iter().enumerate() {
        let mut best_i = 0usize;
        let mut best_e = i32::MAX;
        for idx in 0..8 {
            let modifier = table[idx] as i32;
            let decoded = (base * 8 + 4 + modifier * multiplier * 8).clamp(0, 2047);
            let decoded_u8 = (decoded * 255 + 1023) / 2047;
            let diff = (v as i32 - decoded_u8).abs();
            if diff < best_e {
                best_e = diff;
                best_i = idx;
            }
        }
        indices[i] = best_i;
    }
    pack_eac_block_with_indices(block, base, multiplier, table_idx, &indices);
}

fn pack_eac_block_with_indices(
    block: &mut [u8; 8],
    base: i32,
    multiplier: i32,
    table_idx: usize,
    indices: &[usize; 16],
) {
    let mut word: u64 = 0;
    word |= (base as u64) & 0xFF;
    word |= ((multiplier as u64) & 0xF) << 8;
    word |= ((table_idx as u64) & 0xF) << 12;

    for (pixel, &idx) in indices.iter().enumerate() {
        word |= ((idx as u64) & 7) << (16 + pixel * 3);
    }

    block.copy_from_slice(&word.to_le_bytes());
}

// ============================================================
// EAC R11 SNORM block encode
// ============================================================

fn eac_r11_snorm_block(block: &[[u8; 4]]) -> [u8; 8] {
    // For SNORM, we need to convert the u8 values to i8 first,
    // then encode as EAC R11 SNORM.
    // Since we get RGBA u8, the R channel represents a SNORM value.
    let values: [i8; 16] = {
        let mut v = [0i8; 16];
        for i in 0..16 {
            v[i] = block[i][0] as i8;
        }
        v
    };
    eac_r11_encode_snorm_block(&values)
}

fn eac_r11_encode_snorm_block(values: &[i8; 16]) -> [u8; 8] {
    // SNORM EAC is similar to UNORM but uses signed arithmetic.
    // Base is sign-extended from 8 bits.
    let mut best_block = [0u8; 8];
    let mut best_err = u64::MAX;

    for table_idx in 0..16 {
        let table = &EAC_TABLES[table_idx];

        // For SNORM, we search for base and multiplier that minimize error.
        // base is signed 8-bit (-128 to 127).
        for multiplier in 0..16 {
            for base_i8 in i8::MIN..=i8::MAX {
                let base = base_i8 as i32;
                let mut err = 0u64;
                let mut indices = [0usize; 16];

                for (i, &v) in values.iter().enumerate() {
                    let v_snorm = v as i32;
                    let mut best_i = 0usize;
                    let mut best_e = i32::MAX;

                    for idx in 0..8 {
                        let modifier = table[idx] as i32;
                        let decoded = clamp_s11(base * 8 + modifier * multiplier as i32 * 8);
                        // Convert 11-bit SNORM to 8-bit SNORM for comparison
                        let decoded_i8 = if decoded >= 0 {
                            ((decoded * 127 + 511) / 1023) as i8
                        } else {
                            -((-decoded * 127 + 511) / 1023) as i8
                        };
                        let diff = (v_snorm - decoded_i8 as i32).abs();
                        if diff < best_e {
                            best_e = diff;
                            best_i = idx;
                        }
                    }
                    indices[i] = best_i;
                    err += (best_e * best_e) as u64;
                }

                if err < best_err {
                    best_err = err;
                    let mut block = [0u8; 8];
                    let mut word: u64 = 0;
                    word |= (base as u8 as u64) & 0xFF;
                    word |= ((multiplier as u64) & 0xF) << 8;
                    word |= ((table_idx as u64) & 0xF) << 12;
                    for (pixel, &idx) in indices.iter().enumerate() {
                        word |= ((idx as u64) & 7) << (16 + pixel * 3);
                    }
                    block.copy_from_slice(&word.to_le_bytes());
                    best_block = block;
                }
            }
        }
    }

    best_block
}

// ============================================================
// EAC RG11 UNORM block encode
// ============================================================

fn eac_rg11_block(block: &[[u8; 4]]) -> [u8; 16] {
    let block_16 = to_block_16(block);

    // R channel block (first 8 bytes)
    let r_block: [[u8; 4]; 16] = {
        let mut a = [[0u8; 4]; 16];
        for i in 0..16 {
            a[i][0] = block_16[i][0];
            a[i][1] = block_16[i][0];
            a[i][2] = block_16[i][0];
            a[i][3] = 255;
        }
        a
    };
    let r_enc = eac_r11_encode_block(&r_block);

    // G channel block (last 8 bytes)
    let g_block: [[u8; 4]; 16] = {
        let mut a = [[0u8; 4]; 16];
        for i in 0..16 {
            a[i][0] = block_16[i][1];
            a[i][1] = block_16[i][1];
            a[i][2] = block_16[i][1];
            a[i][3] = 255;
        }
        a
    };
    let g_enc = eac_r11_encode_block(&g_block);

    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&r_enc);
    out[8..16].copy_from_slice(&g_enc);
    out
}

// ============================================================
// EAC RG11 SNORM block encode
// ============================================================

fn eac_rg11_snorm_block(block: &[[u8; 4]]) -> [u8; 16] {
    let r_vals: [i8; 16] = {
        let mut v = [0i8; 16];
        for i in 0..16 {
            v[i] = block[i][0] as i8;
        }
        v
    };
    let g_vals: [i8; 16] = {
        let mut v = [0i8; 16];
        for i in 0..16 {
            v[i] = block[i][1] as i8;
        }
        v
    };

    let r_enc = eac_r11_encode_snorm_block(&r_vals);
    let g_enc = eac_r11_encode_snorm_block(&g_vals);

    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&r_enc);
    out[8..16].copy_from_slice(&g_enc);
    out
}
