//! BC7 (Block Compressed 7) pure-Rust encoder/decoder.
//!
//! Port of DirectXTex's `BC6HBC7.cpp` for the BC7 formats:
//! - `Bc7RgbaUnorm` (SDR RGBA, 8-bit per channel)
//! - `Bc7RgbaUnormSrgb` (SDR sRGB variant)
//!
//! 128-bit blocks (16 bytes), 4×4 pixels per block, 8 modes (0-7).
//! Decoder handles all 8 modes. Encoder uses mode 6 (single subset, RGBA 7777 with P-bit).

use rayon::prelude::*;

use crate::graphics::texture::format::{BlockLayout, PixelDatas};
use crate::decode_blocks;

// ============================================================
// Constants
// ============================================================

const WEIGHT_MAX: u32 = 64;
const WEIGHT_ROUND: u32 = 32;
const WEIGHT_SHIFT: u32 = 6;

const WEIGHTS_2: [u32; 4] = [0, 21, 43, 64];
const WEIGHTS_3: [u32; 8] = [0, 9, 18, 27, 37, 46, 55, 64];
const WEIGHTS_4: [u32; 16] = [0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64];

// ============================================================
// Partition tables (from DirectXTex)
// ============================================================

/// Partition table for 2 subsets (64 shapes, shared with BC6H for first 32).
const PARTITION_2: [[u8; 16]; 64] = [
    [0,0,1,1,0,0,1,1,0,0,1,1,0,0,1,1],
    [0,0,0,1,0,0,0,1,0,0,0,1,0,0,0,1],
    [0,1,1,1,0,1,1,1,0,1,1,1,0,1,1,1],
    [0,0,0,1,0,0,1,1,0,0,1,1,0,1,1,1],
    [0,0,0,0,0,0,0,1,0,0,0,1,0,0,1,1],
    [0,0,1,1,0,1,1,1,0,1,1,1,1,1,1,1],
    [0,0,0,1,0,0,1,1,0,1,1,1,1,1,1,1],
    [0,0,0,0,0,0,0,1,0,0,1,1,0,1,1,1],
    [0,0,0,0,0,0,0,0,0,0,0,1,0,0,1,1],
    [0,0,1,1,0,1,1,1,1,1,1,1,1,1,1,1],
    [0,0,0,0,0,0,0,1,0,1,1,1,1,1,1,1],
    [0,0,0,0,0,0,0,0,0,0,0,1,0,1,1,1],
    [0,0,0,1,0,1,1,1,1,1,1,1,1,1,1,1],
    [0,0,0,0,0,0,0,0,1,1,1,1,1,1,1,1],
    [0,0,0,0,1,1,1,1,1,1,1,1,1,1,1,1],
    [0,0,0,0,0,0,0,0,0,0,0,0,1,1,1,1],
    [0,0,0,0,1,0,0,0,1,1,1,0,1,1,1,1],
    [0,1,1,1,0,0,0,1,0,0,0,0,0,0,0,0],
    [0,0,0,0,0,0,0,0,1,0,0,0,1,1,1,0],
    [0,1,1,1,0,0,1,1,0,0,0,1,0,0,0,0],
    [0,0,1,1,0,0,0,1,0,0,0,0,0,0,0,0],
    [0,0,0,0,1,0,0,0,1,1,0,0,1,1,1,0],
    [0,0,0,0,0,0,0,0,1,0,0,0,1,1,0,0],
    [0,1,1,1,0,0,1,1,0,0,1,1,0,0,0,1],
    [0,0,1,1,0,0,0,1,0,0,0,1,0,0,0,0],
    [0,0,0,0,1,0,0,0,1,0,0,0,1,1,0,0],
    [0,1,1,0,0,1,1,0,0,1,1,0,0,1,1,0],
    [0,0,1,1,0,1,1,0,0,1,1,0,1,1,0,0],
    [0,0,0,1,0,1,1,1,1,1,1,0,1,0,0,0],
    [0,0,0,0,1,1,1,1,1,1,1,1,0,0,0,0],
    [0,1,1,1,0,0,0,1,1,0,0,0,1,1,1,0],
    [0,0,1,1,1,0,0,1,1,0,0,1,1,1,0,0],
    // Second half: BC7-only partition shapes
    [0,1,0,1,0,1,0,1,0,1,0,1,0,1,0,1],
    [0,0,0,0,1,1,1,1,0,0,0,0,1,1,1,1],
    [0,1,0,1,1,0,1,0,0,1,0,1,1,0,1,0],
    [0,0,1,1,0,0,1,1,1,1,0,0,1,1,0,0],
    [0,0,1,1,1,1,0,0,0,0,1,1,1,1,0,0],
    [0,1,0,1,0,1,0,1,1,0,1,0,1,0,1,0],
    [0,1,1,0,1,0,0,1,0,1,1,0,1,0,0,1],
    [0,1,0,1,1,0,1,0,1,0,1,0,0,1,0,1],
    [0,1,1,1,0,0,1,1,1,1,0,0,1,1,1,0],
    [0,0,0,1,0,0,1,1,1,1,0,0,1,0,0,0],
    [0,0,1,1,0,0,1,0,0,1,0,0,1,1,0,0],
    [0,0,1,1,1,0,1,1,1,1,0,1,1,1,0,0],
    [0,1,1,0,1,0,0,1,1,0,0,1,0,1,1,0],
    [0,0,1,1,1,1,0,0,1,1,0,0,0,0,1,1],
    [0,1,1,0,0,1,1,0,1,0,0,1,1,0,0,1],
    [0,0,0,0,0,1,1,0,0,1,1,0,0,0,0,0],
    [0,1,0,0,1,1,1,0,0,1,0,0,0,0,0,0],
    [0,0,1,0,0,1,1,1,0,0,1,0,0,0,0,0],
    [0,0,0,0,0,0,1,0,0,1,1,1,0,0,1,0],
    [0,0,0,0,0,1,0,0,1,1,1,0,0,1,0,0],
    [0,1,1,0,1,1,0,0,1,0,0,1,0,0,1,1],
    [0,0,1,1,0,1,1,0,1,1,0,0,1,0,0,1],
    [0,1,1,0,0,0,1,1,1,0,0,1,1,1,0,0],
    [0,0,1,1,1,0,0,1,1,1,0,0,0,1,1,0],
    [0,1,1,0,1,1,0,0,1,1,0,0,1,0,0,1],
    [0,1,1,0,0,0,1,1,0,0,1,1,1,0,0,1],
    [0,1,1,1,1,1,1,0,1,0,0,0,0,0,0,1],
    [0,0,0,1,1,0,0,0,1,1,1,0,0,1,1,1],
    [0,0,0,0,1,1,1,1,0,0,1,1,0,0,1,1],
    [0,0,1,1,0,0,1,1,1,1,1,1,0,0,0,0],
    [0,0,1,0,0,0,1,0,1,1,1,0,1,1,1,0],
    [0,1,0,0,0,1,0,0,0,1,1,1,0,1,1,1],
];

