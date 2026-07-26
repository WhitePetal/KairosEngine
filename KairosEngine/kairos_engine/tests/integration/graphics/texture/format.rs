use half::f16;

use kairos_engine::graphics::texture::{
    PixelDatas, TextureFormat,
    format::{RawPixelType, decode, encode},
};

/// Create a simple 4×4 RGBA8 gradient test image.
fn make_test_rgba(w: usize, h: usize) -> Vec<u8> {
    let mut rgba = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 4;
            rgba[i] = (x * 255 / w.max(1)) as u8;
            rgba[i + 1] = (y * 255 / h.max(1)) as u8;
            rgba[i + 2] = 128;
            rgba[i + 3] = 255;
        }
    }
    rgba
}

#[test]
fn bc1_roundtrip_4x4() {
    let w = 4;
    let h = 4;
    let rgba = make_test_rgba(w, h);
    let input = PixelDatas::U8(rgba.clone());
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::Bc1RgbaUnorm);
    let encoded_bytes = match &encoded {
        PixelDatas::U8(b) => b,
        _ => panic!("expected U8"),
    };
    // 4×4 → one 4×4 block → 8 bytes
    assert_eq!(encoded_bytes.len(), 8, "BC1 4x4 should produce 8 bytes");
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Bc1RgbaUnorm);
    let decoded_bytes = match &decoded {
        PixelDatas::U8(b) => b.as_slice(),
        _ => panic!("expected U8"),
    };
    assert_eq!(decoded_bytes.len(), rgba.len());
}

#[test]
fn bc3_roundtrip_8x8() {
    let w = 8;
    let h = 8;
    let rgba = make_test_rgba(w, h);
    let input = PixelDatas::U8(rgba.clone());
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::Bc3RgbaUnorm);
    // 8x8 → 2×2 = 4 blocks, 16 bytes each → 64 bytes
    assert_eq!(encoded.as_bytes().len(), 64);
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Bc3RgbaUnorm);
    assert_eq!(decoded.as_bytes().len(), rgba.len());
}

#[test]
fn r8_encode_decode() {
    let rgba = make_test_rgba(16, 16);
    let input = PixelDatas::U8(rgba.clone());
    let encoded = encode(&input, 16, 16, TextureFormat::R8Unorm);
    assert_eq!(encoded.as_bytes().len(), 256); // 16×16 = 256 bytes for R8
    let decoded = decode(&encoded, 16, 16, TextureFormat::R8Unorm);
    assert_eq!(decoded.as_bytes().len(), 1024); // 16×16×4 = 1024 bytes decoded
}

#[test]
fn rgba8_pass_through() {
    let rgba = make_test_rgba(8, 8);
    let input = PixelDatas::U8(rgba.clone());
    let encoded = encode(&input, 8, 8, TextureFormat::Rgba8Unorm);
    // RGBA8 pass-through preserves bytes
    assert_eq!(encoded.as_bytes(), rgba.as_slice());
}

#[test]
fn bc_encode_preserves_variant() {
    // BC encoding should return U8 variant.
    let rgba = make_test_rgba(4, 4);
    let input = PixelDatas::U8(rgba);
    let encoded = encode(&input, 4, 4, TextureFormat::Bc1RgbaUnorm);
    assert!(matches!(encoded, PixelDatas::U8(_)));
}

// ============================================================
// Group A: Uncompressed SDR format tests
// ============================================================

#[test]
fn rgba8_snorm_pass_through() {
    let rgba = make_test_rgba(8, 8);
    let input = PixelDatas::U8(rgba.clone());
    let encoded = encode(&input, 8, 8, TextureFormat::Rgba8Snorm);
    assert_eq!(encoded.as_bytes(), rgba.as_slice());
    let decoded = decode(&encoded, 8, 8, TextureFormat::Rgba8Snorm);
    assert_eq!(decoded.as_bytes(), rgba.as_slice());
}

#[test]
fn rgba8_uint_pass_through() {
    let rgba = make_test_rgba(8, 8);
    let input = PixelDatas::U8(rgba.clone());
    let encoded = encode(&input, 8, 8, TextureFormat::Rgba8Uint);
    assert_eq!(encoded.as_bytes(), rgba.as_slice());
    let decoded = decode(&encoded, 8, 8, TextureFormat::Rgba8Uint);
    assert_eq!(decoded.as_bytes(), rgba.as_slice());
}

#[test]
fn rgba8_sint_pass_through() {
    let rgba = make_test_rgba(8, 8);
    let input = PixelDatas::U8(rgba.clone());
    let encoded = encode(&input, 8, 8, TextureFormat::Rgba8Sint);
    assert_eq!(encoded.as_bytes(), rgba.as_slice());
    let decoded = decode(&encoded, 8, 8, TextureFormat::Rgba8Sint);
    assert_eq!(decoded.as_bytes(), rgba.as_slice());
}

#[test]
fn rg8_encode_decode() {
    let rgba = make_test_rgba(16, 16);
    let input = PixelDatas::U8(rgba.clone());
    let encoded = encode(&input, 16, 16, TextureFormat::Rg8Unorm);
    // 16×16 → 2 bytes per pixel = 512 bytes
    assert_eq!(encoded.as_bytes().len(), 512);
    // Verify R and G are preserved, B/A dropped
    let enc_bytes = encoded.as_bytes();
    for y in 0..16 {
        for x in 0..16 {
            let src_idx = (y * 16 + x) * 4;
            let enc_idx = (y * 16 + x) * 2;
            assert_eq!(enc_bytes[enc_idx], rgba[src_idx], "R at ({},{})", x, y);
            assert_eq!(
                enc_bytes[enc_idx + 1],
                rgba[src_idx + 1],
                "G at ({},{})",
                x,
                y
            );
        }
    }
    // Decode back
    let decoded = decode(&encoded, 16, 16, TextureFormat::Rg8Unorm);
    assert_eq!(decoded.as_bytes().len(), 1024);
    let dec_bytes = decoded.as_bytes();
    for y in 0..16 {
        for x in 0..16 {
            let idx = (y * 16 + x) * 4;
            let src_idx = (y * 16 + x) * 4;
            assert_eq!(dec_bytes[idx], rgba[src_idx], "decoded R at ({},{})", x, y);
            assert_eq!(
                dec_bytes[idx + 1],
                rgba[src_idx + 1],
                "decoded G at ({},{})",
                x,
                y
            );
            assert_eq!(
                dec_bytes[idx + 2],
                0,
                "decoded B at ({},{}) should be 0",
                x,
                y
            );
            assert_eq!(
                dec_bytes[idx + 3],
                0,
                "decoded A at ({},{}) should be 255",
                x,
                y
            );
        }
    }
}

#[test]
fn rg8_snorm_encode_decode() {
    let rgba = make_test_rgba(4, 4);
    let input = PixelDatas::U8(rgba);
    let encoded = encode(&input, 4, 4, TextureFormat::Rg8Snorm);
    assert_eq!(encoded.as_bytes().len(), 32); // 4×4×2
    let decoded = decode(&encoded, 4, 4, TextureFormat::Rg8Snorm);
    assert_eq!(decoded.as_bytes().len(), 64); // 4×4×4
}

#[test]
fn rg8_uint_encode_decode() {
    let rgba = make_test_rgba(4, 4);
    let input = PixelDatas::U8(rgba);
    let encoded = encode(&input, 4, 4, TextureFormat::Rg8Uint);
    assert_eq!(encoded.as_bytes().len(), 32);
    let decoded = decode(&encoded, 4, 4, TextureFormat::Rg8Uint);
    assert_eq!(decoded.as_bytes().len(), 64);
}

#[test]
fn rg8_sint_encode_decode() {
    let rgba = make_test_rgba(4, 4);
    let input = PixelDatas::U8(rgba);
    let encoded = encode(&input, 4, 4, TextureFormat::Rg8Sint);
    assert_eq!(encoded.as_bytes().len(), 32);
    let decoded = decode(&encoded, 4, 4, TextureFormat::Rg8Sint);
    assert_eq!(decoded.as_bytes().len(), 64);
}

#[test]
fn bgra8_encode_decode() {
    let rgba = make_test_rgba(8, 8);
    let input = PixelDatas::U8(rgba.clone());
    let encoded = encode(&input, 8, 8, TextureFormat::Bgra8Unorm);
    assert_eq!(encoded.as_bytes().len(), 256); // 8×8×4
    // Verify R↔B swap
    let enc_bytes = encoded.as_bytes();
    for y in 0..8 {
        for x in 0..8 {
            let src_idx = (y * 8 + x) * 4;
            let enc_idx = (y * 8 + x) * 4;
            assert_eq!(
                enc_bytes[enc_idx],
                rgba[src_idx + 2],
                "B (was R) at ({},{})",
                x,
                y
            );
            assert_eq!(
                enc_bytes[enc_idx + 1],
                rgba[src_idx + 1],
                "G at ({},{})",
                x,
                y
            );
            assert_eq!(
                enc_bytes[enc_idx + 2],
                rgba[src_idx],
                "R (was B) at ({},{})",
                x,
                y
            );
            assert_eq!(
                enc_bytes[enc_idx + 3],
                rgba[src_idx + 3],
                "A at ({},{})",
                x,
                y
            );
        }
    }
    // Decode back should restore original
    let decoded = decode(&encoded, 8, 8, TextureFormat::Bgra8Unorm);
    assert_eq!(decoded.as_bytes(), rgba.as_slice());
}

#[test]
fn bgra8_srgb_encode_decode() {
    let rgba = make_test_rgba(8, 8);
    let input = PixelDatas::U8(rgba.clone());
    let encoded = encode(&input, 8, 8, TextureFormat::Bgra8UnormSrgb);
    assert_eq!(encoded.as_bytes().len(), 256);
    // Decode back should restore original
    let decoded = decode(&encoded, 8, 8, TextureFormat::Bgra8UnormSrgb);
    assert_eq!(decoded.as_bytes(), rgba.as_slice());
}

