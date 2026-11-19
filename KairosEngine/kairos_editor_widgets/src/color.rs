//! `kairos_color::Color` → egui conversions for the editor.
//!
//! `kairos_color` is an engine-side, UI-agnostic value crate, so the egui
//! bridge belongs to the editor's appearance layer (L0/L1) rather than to the
//! color crate. Components and the app shell share this conversion.
//!
//! Rust's orphan rules are why this is an extension trait instead of
//! `impl From<Color> for egui::Color32`: both `From` and `egui::Color32` are
//! foreign here, and `Color` is defined in `kairos_color`, so only that crate
//! could write the `From` impl.

use egui::Color32 as EguiColor;
use kairos_color::Color;

/// Extension methods for painting canonical linear colors.
pub trait ColorExt {
    /// Linear color → displayable sRGB → egui's premultiplied color.
    fn to_egui(self) -> EguiColor;
}

impl ColorExt for Color {
    fn to_egui(self) -> EguiColor {
        let [r, g, b, a] = self.to_srgb8();
        EguiColor::from_rgba_unmultiplied(r, g, b, a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_linear_color_to_displayable_egui() {
        assert_eq!(
            Color::from_srgb8([20, 40, 60, 255]).to_egui(),
            EguiColor::from_rgb(20, 40, 60)
        );
        assert_eq!(
            Color::from_srgb8([200, 150, 100, 128]).to_egui(),
            EguiColor::from_rgba_unmultiplied(200, 150, 100, 128)
        );
    }
}