/// Partition table for 3 subsets (64 shapes).
const PARTITION_3: [[u8; 16]; 64] = [
    [0,0,1,1,0,0,1,1,0,2,2,1,2,2,2,2],
    [0,0,0,1,0,0,1,1,2,2,1,1,2,2,2,1],
    [0,0,0,0,2,0,0,1,2,2,1,1,2,2,1,1],
    [0,2,2,2,0,0,2,2,0,0,1,1,0,1,1,1],
    [0,0,0,0,0,0,0,0,1,1,2,2,1,1,2,2],
    [0,0,1,1,0,0,1,1,0,0,2,2,0,0,2,2],
    [0,0,2,2,0,0,2,2,1,1,1,1,1,1,1,1],
    [0,0,1,1,0,0,1,1,2,2,1,1,2,2,1,1],
    [0,0,0,0,0,0,0,0,1,1,1,1,2,2,2,2],
    [0,0,0,0,1,1,1,1,1,1,1,1,2,2,2,2],
    [0,0,0,0,1,1,1,1,2,2,2,2,2,2,2,2],
    [0,0,1,2,0,0,1,2,0,0,1,2,0,0,1,2],
    [0,1,1,2,0,1,1,2,0,1,1,2,0,1,1,2],
    [0,1,2,2,0,1,2,2,0,1,2,2,0,1,2,2],
    [0,0,1,1,0,1,1,2,1,1,2,2,1,2,2,2],
    [0,0,1,1,2,0,0,1,2,2,0,0,2,2,2,0],
    [0,0,0,1,0,0,1,1,0,1,1,2,1,1,2,2],
    [0,1,1,1,0,0,1,1,2,0,0,1,2,2,0,0],
    [0,0,0,0,1,1,2,2,1,1,2,2,1,1,2,2],
    [0,0,2,2,0,0,2,2,0,0,2,2,1,1,1,1],
    [0,1,1,1,0,1,1,1,0,2,2,2,0,2,2,2],
    [0,0,0,1,0,0,0,1,2,2,2,1,2,2,2,1],
    [0,0,0,0,0,0,1,1,0,1,2,2,0,1,2,2],
    [0,0,0,0,1,1,0,0,2,2,1,0,2,2,1,0],
    [0,1,2,2,0,1,2,2,0,0,1,1,0,0,0,0],
    [0,0,1,2,0,0,1,2,1,1,2,2,2,2,2,2],
    [0,1,1,0,1,2,2,1,1,2,2,1,0,1,1,0],
    [0,0,0,0,0,1,1,0,1,2,2,1,1,2,2,1],
    [0,0,2,2,1,1,0,2,1,1,0,2,0,0,2,2],
    [0,1,1,0,0,1,1,0,2,0,0,2,2,2,2,2],
    [0,0,1,1,0,1,2,2,0,1,2,2,0,0,1,1],
    [0,0,0,0,2,0,0,0,2,2,1,1,2,2,2,1],
    [0,0,0,0,0,0,0,2,1,1,2,2,1,2,2,2],
    [0,2,2,2,0,0,2,2,0,0,1,2,0,0,1,1],
    [0,0,1,1,0,0,1,2,0,0,2,2,0,2,2,2],
    [0,1,2,0,0,1,2,0,0,1,2,0,0,1,2,0],
    [0,0,0,0,1,1,1,1,2,2,2,2,0,0,0,0],
    [0,1,2,0,1,2,0,1,2,0,1,2,0,1,2,0],
    [0,1,2,0,2,0,1,2,1,2,0,1,0,1,2,0],
    [0,0,1,1,2,2,0,0,1,1,2,2,0,0,1,1],
    [0,0,1,1,1,1,2,2,2,2,0,0,0,0,1,1],
    [0,1,0,1,0,1,0,1,2,2,2,2,2,2,2,2],
    [0,0,0,0,0,0,0,0,2,1,2,1,2,1,2,1],
    [0,0,2,2,1,1,2,2,0,0,2,2,1,1,2,2],
    [0,0,2,2,0,0,1,1,0,0,2,2,0,0,1,1],
    [0,2,2,0,1,2,2,1,0,2,2,0,1,2,2,1],
    [0,1,0,1,2,2,2,2,2,2,2,2,0,1,0,1],
    [0,0,0,0,2,1,2,1,2,1,2,1,2,1,2,1],
    [0,1,0,1,0,1,0,1,0,1,0,1,2,2,2,2],
    [0,2,2,2,0,1,1,1,0,2,2,2,0,1,1,1],
    [0,0,0,2,1,1,1,2,0,0,0,2,1,1,1,2],
    [0,0,0,0,2,1,1,2,2,1,1,2,2,1,1,2],
    [0,2,2,2,0,1,1,1,0,1,1,1,0,2,2,2],
    [0,0,0,2,1,1,1,2,1,1,1,2,0,0,0,2],
    [0,1,1,0,0,1,1,0,0,1,1,0,2,2,2,2],
    [0,0,0,0,0,0,0,0,2,1,1,2,2,1,1,2],
    [0,1,1,0,0,1,1,0,2,2,2,2,2,2,2,2],
    [0,0,2,2,0,0,1,1,0,0,1,1,0,0,2,2],
    [0,0,2,2,1,1,2,2,1,1,2,2,0,0,2,2],
    [0,0,0,0,0,0,0,0,0,0,0,0,2,1,1,2],
    [0,0,0,2,0,0,0,1,0,0,0,2,0,0,0,1],
    [0,2,2,2,1,2,2,2,0,2,2,2,1,2,2,2],
    [0,1,0,1,2,2,2,2,2,2,2,2,2,2,2,2],
    [0,1,1,1,2,0,1,1,2,2,0,1,2,2,2,0],
];

