//! BC6h (Block Compressed HDR) pure-Rust encoder/decoder.
//!
//! Port of DirectXTex's BC6HBC7.cpp for the two BC6h formats:
//! - `Bc6hRgbUfloat` (unsigned half-float → 4×4 block → 16 raw bytes)
//! - `Bc6hRgbFloat`  (signed half-float   → 4×4 block → 16 raw bytes)
//!
//! 128-bit blocks (16 bytes), 4×4 pixels per block.
//! 14 valid modes: 0–9 partitioned (2 regions), 10–13 non-partitioned.
//!
//! Encoder uses a simple non-partitioned mode 10 (4-bit indices, 10-bit endpoints).
//! Decoder handles all 14 modes.

use half::f16;
use rayon::prelude::*;

use crate::graphics::texture::format::PixelDatas;

// ============================================================
// Constants — shared spec tables
// ============================================================

/// Partition table for 2-subset modes (32 shapes × 16 pixels).
const PARTITION: [[u8; 16]; 32] = [
    [0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1],
    [0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1],
    [0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1],
    [0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 1, 1, 1],
    [0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 1, 1],
    [0, 0, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1],
    [0, 0, 0, 1, 0, 0, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1],
    [0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 1, 1, 0, 1, 1, 1],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 1, 1],
    [0, 0, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
    [0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 1, 1, 1, 1, 1, 1],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 1, 1],
    [0, 0, 0, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
    [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1],
    [0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1],
    [0, 0, 0, 0, 1, 0, 0, 0, 1, 1, 1, 0, 1, 1, 1, 1],
    [0, 1, 1, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 1, 1, 0],
    [0, 1, 1, 1, 0, 0, 1, 1, 0, 0, 0, 1, 0, 0, 0, 0],
    [0, 0, 1, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 1, 0, 0, 0, 1, 1, 0, 0, 1, 1, 1, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 1, 0, 0],
    [0, 1, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 0, 1],
    [0, 0, 1, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0],
    [0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 1, 0, 0],
    [0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0],
    [0, 0, 1, 1, 0, 1, 1, 0, 0, 1, 1, 0, 1, 1, 0, 0],
    [0, 0, 0, 1, 0, 1, 1, 1, 1, 1, 1, 0, 1, 0, 0, 0],
    [0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0],
    [0, 1, 1, 1, 0, 0, 0, 1, 1, 0, 0, 0, 1, 1, 1, 0],
    [0, 0, 1, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 1, 0, 0],
];

/// Fix-up index for second subset in each of the 32 partition shapes.
const FIXUP: [u8; 32] = [
    15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15,
    15, 2, 8, 2, 2, 8, 8, 15, 2, 8, 2, 2, 8, 8, 2, 2,
];

/// Interpolation weights for 3-bit indices (modes 0–9).
const W3: [i32; 8] = [0, 9, 18, 27, 37, 46, 55, 64];

/// Interpolation weights for 4-bit indices (modes 10–13).
const W4: [i32; 16] = [0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64];

/// Maps the 5-bit mode value (0..31) to the mode index (0..13) or -1 for invalid.
const MODE_TO_INFO: [i8; 32] = [
    0, 1, 2, 10, -1, -1, 3, 11, -1, -1, 4, 12, -1, -1, 5, 13,
    -1, -1, 6, -1, -1, -1, 7, -1, -1, -1, 8, -1, -1, -1, 9, -1,
];

// ============================================================
// Mode descriptor tables — per-mode per-channel field bit counts
// ============================================================

/// Describes the bit layout for one BC6h mode.
struct ModeDesc {
    /// True if the mode uses 2-region partitioning.
    partitioned: bool,
    /// True if the mode uses delta transformation.
    transformed: bool,
    /// Index precision (3 for modes 0–9, 4 for modes 10–13).
    index_prec: u8,
    /// Per-channel bit counts for the four endpoint fields.
    /// W = subset 0 endpoint A, X = subset 1 endpoint A,
    /// Y = subset 0 endpoint B (delta if transformed), Z = subset 1 endpoint B (delta if transformed).
    w: [u8; 3],
    x: [u8; 3],
    y: [u8; 3],
    z: [u8; 3],
    /// Total header bits consumed before index data starts.
    header_bits: usize,
}

