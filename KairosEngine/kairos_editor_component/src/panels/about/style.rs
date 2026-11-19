//! About panel appearance.
//!
//! Code defaults for `AboutPanelStyle`, persisted per component to
//! `Preferences/Styles/AboutPanel.ron` (dark + light). The panel loads the file
//! in `Panel::init`; editing it restyles About without a rebuild.
//!
//! What is **not** here: the product copy. The name, version, description,
//! license and links are the composition root's identity
//! (`EditorInfo`), not appearance — keeping them out of the skin file is what
//! lets one skin serve every product identity, and what keeps text translatable
//! without shipping a second RON file per language.

use egui::{CornerRadius, Vec2};
use kairos_color::Color;
use kairos_editor_widgets::button::ButtonStyle;
use kairos_editor_widgets::color::ColorExt;
use kairos_editor_widgets::style::Style;
use kairos_editor_widgets::text::TextStyle;
use serde::{Deserialize, Serialize};

use crate::skin::StyleFields;

/// Appearance tokens for the About panel: palette, type sizes, geometry and the
/// two runtime icon paths.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, StyleFields)]
pub struct AboutPanelStyle {
    /// Big product mark and the (small) tab artwork.
    pub icons: AboutIconPaths,

    pub background: Color,
    pub title: Color,
    pub subtitle: Color,
    pub body: Color,
    /// Key column of the identity rows (Build/License/Repository/...).
    pub label: Color,
    /// Value column of the identity rows.
    pub value: Color,
    pub link: Color,
    pub divider: Color,

    pub title_size: f32,
    pub subtitle_size: f32,
    pub body_size: f32,

    /// Height of one identity row.
    pub row_height: f32,
    /// Width of the key column; the value column starts after it.
    pub label_width: f32,
    /// Inner margin of the panel body.
    pub padding: f32,
    /// Gap between the header lines and before the footer.
    pub spacing: f32,
    /// Space above and below a rule.
    pub section_spacing: f32,

    pub logo_size: f32,
    /// Gap between the product mark and the name/version block.
    pub logo_gap: f32,

    /// Body-button chrome: filled and outlined, so a dialog action reads as a
    /// button instead of a text label on the panel background.
    pub button_fill: Color,
    pub button_hover: Color,
    pub button_active: Color,
    pub button_border: Color,
    pub button_border_width: f32,
    pub button_text: Color,
    pub button_height: f32,
    /// Horizontal padding of a body button.
    pub button_padding: f32,
    pub button_corner_radius: f32,
}

impl Default for AboutPanelStyle {
    fn default() -> Self {
        Self::dark()
    }
}

impl Style for AboutPanelStyle {
    const FILE: &'static str = "AboutPanel.ron";

    fn dark() -> Self {
        Self {
            icons: AboutIconPaths::dark(),

            background: Color::from_srgb8([0x36, 0x36, 0x36, 255]),
            title: Color::from_srgb8([0xee, 0xee, 0xee, 255]),
            subtitle: Color::from_srgb8([0xa0, 0xa0, 0xa0, 255]),
            body: Color::from_srgb8([0xc8, 0xc8, 0xc8, 255]),
            label: Color::from_srgb8([0x8c, 0x8c, 0x8c, 255]),
            value: Color::from_srgb8([0xdc, 0xdc, 0xdc, 255]),
            link: Color::from_srgb8([0x4a, 0x90, 0xd9, 255]),
            divider: Color::from_srgb8([0x2a, 0x2a, 0x2a, 255]),

            title_size: 20.0,
            subtitle_size: 12.0,
            body_size: 12.0,

            row_height: 18.0,
            label_width: 78.0,
            padding: 14.0,
            spacing: 6.0,
            section_spacing: 6.0,

            logo_size: 72.0,
            logo_gap: 14.0,

            button_fill: Color::from_srgb8([0x45, 0x45, 0x45, 255]),
            button_hover: Color::from_srgb8([0x56, 0x56, 0x56, 255]),
            button_active: Color::from_srgb8([0x33, 0x33, 0x33, 255]),
            button_border: Color::from_srgb8([0x6e, 0x6e, 0x6e, 255]),
            button_border_width: 1.0,
            button_text: Color::from_srgb8([0xdc, 0xdc, 0xdc, 255]),
            button_height: 22.0,
            button_padding: 10.0,
            button_corner_radius: 2.0,
        }
    }