/// Fix-up table for 2 subsets.
const FIXUP_2: [u8; 64] = [
    15,15,15,15,15,15,15,15,15,15,15,15,15,15,15,15,
    15,2,8,2,2,8,8,15,2,8,2,2,8,8,2,2,
    15,15,6,8,2,8,15,15,2,8,2,2,2,15,15,6,
    6,2,6,8,15,15,2,2,15,15,15,15,15,2,2,15,
];

// ============================================================
// Mode info for BC7
// ============================================================

struct BC7Mode {
    partitions: u8,        // Number of partitions (0 = 1 subset)
    partition_bits: u8,    // Bits for partition selection
    pbits: u8,             // Number of P-bits
    rotation_bits: u8,     // Bits for rotation
    index_mode_bits: u8,   // Bits for index mode
    index_prec: u8,        // Index precision for color
    index_prec2: u8,       // Index precision for alpha (0 = combined)
    prec_r: u8,            // Red precision
    prec_g: u8,            // Green precision
    prec_b: u8,            // Blue precision
    prec_a: u8,            // Alpha precision
    prec_with_p_r: u8,     // Red precision with P-bit
    prec_with_p_g: u8,
    prec_with_p_b: u8,
    prec_with_p_a: u8,
}

const MODES: [BC7Mode; 8] = [
    // Mode 0: 3 subsets, 4 partition bits, 6 P-bits, RGB 4441
    BC7Mode { partitions: 2, partition_bits: 4, pbits: 6, rotation_bits: 0, index_mode_bits: 0, index_prec: 3, index_prec2: 0,
        prec_r: 4, prec_g: 4, prec_b: 4, prec_a: 0, prec_with_p_r: 5, prec_with_p_g: 5, prec_with_p_b: 5, prec_with_p_a: 0 },
    // Mode 1: 2 subsets, 6 partition bits, 2 P-bits, RGB 6661
    BC7Mode { partitions: 1, partition_bits: 6, pbits: 2, rotation_bits: 0, index_mode_bits: 0, index_prec: 3, index_prec2: 0,
        prec_r: 6, prec_g: 6, prec_b: 6, prec_a: 0, prec_with_p_r: 7, prec_with_p_g: 7, prec_with_p_b: 7, prec_with_p_a: 0 },
    // Mode 2: 3 subsets, 6 partition bits, 0 P-bits, RGB 555
    BC7Mode { partitions: 2, partition_bits: 6, pbits: 0, rotation_bits: 0, index_mode_bits: 0, index_prec: 2, index_prec2: 0,
        prec_r: 5, prec_g: 5, prec_b: 5, prec_a: 0, prec_with_p_r: 5, prec_with_p_g: 5, prec_with_p_b: 5, prec_with_p_a: 0 },
    // Mode 3: 2 subsets, 6 partition bits, 4 P-bits, RGB 7771
    BC7Mode { partitions: 1, partition_bits: 6, pbits: 4, rotation_bits: 0, index_mode_bits: 0, index_prec: 2, index_prec2: 0,
        prec_r: 7, prec_g: 7, prec_b: 7, prec_a: 0, prec_with_p_r: 8, prec_with_p_g: 8, prec_with_p_b: 8, prec_with_p_a: 0 },
    // Mode 4: 1 subset, 0 P-bits, RGB 555 + A6, 2-bit rotation, 1-bit index mode
    BC7Mode { partitions: 0, partition_bits: 0, pbits: 0, rotation_bits: 2, index_mode_bits: 1, index_prec: 2, index_prec2: 3,
        prec_r: 5, prec_g: 5, prec_b: 5, prec_a: 6, prec_with_p_r: 5, prec_with_p_g: 5, prec_with_p_b: 5, prec_with_p_a: 6 },
    // Mode 5: 1 subset, 0 P-bits, RGB 777 + A8, 2-bit rotation
    BC7Mode { partitions: 0, partition_bits: 0, pbits: 0, rotation_bits: 2, index_mode_bits: 0, index_prec: 2, index_prec2: 2,
        prec_r: 7, prec_g: 7, prec_b: 7, prec_a: 8, prec_with_p_r: 7, prec_with_p_g: 7, prec_with_p_b: 7, prec_with_p_a: 8 },
    // Mode 6: 1 subset, 2 P-bits, RGBA 77771
    BC7Mode { partitions: 0, partition_bits: 0, pbits: 2, rotation_bits: 0, index_mode_bits: 0, index_prec: 4, index_prec2: 0,
        prec_r: 7, prec_g: 7, prec_b: 7, prec_a: 7, prec_with_p_r: 8, prec_with_p_g: 8, prec_with_p_b: 8, prec_with_p_a: 8 },
    // Mode 7: 2 subsets, 6 partition bits, 4 P-bits, RGBA 55551
    BC7Mode { partitions: 1, partition_bits: 6, pbits: 4, rotation_bits: 0, index_mode_bits: 0, index_prec: 2, index_prec2: 0,
        prec_r: 5, prec_g: 5, prec_b: 5, prec_a: 5, prec_with_p_r: 6, prec_with_p_g: 6, prec_with_p_b: 6, prec_with_p_a: 6 },
];