#[test]
fn all_group_a_supports_encoding() {
    let formats = [
        TextureFormat::Rg8Unorm,
        TextureFormat::Rg8Snorm,
        TextureFormat::Rg8Uint,
        TextureFormat::Rg8Sint,
        TextureFormat::Rgba8Snorm,
        TextureFormat::Rgba8Uint,
        TextureFormat::Rgba8Sint,
        TextureFormat::Bgra8Unorm,
        TextureFormat::Bgra8UnormSrgb,
    ];
    for fmt in &formats {
        assert!(fmt.supports_encoding(), "{fmt:?} should support encoding");
    }
}

/// Roundtrip test: random RGBA8 pixels → encode → decode → original
#[test]
fn rg8_roundtrip_random() {
    // Simple LCG RNG
    let mut state: u32 = 42;
    let mut next_rand = || -> u8 {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        (state >> 24) as u8
    };
    let w = 16;
    let h = 16;
    let mut rgba = vec![0u8; w * h * 4];
    for px in rgba.chunks_mut(4) {
        px[0] = next_rand();
        px[1] = next_rand();
        px[2] = next_rand();
        px[3] = next_rand();
    }
    let input = PixelDatas::U8(rgba.clone());
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::Rg8Unorm);
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Rg8Unorm);
    let dec = decoded.as_bytes();
    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) * 4;
            assert_eq!(dec[idx], rgba[idx], "R at ({},{})", x, y);
            assert_eq!(dec[idx + 1], rgba[idx + 1], "G at ({},{})", x, y);
            assert_eq!(dec[idx + 2], 0, "B at ({},{})", x, y);
            assert_eq!(dec[idx + 3], 0, "A at ({},{})", x, y);
        }
    }
}

#[test]
fn bgra8_roundtrip_random() {
    let mut state: u32 = 99;
    let mut next_rand = || -> u8 {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        (state >> 24) as u8
    };
    let w = 16;
    let h = 16;
    let mut rgba = vec![0u8; w * h * 4];
    for px in rgba.chunks_mut(4) {
        px[0] = next_rand();
        px[1] = next_rand();
        px[2] = next_rand();
        px[3] = 255;
    }
    let input = PixelDatas::U8(rgba.clone());
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::Bgra8Unorm);
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Bgra8Unorm);
    assert_eq!(decoded.as_bytes(), rgba.as_slice(), "BGRA8 roundtrip");
}

#[test]
fn rgba8_snorm_roundtrip_random() {
    let mut state: u32 = 77;
    let mut next_rand = || -> u8 {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        (state >> 24) as u8
    };
    let w = 8;
    let h = 8;
    let mut rgba = vec![0u8; w * h * 4];
    for px in rgba.chunks_mut(4) {
        // Snorm range: 0..=255 maps to -1..=1, but bytes are stored as-is
        px[0] = next_rand();
        px[1] = next_rand();
        px[2] = next_rand();
        px[3] = next_rand();
    }
    let input = PixelDatas::U8(rgba.clone());
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::Rgba8Snorm);
    // Pass-through: encode should return identical bytes
    assert_eq!(encoded.as_bytes(), rgba.as_slice());
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Rgba8Snorm);
    assert_eq!(decoded.as_bytes(), rgba.as_slice());
}

#[test]
fn all_group_a_encode_decode_sizes() {
    let formats = [
        (TextureFormat::Rg8Unorm, 2usize),
        (TextureFormat::Rg8Snorm, 2),
        (TextureFormat::Rg8Uint, 2),
        (TextureFormat::Rg8Sint, 2),
        (TextureFormat::Rgba8Snorm, 4),
        (TextureFormat::Rgba8Uint, 4),
        (TextureFormat::Rgba8Sint, 4),
        (TextureFormat::Bgra8Unorm, 4),
        (TextureFormat::Bgra8UnormSrgb, 4),
    ];
    for (fmt, bpp) in &formats {
        let rgba = make_test_rgba(4, 4);
        let input = PixelDatas::U8(rgba);
        let encoded = encode(&input, 4, 4, *fmt);
        assert_eq!(
            encoded.as_bytes().len(),
            4 * 4 * bpp,
            "{fmt:?} encoded size mismatch"
        );
        let decoded = decode(&encoded, 4, 4, *fmt);
        assert_eq!(
            decoded.as_bytes().len(),
            4 * 4 * 4,
            "{fmt:?} decoded size should be RGBA8"
        );
    }
}

// ============================================================
// Group B: Wide format tests (R16, Rg16, Rgba16 Uint/Sint/Float)
// ============================================================

#[test]
fn all_group_b_supports_encoding() {
    let formats = [
        TextureFormat::R16Uint,
        TextureFormat::R16Sint,
        TextureFormat::R16Float,
        TextureFormat::Rg16Uint,
        TextureFormat::Rg16Sint,
        TextureFormat::Rg16Float,
        TextureFormat::Rgba16Uint,
        TextureFormat::Rgba16Sint,
        TextureFormat::Rgba16Float,
    ];
    for fmt in &formats {
        assert!(fmt.supports_encoding(), "{fmt:?} should support encoding");
    }
}

#[test]
fn r16_uint_roundtrip() {
    let w = 8;
    let h = 8;
    let mut state: u32 = 42;
    let mut next_rand = || -> u8 {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        (state >> 24) as u8
    };
    let mut rgba = vec![0u8; w * h * 4];
    for px in rgba.chunks_mut(4) {
        px[0] = next_rand();
        px[1] = next_rand();
        px[2] = next_rand();
        px[3] = next_rand();
    }
    let input = PixelDatas::U8(rgba.clone());
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::R16Uint);
    assert_eq!(encoded.as_bytes().len(), w * h * 2);
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::R16Uint);
    assert_eq!(decoded.as_bytes().len(), w * h * 8);
    let dec: &[u16] = bytemuck::cast_slice(decoded.as_bytes());
    for y in 0..h {
        for x in 0..w {
            let px = (y * w + x) * 4;
            assert_eq!(dec[px], rgba[px] as u16, "R at ({},{})", x, y);
            assert_eq!(dec[px + 1], 0, "G at ({},{})", x, y);
            assert_eq!(dec[px + 2], 0, "B at ({},{})", x, y);
            assert_eq!(dec[px + 3], 0, "A at ({},{})", x, y);
        }
    }
}

#[inline(always)]
fn u8_to_i16(v: u8) -> i16 {
    (v as i16 - 128) as i16
}

#[test]
fn r16_sint_roundtrip() {
    let w = 8;
    let h = 8;
    let mut state: u32 = 43;
    let mut next_rand = || -> u8 {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        (state >> 24) as u8
    };
    let mut rgba = vec![0u8; w * h * 4];
    for px in rgba.chunks_mut(4) {
        px[0] = next_rand();
        px[1] = next_rand();
        px[2] = next_rand();
        px[3] = next_rand();
    }
    let input = PixelDatas::U8(rgba.clone());
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::R16Sint);
    assert_eq!(encoded.as_bytes().len(), w * h * 2);
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::R16Sint);
    assert_eq!(decoded.as_bytes().len(), w * h * 8);
    let dec: &[i16] = bytemuck::cast_slice(decoded.as_bytes());
    for y in 0..h {
        for x in 0..w {
            let px = (y * w + x) * 4;
            assert_eq!(dec[px], u8_to_i16(rgba[px]), "R at ({},{})", x, y);
            assert_eq!(dec[px + 1], 0, "G at ({},{})", x, y);
            assert_eq!(dec[px + 2], 0, "B at ({},{})", x, y);
            assert_eq!(dec[px + 3], 0, "A at ({},{})", x, y);
        }
    }
}

#[test]
fn r16_float_roundtrip() {
    let w = 8;
    let h = 8;
    let pixel_count = w * h;
    let mut rgba_f16 = vec![half::f16::ZERO; pixel_count * 4];
    for i in 0..pixel_count {
        let idx = i * 4;
        rgba_f16[idx] = half::f16::from_f32(0.5);
        rgba_f16[idx + 1] = half::f16::from_f32(0.25);
        rgba_f16[idx + 2] = half::f16::from_f32(0.75);
        rgba_f16[idx + 3] = half::f16::from_f32(1.0);
    }
    let input = PixelDatas::F16(rgba_f16.clone());
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::R16Float);
    // R16Float: 1 f16 per pixel
    assert_eq!(encoded.as_bytes().len(), pixel_count * 2);
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::R16Float);
    assert_eq!(decoded.as_bytes().len(), pixel_count * 8); // 4 f16 per pixel
    let dec: &[half::f16] = bytemuck::cast_slice(decoded.as_bytes());
    for i in 0..pixel_count {
        let idx = i * 4;
        assert_eq!(dec[idx], rgba_f16[idx], "R at pixel {}", i);
        assert_eq!(dec[idx + 1], half::f16::ZERO, "G at pixel {}", i);
        assert_eq!(dec[idx + 2], half::f16::ZERO, "B at pixel {}", i);
        assert_eq!(dec[idx + 3], half::f16::ZERO, "A at pixel {}", i);
    }
}

