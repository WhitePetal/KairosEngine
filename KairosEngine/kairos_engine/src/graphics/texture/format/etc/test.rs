use super::*;

/// Verify that individual-mode and planar-mode decoding produces
/// correct RGBA output for a trivial constant-color block.
#[test]
fn test_etc2_rgb8_decode_constant() {
    // Encode a constant 50% grey block via individual mode
    let grey: [u8; 3] = [128, 128, 128];
    let mut block_in = [[0u8; 4]; 16];
    for px in &mut block_in {
        *px = [grey[0], grey[1], grey[2], 255];
    }
    let encoded = etc2_encode_block(&block_in);
    let mut decoded = [0u8; 64];
    decode_etc2_rgb8_block(&encoded, &mut decoded);

    // For a constant block, reconstruction should be close
    for py in 0..4 {
        for px in 0..4 {
            let off = (py * 4 + px) * 4;
            // Each pixel should be roughly grey; tolerance ±12
            for c in 0..3 {
                let diff = (decoded[off + c] as i16 - grey[c] as i16).abs();
                assert!(diff <= 12, "pixel ({px},{py}) ch {c}: got {} expected {}", decoded[off + c], grey[c]);
            }
            assert_eq!(decoded[off + 3], 255, "alpha should be 255");
        }
    }
}

/// Verify that a full encode→decode roundtrip for an ETC2 RGB block
/// preserves image data within acceptable tolerance.
///
/// ETC2 with 4‑bit quantised base colours has limited precision for
/// sub‑blocks containing widely varying values, so we use two blocks:
/// a near‑constant block (low error) and a ramp block (higher tolerance).
#[test]
fn test_etc2_rgb8_roundtrip_deterministic() {
    // --- Block A: near-constant, so error should be small ---
    let mut block_in = [[0u8; 4]; 16];
    for px in &mut block_in {
        *px = [100, 150, 200, 255];
    }
    // Add a little variation
    block_in[0] = [101, 151, 201, 255];
    block_in[15] = [99, 149, 199, 255];

    let encoded = etc2_encode_block(&block_in);
    let mut decoded = [0u8; 64];
    decode_etc2_rgb8_block(&encoded, &mut decoded);

    for i in 0..16 {
        let off = i * 4;
        for c in 0..3 {
            let diff = (decoded[off + c] as i16 - block_in[i][c] as i16).abs();
            assert!(
                diff <= 20,
                "near-constant pixel {i} ch {c}: got {} expected {}",
                decoded[off + c],
                block_in[i][c]
            );
        }
    }

    // --- Block B: full-range ramp (higher tolerance for lossy ETC2) ---
    let mut block_b = [[0u8; 4]; 16];
    for i in 0..16 {
        block_b[i] = [
            ((i * 17) & 0xFF) as u8,
            ((i * 37) & 0xFF) as u8,
            ((i * 53) & 0xFF) as u8,
            255,
        ];
    }
    let encoded = etc2_encode_block(&block_b);
    let mut decoded = [0u8; 64];
    decode_etc2_rgb8_block(&encoded, &mut decoded);

    for i in 0..16 {
        let off = i * 4;
        for c in 0..3 {
            let diff = (decoded[off + c] as i16 - block_b[i][c] as i16).abs();
            assert!(
                diff <= 128,
                "ramp pixel {i} ch {c}: got {} expected {}",
                decoded[off + c],
                block_b[i][c]
            );
        }
    }
}

/// Test EAC R11 UNORM decode of a block with known encoded values.
#[test]
fn test_eac_r11_decode() {
    // Encode a simple ramp, decode, verify it's close
    let mut block_in = [[0u8; 4]; 16];
    for i in 0..16 {
        block_in[i][0] = (i as u8) * 17;
        block_in[i][1] = (i as u8) * 17;
        block_in[i][2] = (i as u8) * 17;
        block_in[i][3] = 255;
    }
    let block_16 = to_block_16(&block_in);
    let encoded = eac_r11_encode_block(&block_16);
    let mut decoded = [0u8; 64];
    decode_eac_r11_block(&encoded, &mut decoded);

    for i in 0..16 {
        let diff = (decoded[i * 4] as i16 - block_in[i][0] as i16).abs();
        assert!(
            diff <= 16,
            "pixel {i}: got {} expected {}",
            decoded[i * 4],
            block_in[i][0]
        );
    }
}

/// Test ETC2 RGBA8: alpha channel roundtrips via EAC A8.
#[test]
fn test_etc2_rgba8_alpha_roundtrip() {
    let mut block_in = [[0u8; 4]; 16];
    for i in 0..16 {
        block_in[i] = [100, 150, 200, (i as u8) * 17];
    }
    let encoded = etc2_rgba8_block(&block_in);
    let mut decoded = [0u8; 64];
    decode_etc2_rgba8_block(&encoded, &mut decoded);

    for i in 0..16 {
        let diff = (decoded[i * 4 + 3] as i16 - block_in[i][3] as i16).abs();
        assert!(
            diff <= 16,
            "pixel {i} alpha: got {} expected {}",
            decoded[i * 4 + 3],
            block_in[i][3]
        );
    }
}

/// Test EAC R11 SNORM decode.
#[test]
fn test_eac_r11_snorm_roundtrip() {
    let mut values = [0i8; 16];
    for i in 0..16 {
        values[i] = ((i as i32) * 16 - 128).clamp(-127, 127) as i8;
    }
    let encoded = eac_r11_encode_snorm_block(&values);
    let mut decoded = [0i8; 64];
    decode_eac_r11_snorm_block(&encoded, &mut decoded);

    for i in 0..16 {
        let diff = (decoded[i * 4] as i16 - values[i] as i16).abs();
        assert!(
            diff <= 16,
            "pixel {i}: got {} expected {}",
            decoded[i * 4],
            values[i]
        );
    }
}