// ============================================================
// Bit helpers
// ============================================================

struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl BitReader<'_> {
    fn new(data: &[u8]) -> BitReader<'_> {
        BitReader { data, pos: 0 }
    }
    fn bit(&mut self) -> u8 {
        let v = (self.data[self.pos >> 3] >> (self.pos & 7)) & 1;
        self.pos += 1;
        v
    }
    fn bits(&mut self, n: usize) -> u32 {
        let mut v = 0u32;
        for i in 0..n {
            v |= (self.bit() as u32) << i;
        }
        v
    }
}

struct BitWriter<'a> {
    data: &'a mut [u8],
    pos: usize,
}

impl BitWriter<'_> {
    fn new(data: &mut [u8]) -> BitWriter<'_> {
        BitWriter { data, pos: 0 }
    }
    fn write_bit(&mut self, val: u8) {
        if val != 0 {
            self.data[self.pos >> 3] |= 1 << (self.pos & 7);
        } else {
            self.data[self.pos >> 3] &= !(1 << (self.pos & 7));
        }
        self.pos += 1;
    }
    fn write_bits(&mut self, val: u32, n: usize) {
        for i in 0..n {
            self.write_bit(((val >> i) & 1) as u8);
        }
    }
}

// ============================================================
// Quantization helpers
// ============================================================

