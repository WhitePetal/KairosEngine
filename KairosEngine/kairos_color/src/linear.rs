//! Linear-light colors, their field metadata, and the editor's color property.
//!
//! Color is the canonical engine color: four half-float channels in linear
//! light, straight (unmultiplied) alpha, and a fixed eight-byte layout. RGB is
//! not clamped, so an emissive color may store energy above white; alpha is
//! still a conventional 0..1 coverage value when displayed.
//!
//! The storage form is always linear. sRGB is an encoding used at the edges:
//! Color::from_srgb8 / Color::to_srgb8 convert eight-bit UI colors,
//! Color::from_srgb_f32 / Color::to_srgb_f32 convert normalized channel
//! values, and Color::from_hex / Color::to_hex use the same sRGB form.
//! There is no second "sRGB color" value type.
//!
//! HDR versus LDR is not a storage variant. It is a field policy carried by
//! ColorUsage: an HDR field lets the picker edit values above 1 and shows an
//! Intensity control, while an LDR field clamps the edited base. That mirrors
//! Unity's [ColorUsage] attribute, which lives on the field rather than in the
//! value. The editor's value+usage pair lives in the color widget layer.

use std::fmt;

use bytemuck::{Pod, Zeroable};
use half::f16;
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

/// A linear-light RGBA color stored as four f16 channels.
///
/// The value is eight bytes and can be uploaded directly as a `vec4<f16>`;
/// Color deliberately has no per-value HDR/color-space tag, so arrays of
/// values keep a compact, uniform GPU layout. Use ColorUsage on the field
/// that owns the value to describe how it is edited.
#[repr(C, align(8))]
#[derive(Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct Color {
    rgba: [f16; 4],
}

impl Color {
    /// Transparent black. This is also the default value.
    pub const TRANSPARENT: Self = Self::from_bits([0, 0, 0, 0]);
    /// Opaque black.
    pub const BLACK: Self = Self::from_bits([0, 0, 0, 0x3C00]);
    /// Opaque white.
    pub const WHITE: Self = Self::from_bits([0x3C00; 4]);
    /// Opaque red.
    pub const RED: Self = Self::from_bits([0x3C00, 0, 0, 0x3C00]);

    /// Build a linear color from raw channel bits. Primarily for constants and
    /// GPU round-trips; prefer Color::new or Color::from_linear_f32.
    pub const fn from_bits(rgba: [u16; 4]) -> Self {
        Self {
            rgba: [
                f16::from_bits(rgba[0]),
                f16::from_bits(rgba[1]),
                f16::from_bits(rgba[2]),
                f16::from_bits(rgba[3]),
            ],
        }
    }

    /// Raw half-float bits, in RGBA order.
    pub const fn to_bits(self) -> [u16; 4] {
        [
            self.rgba[0].to_bits(),
            self.rgba[1].to_bits(),
            self.rgba[2].to_bits(),
            self.rgba[3].to_bits(),
        ]
    }