#[test]
fn rg16_uint_roundtrip() {
    let w = 8;
    let h = 8;
    let mut state: u32 = 44;
    let mut next_rand = || -> u8 {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        (state >> 24) as u8
    };
    let mut rgba = vec![0u8; w * h * 4];
    for px in rgba.chunks_mut(4) {
        px[0] = next_rand();
        px[1] = next_rand();
        px[2] = next_rand();
        px[3] = next_rand();
    }
    let input = PixelDatas::U8(rgba.clone());
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::Rg16Uint);
    assert_eq!(encoded.as_bytes().len(), w * h * 4);
    // Verify encoding preserves R and G
    let enc = encoded.as_bytes();
    for y in 0..h {
        for x in 0..w {
            let src_idx = (y * w + x) * 4;
            let enc_idx = (y * w + x) * 4;
            let r_enc = u16::from_le_bytes([enc[enc_idx], enc[enc_idx + 1]]);
            let g_enc = u16::from_le_bytes([enc[enc_idx + 2], enc[enc_idx + 3]]);
            assert_eq!(r_enc as u8, rgba[src_idx], "R at ({},{})", x, y);
            assert_eq!(g_enc as u8, rgba[src_idx + 1], "G at ({},{})", x, y);
        }
    }
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Rg16Uint);
    assert_eq!(decoded.as_bytes().len(), w * h * 8);
    let dec: &[u16] = bytemuck::cast_slice(decoded.as_bytes());
    for y in 0..h {
        for x in 0..w {
            let px = (y * w + x) * 4;
            assert_eq!(dec[px], rgba[px] as u16, "R at ({},{})", x, y);
            assert_eq!(dec[px + 1], rgba[px + 1] as u16, "G at ({},{})", x, y);
            assert_eq!(dec[px + 2], 0, "B at ({},{})", x, y);
            assert_eq!(dec[px + 3], 0, "A at ({},{})", x, y);
        }
    }
}

#[test]
fn rg16_sint_roundtrip() {
    let w = 8;
    let h = 8;
    let mut state: u32 = 45;
    let mut next_rand = || -> u8 {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        (state >> 24) as u8
    };
    let mut rgba = vec![0u8; w * h * 4];
    for px in rgba.chunks_mut(4) {
        px[0] = next_rand();
        px[1] = next_rand();
        px[2] = next_rand();
        px[3] = next_rand();
    }
    let input = PixelDatas::U8(rgba.clone());
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::Rg16Sint);
    assert_eq!(encoded.as_bytes().len(), w * h * 4);
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Rg16Sint);
    assert_eq!(decoded.as_bytes().len(), w * h * 8);
    let dec: &[i16] = bytemuck::cast_slice(decoded.as_bytes());
    for y in 0..h {
        for x in 0..w {
            let px = (y * w + x) * 4;
            assert_eq!(dec[px], u8_to_i16(rgba[px]), "R at ({},{})", x, y);
            assert_eq!(dec[px + 1], u8_to_i16(rgba[px + 1]), "G at ({},{})", x, y);
            assert_eq!(dec[px + 2], 0, "B at ({},{})", x, y);
            assert_eq!(dec[px + 3], 0, "A at ({},{})", x, y);
        }
    }
}

#[test]
fn rg16_float_roundtrip() {
    let w = 8;
    let h = 8;
    let pixel_count = w * h;
    let mut rgba_f16 = vec![half::f16::ZERO; pixel_count * 4];
    let half_one = half::f16::from_f32(1.0);
    for i in 0..pixel_count {
        let idx = i * 4;
        rgba_f16[idx] = half::f16::from_f32(0.3);
        rgba_f16[idx + 1] = half::f16::from_f32(0.6);
        rgba_f16[idx + 2] = half::f16::from_f32(0.9);
        rgba_f16[idx + 3] = half_one;
    }
    let input = PixelDatas::F16(rgba_f16.clone());
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::Rg16Float);
    // Rg16Float: 2 f16 per pixel
    assert_eq!(encoded.as_bytes().len(), pixel_count * 4);
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Rg16Float);
    assert_eq!(decoded.as_bytes().len(), pixel_count * 8);
    let dec: &[half::f16] = bytemuck::cast_slice(decoded.as_bytes());
    for i in 0..pixel_count {
        let idx = i * 4;
        assert_eq!(dec[idx], rgba_f16[idx], "R at pixel {}", i);
        assert_eq!(dec[idx + 1], rgba_f16[idx + 1], "G at pixel {}", i);
        assert_eq!(dec[idx + 2], half::f16::ZERO, "B at pixel {}", i);
        assert_eq!(dec[idx + 3], half::f16::ZERO, "A at pixel {}", i);
    }
}

#[test]
fn rgba16_uint_roundtrip() {
    let w = 8;
    let h = 8;
    let mut state: u32 = 46;
    let mut next_rand = || -> u8 {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        (state >> 24) as u8
    };
    let mut rgba = vec![0u8; w * h * 4];
    for px in rgba.chunks_mut(4) {
        px[0] = next_rand();
        px[1] = next_rand();
        px[2] = next_rand();
        px[3] = next_rand();
    }
    let input = PixelDatas::U8(rgba.clone());
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::Rgba16Uint);
    assert_eq!(encoded.as_bytes().len(), w * h * 8);
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Rgba16Uint);
    assert_eq!(decoded.as_bytes().len(), w * h * 8);
    let dec: &[u16] = bytemuck::cast_slice(decoded.as_bytes());
    for i in 0..w * h * 4 {
        assert_eq!(dec[i], rgba[i] as u16, "channel {} mismatch", i);
    }
}

#[test]
fn rgba16_sint_roundtrip() {
    let w = 8;
    let h = 8;
    let mut state: u32 = 47;
    let mut next_rand = || -> u8 {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        (state >> 24) as u8
    };
    let mut rgba = vec![0u8; w * h * 4];
    for px in rgba.chunks_mut(4) {
        px[0] = next_rand();
        px[1] = next_rand();
        px[2] = next_rand();
        px[3] = next_rand();
    }
    let input = PixelDatas::U8(rgba.clone());
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::Rgba16Sint);
    assert_eq!(encoded.as_bytes().len(), w * h * 8);
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Rgba16Sint);
    assert_eq!(decoded.as_bytes().len(), w * h * 8);
    let dec: &[i16] = bytemuck::cast_slice(decoded.as_bytes());
    for i in 0..w * h * 4 {
        assert_eq!(dec[i], u8_to_i16(rgba[i]), "channel {} mismatch", i);
    }
}

#[test]
fn rgba16_float_roundtrip() {
    let w = 8;
    let h = 8;
    let pixel_count = w * h;
    let mut rgba_f16 = vec![half::f16::ZERO; pixel_count * 4];
    for i in 0..pixel_count {
        let idx = i * 4;
        rgba_f16[idx] = half::f16::from_f32(0.1);
        rgba_f16[idx + 1] = half::f16::from_f32(0.2);
        rgba_f16[idx + 2] = half::f16::from_f32(0.3);
        rgba_f16[idx + 3] = half::f16::from_f32(0.4);
    }
    let input = PixelDatas::F16(rgba_f16.clone());
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::Rgba16Float);
    // Rgba16Float: passthrough, 4 f16 per pixel
    assert_eq!(encoded.as_bytes().len(), pixel_count * 8);
    // Verify passthrough
    let enc: &[half::f16] = bytemuck::cast_slice(encoded.as_bytes());
    assert_eq!(enc, rgba_f16.as_slice(), "Rgba16Float passthrough");
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Rgba16Float);
    assert_eq!(decoded.as_bytes().len(), pixel_count * 8);
    let dec: &[half::f16] = bytemuck::cast_slice(decoded.as_bytes());
    assert_eq!(dec, rgba_f16.as_slice(), "Rgba16Float decode");
}

#[test]
fn all_group_b_encode_decode_sizes() {
    let formats = [
        (TextureFormat::R16Uint, 2usize),
        (TextureFormat::R16Sint, 2),
        (TextureFormat::R16Float, 2), // 1 f16 = 2 bytes
        (TextureFormat::Rg16Uint, 4),
        (TextureFormat::Rg16Sint, 4),
        (TextureFormat::Rg16Float, 4), // 2 f16 = 4 bytes
        (TextureFormat::Rgba16Uint, 8),
        (TextureFormat::Rgba16Sint, 8),
        (TextureFormat::Rgba16Float, 8), // 4 f16 = 8 bytes
    ];
    for (fmt, bpp) in &formats {
        let input = match fmt.raw_pixel_type() {
            RawPixelType::U8 => PixelDatas::U8(make_test_rgba(4, 4)),
            RawPixelType::F16 => {
                let pixel_count = 4 * 4;
                let mut f16_data = vec![half::f16::ZERO; pixel_count * 4];
                for i in 0..pixel_count {
                    let idx = i * 4;
                    f16_data[idx] = half::f16::from_f32(0.5);
                    f16_data[idx + 1] = half::f16::from_f32(0.5);
                    f16_data[idx + 2] = half::f16::from_f32(0.5);
                    f16_data[idx + 3] = half::f16::from_f32(1.0);
                }
                PixelDatas::F16(f16_data)
            }
            RawPixelType::S8 => {
                let pixel_count = 4 * 4;
                let mut s8_data = vec![0i8; pixel_count * 4];
                for i in 0..pixel_count {
                    let idx = i * 4;
                    s8_data[idx] = 0;
                    s8_data[idx + 1] = 0;
                    s8_data[idx + 2] = 0;
                    s8_data[idx + 3] = i8::MAX;
                }
                PixelDatas::S8(s8_data)
            },
            RawPixelType::U16 => {
                let pixel_count = 4 * 4;
                let mut u16_data = vec![0u16; pixel_count * 4];
                for i in 0..pixel_count {
                    let idx = i * 4;
                    u16_data[idx] = u16::MAX / 2;
                    u16_data[idx + 1] = u16::MAX / 2;
                    u16_data[idx + 2] = u16::MAX / 2;
                    u16_data[idx + 3] = u16::MAX;
                }
                PixelDatas::U16(u16_data)
            },
            RawPixelType::S16 => {
                let pixel_count = 4 * 4;
                let mut s16_data = vec![0i16; pixel_count * 4];
                for i in 0..pixel_count {
                    let idx = i * 4;
                    s16_data[idx] = 0;
                    s16_data[idx + 1] = 0;
                    s16_data[idx + 2] = 0;
                    s16_data[idx + 3] = i16::MAX;
                }
                PixelDatas::S16(s16_data)
            },
            RawPixelType::F32 => {
                let pixel_count = 4 * 4;
                let mut f32_data = vec![0.0f32; pixel_count * 4];
                for i in 0..pixel_count {
                    let idx = i * 4;
                    f32_data[idx] = 0.5;
                    f32_data[idx + 1] = 0.5;
                    f32_data[idx + 2] = 0.5;
                    f32_data[idx + 3] = 1.0;
                }
                PixelDatas::F32(f32_data)
            },
        };
        let encoded = encode(&input, 4, 4, *fmt);
        assert_eq!(
            encoded.as_bytes().len(),
            4 * 4 * bpp,
            "{fmt:?} encoded size mismatch"
        );
        let decoded = decode(&encoded, 4, 4, *fmt);
        let expected_dec_size = match fmt.raw_pixel_type() {
            RawPixelType::U8 => 4 * 4 * 4,  // RGBA8
            RawPixelType::U16 => 4 * 4 * 8, // RGBA16
            RawPixelType::F16 => 4 * 4 * 8, // RGBA f16
            RawPixelType::S8 => 4 * 4 * 4,
            RawPixelType::S16 => 4 * 4 * 8,
            RawPixelType::F32 => 4 * 4 * 16,
        };
        assert_eq!(
            decoded.as_bytes().len(),
            expected_dec_size,
            "{fmt:?} decoded size mismatch"
        );
    }
}

