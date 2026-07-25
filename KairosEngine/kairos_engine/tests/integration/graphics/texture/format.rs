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
                255,
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
            assert_eq!(dec[idx + 3], 255, "A at ({},{})", x, y);
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
            assert_eq!(dec[px + 3], 65535, "A at ({},{})", x, y);
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
            assert_eq!(dec[px + 1], u8_to_i16(rgba[px + 1]), "G at ({},{})", x, y);
            assert_eq!(dec[px + 2], u8_to_i16(rgba[px + 2]), "B at ({},{})", x, y);
            assert_eq!(dec[px + 3], u8_to_i16(rgba[px] + 3), "A at ({},{})", x, y);
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
        let half_one = half::f16::from_f32(1.0);
        assert_eq!(dec[idx], rgba_f16[idx], "R at pixel {}", i);
        assert_eq!(dec[idx + 1], half::f16::ZERO, "G at pixel {}", i);
        assert_eq!(dec[idx + 2], half::f16::ZERO, "B at pixel {}", i);
        assert_eq!(dec[idx + 3], half_one, "A at pixel {}", i);
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
            assert_eq!(dec[px + 3], 65535, "A at ({},{})", x, y);
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
    let dec: &[u16] = bytemuck::cast_slice(decoded.as_bytes());
    for y in 0..h {
        for x in 0..w {
            let px = (y * w + x) * 4;
            assert_eq!(dec[px], sext(rgba[px]), "R at ({},{})", x, y);
            assert_eq!(dec[px + 1], sext(rgba[px + 1]), "G at ({},{})", x, y);
            assert_eq!(dec[px + 2], 0, "B at ({},{})", x, y);
            assert_eq!(dec[px + 3], 65535, "A at ({},{})", x, y);
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
        assert_eq!(dec[idx + 3], half_one, "A at pixel {}", i);
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
    let dec: &[u16] = bytemuck::cast_slice(decoded.as_bytes());
    for i in 0..w * h * 4 {
        assert_eq!(dec[i], sext(rgba[i]), "channel {} mismatch", i);
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
    let r = u16::from_le_bytes([enc[0], enc[1]]);
    // 200 as i8 = -56, as i16 = -56 = 0xFFC8 = 65480
    assert_eq!(r, (-56i16) as u16, "R should be sign-extended");
    // Decode back preserves the sign-extended u16 value
    let decoded = decode(&encoded, w as u32, h as u32, TextureFormat::R16Sint);
    let dec: &[u16] = bytemuck::cast_slice(decoded.as_bytes());
    assert_eq!(dec[0], (-56i16) as u16, "R should be sign-extended");
    assert_eq!(dec[1], 0, "G");
    assert_eq!(dec[2], 0, "B");
    assert_eq!(dec[3], 65535, "A");
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

// ============================================================
// Stress tests: non-power-of-two dimensions, multi-chunk images
// ============================================================

fn sext(v: u8) -> u16 {
    (v as i8) as i16 as u16
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
            let expected = |v: u8| -> u16 { if sign_extend { sext(v) } else { v as u16 } };
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
                assert_eq!(dec[px + 3], 65535, "A at ({},{}) fmt={fmt:?}", x, y);
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
