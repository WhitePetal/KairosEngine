//! `kairos_color::Color32` → egui conversions for the editor.
//!
//! `kairos_color` is an engine-side, UI-agnostic value crate, so the egui bridge
//! belongs to the editor's appearance layer (L0/L1) rather than to the color
//! crate. Components and the app shell share this one conversion.
//!
//! Rust's orphan rules are why this is an extension trait instead of
//! `impl From<Color32> for egui::Color32`: both `From` and `egui::Color32` are
//! foreign here, and `Color32` is defined in `kairos_color`, so only that crate
//! could write the `From` impl.

use egui::Color32 as EguiColor;
use kairos_color::Color32;

/// Extension methods for painting shared style colors.
pub trait Color32Ext {
    /// Straight (unmultiplied) sRGBA → egui's premultiplied color.
    fn to_egui(self) -> EguiColor;
}

impl Color32Ext for Color32 {
    fn to_egui(self) -> EguiColor {
        EguiColor::from_rgba_unmultiplied(self.r, self.g, self.b, self.a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_straight_rgba_to_egui() {
        assert_eq!(
            Color32::new(20, 40, 60, 255).to_egui(),
            EguiColor::from_rgb(20, 40, 60)
        );
        assert_eq!(
            Color32::new(200, 150, 100, 128).to_egui(),
            EguiColor::from_rgba_unmultiplied(200, 150, 100, 128)
        );
    }
}