/// All 14 valid BC6h modes indexed by the MODE_TO_INFO lookup.
static MODE_DESC: [ModeDesc; 14] = [
    // 0: 0x00 — A(10,10,10) B(5,5,5) partitioned transformed 3-bit index
    ModeDesc {
        partitioned: true,
        transformed: true,
        index_prec: 3,
        w: [10, 10, 10],
        x: [5, 5, 5],
        y: [5, 5, 5],
        z: [5, 5, 5],
        header_bits: 82,
    },
    // 1: 0x01 — A(7,7,7) B(6,6,6) partitioned transformed 3-bit index
    ModeDesc {
        partitioned: true,
        transformed: true,
        index_prec: 3,
        w: [7, 7, 7],
        x: [6, 6, 6],
        y: [6, 6, 6],
        z: [6, 6, 6],
        header_bits: 82,
    },
    // 2: 0x02 — A(11,11,11) B(5,4,4) partitioned transformed 3-bit index
    ModeDesc {
        partitioned: true,
        transformed: true,
        index_prec: 3,
        w: [11, 11, 11],
        x: [4, 5, 4],
        y: [5, 4, 4],
        z: [4, 4, 5],
        header_bits: 82,
    },
    // 3: 0x06 — A(11,11,11) B(4,5,4) partitioned transformed 3-bit index
    ModeDesc {
        partitioned: true,
        transformed: true,
        index_prec: 3,
        w: [11, 11, 11],
        x: [4, 4, 5],
        y: [4, 5, 4],
        z: [5, 4, 4],
        header_bits: 82,
    },
    // 4: 0x0a — A(11,11,11) B(4,4,5) partitioned transformed 3-bit index
    ModeDesc {
        partitioned: true,
        transformed: true,
        index_prec: 3,
        w: [11, 11, 11],
        x: [5, 4, 4],
        y: [4, 4, 5],
        z: [4, 5, 4],
        header_bits: 82,
    },
    // 5: 0x0e — A(9,9,9) B(5,5,5) partitioned transformed 3-bit index
    ModeDesc {
        partitioned: true,
        transformed: true,
        index_prec: 3,
        w: [9, 9, 9],
        x: [5, 5, 5],
        y: [5, 5, 5],
        z: [5, 5, 5],
        header_bits: 82,
    },
    // 6: 0x12 — A(8,8,8) B(6,5,5) partitioned transformed 3-bit index
    ModeDesc {
        partitioned: true,
        transformed: true,
        index_prec: 3,
        w: [8, 8, 8],
        x: [5, 5, 6],
        y: [6, 5, 5],
        z: [5, 6, 5],
        header_bits: 82,
    },
    // 7: 0x16 — A(8,8,8) B(5,6,5) partitioned transformed 3-bit index
    ModeDesc {
        partitioned: true,
        transformed: true,
        index_prec: 3,
        w: [8, 8, 8],
        x: [6, 5, 5],
        y: [5, 6, 5],
        z: [5, 5, 6],
        header_bits: 82,
    },
    // 8: 0x1a — A(8,8,8) B(5,5,6) partitioned transformed 3-bit index
    ModeDesc {
        partitioned: true,
        transformed: true,
        index_prec: 3,
        w: [8, 8, 8],
        x: [5, 6, 5],
        y: [5, 5, 6],
        z: [6, 5, 5],
        header_bits: 82,
    },
    // 9: 0x1e — A(6,6,6) B(6,6,6) partitioned NOT-transformed 3-bit index
    ModeDesc {
        partitioned: true,
        transformed: false,
        index_prec: 3,
        w: [6, 6, 6],
        x: [6, 6, 6],
        y: [6, 6, 6],
        z: [6, 6, 6],
        header_bits: 82,
    },
    // 10: 0x03 — A(10,10,10) B(10,10,10) non-partitioned NOT-transformed 4-bit index
    ModeDesc {
        partitioned: false,
        transformed: false,
        index_prec: 4,
        w: [10, 10, 10],
        x: [0, 0, 0],
        y: [10, 10, 10],
        z: [0, 0, 0],
        header_bits: 65,
    },
    // 11: 0x07 — A(11,11,11) B(9,9,9) non-partitioned transformed 4-bit index
    ModeDesc {
        partitioned: false,
        transformed: true,
        index_prec: 4,
        w: [11, 11, 11],
        x: [0, 0, 0],
        y: [9, 9, 9],
        z: [0, 0, 0],
        header_bits: 65,
    },
    // 12: 0x0b — A(12,12,12) B(8,8,8) non-partitioned transformed 4-bit index
    ModeDesc {
        partitioned: false,
        transformed: true,
        index_prec: 4,
        w: [12, 12, 12],
        x: [0, 0, 0],
        y: [8, 8, 8],
        z: [0, 0, 0],
        header_bits: 65,
    },
    // 13: 0x0f — A(16,16,16) B(4,4,4) non-partitioned transformed 4-bit index
    ModeDesc {
        partitioned: false,
        transformed: true,
        index_prec: 4,
        w: [16, 16, 16],
        x: [0, 0, 0],
        y: [4, 4, 4],
        z: [0, 0, 0],
        header_bits: 65,
    },
];