#[test]
fn r16_sint_sign_extension() {
    // Verify that values >= 128 are sign-extended for Sint encoding
    let w = 1usize;
    let h = 1usize;
    // R=200, G=100, B=50, A=255
    let rgba = vec![200u8, 100, 50, 255];
    let input = PixelDatas::U8(rgba);
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::R16Sint);
    let enc = encoded.as_bytes();
    let r = i16::from_le_bytes([enc[0], enc[1]]);

    assert_eq!(r, 200 - 128, "R should be sign-extended");
    // Decode back preserves the sign-extended u16 value
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::R16Sint);
    let dec: &[i16] = bytemuck::cast_slice(decoded.as_bytes());
    assert_eq!(dec[0], 200 - 128, "R should be sign-extended");
    assert_eq!(dec[1], 0, "G");
    assert_eq!(dec[2], 0, "B");
    assert_eq!(dec[3], 0, "A");
}

#[test]
fn r16_uint_zero_extension() {
    // Verify Uint encoding uses zero-extension (not sign-extension)
    let w = 1usize;
    let h = 1usize;
    let rgba = vec![200u8, 100, 50, 255];
    let input = PixelDatas::U8(rgba);
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::R16Uint);
    let enc = encoded.as_bytes();
    let r = u16::from_le_bytes([enc[0], enc[1]]);
    // 200 as u16 = 200 (zero-extended, not sign-extended)
    assert_eq!(r, 200u16, "R should be zero-extended");
    // Decode back
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::R16Uint);
    let dec: &[u16] = bytemuck::cast_slice(decoded.as_bytes());
    assert_eq!(dec[0], 200, "R should be 200");
}

// ============================================================
// Golden data tests — known inputs produce known byte sequences
// ============================================================

#[test]
fn r16_uint_golden() {
    // 2×1 image: R=[128, 255], G/B/A=[0,0,0]
    let rgba = vec![128u8, 0, 0, 0, 255u8, 0, 0, 0];
    let input = PixelDatas::U8(rgba);
    let encoded = encode(&input, 2, 1, TextureFormat::R16Uint);
    // R16Uint: 2 bytes per pixel = 4 bytes total
    assert_eq!(encoded.as_bytes().len(), 4);
    let enc = encoded.as_bytes();
    // Pixel 0: R=128 → u16=128 → LE=[0x80, 0x00]
    assert_eq!(enc[0], 0x80, "R lo byte pixel 0");
    assert_eq!(enc[1], 0x00, "R hi byte pixel 0");
    // Pixel 1: R=255 → u16=255 → LE=[0xFF, 0x00]
    assert_eq!(enc[2], 0xFF, "R lo byte pixel 1");
    assert_eq!(enc[3], 0x00, "R hi byte pixel 1");
}

#[test]
fn r16_sint_golden() {
    // 2×1 image: R=[0, 255], G/B/A=[0,0,0]
    let rgba = vec![0u8, 0, 0, 0, 255u8, 0, 0, 0];
    let input = PixelDatas::U8(rgba);
    let encoded = encode(&input, 2, 1, TextureFormat::R16Sint);
    let enc = encoded.as_bytes();
    assert_eq!(enc.len(), 4);
    assert_eq!(enc[0], 0x80);
    assert_eq!(enc[1], 0xFF);
    assert_eq!(enc[2], 0x7F);
    assert_eq!(enc[3], 0x00);
}

#[test]
fn r16_float_golden() {
    // 1×1 image with known f16 values
    let rgba_f16 = vec![
        half::f16::from_f32(1.0),
        half::f16::from_f32(0.5),
        half::f16::from_f32(0.25),
        half::f16::from_f32(2.0),
    ];
    let input = PixelDatas::F16(rgba_f16);
    let encoded = encode(&input, 1, 1, TextureFormat::R16Float);
    // R16Float: 1 f16 per pixel = 2 bytes
    assert_eq!(encoded.as_bytes().len(), 2);
    let enc: &[half::f16] = bytemuck::cast_slice(encoded.as_bytes());
    assert_eq!(enc[0], half::f16::from_f32(1.0), "R channel should be 1.0");
}

#[test]
fn rgba16_uint_golden() {
    // 1×1 image with all channels set
    let rgba = vec![0x12u8, 0x34, 0xAB, 0xCD];
    let input = PixelDatas::U8(rgba);
    let encoded = encode(&input, 1, 1, TextureFormat::Rgba16Uint);
    assert_eq!(encoded.as_bytes().len(), 8);
    let enc = encoded.as_bytes();
    // R=0x12 → u16=0x12 → LE=[0x12, 0x00]
    assert_eq!(enc[0], 0x12);
    assert_eq!(enc[1], 0x00);
    // G=0x34 → u16=0x34 → LE=[0x34, 0x00]
    assert_eq!(enc[2], 0x34);
    assert_eq!(enc[3], 0x00);
    // B=0xAB → u16=0xAB → LE=[0xAB, 0x00]
    assert_eq!(enc[4], 0xAB);
    assert_eq!(enc[5], 0x00);
    // A=0xCD → u16=0xCD → LE=[0xCD, 0x00]
    assert_eq!(enc[6], 0xCD);
    assert_eq!(enc[7], 0x00);
}

#[test]
fn rgba16_float_golden() {
    // 1×1 image: passthrough should preserve exact f16 values
    let rgba_f16 = vec![
        half::f16::from_f32(0.1),
        half::f16::from_f32(0.2),
        half::f16::from_f32(0.3),
        half::f16::from_f32(0.4),
    ];
    let input = PixelDatas::F16(rgba_f16.clone());
    let encoded = encode(&input, 1, 1, TextureFormat::Rgba16Float);
    assert_eq!(encoded.as_bytes().len(), 8);
    let enc: &[half::f16] = bytemuck::cast_slice(encoded.as_bytes());
    assert_eq!(enc[0], half::f16::from_f32(0.1), "R");
    assert_eq!(enc[1], half::f16::from_f32(0.2), "G");
    assert_eq!(enc[2], half::f16::from_f32(0.3), "B");
    assert_eq!(enc[3], half::f16::from_f32(0.4), "A");
}

/// Run a roundtrip encode→decode for a format, with verification.
fn stress_roundtrip_int(
    fmt: TextureFormat,
    w: usize,
    h: usize,
    bpp: usize, // bytes per pixel in encoded form
    fill_g: bool,
    fill_b: bool,
    fill_a: bool,
    sign_extend: bool,
) {
    let pixel_count = w * h;
    let mut state: u32 = 12345;
    let mut next_rand = || -> u8 {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        (state >> 24) as u8
    };
    let mut rgba = vec![0u8; pixel_count * 4];
    for px in rgba.chunks_mut(4) {
        px[0] = next_rand();
        px[1] = next_rand();
        px[2] = next_rand();
        px[3] = next_rand();
    }
    let input = PixelDatas::U8(rgba.clone());
    let encoded = encode(&input, w as u32, h as u32, fmt);
    assert_eq!(
        encoded.as_bytes().len(),
        pixel_count * bpp,
        "{fmt:?} encoded size mismatch at {w}x{h}"
    );
    let decoded = decode(&encoded, w as u32, h as u32, fmt);
    assert_eq!(
        decoded.as_bytes().len(),
        pixel_count * 8,
        "{fmt:?} decoded size mismatch at {w}x{h}"
    );
    let dec: &[u16] = bytemuck::cast_slice(decoded.as_bytes());
    for y in 0..h {
        for x in 0..w {
            let px = (y * w + x) * 4;
            let expected = |v: u8| -> u16 { if sign_extend { u8_to_i16(v) as u16 } else { v as u16 } };
            assert_eq!(
                dec[px],
                expected(rgba[px]),
                "R at ({},{}) fmt={fmt:?}",
                x,
                y
            );
            if fill_g {
                assert_eq!(
                    dec[px + 1],
                    expected(rgba[px + 1]),
                    "G at ({},{}) fmt={fmt:?}",
                    x,
                    y
                );
            } else {
                assert_eq!(dec[px + 1], 0, "G at ({},{}) fmt={fmt:?}", x, y);
            }
            if fill_b {
                assert_eq!(
                    dec[px + 2],
                    expected(rgba[px + 2]),
                    "B at ({},{}) fmt={fmt:?}",
                    x,
                    y
                );
            } else {
                assert_eq!(dec[px + 2], 0, "B at ({},{}) fmt={fmt:?}", x, y);
            }
            if fill_a {
                assert_eq!(
                    dec[px + 3],
                    expected(rgba[px + 3]),
                    "A at ({},{}) fmt={fmt:?}",
                    x,
                    y
                );
            } else {
                assert_eq!(dec[px + 3], 0, "A at ({},{}) fmt={fmt:?}", x, y);
            }
        }
    }
}