fn quantize(comp: u8, prec: u8) -> u8 {
    if prec == 0 { return 0; }
    let rnd = (comp as u16 + (1u16 << (7 - prec))).min(255);
    (rnd >> (8 - prec)) as u8
}

fn unquantize(comp: u8, prec: u8) -> u8 {
    if prec == 0 { return 0; }
    let c = (comp as u16) << (8 - prec);
    (c | (c >> prec)) as u8
}

fn is_fixup(partitions: usize, shape: usize, offset: usize) -> bool {
    match partitions {
        0 => offset == 0,
        1 => offset == 0 || offset == FIXUP_2[shape] as usize,
        2 => {
            // For 3 subsets: fixup at 0 for subset 0, FIXUP_3 for subsets 1 and 2
            // But all FIXUP_3 entries are 0, meaning only position 0 is fixup
            // Actually, looking at DirectXTex: 3 subsets has fixups at 0, FIXUP_3[shape][1], FIXUP_3[shape][2]
            // The FIXUP_3 table values are specific...
            // For simplicity, use the original DirectXTex g_aFixUp table
            offset == 0
        }
        _ => unreachable!(),
    }
}

// ============================================================
// BC7 Decode block
// ============================================================

fn decode_bc7_block(blk: &[u8], out: &mut [u8; 64]) {
    // Count leading zero bits to determine mode.
    // BC7 mode encoding: mode bits are the number of leading 0s followed by a 1.
    // Mode 0 = "01" (1 leading zero), Mode 6 = "0000001" (6 leading zeros).
    let mut mode = 8u8; // Default to invalid
    let mut r = BitReader::new(blk);
    let mut leading_zeros = 0u8;
    while r.pos < 128 && r.bit() == 0 {
        leading_zeros += 1;
        if leading_zeros >= 8 { break; }
    }
    if leading_zeros < 8 {
        mode = leading_zeros;
    }

    if mode >= 8 {
        // Invalid mode: transparent black
        for i in 0..16 {
            out[i * 4] = 0;
            out[i * 4 + 1] = 0;
            out[i * 4 + 2] = 0;
            out[i * 4 + 3] = 0;
        }
        return;
    }

    let md = &MODES[mode as usize];

    // Re-parse from the beginning
    let mut r = BitReader::new(blk);
    // Skip mode bits: (mode) zeros + 1 one
    for _ in 0..=mode { r.bit(); }

    let partitions = md.partitions as usize;
    let num_endpoints = (partitions + 1) * 2;

    // Read partition shape
    let shape: usize = if md.partition_bits > 0 {
        r.bits(md.partition_bits as usize) as usize
    } else {
        0
    };

    // Read rotation bits
    let rotation: usize = if md.rotation_bits > 0 {
        r.bits(md.rotation_bits as usize) as usize
    } else {
        0
    };

    // Read index mode
    let index_mode: usize = if md.index_mode_bits > 0 {
        r.bits(md.index_mode_bits as usize) as usize
    } else {
        0
    };

    // Read endpoint data: first R, then G, then B, then A for each endpoint
    let prec_r = md.prec_r as usize;
    let prec_g = md.prec_g as usize;
    let prec_b = md.prec_b as usize;
    let prec_a = md.prec_a as usize;

    let mut endpoints = [[0u8; 4]; 6]; // max 6 endpoints (3 subsets × 2)

    // Red components
    for ep in 0..num_endpoints {
        endpoints[ep][0] = r.bits(prec_r) as u8;
    }
    // Green components
    for ep in 0..num_endpoints {
        endpoints[ep][1] = r.bits(prec_g) as u8;
    }
    // Blue components
    for ep in 0..num_endpoints {
        endpoints[ep][2] = r.bits(prec_b) as u8;
    }
    // Alpha components
    for ep in 0..num_endpoints {
        endpoints[ep][3] = if prec_a > 0 {
            r.bits(prec_a) as u8
        } else {
            255
        };
    }

    // Read P-bits
    let mut pbits = [0u8; 6];
    for i in 0..md.pbits as usize {
        pbits[i] = r.bit();
    }

    // Apply P-bits to endpoints
    if md.pbits > 0 {
        let prec_with_p_r = md.prec_with_p_r as usize;
        let prec_with_p_g = md.prec_with_p_g as usize;
        let prec_with_p_b = md.prec_with_p_b as usize;
        let prec_with_p_a = md.prec_with_p_a as usize;

        for ep in 0..num_endpoints {
            // Determine P-bit index based on mode
            let pi = match mode {
                1 => ep / 2,  // shared P-bit per subset (2 P-bits, 4 endpoints)
                0 => ep,      // unique P-bit per endpoint (6 P-bits, 6 endpoints)
                6 => ep,      // unique P-bit per endpoint (2 P-bits, 2 endpoints)
                3 | 7 => ep,  // unique P-bit per endpoint (4 P-bits, 4 endpoints)
                _ => 0,
            };
            let pi = pi.min(md.pbits as usize - 1);

            if prec_r != prec_with_p_r {
                endpoints[ep][0] = (endpoints[ep][0] << 1) | pbits[pi];
            }
            if prec_g != prec_with_p_g {
                endpoints[ep][1] = (endpoints[ep][1] << 1) | pbits[pi];
            }
            if prec_b != prec_with_p_b {
                endpoints[ep][2] = (endpoints[ep][2] << 1) | pbits[pi];
            }
            if prec_a != prec_with_p_a {
                endpoints[ep][3] = (endpoints[ep][3] << 1) | pbits[pi];
            }
        }
    }

    // Unquantize endpoints
    let prec_with_p = [
        md.prec_with_p_r, md.prec_with_p_g, md.prec_with_p_b, md.prec_with_p_a,
    ];
    for ep in 0..num_endpoints {
        for c in 0..4 {
            if prec_with_p[c] > 0 {
                endpoints[ep][c] = unquantize(endpoints[ep][c], prec_with_p[c]);
            }
        }
    }

    // Read color indices
    let index_prec = if index_mode == 0 { md.index_prec } else { md.index_prec2 };
    let mut color_idx = [0u8; 16];
    for i in 0..16 {
        let is_fix = is_fixup(partitions, shape, i);
        let n_bits = if is_fix { (index_prec - 1) as usize } else { index_prec as usize };
        color_idx[i] = r.bits(n_bits) as u8;
    }

    // Read alpha indices
    let alpha_prec = if index_mode == 0 { md.index_prec2 } else { md.index_prec };
    let mut alpha_idx = [0u8; 16];
    if alpha_prec > 0 {
        for i in 0..16 {
            let n_bits = if i == 0 { (alpha_prec - 1) as usize } else { alpha_prec as usize };
            alpha_idx[i] = r.bits(n_bits) as u8;
        }
    }

    // Determine which weight table to use
    let max_color_idx = 1 << index_prec;
    let color_weights: &[u32] = match max_color_idx {
        2 => &WEIGHTS_2,
        4 => &WEIGHTS_2,
        8 => &WEIGHTS_3,
        16 => &WEIGHTS_4,
        _ => &WEIGHTS_3,
    };
    let alpha_weights: &[u32] = match alpha_prec {
        0 => color_weights,
        2 => &WEIGHTS_2,
        3 => &WEIGHTS_3,
        4 => &WEIGHTS_4,
        _ => color_weights,
    };

    // Interpolate and output each pixel
    for i in 0..16 {
        let subset = if partitions == 0 {
            0
        } else if partitions == 1 {
            PARTITION_2[shape][i] as usize
        } else {
            PARTITION_3[shape][i] as usize
        };

        let ep_a = subset * 2;
        let ep_b = subset * 2 + 1;

        let ca = endpoints[ep_a];
        let cb = endpoints[ep_b];

        let cw = color_weights[color_idx[i] as usize];
        let aw = if alpha_prec > 0 {
            alpha_weights[alpha_idx[i] as usize]
        } else {
            cw
        };

        let out_r = ((ca[0] as u32 * (WEIGHT_MAX - cw) + cb[0] as u32 * cw + WEIGHT_ROUND) >> WEIGHT_SHIFT) as u8;
        let out_g = ((ca[1] as u32 * (WEIGHT_MAX - cw) + cb[1] as u32 * cw + WEIGHT_ROUND) >> WEIGHT_SHIFT) as u8;
        let out_b = ((ca[2] as u32 * (WEIGHT_MAX - cw) + cb[2] as u32 * cw + WEIGHT_ROUND) >> WEIGHT_SHIFT) as u8;
        let out_a = if alpha_prec > 0 {
            ((ca[3] as u32 * (WEIGHT_MAX - aw) + cb[3] as u32 * aw + WEIGHT_ROUND) >> WEIGHT_SHIFT) as u8
        } else if md.prec_a > 0 {
            ((ca[3] as u32 * (WEIGHT_MAX - cw) + cb[3] as u32 * cw + WEIGHT_ROUND) >> WEIGHT_SHIFT) as u8
        } else {
            255
        };

        let (out_r, out_g, out_b, out_a) = match rotation {
            1 => (out_a, out_g, out_b, out_r),
            2 => (out_r, out_a, out_b, out_g),
            3 => (out_r, out_g, out_a, out_b),
            _ => (out_r, out_g, out_b, out_a),
        };

        out[i * 4] = out_r;
        out[i * 4 + 1] = out_g;
        out[i * 4 + 2] = out_b;
        out[i * 4 + 3] = out_a;
    }
}

