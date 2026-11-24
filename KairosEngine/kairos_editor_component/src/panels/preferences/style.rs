//! Preferences panel appearance.
//!
//! Code defaults for `PreferencesPanelStyle`, persisted per component to
//! `Preferences/Styles/PreferencesPanel.ron` (dark + light). The panel loads the
//! file in `Panel::init`. Because the type derives `StyleRow`, the panel can
//! also edit *itself* in its own field editor — the same file, the same tokens.
//!
//! This is the *panel*: the page list column, the page heading and its file
//! line, the rules, the padding and the Reset button. The rows themselves are
//! not the panel's — a form page is drawn by the widget kit's `FormRows`, whose
//! row grid is the shared `Form` skin (`EditorFormStyle`), and the value boxes
//! are the shared `Field` skin. A second form (an object inspector) must not
//! inherit this panel's geometry by accident.
//!
//! The palette deliberately repeats the About panel's: both are dialog-shaped
//! panels on the same dark chrome, and the component convention is one skin per
//! panel. When a second dialog panel appears, the shared tokens move into a
//! dialog skin — not before.

use egui::{CornerRadius, Vec2};
use kairos_color::Color;
use kairos_editor_widgets::button::ButtonStyle;
use kairos_editor_widgets::color::ColorExt;
use kairos_editor_widgets::style::Style;
use kairos_editor_widgets::text::TextStyle;
use serde::{Deserialize, Serialize};

use crate::style_row::StyleRow;

/// Appearance tokens for the Preferences panel: panel body, page-list column,
/// heading, rules, padding and the dialog button.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, StyleRow)]
pub struct PreferencesPanelStyle {
    /// Panel body, behind everything.
    pub background: Color,
    /// Left-hand page list column.
    pub nav_background: Color,
    pub nav_hover: Color,
    /// Fill of the current page's list entry.
    pub nav_selected: Color,
    pub nav_text: Color,

    /// Page heading (the panel being edited).
    pub title: Color,
    /// The RON file the page writes to, under the heading.
    pub path: Color,
    /// Rules between the heading, the rows and the footer.
    pub divider: Color,

    /// Selected outline, active slider fill, and the page-list stripe.
    pub accent: Color,

    pub accent_width: f32,
    pub devider_width: f32,

    pub title_size: f32,
    pub body_size: f32,

    /// Inner margin of the panel body.
    pub padding: f32,
    /// Width of the page-list column.
    pub nav_width: f32,
    pub nav_row_height: f32,
    pub nav_corner_radius: f32,
    pub nav_shrink: f32,
    /// Gap between the page list and the page body.
    pub column_gap: f32,
    /// Space above and below the rule under a page heading.
    pub section_spacing: f32,
    /// Right-hand gutter kept clear of the page scroll bar.
    pub scrollbar_gap: f32,

    /// Space between the last row and the rule above the footer.
    pub footer_gap: f32,
    pub footer_height: f32,

    /// Dialog buttons (Reset): filled and outlined, so they do not read as
    /// labels on the panel background.
    pub button_fill: Color,
    pub button_hover: Color,
    pub button_active: Color,
    pub button_border: Color,
    pub button_border_width: f32,
    pub button_text: Color,
    pub button_height: f32,
    pub button_padding: f32,
    pub button_corner_radius: f32,
}

impl Default for PreferencesPanelStyle {
    fn default() -> Self {
        Self::dark()
    }
}

impl Style for PreferencesPanelStyle {
    const FILE: &'static str = "PreferencesPanel.ron";

