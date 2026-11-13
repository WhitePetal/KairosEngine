use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

/// A straight (unmultiplied) sRGBA color.
///
/// Channels are stored as authored, not premultiplied: `(255, 0, 0, 128)` is
/// half-transparent red. UI libraries such as egui store premultiplied
/// channels; their conversion lives in the editor module that needs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Color32 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color32 {
    pub const BLACK: Self = Self::new(0, 0, 0, 255);
    pub const WHITE: Self = Self::new(255, 255, 255, 255);
    pub const RED: Self = Self::new(255, 0, 0, 255);

    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Opaque color from RGB channels.
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self::new(r, g, b, 255)
    }

    /// Format as `#RRGGBB` when opaque, `#RRGGBBAA` when translucent.
    ///
    /// Dropping the alpha byte for opaque colors keeps hand-edited styles
    /// (and TOML/RON files) short; the parser accepts both forms.
    pub fn to_hex(&self) -> String {
        if self.a == 255 {
            format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
        } else {
            format!("#{:02X}{:02X}{:02X}{:02X}", self.r, self.g, self.b, self.a)
        }
    }

    /// Parse `#RRGGBB` or `#RRGGBBAA`.
    ///
    /// The leading `#` is optional and surrounding whitespace is ignored, so
    /// values pasted into a TOML/RON file by hand do not fail on formatting.
    pub fn from_hex(hex: &str) -> Result<Self, Color32ParseError> {
        let text = hex.trim();
        let digits = text.strip_prefix('#').unwrap_or(text);

        if digits.is_empty() {
            return Err(Color32ParseError::Empty);
        }
        if !digits.is_ascii() {
            return Err(Color32ParseError::InvalidChannel {
                text: text.to_owned(),
                channel: digits.to_owned(),
            });
        }

        match digits.len() {
            6 => Ok(Self::new(
                channel(text, digits, 0..2)?,
                channel(text, digits, 2..4)?,
                channel(text, digits, 4..6)?,
                255,
            )),
            8 => Ok(Self::new(
                channel(text, digits, 0..2)?,
                channel(text, digits, 2..4)?,
                channel(text, digits, 4..6)?,
                channel(text, digits, 6..8)?,
            )),
            len => Err(Color32ParseError::InvalidLength {
                text: text.to_owned(),
                len,
            }),
        }
    }

    pub fn to_rgb_array(&self) -> [u8; 3] {
        [self.r, self.g, self.b]
    }

    pub fn to_rgba_array(&self) -> [u8; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

impl From<[u8; 3]> for Color32 {
    fn from(rgb: [u8; 3]) -> Self {
        Color32::new(rgb[0], rgb[1], rgb[2], 255)
    }
}

impl From<[u8; 4]> for Color32 {
    fn from(rgba: [u8; 4]) -> Self {
        Color32::new(rgba[0], rgba[1], rgba[2], rgba[3])
    }
}

fn channel(
    text: &str,
    digits: &str,
    range: std::ops::Range<usize>,
) -> Result<u8, Color32ParseError> {
    let channel = &digits[range];
    u8::from_str_radix(channel, 16).map_err(|_| Color32ParseError::InvalidChannel {
        text: text.to_owned(),
        channel: channel.to_owned(),
    })
}

/// Why a hex color string could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Color32ParseError {
    /// The string is empty (after trimming whitespace and an optional `#`).
    #[error("empty color string")]
    Empty,
    /// The hex payload is not 6 or 8 digits long.
    #[error("invalid color {text:?}: expected 6 or 8 hex digits, got {len}")]
    InvalidLength { text: String, len: usize },
    /// A channel contains a non-hexadecimal byte.
    #[error("invalid color {text:?}: {channel:?} is not a hexadecimal byte")]
    InvalidChannel { text: String, channel: String },
}

impl Serialize for Color32 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Color32 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let hex = String::deserialize(deserializer)?;
        Self::from_hex(&hex).map_err(serde::de::Error::custom)
    }
}
