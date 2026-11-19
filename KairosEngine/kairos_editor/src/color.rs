//! Color conversions for the legacy editor.
//!
//! `kairos_color::Color` (re-exported as `kairos_engine::math::Color`) is
//! UI-agnostic: the crate that consumes egui/syntect owns their conversions.
//! This module is that owner for the legacy editor.

use kairos_engine::math::Color;

/// Linear color → displayable sRGB → egui's premultiplied color.
pub fn to_egui(color: Color) -> egui::Color32 {
    let [r, g, b, a] = color.to_srgb8();
    egui::Color32::from_rgba_unmultiplied(r, g, b, a)
}

/// Linear color → displayable sRGB → syntect theme color.
pub fn to_syntect(color: Color) -> syntect::highlighting::Color {
    let [r, g, b, a] = color.to_srgb8();
    syntect::highlighting::Color { r, g, b, a }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_straight_rgba_to_egui() {
        let color = Color::from_srgb8([200, 150, 100, 128]);
        assert_eq!(
            to_egui(color),
            egui::Color32::from_rgba_unmultiplied(200, 150, 100, 128)
        );
    }

    #[test]
    fn converts_channels_to_syntect() {
        let color = Color::from_srgb8([1, 2, 3, 4]);
        let syntect = to_syntect(color);
        assert_eq!((syntect.r, syntect.g, syntect.b, syntect.a), (1, 2, 3, 4));
    }
}