    /// Build a linear color from normalized f32 channels.
    ///
    /// Values are saturated to the finite half-float range; NaN is preserved so
    /// Color::is_finite can report it.
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self::from_linear_f32([r, g, b, a])
    }

    /// Build a linear color from an RGBA array.
    pub fn from_linear_f32(rgba: [f32; 4]) -> Self {
        Self {
            rgba: rgba.map(channel),
        }
    }

    /// The linear-light channels.
    pub fn to_linear_f32(self) -> [f32; 4] {
        [
            self.rgba[0].to_f32(),
            self.rgba[1].to_f32(),
            self.rgba[2].to_f32(),
            self.rgba[3].to_f32(),
        ]
    }

    /// The raw half-float channels, for GPU upload or bit-exact interop.
    pub fn as_f16(self) -> [f16; 4] {
        self.rgba
    }

    /// Convert an authored sRGB byte array to linear light.
    pub fn from_srgb8(rgba: [u8; 4]) -> Self {
        Self::from_srgb_f32([
            rgba[0] as f32 / 255.0,
            rgba[1] as f32 / 255.0,
            rgba[2] as f32 / 255.0,
            rgba[3] as f32 / 255.0,
        ])
    }

    /// Convert normalized sRGB channels to linear light.
    pub fn from_srgb_f32(rgba: [f32; 4]) -> Self {
        Self::from_linear_f32([
            srgb_to_linear(rgba[0]),
            srgb_to_linear(rgba[1]),
            srgb_to_linear(rgba[2]),
            rgba[3].clamp(0.0, 1.0),
        ])
    }

    /// Convert to an eight-bit sRGB array, clamping for display.
    pub fn to_srgb8(self) -> [u8; 4] {
        [
            display_channel(self.r()),
            display_channel(self.g()),
            display_channel(self.b()),
            (self.a().clamp(0.0, 1.0) * 255.0).round() as u8,
        ]
    }

    /// The sRGB-encoded channels. RGB may exceed 1 for HDR values; this method
    /// does not clamp them (use Color::to_srgb8 for display).
    pub fn to_srgb_f32(self) -> [f32; 4] {
        [
            linear_to_srgb(self.r()),
            linear_to_srgb(self.g()),
            linear_to_srgb(self.b()),
            self.a(),
        ]
    }

    /// Parse #RRGGBB / #RRGGBBAA as sRGB and store it as linear light.
    pub fn from_hex(hex: &str) -> Result<Self, ColorParseError> {
        parse_hex(hex).map(Self::from_srgb8)
    }

    /// Format as sRGB hex, clamping HDR values for display.
    pub fn to_hex(self) -> String {
        let [r, g, b, a] = self.to_srgb8();
        if a == 255 {
            format!("#{r:02X}{g:02X}{b:02X}")
        } else {
            format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
        }
    }

    /// Red channel in linear light.
    pub fn r(self) -> f32 {
        self.rgba[0].to_f32()
    }

    /// Green channel in linear light.
    pub fn g(self) -> f32 {
        self.rgba[1].to_f32()
    }

    /// Blue channel in linear light.
    pub fn b(self) -> f32 {
        self.rgba[2].to_f32()
    }

    /// Alpha channel, straight (unmultiplied).
    pub fn a(self) -> f32 {
        self.rgba[3].to_f32()
    }

    /// The largest linear RGB channel; useful for deriving an exposure.
    pub fn max_rgb(self) -> f32 {
        self.r().max(self.g()).max(self.b())
    }

    /// The exposure, in stops, that makes the brightest channel 1.0.
    pub fn exposure_stops(self) -> f32 {
        let max = self.max_rgb();
        if max > 1.0 { max.log2() } else { 0.0 }
    }

    /// Multiply RGB by 2^stops, leaving alpha alone.
    pub fn scaled(self, stops: f32) -> Self {
        let scale = 2.0_f32.powf(stops);
        let [r, g, b, a] = self.to_linear_f32();
        Self::new(r * scale, g * scale, b * scale, a)
    }

    /// Whether every channel is finite (not NaN or infinite).
    pub fn is_finite(self) -> bool {
        self.to_linear_f32().iter().all(|value| value.is_finite())
    }

    /// Whether every channel fits the displayable LDR range.
    ///
    /// This is a value test used by the compact hex serialization, not the
    /// field's HDR policy: an HDR field may legitimately hold a dim color.
    pub fn is_ldr(self) -> bool {
        self.is_finite()
            && self
                .to_linear_f32()
                .iter()
                .all(|value| (0.0..=1.0).contains(value))
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::TRANSPARENT
    }
}

impl fmt::Debug for Color {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("Color")
            .field(&self.to_linear_f32())
            .finish()
    }
}

impl fmt::Display for Color {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

impl From<[f32; 4]> for Color {
    fn from(rgba: [f32; 4]) -> Self {
        Self::from_linear_f32(rgba)
    }
}

/// How a color field interprets and presents its value.
///
/// The value itself is always linear; the space only chooses the encoding the
/// editor uses at its boundary. The default is authored sRGB with alpha shown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColorSpace {
    /// Author and display through the sRGB transfer function.
    #[default]
    Srgb,
    /// Author and display raw linear values (for data-like colors).
    Linear,
}

/// Field-level color policy, the Kairos equivalent of Unity's [ColorUsage].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColorUsage {
    /// Allow values above 1 and show the picker's Intensity control.
    pub hdr: bool,
    /// Show the alpha channel in the picker.
    pub alpha: bool,
    /// Encoding the inspector should present; the stored Color is linear.
    ///
    /// The shared picker currently presents sRGB for every field. `Linear`
    /// is reserved for data-like fields whose numeric editor should show raw
    /// linear values; it never changes the stored bytes.
    pub space: ColorSpace,
    /// Show the eyedropper affordance in the field and picker.
    pub eyedropper: bool,
}

impl Default for ColorUsage {
    fn default() -> Self {
        Self::LDR
    }
}

impl ColorUsage {
    /// A normal authored color: LDR, sRGB, alpha visible, eyedropper shown.
    pub const LDR: Self = Self {
        hdr: false,
        alpha: true,
        space: ColorSpace::Srgb,
        eyedropper: true,
    };

    /// An HDR authored color: intensity and values above 1 are allowed.
    pub const HDR: Self = Self {
        hdr: true,
        alpha: true,
        space: ColorSpace::Srgb,
        eyedropper: true,
    };

    /// Change the display/authoring space.
    pub const fn with_space(mut self, space: ColorSpace) -> Self {
        self.space = space;
        self
    }

