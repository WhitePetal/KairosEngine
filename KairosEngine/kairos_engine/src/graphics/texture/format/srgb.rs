//! sRGB ↔ linear conversion LUTs.
//!
//! Both LUTs are 256-entry `[u8; 256]` (512 bytes total, zero runtime cost
//! after initialization). sRGB → linear uses the standard piecewise sRGB
//! transfer function. Linear → sRGB uses the inverse piecewise function.
//!
//! Tables are lazily initialized on first access via `OnceLock`.
//!
//! Note: The LUTs and helper functions are not yet wired into encode/decode.
//! They are provided as shared infrastructure for future format implementations.

#![allow(dead_code)]

use std::sync::OnceLock;

/// sRGB 8-bit → linear 8-bit lookup table.
pub fn srgb_to_linear() -> &'static [u8; 256] {
    static TABLE: OnceLock<[u8; 256]> = OnceLock::new();
    TABLE.get_or_init(build_srgb_to_linear)
}

/// Linear 8-bit → sRGB 8-bit lookup table.
pub fn linear_to_srgb() -> &'static [u8; 256] {
    static TABLE: OnceLock<[u8; 256]> = OnceLock::new();
    TABLE.get_or_init(build_linear_to_srgb)
}

fn build_srgb_to_linear() -> [u8; 256] {
    let mut table = [0u8; 256];
    for i in 0..256 {
        let c = i as f32 / 255.0;
        let linear = if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        };
        table[i] = (linear * 255.0 + 0.5).min(255.0) as u8;
    }
    table
}

fn build_linear_to_srgb() -> [u8; 256] {
    let mut table = [0u8; 256];
    for i in 0..256 {
        let c = i as f32 / 255.0;
        let srgb = if c <= 0.0031308 {
            c * 12.92
        } else {
            1.055 * c.powf(1.0 / 2.4) - 0.055
        };
        table[i] = (srgb * 255.0 + 0.5).min(255.0) as u8;
    }
    table
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_srgb_roundtrip() {
        let stl = srgb_to_linear();
        let lts = linear_to_srgb();
        // Black and white should roundtrip perfectly.
        assert_eq!(lts[stl[0] as usize], 0);
        assert_eq!(lts[stl[255] as usize], 255);

        // Mid-gray should be approximately correct.
        let linear = stl[128]; // sRGB 128 → linear
        let back = lts[linear as usize]; // Back to sRGB
        let diff = (back as i16 - 128i16).abs();
        assert!(diff <= 2, "sRGB roundtrip for 128 differs by {}", diff);
    }

    #[test]
    fn test_srgb_monotonic() {
        let stl = srgb_to_linear();
        // sRGB→linear should be monotonic.
        for i in 0..255 {
            assert!(
                stl[i] <= stl[i + 1],
                "SRGB_TO_LINEAR not monotonic at {}",
                i
            );
        }
    }

    #[test]
    fn test_linear_monotonic() {
        let lts = linear_to_srgb();
        // Linear→sRGB should be monotonic.
        for i in 0..255 {
            assert!(
                lts[i] <= lts[i + 1],
                "LINEAR_TO_SRGB not monotonic at {}",
                i
            );
        }
    }
}