fn stress_roundtrip_f16(fmt: TextureFormat, w: usize, h: usize) {
    let pixel_count = w * h;
    let mut rgba_f16 = vec![half::f16::ZERO; pixel_count * 4];
    for i in 0..pixel_count {
        let idx = i * 4;
        rgba_f16[idx] = half::f16::from_f32(0.1 * (i % 10) as f32);
        rgba_f16[idx + 1] = half::f16::from_f32(0.2 * ((i + 1) % 10) as f32);
        rgba_f16[idx + 2] = half::f16::from_f32(0.3 * ((i + 2) % 10) as f32);
        rgba_f16[idx + 3] = half::f16::from_f32(1.0);
    }
    let input = PixelDatas::F16(rgba_f16.clone());
    let encoded = encode(&input, w as u32, h as u32, fmt);
    let decoded = decode(&encoded, w as u32, h as u32, fmt);
    assert_eq!(
        decoded.as_bytes().len(),
        pixel_count * 8,
        "{fmt:?} decoded size mismatch at {w}x{h}"
    );
}

const STRESS_SIZES: &[(usize, usize)] = &[
    (100, 113),   // not a multiple of 1024 or 4096
    (17, 251),    // prime-ish
    (256, 256),   // 65536 pixels, multi-chunk
    (127, 127),   // odd
    (300, 300),   // 90000 pixels
    (500, 1),     // wide strip
    (1, 500),     // tall strip
    (2048, 2048), // large multi-chunk
];

#[test]
fn r16_uint_stress() {
    for &(w, h) in STRESS_SIZES {
        stress_roundtrip_int(TextureFormat::R16Uint, w, h, 2, false, false, false, false);
    }
}

#[test]
fn r16_sint_stress() {
    for &(w, h) in STRESS_SIZES {
        stress_roundtrip_int(TextureFormat::R16Sint, w, h, 2, false, false, false, true);
    }
}

#[test]
fn r16_float_stress() {
    for &(w, h) in STRESS_SIZES {
        stress_roundtrip_f16(TextureFormat::R16Float, w, h);
    }
}

#[test]
fn rg16_uint_stress() {
    for &(w, h) in STRESS_SIZES {
        stress_roundtrip_int(TextureFormat::Rg16Uint, w, h, 4, true, false, false, false);
    }
}

#[test]
fn rg16_sint_stress() {
    for &(w, h) in STRESS_SIZES {
        stress_roundtrip_int(TextureFormat::Rg16Sint, w, h, 4, true, false, false, true);
    }
}

#[test]
fn rg16_float_stress() {
    for &(w, h) in STRESS_SIZES {
        stress_roundtrip_f16(TextureFormat::Rg16Float, w, h);
    }
}

#[test]
fn rgba16_uint_stress() {
    for &(w, h) in STRESS_SIZES {
        stress_roundtrip_int(TextureFormat::Rgba16Uint, w, h, 8, true, true, true, false);
    }
}

#[test]
fn rgba16_sint_stress() {
    for &(w, h) in STRESS_SIZES {
        stress_roundtrip_int(TextureFormat::Rgba16Sint, w, h, 8, true, true, true, true);
    }
}

#[test]
fn rgba16_float_stress() {
    for &(w, h) in STRESS_SIZES {
        stress_roundtrip_f16(TextureFormat::Rgba16Float, w, h);
    }
}

// ============================================================
// Cross-variant tests: U8→float encode (the reported OOB path)
// ============================================================

#[test]
fn r16_float_from_uint8() {
    // RGBA8 → R16Float: U8 input to float encode (the crash scenario)
    let w = 256;
    let h = 256;
    let mut rgba8 = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 4;
            rgba8[i] = (x % 256) as u8;
            rgba8[i + 1] = (y % 256) as u8;
            rgba8[i + 2] = 128;
            rgba8[i + 3] = 255;
        }
    }
    let input = PixelDatas::U8(rgba8);
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::R16Float);
    // R16Float: 1 f16 per pixel = 2 bytes per pixel
    assert_eq!(encoded.as_bytes().len(), w * h * 2);
    // Decode back
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::R16Float);
    assert_eq!(decoded.as_bytes().len(), w * h * 8);
}

#[test]
fn rg16_float_from_uint8() {
    let w = 128;
    let h = 128;
    let mut rgba8 = vec![0u8; w * h * 4];
    for i in (0..rgba8.len()).step_by(4) {
        rgba8[i] = 100;
        rgba8[i + 1] = 200;
        rgba8[i + 2] = 50;
        rgba8[i + 3] = 255;
    }
    let input = PixelDatas::U8(rgba8);
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::Rg16Float);
    assert_eq!(encoded.as_bytes().len(), w * h * 4);
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Rg16Float);
    assert_eq!(decoded.as_bytes().len(), w * h * 8);
}

#[test]
fn rgba16_float_from_uint8() {
    let w = 64;
    let h = 64;
    let mut rgba8 = vec![0u8; w * h * 4];
    for i in (0..rgba8.len()).step_by(4) {
        rgba8[i] = 10;
        rgba8[i + 1] = 20;
        rgba8[i + 2] = 30;
        rgba8[i + 3] = 255;
    }
    let input = PixelDatas::U8(rgba8);
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::Rgba16Float);
    assert_eq!(encoded.as_bytes().len(), w * h * 8);
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Rgba16Float);
    assert_eq!(decoded.as_bytes().len(), w * h * 8);
}

#[test]
fn to_rgba8_converts_f16() {
    // F16 → U8 conversion with clamping
    let f16_data = vec![
        half::f16::from_f32(-0.5), // clamped to 0
        half::f16::from_f32(0.0),
        half::f16::from_f32(0.5), // → 128
        half::f16::from_f32(1.0), // → 255
        half::f16::from_f32(2.0), // clamped to 255
    ];
    let pixels = PixelDatas::F16(f16_data);
    let rgba8 = pixels.convert_to_u8();
    match rgba8 {
        PixelDatas::U8(data) => {
            assert_eq!(data.len(), 5);
            assert_eq!(data[0], 0, "-0.5 → 0");
            assert_eq!(data[1], 0, "0.0 → 0");
            assert_eq!(data[2], 128, "0.5 → 128");
            assert_eq!(data[3], 255, "1.0 → 255");
            assert_eq!(data[4], 255, "2.0 → 255");
        }
        _ => panic!("expected U8 variant"),
    }
}

#[test]
fn to_rgba8_passthrough_u8() {
    let u8_data = vec![100u8, 150, 200, 255];
    let pixels = PixelDatas::U8(u8_data.clone());
    let rgba8 = pixels.convert_to_u8();
    match rgba8 {
        PixelDatas::U8(data) => assert_eq!(data, u8_data),
        _ => panic!("expected U8 variant"),
    }
}

// ============================================================
// Group C: Packed + f32 format tests (Rgb10a2, Rg11b10, R32/Rg32/Rgba32)
// ============================================================

#[test]
fn r32_uint_roundtrip() {
    let w = 8;
    let h = 8;
    let pixel_count = w * h;
    let mut state: u32 = 42;
    let mut next_rand = || -> u8 {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        (state >> 24) as u8
    };
    let mut rgba = vec![0u8; pixel_count * 4];
    for px in rgba.chunks_mut(4) {
        px[0] = next_rand();
        px[1] = next_rand();
        px[2] = next_rand();
        px[3] = next_rand();
    }
    let input = PixelDatas::U8(rgba.clone());
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::R32Uint);
    assert_eq!(encoded.as_bytes().len(), pixel_count * 4);
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::R32Uint);
    assert_eq!(decoded.as_bytes().len(), pixel_count * 16);
    let dec: &[f32] = bytemuck::cast_slice(decoded.as_bytes());
    for i in 0..pixel_count {
        let idx = i * 4;
        assert_eq!(dec[idx], rgba[idx] as f32, "R at pixel {}", i);
        assert_eq!(dec[idx + 1], 0.0, "G at pixel {}", i);
        assert_eq!(dec[idx + 2], 0.0, "B at pixel {}", i);
        assert_eq!(dec[idx + 3], 0.0, "A at pixel {}", i);
    }
}

#[test]
fn r32_sint_roundtrip() {
    let w = 8;
    let h = 8;
    let pixel_count = w * h;
    let mut state: u32 = 43;
    let mut next_rand = || -> u8 {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        (state >> 24) as u8
    };
    let mut rgba = vec![0u8; pixel_count * 4];
    for px in rgba.chunks_mut(4) {
        px[0] = next_rand();
        px[1] = next_rand();
        px[2] = next_rand();
        px[3] = next_rand();
    }
    let input = PixelDatas::U8(rgba.clone());
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::R32Sint);
    assert_eq!(encoded.as_bytes().len(), pixel_count * 4, "R32Sint encoded size");
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::R32Sint);
    assert_eq!(decoded.as_bytes().len(), pixel_count * 16, "R32Sint decoded size");
    let dec: &[f32] = bytemuck::cast_slice(decoded.as_bytes());
    for i in 0..pixel_count {
        let idx = i * 4;
        let expected_r = (rgba[idx] as i16 - 128) as f32;
        assert_eq!(dec[idx], expected_r, "R at pixel {} (expected {})", i, expected_r);
        assert_eq!(dec[idx + 1], 0.0, "G at pixel {}", i);
        assert_eq!(dec[idx + 2], 0.0, "B at pixel {}", i);
        assert_eq!(dec[idx + 3], 0.0, "A at pixel {}", i);
    }
}