// ============================================================
// Bit-level helpers
// ============================================================

/// Reads bits LSB-first from a byte slice.
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

/// Writes bits LSB-first into a byte slice.
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
// Core helper functions
// ============================================================

/// Sign-extend an `nb`-bit value to `i32` (2's complement).
fn sign_extend(x: i32, nb: u8) -> i32 {
    if nb == 0 {
        return 0;
    }
    let shift = 32 - nb;
    (x << shift) >> shift
}

/// Convert an f16 to the integer domain used by BC6h.
fn f16_to_int(f: f16, signed: bool) -> i32 {
    let bits = f.to_bits() as i32;
    if signed {
        // f16 sign-magnitude → 2's complement i32
        let sign = bits & 0x8000;
        let mag = bits & 0x7FFF;
        if sign == 0 { mag } else { -mag }
    } else {
        // Unsigned: treat the f16 bit pattern as a positive integer
        bits & 0x7FFF
    }
}

/// Convert an integer back to f16 (inverse of f16_to_int).
fn int_to_f16(input: i32, signed: bool) -> f16 {
    if signed {
        if input < 0 {
            let mag = (-input).min(0x7FFF) as u16;
            f16::from_bits(mag | 0x8000)
        } else {
            let mag = input.min(0x7FFF) as u16;
            f16::from_bits(mag)
        }
    } else {
        let v = input.max(0).min(0x7FFF) as u16;
        f16::from_bits(v)
    }
}

/// Quantize a value to the given bit precision.
fn quantize(value: i32, prec: u8, signed: bool) -> i32 {
    if signed {
        let max_val = (1 << (prec - 1)) - 1;
        let min_val = -(1 << (prec - 1));
        let clamped = value.clamp(min_val, max_val);
        clamped & ((1 << prec) - 1)
    } else {
        let max_val = (1 << prec) - 1;
        value.max(0).min(max_val)
    }
}

/// Unquantize from `bits_per_comp`-bit representation to 16-bit range.
fn unquantize(comp: i32, bits_per_comp: u8, signed: bool) -> i32 {
    if !signed {
        if bits_per_comp >= 16 || bits_per_comp == 0 {
            return comp;
        }
        let max_val = (1 << bits_per_comp) - 1;
        // comp is unsigned in [0, max_val]
        (comp * 65536 + (1 << (bits_per_comp - 1))) / max_val
    } else {
        if bits_per_comp >= 16 || bits_per_comp == 0 {
            return comp;
        }
        let max_val = (1 << bits_per_comp) - 1;
        let sign = if comp < 0 { -1 } else { 1 };
        let abs_val = comp.abs();
        let result = (abs_val * 65536 + (1 << (bits_per_comp - 1))) / max_val;
        sign * result.min(0x7FFF)
    }
}

