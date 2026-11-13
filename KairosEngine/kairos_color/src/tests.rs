use serde::{Deserialize, Serialize};

use crate::{Color32, Color32ParseError};

#[test]
fn opaque_colors_format_without_alpha() {
    let color = Color32::new(0x12, 0x34, 0x56, 255);
    assert_eq!(color.to_hex(), "#123456");
    assert_eq!(Color32::from_hex("#123456"), Ok(color));
}

#[test]
fn translucent_colors_keep_their_alpha() {
    let color = Color32::new(0xAB, 0xCD, 0xEF, 0x80);
    assert_eq!(color.to_hex(), "#ABCDEF80");
    assert_eq!(Color32::from_hex("#ABCDEF80"), Ok(color));
}

#[test]
fn from_hex_accepts_optional_hash_case_and_whitespace() {
    let expected = Color32::new(0xAB, 0xCD, 0xEF, 0x80);
    assert_eq!(Color32::from_hex("ABCDEF80"), Ok(expected));
    assert_eq!(Color32::from_hex("  #abcdef80  "), Ok(expected));
}

#[test]
fn from_hex_rejects_invalid_input() {
    assert_eq!(Color32::from_hex(""), Err(Color32ParseError::Empty));
    assert_eq!(Color32::from_hex("   "), Err(Color32ParseError::Empty));
    assert_eq!(Color32::from_hex("#"), Err(Color32ParseError::Empty));
    assert!(matches!(
        Color32::from_hex("#FFF"),
        Err(Color32ParseError::InvalidLength { len: 3, .. })
    ));
    assert!(matches!(
        Color32::from_hex("#1234567"),
        Err(Color32ParseError::InvalidLength { len: 7, .. })
    ));
    assert!(matches!(
        Color32::from_hex("#GGGGGG"),
        Err(Color32ParseError::InvalidChannel { .. })
    ));
    assert!(matches!(
        Color32::from_hex("#1234G6"),
        Err(Color32ParseError::InvalidChannel { .. })
    ));
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct ThemeSample {
    color: Color32,
}

#[test]
fn serde_round_trips_through_hex_strings() {
    let sample = ThemeSample {
        color: Color32::new(0x12, 0x34, 0x56, 0x78),
    };
    let text = ron::to_string(&sample).expect("serialize");
    assert!(text.contains("\"#12345678\""), "{text}");

    let back: ThemeSample = ron::from_str(&text).expect("deserialize");
    assert_eq!(back, sample);
}

#[test]
fn array_conversions_default_to_opaque() {
    assert_eq!(Color32::from([1, 2, 3]), Color32::new(1, 2, 3, 255));
    assert_eq!(Color32::from([1, 2, 3, 4]), Color32::new(1, 2, 3, 4));
    assert_eq!(Color32::new(1, 2, 3, 4).to_rgba_array(), [1, 2, 3, 4]);
}