#[test]
fn r32_float_roundtrip() {
    let w = 8;
    let h = 8;
    let pixel_count = w * h;
    let mut rgba_f32 = vec![0.0f32; pixel_count * 4];
    for i in 0..pixel_count {
        let idx = i * 4;
        rgba_f32[idx] = 0.1 * (i % 10) as f32;
        rgba_f32[idx + 1] = 0.2 * ((i + 1) % 10) as f32;
        rgba_f32[idx + 2] = 0.3 * ((i + 2) % 10) as f32;
        rgba_f32[idx + 3] = 1.0;
    }
    let input = PixelDatas::F32(rgba_f32.clone());
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::R32Float);
    assert_eq!(encoded.as_bytes().len(), pixel_count * 4, "R32Float encoded size");
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::R32Float);
    assert_eq!(decoded.as_bytes().len(), pixel_count * 16, "R32Float decoded size");
    let dec: &[f32] = bytemuck::cast_slice(decoded.as_bytes());
    for i in 0..pixel_count {
        let idx = i * 4;
        assert!((dec[idx] - rgba_f32[idx]).abs() < 1e-6, "R at pixel {}", i);
        assert_eq!(dec[idx + 1], 0.0, "G at pixel {}", i);
        assert_eq!(dec[idx + 2], 0.0, "B at pixel {}", i);
        assert!((dec[idx + 3] - 1.0).abs() < 1e-6, "A at pixel {}", i);
    }
}

#[test]
fn rg32_uint_roundtrip() {
    let w = 8;
    let h = 8;
    let pixel_count = w * h;
    let mut state: u32 = 44;
    let mut next_rand = || -> u8 {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        (state >> 24) as u8
    };
    let mut rgba = vec![0u8; pixel_count * 4];
    for px in rgba.chunks_mut(4) {
        px[0] = next_rand();
        px[1] = next_rand();
        px[2] = next_rand();
        px[3] = next_rand();
    }
    let input = PixelDatas::U8(rgba.clone());
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::Rg32Uint);
    assert_eq!(encoded.as_bytes().len(), pixel_count * 8, "Rg32Uint encoded size");
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Rg32Uint);
    assert_eq!(decoded.as_bytes().len(), pixel_count * 16, "Rg32Uint decoded size");
    let dec: &[f32] = bytemuck::cast_slice(decoded.as_bytes());
    for i in 0..pixel_count {
        let idx = i * 4;
        assert_eq!(dec[idx], rgba[idx] as f32, "R at pixel {}", i);
        assert_eq!(dec[idx + 1], rgba[idx + 1] as f32, "G at pixel {}", i);
        assert_eq!(dec[idx + 2], 0.0, "B at pixel {}", i);
        assert_eq!(dec[idx + 3], 0.0, "A at pixel {}", i);
    }
}

#[test]
fn rg32_sint_roundtrip() {
    let w = 8;
    let h = 8;
    let pixel_count = w * h;
    let mut state: u32 = 45;
    let mut next_rand = || -> u8 {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        (state >> 24) as u8
    };
    let mut rgba = vec![0u8; pixel_count * 4];
    for px in rgba.chunks_mut(4) {
        px[0] = next_rand();
        px[1] = next_rand();
        px[2] = next_rand();
        px[3] = next_rand();
    }
    let input = PixelDatas::U8(rgba.clone());
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::Rg32Sint);
    assert_eq!(encoded.as_bytes().len(), pixel_count * 8, "Rg32Sint encoded size");
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Rg32Sint);
    assert_eq!(decoded.as_bytes().len(), pixel_count * 16, "Rg32Sint decoded size");
    let dec: &[f32] = bytemuck::cast_slice(decoded.as_bytes());
    for i in 0..pixel_count {
        let idx = i * 4;
        let expected_r = (rgba[idx] as i16 - 128) as f32;
        let expected_g = (rgba[idx + 1] as i16 - 128) as f32;
        assert_eq!(dec[idx], expected_r, "R at pixel {}", i);
        assert_eq!(dec[idx + 1], expected_g, "G at pixel {}", i);
        assert_eq!(dec[idx + 2], 0.0, "B at pixel {}", i);
        assert_eq!(dec[idx + 3], 0.0, "A at pixel {}", i);
    }
}

#[test]
fn rg32_float_roundtrip() {
    let w = 8;
    let h = 8;
    let pixel_count = w * h;
    let mut rgba_f32 = vec![0.0f32; pixel_count * 4];
    for i in 0..pixel_count {
        let idx = i * 4;
        rgba_f32[idx] = 0.1 * (i % 10) as f32;
        rgba_f32[idx + 1] = 0.2 * ((i + 1) % 10) as f32;
        rgba_f32[idx + 2] = 0.3 * ((i + 2) % 10) as f32;
        rgba_f32[idx + 3] = 1.0;
    }
    let input = PixelDatas::F32(rgba_f32.clone());
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::Rg32Float);
    assert_eq!(encoded.as_bytes().len(), pixel_count * 8, "Rg32Float encoded size");
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Rg32Float);
    assert_eq!(decoded.as_bytes().len(), pixel_count * 16, "Rg32Float decoded size");
    let dec: &[f32] = bytemuck::cast_slice(decoded.as_bytes());
    for i in 0..pixel_count {
        let idx = i * 4;
        assert!((dec[idx] - rgba_f32[idx]).abs() < 1e-6, "R at pixel {}", i);
        assert!((dec[idx + 1] - rgba_f32[idx + 1]).abs() < 1e-6, "G at pixel {}", i);
        assert_eq!(dec[idx + 2], 0.0, "B at pixel {}", i);
        assert!((dec[idx + 3] - 1.0).abs() < 1e-6, "A at pixel {}", i);
    }
}

#[test]
fn rgba32_uint_roundtrip() {
    let w = 8;
    let h = 8;
    let pixel_count = w * h;
    let mut state: u32 = 46;
    let mut next_rand = || -> u8 {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        (state >> 24) as u8
    };
    let mut rgba = vec![0u8; pixel_count * 4];
    for px in rgba.chunks_mut(4) {
        px[0] = next_rand();
        px[1] = next_rand();
        px[2] = next_rand();
        px[3] = next_rand();
    }
    let input = PixelDatas::U8(rgba.clone());
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::Rgba32Uint);
    assert_eq!(encoded.as_bytes().len(), pixel_count * 16, "Rgba32Uint encoded size");
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Rgba32Uint);
    assert_eq!(decoded.as_bytes().len(), pixel_count * 16, "Rgba32Uint decoded size");
    let dec: &[f32] = bytemuck::cast_slice(decoded.as_bytes());
    for i in 0..pixel_count {
        let idx = i * 4;
        assert_eq!(dec[idx], rgba[idx] as f32, "R at pixel {}", i);
        assert_eq!(dec[idx + 1], rgba[idx + 1] as f32, "G at pixel {}", i);
        assert_eq!(dec[idx + 2], rgba[idx + 2] as f32, "B at pixel {}", i);
        assert_eq!(dec[idx + 3], rgba[idx + 3] as f32, "A at pixel {}", i);
    }
}

#[test]
fn rgba32_sint_roundtrip() {
    let w = 8;
    let h = 8;
    let pixel_count = w * h;
    let mut state: u32 = 47;
    let mut next_rand = || -> u8 {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        (state >> 24) as u8
    };
    let mut rgba = vec![0u8; pixel_count * 4];
    for px in rgba.chunks_mut(4) {
        px[0] = next_rand();
        px[1] = next_rand();
        px[2] = next_rand();
        px[3] = next_rand();
    }
    let input = PixelDatas::U8(rgba.clone());
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::Rgba32Sint);
    assert_eq!(encoded.as_bytes().len(), pixel_count * 16, "Rgba32Sint encoded size");
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Rgba32Sint);
    assert_eq!(decoded.as_bytes().len(), pixel_count * 16, "Rgba32Sint decoded size");
    let dec: &[f32] = bytemuck::cast_slice(decoded.as_bytes());
    for i in 0..pixel_count {
        let idx = i * 4;
        let expected_r = (rgba[idx] as i16 - 128) as f32;
        let expected_g = (rgba[idx + 1] as i16 - 128) as f32;
        let expected_b = (rgba[idx + 2] as i16 - 128) as f32;
        let expected_a = (rgba[idx + 3] as i16 - 128) as f32;
        assert_eq!(dec[idx], expected_r, "R at pixel {}", i);
        assert_eq!(dec[idx + 1], expected_g, "G at pixel {}", i);
        assert_eq!(dec[idx + 2], expected_b, "B at pixel {}", i);
        assert_eq!(dec[idx + 3], expected_a, "A at pixel {}", i);
    }
}

#[test]
fn rgba32_float_roundtrip() {
    let w = 8;
    let h = 8;
    let pixel_count = w * h;
    let mut rgba_f32 = vec![0.0f32; pixel_count * 4];
    for i in 0..pixel_count {
        let idx = i * 4;
        rgba_f32[idx] = 0.1;
        rgba_f32[idx + 1] = 0.2;
        rgba_f32[idx + 2] = 0.3;
        rgba_f32[idx + 3] = 0.4;
    }
    let input = PixelDatas::F32(rgba_f32.clone());
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::Rgba32Float);
    assert_eq!(encoded.as_bytes().len(), pixel_count * 16, "Rgba32Float encoded size");
    let enc: &[f32] = bytemuck::cast_slice(encoded.as_bytes());
    assert_eq!(enc, rgba_f32.as_slice(), "Rgba32Float passthrough");
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Rgba32Float);
    assert_eq!(decoded.as_bytes().len(), pixel_count * 16, "Rgba32Float decoded size");
    let dec: &[f32] = bytemuck::cast_slice(decoded.as_bytes());
    assert_eq!(dec, rgba_f32.as_slice(), "Rgba32Float decode");
}