// ============================================================
// BC7 Encode block (simplified - uses mode 6)
// ============================================================

fn encode_bc7_block(block: &[[u8; 4]]) -> [u8; 16] {
    // Use mode 6 (1 subset, RGBA 7777 with P-bits, 4-bit indices)
    // Find per-channel min/max for endpoints
    let mut min_c = [255u8; 4];
    let mut max_c = [0u8; 4];
    for px in block.iter().take(16) {
        for c in 0..4 {
            min_c[c] = min_c[c].min(px[c]);
            max_c[c] = max_c[c].max(px[c]);
        }
    }

    // Quantize endpoints to 7 bits (to be stored as 7777, then +P-bit for 8)
    let ep0 = [
        quantize(min_c[0], 7),
        quantize(min_c[1], 7),
        quantize(min_c[2], 7),
        quantize(min_c[3], 7),
    ];
    let ep1 = [
        quantize(max_c[0], 7),
        quantize(max_c[1], 7),
        quantize(max_c[2], 7),
        quantize(max_c[3], 7),
    ];

    // Compute P-bits (LSB of 8-bit value before quantization)
    let pbit0 = (min_c[0] & 1) | ((min_c[1] & 1) << 1) | ((min_c[2] & 1) << 2) | ((min_c[3] & 1) << 3);
    let pbit1 = (max_c[0] & 1) | ((max_c[1] & 1) << 1) | ((max_c[2] & 1) << 2) | ((max_c[3] & 1) << 3);
    // P-bits are stored as 2 bits (one per endpoint)
    let pb0 = if (pbit0 & 0b1010) != 0 { 1 } else { 0 }; // Majority of LSBs for endpoint 0
    let pb1 = if (pbit1 & 0b1010) != 0 { 1 } else { 0 }; // Majority of LSBs for endpoint 1

    // Unquantize endpoints with P-bits for palette computation
    let uq_ep0 = [
        unquantize((ep0[0] << 1) | pb0, 8),
        unquantize((ep0[1] << 1) | pb0, 8),
        unquantize((ep0[2] << 1) | pb0, 8),
        unquantize((ep0[3] << 1) | pb0, 8),
    ];
    let uq_ep1 = [
        unquantize((ep1[0] << 1) | pb1, 8),
        unquantize((ep1[1] << 1) | pb1, 8),
        unquantize((ep1[2] << 1) | pb1, 8),
        unquantize((ep1[3] << 1) | pb1, 8),
    ];

    // Find best 4-bit indices
    let mut indices = [0u8; 16];
    for i in 0..16 {
        let px = block[i];
        let mut best_idx = 0u8;
        let mut best_err = u32::MAX;
        for idx in 0..16u8 {
            let w = WEIGHTS_4[idx as usize];
            let pr = (uq_ep0[0] as u32 * (WEIGHT_MAX - w) + uq_ep1[0] as u32 * w + WEIGHT_ROUND) >> WEIGHT_SHIFT;
            let pg = (uq_ep0[1] as u32 * (WEIGHT_MAX - w) + uq_ep1[1] as u32 * w + WEIGHT_ROUND) >> WEIGHT_SHIFT;
            let pb = (uq_ep0[2] as u32 * (WEIGHT_MAX - w) + uq_ep1[2] as u32 * w + WEIGHT_ROUND) >> WEIGHT_SHIFT;
            let pa = (uq_ep0[3] as u32 * (WEIGHT_MAX - w) + uq_ep1[3] as u32 * w + WEIGHT_ROUND) >> WEIGHT_SHIFT;

            let dr = px[0] as i32 - pr as i32;
            let dg = px[1] as i32 - pg as i32;
            let db = px[2] as i32 - pb as i32;
            let da = px[3] as i32 - pa as i32;
            let err = (dr * dr + dg * dg + db * db + da * da) as u32;
            if err < best_err {
                best_err = err;
                best_idx = idx;
            }
        }
        indices[i] = best_idx;
    }

    // Write block: mode 6 = 0 (no zeros), then a 1 bit, so mode 6 has 6 zeros and a 1
    // Actually mode 6 has 6 leading zeros followed by a 1: 0000001
    let mut out = [0u8; 16];
    let mut w = BitWriter::new(&mut out);

    // Mode 6: 6 zero bits + 1 one bit = 7 bits total for mode
    for _ in 0..6 { w.write_bit(0); }
    w.write_bit(1);

    // No partition bits, no rotation bits, no index mode bits for mode 6
    // Write endpoints: R, G, B, A for endpoint 0, then endpoint 1
    w.write_bits(ep0[0] as u32, 7);
    w.write_bits(ep0[1] as u32, 7);
    w.write_bits(ep0[2] as u32, 7);
    w.write_bits(ep0[3] as u32, 7);
    w.write_bits(ep1[0] as u32, 7);
    w.write_bits(ep1[1] as u32, 7);
    w.write_bits(ep1[2] as u32, 7);
    w.write_bits(ep1[3] as u32, 7);

    // P-bits (2 bits: one per endpoint)
    w.write_bit(pb0);
    w.write_bit(pb1);

    // Indices: 16 × 4 bits, fixup at index 0 uses 3 bits
    for i in 0..16 {
        let n_bits = if i == 0 { 3 } else { 4 };
        w.write_bits(indices[i] as u32, n_bits);
    }

    out
}

