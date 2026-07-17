mod converts;
mod errors;
mod serialies;

use crate::math::color::errors::Color32ParseError;

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

    pub fn to_hex(&self) -> String {
        format!("#{:02X}{:02X}{:02X}{:02x}", self.r, self.g, self.b, self.a)
    }

    pub fn from_hex(hex: &str) -> Result<Self, Color32ParseError> {
        if hex.is_empty() {
            return Err(Color32ParseError::EmptyString);
        }

        let hex = match hex.strip_prefix('#') {
            Some(s) => s,
            None => return Err(Color32ParseError::NoHead),
        };

        let len = hex.len();
        match len {
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16)
                    .map_err(|_| Color32ParseError::InvalidHexChar(hex[0..2].to_string()))?;
                let g = u8::from_str_radix(&hex[2..4], 16)
                    .map_err(|_| Color32ParseError::InvalidHexChar(hex[2..4].to_string()))?;
                let b = u8::from_str_radix(&hex[4..6], 16)
                    .map_err(|_| Color32ParseError::InvalidHexChar(hex[4..6].to_string()))?;

                Ok(Self::new(r, g, b, 255))
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16)
                    .map_err(|_| Color32ParseError::InvalidHexChar(hex[0..2].to_string()))?;
                let g = u8::from_str_radix(&hex[2..4], 16)
                    .map_err(|_| Color32ParseError::InvalidHexChar(hex[2..4].to_string()))?;
                let b = u8::from_str_radix(&hex[4..6], 16)
                    .map_err(|_| Color32ParseError::InvalidHexChar(hex[4..6].to_string()))?;
                let a = u8::from_str_radix(&hex[6..8], 16)
                    .map_err(|_| Color32ParseError::InvalidHexChar(hex[6..8].to_string()))?;

                Ok(Self::new(r, g, b, a))
            }
            _ => Err(Color32ParseError::InvalidLength(len)),
        }
    }

    pub fn to_rgb_array(&self) -> [u8; 3] {
        [self.r, self.g, self.b]
    }

    pub fn to_rgba_array(&self) -> [u8; 4] {
        [self.r, self.g, self.b, self.a]
    }
}