/// Final unquantize: bring the 16-bit interpolated value into f16 range.
fn finish_unquantize(comp: i32, signed: bool) -> i32 {
    if !signed {
        // unsigned: *31/64 compresses [0,65535] → [0,~31743] (f16 max normal ≅ 0x7BFF)
        (comp * 31) / 64
    } else {
        // signed: *31/32 compresses [-32767,32767] → [-31743,31743]
        if comp < 0 {
            -((-comp * 31) / 32)
        } else {
            (comp * 31) / 32
        }
    }
}

// ============================================================
// Block-level decode
// ============================================================

fn decode_bc6h_block(blk: &[u8], out: &mut [f16; 64], is_signed: bool) {
    // --- Step 1: Determine the 5-bit mode value ---
    let m0 = (blk[0] >> 0) & 1;
    let m1 = (blk[0] >> 1) & 1;

    let mode5: u8 = if m0 == 0 && m1 == 0 {
        0x00
    } else if m1 == 0 && m0 == 1 {
        0x01
    } else {
        // Read 3 more mode bits (positions 2, 3, 4)
        let m2 = (blk[0] >> 2) & 1;
        let m3 = (blk[0] >> 3) & 1;
        let m4 = (blk[0] >> 4) & 1;
        m0 | (m1 << 1) | (m2 << 2) | (m3 << 3) | (m4 << 4)
    };

    let info_idx = if (mode5 as usize) < 32 {
        MODE_TO_INFO[mode5 as usize]
    } else {
        -1
    };

    if info_idx < 0 {
        // Invalid mode: fill with opaque black
        for i in 0..16 {
            out[i * 4] = f16::ZERO;
            out[i * 4 + 1] = f16::ZERO;
            out[i * 4 + 2] = f16::ZERO;
            out[i * 4 + 3] = f16::ONE;
        }
        return;
    }

    let md = &MODE_DESC[info_idx as usize];

    // --- Step 2: Read the header bits via BitReader ---
    let mut r = BitReader::new(blk);

    // Consume mode bits
    let mode_bits_count: usize = if mode5 <= 1 { 2 } else { 5 };
    r.bits(mode_bits_count);

    // Consume partition bits
    let shape: usize = if md.partitioned {
        r.bits(5) as usize
    } else {
        0
    };

    // Read the four endpoint fields per channel in order W, X, Y, Z
    let mut aw = [0u32; 3]; // subset 0 endpoint A
    let mut ax = [0u32; 3]; // subset 1 endpoint A
    let mut ay = [0u32; 3]; // subset 0 endpoint B / delta
    let mut az = [0u32; 3]; // subset 1 endpoint B / delta

    for c in 0..3 {
        aw[c] = r.bits(md.w[c] as usize);
    }
    for c in 0..3 {
        ax[c] = r.bits(md.x[c] as usize);
    }
    for c in 0..3 {
        ay[c] = r.bits(md.y[c] as usize);
    }
    for c in 0..3 {
        az[c] = r.bits(md.z[c] as usize);
    }

    // --- Step 3: Compute total endpoint data bits and skip padding ---
    let total_field_bits = md.w.iter().chain(md.x.iter()).chain(md.y.iter()).chain(md.z.iter()).map(|&b| b as usize).sum::<usize>()
        + if md.partitioned { 5 + mode_bits_count } else { mode_bits_count };
    // Some modes have NA padding bits between the endpoint data and the index data.
    // header_bits is the total header. The number of NA padding bits within the header:
    let na_padding = if md.header_bits > total_field_bits {
        md.header_bits - total_field_bits
    } else {
        md.header_bits.saturating_sub(total_field_bits)
    };
    r.bits(na_padding);

    // --- Step 4: Read indices ---
    let mut indices = [0u16; 16];
    let num_subsets = if md.partitioned { 2 } else { 1 };
    for i in 0..16 {
        let is_fixup = if num_subsets > 1 {
            if i == 0 || i == FIXUP[shape] as usize {
                true
            } else {
                false
            }
        } else {
            i == 0
        };
        let n_bits = if is_fixup {
            (md.index_prec - 1) as usize
        } else {
            md.index_prec as usize
        };
        indices[i] = r.bits(n_bits) as u16;
    }

    // --- Step 5: Reconstruct endpoints per subset ---
    // For partitioned modes:
    //   subset 0: A = aw[c] @ w_bits[c], delta = ay[c] @ y_bits[c]
    //   subset 1: A = ax[c] @ x_bits[c], delta = az[c] @ z_bits[c]
    // For non-transformed modes, delta is absolute (endpoint B = delta value).
    // For transformed modes, B = A + delta (wrapping at the precision of A).

    // ep[subset][channel] for A and B values before unquantize
    let mut ep_a = [[0i32; 3]; 2];
    let mut ep_b = [[0i32; 3]; 2];
    let mut precs = [[0u8; 3]; 2]; // precision in bits for each subset/channel

    // --- subset 0 ---
    for c in 0..3 {
        precs[0][c] = md.w[c];
        ep_a[0][c] = aw[c] as i32;
        if md.transformed {
            // delta is sign-extended for signed mode always; for unsigned only when transformed
            let delta = if is_signed {
                sign_extend(ay[c] as i32, md.y[c])
            } else {
                // For unsigned transformed, the delta is a signed offset stored in unsigned form.
                // We need to handle it as a 2's complement value in the field's bit width.
                sign_extend(ay[c] as i32, md.y[c])
            };
            let mask = (1i32 << md.w[c]) - 1;
            ep_b[0][c] = (ep_a[0][c] + delta) & mask;
        } else {
            // Non-transformed: Y is absolute endpoint B
            ep_b[0][c] = ay[c] as i32;
        }
    }

    // --- subset 1 ---
    for c in 0..3 {
        if md.partitioned {
            precs[1][c] = md.x[c];
            ep_a[1][c] = ax[c] as i32;
            if md.transformed {
                let delta = if is_signed {
                    sign_extend(az[c] as i32, md.z[c])
                } else {
                    sign_extend(az[c] as i32, md.z[c])
                };
                let mask = (1i32 << md.x[c]) - 1;
                ep_b[1][c] = (ep_a[1][c] + delta) & mask;
            } else {
                // Non-transformed: Z is absolute endpoint B for subset 1
                ep_b[1][c] = az[c] as i32;
            }
        } else {
            // Non-partitioned: reuse subset 0's values
            precs[1][c] = md.w[c];
            ep_a[1][c] = ep_a[0][c];
            ep_b[1][c] = ep_b[0][c];
        }
    }

    // --- Step 6: Sign-extend endpoints (only for signed mode) ---
    if is_signed {
        for s in 0..2 {
            for c in 0..3 {
                ep_a[s][c] = sign_extend(ep_a[s][c], precs[s][c]);
                ep_b[s][c] = sign_extend(ep_b[s][c], precs[s][c]);
            }
        }
    }

    // --- Step 7: Unquantize endpoints ---
    let mut uq_a = [[0i32; 3]; 2];
    let mut uq_b = [[0i32; 3]; 2];
    for s in 0..2 {
        for c in 0..3 {
            if precs[s][c] > 0 {
                uq_a[s][c] = unquantize(ep_a[s][c], precs[s][c], is_signed);
                uq_b[s][c] = unquantize(ep_b[s][c], precs[s][c], is_signed);
            } else {
                uq_a[s][c] = 0;
                uq_b[s][c] = 0;
            }
        }
    }

    // --- Step 8: Interpolate for each pixel ---
    let weights: &[i32] = if md.index_prec == 4 {
        &W4 as &[i32]
    } else {
        &W3 as &[i32]
    };
    for i in 0..16 {
        let subset = if md.partitioned {
            PARTITION[shape][i] as usize
        } else {
            0usize
        };
        let idx = indices[i] as usize;
        let w = weights[idx];
        let r = (uq_a[subset][0] * (64 - w) + uq_b[subset][0] * w + 32) >> 6;
        let g = (uq_a[subset][1] * (64 - w) + uq_b[subset][1] * w + 32) >> 6;
        let b = (uq_a[subset][2] * (64 - w) + uq_b[subset][2] * w + 32) >> 6;

        let r = finish_unquantize(r, is_signed);
        let g = finish_unquantize(g, is_signed);
        let b = finish_unquantize(b, is_signed);

        out[i * 4] = int_to_f16(r, is_signed);
        out[i * 4 + 1] = int_to_f16(g, is_signed);
        out[i * 4 + 2] = int_to_f16(b, is_signed);
        out[i * 4 + 3] = f16::ONE;
    }
}