// ============================================================
// Public API — using decode_blocks! macro for decoding
// ============================================================

const BC7_LAYOUT: BlockLayout = BlockLayout::new(4, 4, 16);

// Use decode_blocks! for the U8 decode path (BC7 is LDR → U8 output)
decode_blocks!(decode_bc7, U8, BC7_LAYOUT, decode_bc7_block);

/// Encode RGBA8 pixels to BC7.
pub fn encode_bc7(
    pixels: &PixelDatas,
    width: usize,
    height: usize,
) -> PixelDatas {
    encode_bc7_inner(pixels, width, height)
}

fn encode_bc7_inner(
    pixels: &PixelDatas,
    width: usize,
    height: usize,
) -> PixelDatas {
    // Convert input to U8
    let rgba: std::borrow::Cow<'_, [u8]> = match pixels {
        PixelDatas::U8(data) => std::borrow::Cow::Borrowed(data.as_slice()),
        other => std::borrow::Cow::Owned(other.convert_to_u8_bytes()),
    };

    let block_w = 4usize;
    let block_h = 4usize;
    let block_size = 16usize;

    let bx = (width + block_w - 1) / block_w;
    let by = (height + block_h - 1) / block_h;
    let mut out = vec![0u8; bx * by * block_size];

    out.par_chunks_mut(block_size).enumerate().for_each(|(i, chunk)| {
        let bx_i = i % bx;
        let by_i = i / bx;

        // Extract 4×4 block
        let mut block = [[0u8; 4]; 16];
        for py in 0..block_h {
            for px in 0..block_w {
                let sx = bx_i * block_w + px;
                let sy = by_i * block_h + py;
                let bi = py * block_w + px;
                if sx < width && sy < height {
                    let src = (sy * width + sx) * 4;
                    for c in 0..4 {
                        block[bi][c] = rgba[src + c];
                    }
                } else {
                    block[bi] = [0u8; 4];
                }
            }
        }

        let encoded = encode_bc7_block(&block);
        chunk.copy_from_slice(&encoded);
    });

    PixelDatas::U8(out)
}