    /// Hide the alpha channel.
    pub const fn without_alpha(mut self) -> Self {
        self.alpha = false;
        self
    }

    /// Hide the eyedropper affordance.
    pub const fn without_eyedropper(mut self) -> Self {
        self.eyedropper = false;
        self
    }
}

fn channel(value: f32) -> f16 {
    if value.is_nan() {
        f16::NAN
    } else {
        let max = f16::MAX.to_f32();
        f16::from_f32(value.clamp(-max, max))
    }
}

/// sRGB-encoded channel in [0, 1] to linear light.
pub fn srgb_to_linear(channel: f32) -> f32 {
    let channel = channel.clamp(0.0, 1.0);
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear-light channel to sRGB encoding; values above 1 are not clamped here.
pub fn linear_to_srgb(channel: f32) -> f32 {
    let channel = channel.max(0.0);
    if channel <= 0.003_130_8 {
        channel * 12.92
    } else {
        1.055 * channel.powf(1.0 / 2.4) - 0.055
    }
}

fn display_channel(linear: f32) -> u8 {
    (linear_to_srgb(linear).clamp(0.0, 1.0) * 255.0).round() as u8
}

fn parse_hex(hex: &str) -> Result<[u8; 4], ColorParseError> {
    let text = hex.trim();
    let digits = text.strip_prefix('#').unwrap_or(text);

    if digits.is_empty() {
        return Err(ColorParseError::Empty);
    }
    if !digits.is_ascii() {
        return Err(ColorParseError::InvalidChannel {
            text: text.to_owned(),
            channel: digits.to_owned(),
        });
    }

    match digits.len() {
        6 => Ok([
            hex_channel(text, digits, 0..2)?,
            hex_channel(text, digits, 2..4)?,
            hex_channel(text, digits, 4..6)?,
            255,
        ]),
        8 => Ok([
            hex_channel(text, digits, 0..2)?,
            hex_channel(text, digits, 2..4)?,
            hex_channel(text, digits, 4..6)?,
            hex_channel(text, digits, 6..8)?,
        ]),
        len => Err(ColorParseError::InvalidLength {
            text: text.to_owned(),
            len,
        }),
    }
}

fn hex_channel(
    text: &str,
    digits: &str,
    range: std::ops::Range<usize>,
) -> Result<u8, ColorParseError> {
    let channel = &digits[range];
    u8::from_str_radix(channel, 16).map_err(|_| ColorParseError::InvalidChannel {
        text: text.to_owned(),
        channel: channel.to_owned(),
    })
}

/// Why a hex color string could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ColorParseError {
    /// The string is empty (after trimming whitespace and an optional #).
    #[error("empty color string")]
    Empty,
    /// The hex payload is not 6 or 8 digits long.
    #[error("invalid color {text:?}: expected 6 or 8 hex digits, got {len}")]
    InvalidLength { text: String, len: usize },
    /// A channel contains a non-hexadecimal byte.
    #[error("invalid color {text:?}: {channel:?} is not a hexadecimal byte")]
    InvalidChannel { text: String, channel: String },
}

impl Serialize for Color {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Keep style files readable: a displayable color is written as sRGB
        // hex, while an HDR/out-of-range value needs the linear float form.
        if self.is_ldr() {
            serializer.serialize_str(&self.to_hex())
        } else {
            self.to_linear_f32().serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ColorVisitor;

        impl<'de> Visitor<'de> for ColorVisitor {
            type Value = Color;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an sRGB hex string or [r, g, b, a] linear channels")
            }

            fn visit_str<E>(self, value: &str) -> Result<Color, E>
            where
                E: de::Error,
            {
                Color::from_hex(value).map_err(E::custom)
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Color, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut rgba = [0.0_f32; 4];
                for (index, channel) in rgba.iter_mut().enumerate() {
                    *channel = sequence.next_element()?.ok_or_else(|| {
                        de::Error::custom(format!("expected four color channels, got {index}"))
                    })?;
                }
                Ok(Color::from_linear_f32(rgba))
            }

            fn visit_map<A>(self, mut map: A) -> Result<Color, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut rgba = [0.0_f32; 4];
                let mut seen = [false; 4];
                while let Some(key) = map.next_key::<String>()? {
                    let index = match key.as_str() {
                        "r" => 0,
                        "g" => 1,
                        "b" => 2,
                        "a" => 3,
                        _ => {
                            let _: de::IgnoredAny = map.next_value()?;
                            continue;
                        }
                    };
                    rgba[index] = map.next_value()?;
                    seen[index] = true;
                }
                if seen.iter().all(|value| *value) {
                    Ok(Color::from_linear_f32(rgba))
                } else {
                    Err(de::Error::custom("missing one of r, g, b, a"))
                }
            }
        }

        deserializer.deserialize_any(ColorVisitor)
    }
}