// ============================================================
// Block-level encode (mode 10 — non-partitioned, 10-bit, 4-bit index)
// ============================================================

fn encode_bc6h_block(block: &[[f16; 4]], is_signed: bool) -> [u8; 16] {
    // Step 1: Convert f16 pixels to integer domain
    let mut pixels = [[0i32; 4]; 16];
    for i in 0..16 {
        for c in 0..3 {
            pixels[i][c] = f16_to_int(block[i][c], is_signed);
        }
        pixels[i][3] = 0; // alpha unused
    }

    // Step 2: Find per-channel min/max for endpoints
    let mut min_r = i32::MAX;
    let mut max_r = i32::MIN;
    let mut min_g = i32::MAX;
    let mut max_g = i32::MIN;
    let mut min_b = i32::MAX;
    let mut max_b = i32::MIN;

    for i in 0..16 {
        let r = pixels[i][0];
        let g = pixels[i][1];
        let b = pixels[i][2];
        if r < min_r { min_r = r; }
        if r > max_r { max_r = r; }
        if g < min_g { min_g = g; }
        if g > max_g { max_g = g; }
        if b < min_b { min_b = b; }
        if b > max_b { max_b = b; }
    }

    // Step 3: Quantize endpoints to 10 bits
    let ep0_r = quantize(min_r, 10, is_signed);
    let ep0_g = quantize(min_g, 10, is_signed);
    let ep0_b = quantize(min_b, 10, is_signed);
    let ep1_r = quantize(max_r, 10, is_signed);
    let ep1_g = quantize(max_g, 10, is_signed);
    let ep1_b = quantize(max_b, 10, is_signed);

    // Step 4: For each pixel, find best 4-bit palette index
    // Palette uses W4 weights: P[idx] = (A * (64 - W4[idx]) + B * W4[idx] + 32) >> 6
    let mut indices = [0u8; 16];

    for i in 0..16 {
        let pr = pixels[i][0];
        let pg = pixels[i][1];
        let pb = pixels[i][2];

        let mut best_idx = 0u8;
        let mut best_err = i64::MAX;

        for idx in 0..16u8 {
            let w = W4[idx as usize];
            let pal_r = (ep0_r * (64 - w) + ep1_r * w + 32) >> 6;
            let pal_g = (ep0_g * (64 - w) + ep1_g * w + 32) >> 6;
            let pal_b = (ep0_b * (64 - w) + ep1_b * w + 32) >> 6;

            let dr = pr as i64 - pal_r as i64;
            let dg = pg as i64 - pal_g as i64;
            let db = pb as i64 - pal_b as i64;
            let err = dr * dr + dg * dg + db * db;

            if err < best_err {
                best_err = err;
                best_idx = idx;
            }
        }
        indices[i] = best_idx;
    }

    // Step 5: Write the block (mode 10 = 0x03)
    let mut out = [0u8; 16];
    let mut w = BitWriter::new(&mut out);

    // Mode 10: 5 bits 0b00011 → LSB first: 1,1,0,0,0
    w.write_bits(0x03, 5);

    // Endpoint A (subset 0): 10 bits each
    w.write_bits(ep0_r as u32, 10);
    w.write_bits(ep0_g as u32, 10);
    w.write_bits(ep0_b as u32, 10);

    // Endpoint B (subset 0): 10 bits each
    w.write_bits(ep1_r as u32, 10);
    w.write_bits(ep1_g as u32, 10);
    w.write_bits(ep1_b as u32, 10);

    // Indices: 16 × 4 bits, fixup at index 0 uses 3 bits
    for i in 0..16 {
        let n_bits = if i == 0 { 3 } else { 4 };
        w.write_bits(indices[i] as u32, n_bits);
    }

    out
}

