use serde::{Deserialize, Serialize};

use crate::{Color, ColorParseError, linear_to_srgb, srgb_to_linear};

#[test]
fn opaque_colors_format_without_alpha() {
    let color = Color::from_srgb8([0x12, 0x34, 0x56, 255]);
    assert_eq!(color.to_hex(), "#123456");
    assert_eq!(Color::from_hex("#123456"), Ok(color));
}

#[test]
fn translucent_colors_keep_their_alpha() {
    let color = Color::from_srgb8([0xAB, 0xCD, 0xEF, 0x80]);
    assert_eq!(color.to_hex(), "#ABCDEF80");
    assert_eq!(Color::from_hex("#ABCDEF80"), Ok(color));
}

#[test]
fn from_hex_accepts_optional_hash_case_and_whitespace() {
    let expected = Color::from_srgb8([0xAB, 0xCD, 0xEF, 0x80]);
    assert_eq!(Color::from_hex("ABCDEF80"), Ok(expected));
    assert_eq!(Color::from_hex("  #abcdef80  "), Ok(expected));
}

#[test]
fn from_hex_rejects_invalid_input() {
    assert_eq!(Color::from_hex(""), Err(ColorParseError::Empty));
    assert_eq!(Color::from_hex("   "), Err(ColorParseError::Empty));
    assert_eq!(Color::from_hex("#"), Err(ColorParseError::Empty));
    assert!(matches!(
        Color::from_hex("#FFF"),
        Err(ColorParseError::InvalidLength { len: 3, .. })
    ));
    assert!(matches!(
        Color::from_hex("#1234567"),
        Err(ColorParseError::InvalidLength { len: 7, .. })
    ));
    assert!(matches!(
        Color::from_hex("#GGGGGG"),
        Err(ColorParseError::InvalidChannel { .. })
    ));
    assert!(matches!(
        Color::from_hex("#1234G6"),
        Err(ColorParseError::InvalidChannel { .. })
    ));
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct ThemeSample {
    color: Color,
}

#[test]
fn serde_round_trips_through_hex_strings() {
    let sample = ThemeSample {
        color: Color::from_srgb8([0x12, 0x34, 0x56, 0x78]),
    };
    let text = ron::to_string(&sample).expect("serialize");
    assert!(text.contains("\"#12345678\""), "{text}");

    let back: ThemeSample = ron::from_str(&text).expect("deserialize");
    assert_eq!(back, sample);
}

#[test]
fn srgb_byte_arrays_round_trip() {
    assert_eq!(Color::from_srgb8([1, 2, 3, 255]).to_srgb8(), [1, 2, 3, 255]);
    assert_eq!(Color::from_srgb8([1, 2, 3, 4]).to_srgb8(), [1, 2, 3, 4]);
}

#[test]
fn hdr_colors_round_trip_through_srgb() {
    for rgba in [
        [0, 0, 0, 255],
        [255, 255, 255, 255],
        [0x12, 0x34, 0x56, 0xFF],
        [0xAB, 0xCD, 0xEF, 0x80],
    ] {
        let color = Color::from_srgb8(rgba);
        assert_eq!(color.to_srgb8(), rgba, "{rgba:?}");
    }
}

#[test]
fn hdr_values_above_one_clip_for_display_but_keep_their_exposure() {
    let bright = Color::new(4.0, 2.0, 1.0, 1.0);
    assert_eq!(bright.to_srgb8(), [255, 255, 255, 255]);
    assert_eq!(bright.exposure_stops(), 2.0);
    assert_eq!(bright.max_rgb(), 4.0);
    assert_eq!(bright.scaled(-1.0), Color::new(2.0, 1.0, 0.5, 1.0));

    // A color that already fits LDR has no exposure to recover.
    let ldr = Color::new(0.5, 0.25, 0.0, 1.0);
    assert_eq!(ldr.exposure_stops(), 0.0);
    assert_eq!(ldr.scaled(0.0), ldr);
}

#[test]
fn srgb_transfer_is_continuous_and_invertible() {
    for channel in [0.0, 0.04045, 0.5, 1.0] {
        let linear = srgb_to_linear(channel);
        assert!((linear_to_srgb(linear) - channel).abs() < 1e-5, "{channel}");
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct HdrSample {
    glow: Color,
}

#[test]
fn color_deserializes_legacy_named_channels() {
    let color: Color = ron::from_str("(r: 1.0, g: 0.5, b: 0.25, a: 1.0)").expect("legacy map");
    assert_eq!(color.to_linear_f32(), [1.0, 0.5, 0.25, 1.0]);
}

#[test]
fn hdr_colors_serde_round_trip() {
    let sample = HdrSample {
        glow: Color::new(4.0, 1.5, 0.25, 1.0),
    };
    let text = ron::to_string(&sample).expect("serialize");
    assert!(text.contains("glow"), "{text}");
    let back: HdrSample = ron::from_str(&text).expect("deserialize");
    assert_eq!(back, sample);
}

#[test]
fn linear_color_is_eight_bytes_and_aligned_for_f16_vectors() {
    assert_eq!(std::mem::size_of::<Color>(), 8);
    assert_eq!(std::mem::align_of::<Color>(), 8);
}

#[test]
fn every_eight_bit_srgb_channel_round_trips_through_linear_half() {
    for value in 0..=255_u8 {
        for rgba in [
            [value, 0, 0, 255],
            [0, value, 0, 255],
            [0, 0, value, 255],
            [0, 0, 0, value],
            [value, value, value, value],
        ] {
            let color = Color::from_srgb8(rgba);
            assert_eq!(color.to_srgb8(), rgba, "input {rgba:?}");
        }
    }
}

#[test]
fn color_alpha_is_straight_and_not_gamma_encoded() {
    let color = Color::from_srgb8([20, 140, 220, 128]);
    assert!((color.a() - 128.0 / 255.0).abs() < 1e-4);
    assert_eq!(color.to_srgb8(), [20, 140, 220, 128]);
}

#[test]
fn ldr_color_serde_uses_hex_and_hdr_uses_linear_floats() {
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct LdrSample {
        color: Color,
    }
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct HdrSample {
        color: Color,
    }

    let ldr = LdrSample {
        color: Color::from_srgb8([0x12, 0x34, 0x56, 0x78]),
    };
    let text = ron::to_string(&ldr).expect("serialize ldr");
    assert!(text.contains("\"#12345678\""), "{text}");
    assert_eq!(
        ron::from_str::<LdrSample>(&text).expect("deserialize ldr"),
        ldr
    );

    let hdr = HdrSample {
        color: Color::new(4.0, 2.0, 1.0, 1.0),
    };
    let text = ron::to_string(&hdr).expect("serialize hdr");
    assert!(text.contains("4.0"), "{text}");
    assert_eq!(
        ron::from_str::<HdrSample>(&text).expect("deserialize hdr"),
        hdr
    );
}

#[test]
fn hdr_values_are_plain_colors() {
    let value = Color::new(2.0, 1.0, 0.5, 1.0);
    assert_eq!(value.to_linear_f32(), [2.0, 1.0, 0.5, 1.0]);
}