#[test]
fn rgb10a2_unorm_roundtrip() {
    let w = 8;
    let h = 8;
    let pixel_count = w * h;
    let mut rgba_f32 = vec![0.0f32; pixel_count * 4];
    for i in 0..pixel_count {
        let idx = i * 4;
        rgba_f32[idx] = (i % 10) as f32 / 10.0;   // R
        rgba_f32[idx + 1] = ((i + 3) % 10) as f32 / 10.0; // G
        rgba_f32[idx + 2] = ((i + 7) % 10) as f32 / 10.0; // B
        rgba_f32[idx + 3] = ((i % 4) as f32) / 3.0; // A in 0..1 (2-bit)
    }
    let input = PixelDatas::F32(rgba_f32.clone());
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::Rgb10a2Unorm);
    assert_eq!(encoded.as_bytes().len(), pixel_count * 4, "Rgb10a2Unorm encoded size");
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Rgb10a2Unorm);
    assert_eq!(decoded.as_bytes().len(), pixel_count * 16, "Rgb10a2Unorm decoded size");
    let dec: &[f32] = bytemuck::cast_slice(decoded.as_bytes());
    for i in 0..pixel_count {
        let idx = i * 4;
        // 10-bit precision: allow 1/1023 ≈ 0.001 tolerance
        let r_quant = (rgba_f32[idx] * 1023.0).round() / 1023.0;
        let g_quant = (rgba_f32[idx + 1] * 1023.0).round() / 1023.0;
        let b_quant = (rgba_f32[idx + 2] * 1023.0).round() / 1023.0;
        let a_quant = (rgba_f32[idx + 3] * 3.0).round() / 3.0;
        assert!((dec[idx] - r_quant).abs() < 0.002, "R at pixel {}: got {}, expected {}", i, dec[idx], r_quant);
        assert!((dec[idx + 1] - g_quant).abs() < 0.002, "G at pixel {}", i);
        assert!((dec[idx + 2] - b_quant).abs() < 0.002, "B at pixel {}", i);
        assert!((dec[idx + 3] - a_quant).abs() < 0.002, "A at pixel {}", i);
    }
}

#[test]
fn rg11b10_ufloat_roundtrip() {
    let w = 8;
    let h = 8;
    let pixel_count = w * h;
    let mut rgba_f32 = vec![0.0f32; pixel_count * 4];
    for i in 0..pixel_count {
        let idx = i * 4;
        rgba_f32[idx] = 0.5 + (i % 10) as f32 * 0.05;
        rgba_f32[idx + 1] = 0.3 + ((i + 2) % 10) as f32 * 0.05;
        rgba_f32[idx + 2] = 1.0 - (i % 10) as f32 * 0.03;
        rgba_f32[idx + 3] = 1.0;
    }
    let input = PixelDatas::F32(rgba_f32.clone());
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::Rg11b10Ufloat);
    assert_eq!(encoded.as_bytes().len(), pixel_count * 4, "Rg11b10Ufloat encoded size");
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Rg11b10Ufloat);
    assert_eq!(decoded.as_bytes().len(), pixel_count * 16, "Rg11b10Ufloat decoded size");
    let dec: &[f32] = bytemuck::cast_slice(decoded.as_bytes());
    for i in 0..pixel_count {
        let idx = i * 4;
        // 6-bit mantissa for R/G = 1/64 ≈ 0.016, 5-bit for B = 1/32 ≈ 0.031
        // Use generous tolerance for packed float precision
        assert!((dec[idx] - rgba_f32[idx]).abs() < 0.02, "R at pixel {}: got {}, expected {}", i, dec[idx], rgba_f32[idx]);
        assert!((dec[idx + 1] - rgba_f32[idx + 1]).abs() < 0.02, "G at pixel {}: got {}, expected {}", i, dec[idx + 1], rgba_f32[idx + 1]);
        assert!((dec[idx + 2] - rgba_f32[idx + 2]).abs() < 0.04, "B at pixel {}: got {}, expected {}", i, dec[idx + 2], rgba_f32[idx + 2]);
        assert!((dec[idx + 3] - 1.0).abs() < 1e-6, "A at pixel {}", i);
    }
}

// ============================================================
// Group C: Golden data tests (bit patterns for packed formats)
// ============================================================

#[test]
fn r32_uint_golden() {
    // 2×1 image: R=[128, 255], G/B/A=[0,0,0]
    let rgba = vec![128u8, 0, 0, 0, 255u8, 0, 0, 0];
    let input = PixelDatas::U8(rgba);
    let encoded = encode(&input, 2, 1, TextureFormat::R32Uint);
    assert_eq!(encoded.as_bytes().len(), 8); // 2 pixels × 4 bytes
    let enc = encoded.as_bytes();
    // Pixel 0: R=128 → u32=128 → LE=[0x80, 0x00, 0x00, 0x00]
    assert_eq!(enc[0], 0x80);
    assert_eq!(enc[1], 0x00);
    assert_eq!(enc[2], 0x00);
    assert_eq!(enc[3], 0x00);
    // Pixel 1: R=255 → u32=255 → LE=[0xFF, 0x00, 0x00, 0x00]
    assert_eq!(enc[4], 0xFF);
    assert_eq!(enc[5], 0x00);
    assert_eq!(enc[6], 0x00);
    assert_eq!(enc[7], 0x00);
}

#[test]
fn rgb10a2_unorm_golden() {
    // 1×1: R=1.0, G=0.5, B=0.25, A=1.0
    // R=1023, G=512, B=256, A=3
    // packed = 3<<30 | 256<<20 | 512<<10 | 1023 = 0xC410_83FF
    let rgba_f32 = vec![1.0f32, 0.5, 0.25, 1.0];
    let input = PixelDatas::F32(rgba_f32);
    let encoded = encode(&input, 1, 1, TextureFormat::Rgb10a2Unorm);
    assert_eq!(encoded.as_bytes().len(), 4);
    let enc = encoded.as_bytes();
    // Verify the packed u32 bit pattern
    let packed = u32::from_le_bytes([enc[0], enc[1], enc[2], enc[3]]);
    let expected = (3u32 << 30) | (256 << 20) | (512 << 10) | 1023;
    assert_eq!(packed, expected, "RGB10A2 packed = 0x{:08X}, expected 0x{:08X}", packed, expected);
}

#[test]
fn rg11b10_ufloat_golden() {
    // 1×1: Known values that produce predictable bit patterns
    // R=1.0 → f16=0x3C00 → uf11 = 0_01111_000000 = 0x1E0
    // G=0.5 → f16=0x3800 → uf11 = 0_01110_000000 = 0x1C0  (wait: 0.5 in f16 = 0x3800 = 0_01110_0000000000)
    // B=2.0 → f16=0x4000 → uf10 = 0_10000_00000 = 0x200
    let test_val = 1.0f32;
    let f = half::f16::from_f32(test_val);
    let f_bits = f.to_bits();
    let exp = (f_bits >> 10) & 0x1F;
    let mant = (f_bits & 0x3FF) >> 4;
    let uf11_val = (exp << 6) | mant as u16;
    // For 1.0: f16=0x3C00 (exp=15, mant=0), uf11=(15<<6 | 0) = 0x3C0
    assert_eq!(uf11_val, 0x3C0, "f32(1.0) → uf11 = 0x{:03X}", uf11_val);

    // Now pack R=1.0, G=0.5, B=2.0
    let rgba_f32 = vec![1.0f32, 0.5, 2.0, 1.0];
    let input = PixelDatas::F32(rgba_f32);
    let encoded = encode(&input, 1, 1, TextureFormat::Rg11b10Ufloat);
    assert_eq!(encoded.as_bytes().len(), 4);
    let enc = encoded.as_bytes();
    let packed = u32::from_le_bytes([enc[0], enc[1], enc[2], enc[3]]);

    // G=0.5 → f16=0x3800 → exp=14, mant=0 → uf11 = (14<<6) | 0 = 0x380
    let g_f = half::f16::from_f32(0.5);
    let g_exp = (g_f.to_bits() >> 10) & 0x1F;
    let g_mant = (g_f.to_bits() & 0x3FF) >> 4;
    let g_uf11 = (g_exp << 6) | g_mant as u16;

    // B=2.0 → f16=0x4000 → exp=16, mant=0 → uf10 = (16<<5) | 0 = 0x200
    let b_f = half::f16::from_f32(2.0);
    let b_exp = (b_f.to_bits() >> 10) & 0x1F;
    let b_mant = (b_f.to_bits() & 0x3FF) >> 5;
    let b_uf10 = (b_exp << 5) | b_mant as u16;

    let expected_packed = (uf11_val as u32) | ((g_uf11 as u32) << 11) | ((b_uf10 as u32) << 22);
    assert_eq!(packed, expected_packed, "RG11B10 packed = 0x{:08X}, expected 0x{:08X}", packed, expected_packed);
}

// ============================================================
// Group C: supports_encoding
// ============================================================

#[test]
fn all_group_c_supports_encoding() {
    let formats = [
        TextureFormat::R32Uint,
        TextureFormat::R32Sint,
        TextureFormat::R32Float,
        TextureFormat::Rg32Uint,
        TextureFormat::Rg32Sint,
        TextureFormat::Rg32Float,
        TextureFormat::Rgba32Uint,
        TextureFormat::Rgba32Sint,
        TextureFormat::Rgba32Float,
        TextureFormat::Rgb10a2Unorm,
        TextureFormat::Rg11b10Ufloat,
    ];
    for fmt in &formats {
        assert!(fmt.supports_encoding(), "{fmt:?} should support encoding");
    }
}

// ============================================================
// Group C: All formats roundtrip sizes
// ============================================================