// ============================================================
// Top-level encode / decode — parallel iteration
// ============================================================

/// Encode f16 pixels to BC6h unsigned (Bc6hRgbUfloat).
pub fn encode_bc6h(
    pixels: &PixelDatas,
    width: usize,
    height: usize,
) -> PixelDatas {
    encode_bc6h_inner(pixels, width, height, false)
}

/// Encode f16 pixels to BC6h signed (Bc6hRgbFloat).
pub fn encode_bc6h_signed(
    pixels: &PixelDatas,
    width: usize,
    height: usize,
) -> PixelDatas {
    encode_bc6h_inner(pixels, width, height, true)
}

fn encode_bc6h_inner(
    pixels: &PixelDatas,
    width: usize,
    height: usize,
    is_signed: bool,
) -> PixelDatas {
    // Convert input to F16
    let rgba: std::borrow::Cow<'_, [f16]> = match pixels {
        PixelDatas::F16(data) => std::borrow::Cow::Borrowed(data.as_slice()),
        PixelDatas::U8(data) => {
            let n = data.len();
            let mut out = vec![f16::ZERO; n];
            out.par_chunks_mut(4096)
                .enumerate()
                .for_each(|(chunk_idx, chunk)| {
                    let base = chunk_idx * 4096;
                    for (j, dst) in chunk.iter_mut().enumerate() {
                        *dst = f16::from_f32(data[base + j] as f32 / 255.0);
                    }
                });
            std::borrow::Cow::Owned(out)
        }
        other => std::borrow::Cow::Owned(other.convert_to_f16_bytes()),
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

        // Extract 4×4 block, handling boundary clamping
        let mut block = [[f16::ZERO; 4]; 16];
        for py in 0..block_h {
            for px in 0..block_w {
                let sx = bx_i * block_w + px;
                let sy = by_i * block_h + py;
                let bi = py * block_w + px;
                if sx < width && sy < height {
                    let src = (sy * width + sx) * 4;
                    block[bi] = [rgba[src], rgba[src + 1], rgba[src + 2], rgba[src + 3]];
                } else {
                    block[bi] = [f16::ZERO; 4];
                }
            }
        }

        let encoded = if is_signed {
            encode_bc6h_block(&block, true)
        } else {
            encode_bc6h_block(&block, false)
        };
        chunk.copy_from_slice(&encoded);
    });

    PixelDatas::U8(out)
}