    fn dark() -> Self {
        Self {
            background: Color::from_srgb8([0x36, 0x36, 0x36, 255]),
            nav_background: Color::from_srgb8([0x2E, 0x2E, 0x2E, 255]),
            nav_hover: Color::from_srgb8([0x3E, 0x3E, 0x3E, 255]),
            nav_selected: Color::from_srgb8([0x45, 0x45, 0x45, 255]),
            nav_text: Color::from_srgb8([0xC8, 0xC8, 0xC8, 255]),

            title: Color::from_srgb8([0xEE, 0xEE, 0xEE, 255]),
            path: Color::from_srgb8([0x8C, 0x8C, 0x8C, 255]),
            divider: Color::from_srgb8([0x2A, 0x2A, 0x2A, 255]),

            accent: Color::from_srgb8([0x4A, 0x90, 0xD9, 255]),

            accent_width: 2.0,
            devider_width: 1.3,

            title_size: 15.0,
            body_size: 12.0,

            padding: 14.0,
            nav_width: 118.0,
            nav_row_height: 24.0,
            nav_corner_radius: 2.0,
            nav_shrink: 4.0,
            column_gap: 14.0,
            section_spacing: 8.0,
            scrollbar_gap: 8.0,

            footer_gap: 10.0,
            footer_height: 22.0,

            button_fill: Color::from_srgb8([0x45, 0x45, 0x45, 255]),
            button_hover: Color::from_srgb8([0x56, 0x56, 0x56, 255]),
            button_active: Color::from_srgb8([0x33, 0x33, 0x33, 255]),
            button_border: Color::from_srgb8([0x6E, 0x6E, 0x6E, 255]),
            button_border_width: 1.0,
            button_text: Color::from_srgb8([0xDC, 0xDC, 0xDC, 255]),
            button_height: 22.0,
            button_padding: 10.0,
            button_corner_radius: 2.0,
        }
    }

    fn light() -> Self {
        Self {
            background: Color::from_srgb8([0xEB, 0xEB, 0xEB, 255]),
            nav_background: Color::from_srgb8([0xE0, 0xE0, 0xE0, 255]),
            nav_hover: Color::from_srgb8([0xD5, 0xD5, 0xD5, 255]),
            nav_selected: Color::from_srgb8([0xC8, 0xC8, 0xC8, 255]),
            nav_text: Color::from_srgb8([0x2A, 0x2A, 0x2A, 255]),

            title: Color::from_srgb8([0x1E, 0x1E, 0x1E, 255]),
            path: Color::from_srgb8([0x6E, 0x6E, 0x6E, 255]),
            divider: Color::from_srgb8([0xC4, 0xC4, 0xC4, 255]),

            accent: Color::from_srgb8([0x2C, 0x5D, 0x87, 255]),

            button_fill: Color::from_srgb8([0xF5, 0xF5, 0xF5, 255]),
            button_hover: Color::WHITE,
            button_active: Color::from_srgb8([0xDC, 0xDC, 0xDC, 255]),
            button_border: Color::from_srgb8([0xA8, 0xA8, 0xA8, 255]),
            button_text: Color::from_srgb8([0x20, 0x20, 0x20, 255]),
            ..Self::dark()
        }
    }
}

impl PreferencesPanelStyle {
    /// Project the dialog-button tokens onto the reusable button widget.
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

    /// The left-hand list entry: flat until hovered or selected.
    pub fn nav_style(&self) -> ButtonStyle {
        ButtonStyle::new(TextStyle::sized(self.nav_text.to_egui(), self.body_size))
            .fills(
                egui::Color32::TRANSPARENT,
                self.nav_hover.to_egui(),
                self.nav_selected.to_egui(),
            )
            .corner_radius(CornerRadius::same(self.nav_corner_radius as u8))
            .padding(Vec2::new(8.0, 0.0))
            .height(self.nav_row_height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kairos_editor_widgets::style::StyleFile;

    #[test]
    fn the_preferences_style_projects_its_button_shapes() {
        let style = PreferencesPanelStyle::dark();

        let button = style.button_style();
        assert_eq!(button.fill, style.button_fill.to_egui());
        assert_eq!(
            button.stroke,
            egui::Stroke::new(style.button_border_width, style.button_border.to_egui()),
            "a dialog button is outlined, not a bare label"
        );
        assert_eq!(button.corner_radius, CornerRadius::same(2));

        let nav = style.nav_style();
        assert_eq!(nav.fill, egui::Color32::TRANSPARENT);
        assert_eq!(nav.hover_fill, style.nav_hover.to_egui());
        assert_eq!(nav.active_fill, style.nav_selected.to_egui());
    }

    /// The checked-in reference skin and the code defaults are the same skin.
    #[test]
    fn shipped_reference_skin_matches_the_code_defaults() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../Preferences/Styles/PreferencesPanel.ron");
        let file = StyleFile::<PreferencesPanelStyle>::load_from(&path)
            .expect("the reference skin parses");

        let defaults = StyleFile::<PreferencesPanelStyle>::defaults();
        assert_eq!(file.dark, defaults.dark, "dark skin drifted from code");
        assert_eq!(file.light, defaults.light, "light skin drifted from code");
    }
}