#[test]
fn all_group_c_encode_decode_sizes() {
    let formats = [
        (TextureFormat::R32Uint, 4usize),
        (TextureFormat::R32Sint, 4),
        (TextureFormat::R32Float, 4),
        (TextureFormat::Rg32Uint, 8),
        (TextureFormat::Rg32Sint, 8),
        (TextureFormat::Rg32Float, 8),
        (TextureFormat::Rgba32Uint, 16),
        (TextureFormat::Rgba32Sint, 16),
        (TextureFormat::Rgba32Float, 16),
        (TextureFormat::Rgb10a2Unorm, 4),
        (TextureFormat::Rg11b10Ufloat, 4),
    ];
    for (fmt, bpp) in &formats {
        let input = match fmt.raw_pixel_type() {
            RawPixelType::U8 => PixelDatas::U8(make_test_rgba(4, 4)),
            RawPixelType::F32 => {
                let pixel_count = 4 * 4;
                let mut f32_data = vec![0.0f32; pixel_count * 4];
                for i in 0..pixel_count {
                    let idx = i * 4;
                    f32_data[idx] = 0.5;
                    f32_data[idx + 1] = 0.5;
                    f32_data[idx + 2] = 0.5;
                    f32_data[idx + 3] = 1.0;
                }
                PixelDatas::F32(f32_data)
            },
            _ => PixelDatas::U8(make_test_rgba(4, 4)),
        };
        let encoded = encode(&input, 4, 4, *fmt);
        assert_eq!(
            encoded.as_bytes().len(),
            4 * 4 * bpp,
            "{fmt:?} encoded size mismatch"
        );
        let decoded = decode(&encoded, 4, 4, *fmt);
        // All Group C formats decode to 4 f32 per pixel = 16 bytes per pixel
        assert_eq!(
            decoded.as_bytes().len(),
            4 * 4 * 16,
            "{fmt:?} decoded size mismatch"
        );
    }
}

// ============================================================
// Group D: BC6h + BC7 encode/decode
// ============================================================

/// Create a 4×4 HDR half-float test image with known values.
fn make_test_f16(w: usize, h: usize) -> Vec<f16> {
    let mut data = vec![f16::ZERO; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 4;
            data[i] = f16::from_f32(x as f32 / w.max(1) as f32);
            data[i + 1] = f16::from_f32(y as f32 / h.max(1) as f32);
            data[i + 2] = f16::from_f32(0.5);
            data[i + 3] = f16::from_f32(1.0);
        }
    }
    data
}

#[test]
fn bc6h_roundtrip_4x4() {
    let w = 4usize;
    let h = 4usize;
    let f16_data = make_test_f16(w, h);
    let input = PixelDatas::F16(f16_data);
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::Bc6hRgbUfloat);
    // 4×4 → one 4×4 block → 16 bytes
    assert_eq!(encoded.as_bytes().len(), 16, "BC6h 4x4 should produce 16 bytes");
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Bc6hRgbUfloat);
    assert_eq!(decoded.as_bytes().len(), w * h * 8, "BC6h decoded should be 8 bytes per pixel");
}

#[test]
fn bc6h_signed_roundtrip_4x4() {
    let w = 4usize;
    let h = 4usize;
    // Use signed test data with negative values
    let mut f16_data = vec![f16::ZERO; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 4;
            f16_data[i] = f16::from_f32((x as f32 / w as f32) * 2.0 - 1.0);
            f16_data[i + 1] = f16::from_f32((y as f32 / h as f32) * 2.0 - 1.0);
            f16_data[i + 2] = f16::from_f32(0.0);
            f16_data[i + 3] = f16::from_f32(1.0);
        }
    }
    let input = PixelDatas::F16(f16_data);
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::Bc6hRgbFloat);
    assert_eq!(encoded.as_bytes().len(), 16);
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Bc6hRgbFloat);
    assert_eq!(decoded.as_bytes().len(), w * h * 8);
}

#[test]
fn bc6h_roundtrip_8x8() {
    let w = 8usize;
    let h = 8usize;
    let f16_data = make_test_f16(w, h);
    let input = PixelDatas::F16(f16_data);
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::Bc6hRgbUfloat);
    // 8x8 → 2×2 = 4 blocks, 16 bytes each → 64 bytes
    assert_eq!(encoded.as_bytes().len(), 64);
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Bc6hRgbUfloat);
    assert_eq!(decoded.as_bytes().len(), w * h * 8);
}

#[test]
fn bc6h_encode_variant() {
    let w = 4usize;
    let h = 4usize;
    let f16_data = make_test_f16(w, h);
    let input = PixelDatas::F16(f16_data);
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::Bc6hRgbUfloat);
    // BC6h encode returns F16 (raw bytes reinterpreted as f16 for storage)
    assert!(matches!(encoded, PixelDatas::F16(_)), "BC6h encode should return F16");
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Bc6hRgbUfloat);
    // BC6h decode returns F16
    assert!(matches!(decoded, PixelDatas::F16(_)));
}

#[test]
fn bc7_roundtrip_4x4() {
    let w = 4usize;
    let h = 4usize;
    let rgba = make_test_rgba(w, h);
    let input = PixelDatas::U8(rgba);
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::Bc7RgbaUnorm);
    assert_eq!(encoded.as_bytes().len(), 16, "BC7 4x4 should produce 16 bytes");
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Bc7RgbaUnorm);
    assert_eq!(decoded.as_bytes().len(), w * h * 4);
}

#[test]
fn bc7_roundtrip_8x8() {
    let w = 8usize;
    let h = 8usize;
    let rgba = make_test_rgba(w, h);
    let input = PixelDatas::U8(rgba);
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::Bc7RgbaUnorm);
    // 8x8 → 2×2 = 4 blocks, 16 bytes each → 64 bytes
    assert_eq!(encoded.as_bytes().len(), 64);
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Bc7RgbaUnorm);
    assert_eq!(decoded.as_bytes().len(), w * h * 4);
}

#[test]
fn bc7_srgb_roundtrip_4x4() {
    let w = 4usize;
    let h = 4usize;
    let rgba = make_test_rgba(w, h);
    let input = PixelDatas::U8(rgba);
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::Bc7RgbaUnormSrgb);
    assert_eq!(encoded.as_bytes().len(), 16);
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Bc7RgbaUnormSrgb);
    assert_eq!(decoded.as_bytes().len(), w * h * 4);
}

#[test]
fn bc7_encode_variant() {
    let w = 4usize;
    let h = 4usize;
    let rgba = make_test_rgba(w, h);
    let input = PixelDatas::U8(rgba);
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::Bc7RgbaUnorm);
    assert!(matches!(encoded, PixelDatas::U8(_)));
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Bc7RgbaUnorm);
    assert!(matches!(decoded, PixelDatas::U8(_)));
}

#[test]
fn bc6h_and_bc7_supports_encoding() {
    let formats = [
        TextureFormat::Bc6hRgbUfloat,
        TextureFormat::Bc6hRgbFloat,
        TextureFormat::Bc7RgbaUnorm,
        TextureFormat::Bc7RgbaUnormSrgb,
    ];
    for fmt in &formats {
        assert!(fmt.supports_encoding(), "{fmt:?} should support encoding");
    }
}

#[test]
fn bc6h_roundtrip_larger() {
    let w = 16usize;
    let h = 16usize;
    let f16_data = make_test_f16(w, h);
    let input = PixelDatas::F16(f16_data);
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::Bc6hRgbUfloat);
    // 16x16 → 4×4 = 16 blocks, 16 bytes each → 256 bytes
    assert_eq!(encoded.as_bytes().len(), 256);
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Bc6hRgbUfloat);
    assert_eq!(decoded.as_bytes().len(), w * h * 8);
}

#[test]
fn bc6h_decode_not_overexposed() {
    // Verify decoded values are in a sane range (not infinity/overexposed white)
    let w = 4usize;
    let h = 4usize;
    let f16_data = make_test_f16(w, h);
    let input = PixelDatas::F16(f16_data);
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::Bc6hRgbUfloat);
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Bc6hRgbUfloat);
    let pixels = match &decoded {
        PixelDatas::F16(d) => d,
        _ => panic!("expected F16"),
    };
    for i in (0..w * h * 4).step_by(4) {
        let r = pixels[i].to_f32();
        let g = pixels[i + 1].to_f32();
        let b = pixels[i + 2].to_f32();
        // Values should be finite and in a reasonable range (not infinity)
        assert!(r.is_finite(), "R should be finite, got {}", r);
        assert!(g.is_finite(), "G should be finite, got {}", g);
        assert!(b.is_finite(), "B should be finite, got {}", b);
        // Should not be massively overexposed (max f16 normal is ~65504)
        assert!(r < 1000.0, "R should be < 1000, got {}", r);
        assert!(g < 1000.0, "G should be < 1000, got {}", g);
        assert!(b < 1000.0, "B should be < 1000, got {}", b);
    }
}

#[test]
fn bc7_decode_not_corrupted() {
    let w = 4usize;
    let h = 4usize;
    let rgba = make_test_rgba(w, h);
    let input = PixelDatas::U8(rgba.clone());
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::Bc7RgbaUnorm);
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Bc7RgbaUnorm);
    let pixels = match &decoded {
        PixelDatas::U8(d) => d,
        _ => panic!("expected U8"),
    };
    // Decoded values should roughly resemble the input
    let mut max_diff = 0i32;
    for i in 0..w * h * 4 {
        let diff = (rgba[i] as i32 - pixels[i] as i32).abs();
        max_diff = max_diff.max(diff);
    }
    // With lossy BC7 compression (simple encoder using mode 6 only),
    // per-channel diff for a gradient should be reasonable.
    assert!(max_diff < 128, "max per-channel diff {max_diff} too large");
}

#[test]
fn bc7_roundtrip_larger() {
    let w = 16usize;
    let h = 16usize;
    let rgba = make_test_rgba(w, h);
    let input = PixelDatas::U8(rgba);
    let encoded = encode(&input, w as u32, h as u32, TextureFormat::Bc7RgbaUnorm);
    assert_eq!(encoded.as_bytes().len(), 256);
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::Bc7RgbaUnorm);
    assert_eq!(decoded.as_bytes().len(), w * h * 4);
}