/// Decode BC6h unsigned (Bc6hRgbUfloat) to f16 pixels.
pub fn decode_bc6h(
    data: &PixelDatas,
    width: usize,
    height: usize,
) -> PixelDatas {
    decode_bc6h_inner(data, width, height, false)
}

/// Decode BC6h signed (Bc6hRgbFloat) to f16 pixels.
pub fn decode_bc6h_signed(
    data: &PixelDatas,
    width: usize,
    height: usize,
) -> PixelDatas {
    decode_bc6h_inner(data, width, height, true)
}

fn decode_bc6h_inner(
    data: &PixelDatas,
    width: usize,
    height: usize,
    is_signed: bool,
) -> PixelDatas {
    let raw = data.convert_to_u8_bytes();
    let block_w = 4usize;
    let block_h = 4usize;
    let block_size = 16usize;

    let bx = (width + block_w - 1) / block_w;
    let by = (height + block_h - 1) / block_h;
    let total = bx * by;
    let mut out = vec![f16::ZERO; width * height * 4];
    let out_addr = out.as_mut_ptr() as usize;

    (0..total).into_par_iter().for_each(|i| {
        let out_ptr = out_addr as *mut f16;
        let bx_i = i % bx;
        let by_i = i / bx;
        let off = i * block_size;
        let mut pixels = [f16::ZERO; 64];
        decode_bc6h_block(&raw[off..off + block_size], &mut pixels, is_signed);
        for py in 0..block_h {
            for px in 0..block_w {
                let sx = bx_i * block_w + px;
                let sy = by_i * block_h + py;
                if sx < width && sy < height {
                    let dst = (sy * width + sx) * 4;
                    let src = (py * block_w + px) * 4;
                    // SAFETY: each (sx, sy) pair is unique per block, no overlap.
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

    PixelDatas::F16(out)
}
