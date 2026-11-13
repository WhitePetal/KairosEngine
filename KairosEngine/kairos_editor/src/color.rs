//! Color conversions for the legacy editor.
//!
//! `kairos_color::Color32` (re-exported as `kairos_engine::math::Color32`) is
//! UI-agnostic: the crate that consumes egui/syntect owns their conversions.
//! This module is that owner for the legacy editor.

use kairos_engine::math::Color32;

/// Straight (unmultiplied) sRGBA → egui's premultiplied color.
pub fn to_egui(color: Color32) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}

/// Straight sRGBA → syntect theme color.
pub fn to_syntect(color: Color32) -> syntect::highlighting::Color {
    syntect::highlighting::Color {
        r: color.r,
        g: color.g,
        b: color.b,
        a: color.a,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_straight_rgba_to_egui() {
        let color = Color32::new(200, 150, 100, 128);
        assert_eq!(
            to_egui(color),
            egui::Color32::from_rgba_unmultiplied(200, 150, 100, 128)
        );
    }

    #[test]
    fn converts_channels_to_syntect() {
        let color = Color32::new(1, 2, 3, 4);
        let syntect = to_syntect(color);
        assert_eq!((syntect.r, syntect.g, syntect.b, syntect.a), (1, 2, 3, 4));
    }
}