    fn light() -> Self {
        Self {
            icons: AboutIconPaths::light(),

            background: Color::from_srgb8([0xeb, 0xeb, 0xeb, 255]),
            title: Color::from_srgb8([0x1e, 0x1e, 0x1e, 255]),
            subtitle: Color::from_srgb8([0x5a, 0x5a, 0x5a, 255]),
            body: Color::from_srgb8([0x33, 0x33, 0x33, 255]),
            label: Color::from_srgb8([0x6e, 0x6e, 0x6e, 255]),
            value: Color::from_srgb8([0x1e, 0x1e, 0x1e, 255]),
            link: Color::from_srgb8([0x2c, 0x5d, 0x87, 255]),
            divider: Color::from_srgb8([0xc4, 0xc4, 0xc4, 255]),

            button_fill: Color::from_srgb8([0xf5, 0xf5, 0xf5, 255]),
            button_hover: Color::WHITE,
            button_active: Color::from_srgb8([0xdc, 0xdc, 0xdc, 255]),
            button_border: Color::from_srgb8([0xa8, 0xa8, 0xa8, 255]),
            button_border_width: 1.0,
            button_text: Color::from_srgb8([0x20, 0x20, 0x20, 255]),
            ..Self::dark()
        }
    }
}

impl AboutPanelStyle {
    /// Project the body-button tokens onto the reusable button widget.
    ///
    /// Unlike the flat menu-bar/toolbar chrome, a dialog action carries a fill
    /// and an outline: on the panel background a text-only button is
    /// indistinguishable from a label.
    pub fn button_style(&self) -> ButtonStyle {
        ButtonStyle::new(TextStyle::sized(self.button_text.to_egui(), self.body_size))
            .fills(
                self.button_fill.to_egui(),
                self.button_hover.to_egui(),
                self.button_active.to_egui(),
            )
            .stroke(egui::Stroke::new(
                self.button_border_width,
                self.button_border.to_egui(),
            ))
            .corner_radius(CornerRadius::same(self.button_corner_radius as u8))
            .padding(Vec2::new(self.button_padding, 0.0))
            .height(self.button_height)
    }
}

/// Runtime artwork paths. Both skins point at the packaged engine icon, so a
/// fresh checkout draws the About panel without extra files; a skin may point
/// `tab` at a dedicated 16 px glyph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, StyleFields)]
pub struct AboutIconPaths {
    /// Product mark in the panel body.
    pub logo: String,
    /// Tab-header icon.
    pub tab: String,
}

impl AboutIconPaths {
    pub fn dark() -> Self {
        Self {
            logo: "Preferences/Textures/engine_icon.png".to_owned(),
            tab: "Preferences/Textures/engine_icon.png".to_owned(),
        }
    }

    pub fn light() -> Self {
        Self::dark()
    }

    /// Every path this skin paints, for `Panel::init`'s preload.
    pub fn all(&self) -> [&str; 2] {
        [&self.logo, &self.tab]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn about_style_projects_square_hover_buttons() {
        let style = AboutPanelStyle::dark();
        let button = style.button_style();

        assert_eq!(button.fill, style.button_fill.to_egui());
        assert_eq!(button.hover_fill, style.button_hover.to_egui());
        assert_eq!(button.active_fill, style.button_active.to_egui());
        assert_eq!(
            button.stroke,
            egui::Stroke::new(style.button_border_width, style.button_border.to_egui()),
            "a dialog button is outlined, not a bare label"
        );
        assert_eq!(button.corner_radius, CornerRadius::same(2));
        assert_eq!(button.height, style.button_height);
        assert_eq!(
            button.text.font_id(&egui::Style::default()).size,
            style.body_size
        );
    }

    /// A skin file written before a token existed must still load: every
    /// `AboutPanelStyle` field is required, so this guards the file format only
    /// through the round-trip below. The point of the test is that the checked-in
    /// reference skin and the code defaults are the same skin.
    #[test]
    fn shipped_reference_skin_matches_the_code_defaults() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../Preferences/Styles/AboutPanel.ron");
        let file = kairos_editor_widgets::style::StyleFile::<AboutPanelStyle>::load_from(&path)
            .expect("the reference skin parses");

        let defaults = kairos_editor_widgets::style::StyleFile::<AboutPanelStyle>::defaults();
        assert_eq!(file.dark, defaults.dark, "dark skin drifted from code");
        assert_eq!(file.light, defaults.light, "light skin drifted from code");
    }
}
