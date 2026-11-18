//! Preferences panel appearance.
//!
//! Code defaults for `PreferencesPanelStyle`, persisted per component to
//! `Preferences/Styles/PreferencesPanel.ron` (dark + light). The panel loads the
//! file in `Panel::init`. Because the type derives `StyleFields`, the panel can
//! also edit *itself* in its own field editor — the same file, the same tokens.
//!
//! The palette deliberately repeats the About panel's: both are dialog-shaped
//! panels on the same dark chrome, and the component convention is one skin per
//! panel. When a second dialog panel appears, the shared tokens move into a
//! dialog skin — not before.

use egui::{CornerRadius, Vec2};
use kairos_color::Color32;
use kairos_editor_widgets::button::ButtonStyle;
use kairos_editor_widgets::color::Color32Ext;
use kairos_editor_widgets::style::Style;
use kairos_editor_widgets::text::TextStyle;
use serde::{Deserialize, Serialize};

use crate::skin::StyleFields;

/// Appearance tokens for the Preferences panel: palette, type sizes, the
/// two-column geometry and the three button shapes it paints.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, StyleFields)]
pub struct PreferencesPanelStyle {
    /// Panel body, behind everything.
    pub background: Color32,
    /// Left-hand page list column.
    pub nav_background: Color32,
    pub nav_hover: Color32,
    /// Fill of the current page's list entry.
    pub nav_selected: Color32,
    pub nav_text: Color32,

    /// Page heading (the panel being edited).
    pub title: Color32,
    /// The RON file the page writes to, under the heading.
    pub path: Color32,
    /// Group heading inside a page ("Icons").
    pub group: Color32,
    /// Row name (the token).
    pub label: Color32,
    /// Rules between the heading, the rows and the footer.
    pub divider: Color32,

    /// Background of a numeric field and slider rail.
    pub control_rail: Color32,
    pub control_hover: Color32,
    pub control_active: Color32,
    /// Field outline.
    pub control_edge: Color32,
    /// Text of a control (numbers, token values).
    pub control_value: Color32,
    /// Filled part of a slider; also the selected-toggle fill.
    pub accent: Color32,

    pub title_size: f32,
    pub body_size: f32,
    pub group_size: f32,

    /// Inner margin of the panel body.
    pub padding: f32,
    /// Width of the page-list column.
    pub nav_width: f32,
    pub nav_row_height: f32,
    pub nav_corner_radius: f32,
    /// Gap between the page list and the page body.
    pub column_gap: f32,
    /// Space above and below the rule under a page heading.
    pub section_spacing: f32,
    /// Space between two rows.
    pub row_gap: f32,
    /// Width of a row's label column; the control starts after it.
    pub label_width: f32,
    /// Width of a numeric/text field.
    pub field_width: f32,
    /// Height of one row (label and control share it).
    pub row_height: f32,
    /// Space between the last row and the rule above the footer.
    pub footer_gap: f32,
    pub footer_height: f32,

    /// Dialog buttons (Close): filled and outlined, so they do not read as
    /// labels on the panel background.
    pub button_fill: Color32,
    pub button_hover: Color32,
    pub button_active: Color32,
    pub button_border: Color32,
    pub button_border_width: f32,
    pub button_text: Color32,
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
            background: Color32::from_rgb(0x36, 0x36, 0x36),
            nav_background: Color32::from_rgb(0x2E, 0x2E, 0x2E),
            nav_hover: Color32::from_rgb(0x3E, 0x3E, 0x3E),
            nav_selected: Color32::from_rgb(0x45, 0x45, 0x45),
            nav_text: Color32::from_rgb(0xC8, 0xC8, 0xC8),

            title: Color32::from_rgb(0xEE, 0xEE, 0xEE),
            path: Color32::from_rgb(0x8C, 0x8C, 0x8C),
            group: Color32::from_rgb(0x9A, 0x9A, 0x9A),
            label: Color32::from_rgb(0xC8, 0xC8, 0xC8),
            divider: Color32::from_rgb(0x2A, 0x2A, 0x2A),

            control_rail: Color32::from_rgb(0x2A, 0x2A, 0x2A),
            control_hover: Color32::from_rgb(0x44, 0x44, 0x44),
            control_active: Color32::from_rgb(0x56, 0x56, 0x56),
            control_edge: Color32::from_rgb(0x4E, 0x4E, 0x4E),
            control_value: Color32::from_rgb(0xDC, 0xDC, 0xDC),
            accent: Color32::from_rgb(0x4A, 0x90, 0xD9),

            title_size: 15.0,
            body_size: 12.0,
            group_size: 11.0,

            padding: 14.0,
            nav_width: 118.0,
            nav_row_height: 24.0,
            nav_corner_radius: 2.0,
            column_gap: 14.0,
            section_spacing: 8.0,
            row_gap: 4.0,
            label_width: 170.0,
            field_width: 120.0,
            row_height: 20.0,
            footer_gap: 10.0,
            footer_height: 22.0,

            button_fill: Color32::from_rgb(0x45, 0x45, 0x45),
            button_hover: Color32::from_rgb(0x56, 0x56, 0x56),
            button_active: Color32::from_rgb(0x33, 0x33, 0x33),
            button_border: Color32::from_rgb(0x6E, 0x6E, 0x6E),
            button_border_width: 1.0,
            button_text: Color32::from_rgb(0xDC, 0xDC, 0xDC),
            button_height: 22.0,
            button_padding: 10.0,
            button_corner_radius: 2.0,
        }
    }

    fn light() -> Self {
        Self {
            background: Color32::from_rgb(0xEB, 0xEB, 0xEB),
            nav_background: Color32::from_rgb(0xE0, 0xE0, 0xE0),
            nav_hover: Color32::from_rgb(0xD5, 0xD5, 0xD5),
            nav_selected: Color32::from_rgb(0xC8, 0xC8, 0xC8),
            nav_text: Color32::from_rgb(0x2A, 0x2A, 0x2A),

            title: Color32::from_rgb(0x1E, 0x1E, 0x1E),
            path: Color32::from_rgb(0x6E, 0x6E, 0x6E),
            group: Color32::from_rgb(0x5A, 0x5A, 0x5A),
            label: Color32::from_rgb(0x1E, 0x1E, 0x1E),
            divider: Color32::from_rgb(0xC4, 0xC4, 0xC4),

            control_rail: Color32::from_rgb(0xF5, 0xF5, 0xF5),
            control_hover: Color32::from_rgb(0xDE, 0xDE, 0xDE),
            control_active: Color32::from_rgb(0xC8, 0xC8, 0xC8),
            control_edge: Color32::from_rgb(0xA8, 0xA8, 0xA8),
            control_value: Color32::from_rgb(0x1E, 0x1E, 0x1E),
            accent: Color32::from_rgb(0x2C, 0x5D, 0x87),

            button_fill: Color32::from_rgb(0xF5, 0xF5, 0xF5),
            button_hover: Color32::WHITE,
            button_active: Color32::from_rgb(0xDC, 0xDC, 0xDC),
            button_border: Color32::from_rgb(0xA8, 0xA8, 0xA8),
            button_text: Color32::from_rgb(0x20, 0x20, 0x20),
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

    /// The boolean control: a filled button whose selected fill is the accent,
    /// so "On" is visible at a glance instead of being a tick mark.
    pub fn toggle_style(&self) -> ButtonStyle {
        ButtonStyle::new(TextStyle::sized(self.control_value.to_egui(), self.body_size))
            .fills(
                self.button_fill.to_egui(),
                self.button_hover.to_egui(),
                self.accent.to_egui(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use kairos_editor_widgets::style::StyleFile;

    #[test]
    fn the_preferences_style_projects_three_button_shapes() {
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

        let toggle = style.toggle_style();
        assert_eq!(toggle.active_fill, style.accent.to_egui());
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

    /// The panel edits itself: every token is reachable through the same field
    /// table the editor draws.
    #[test]
    fn the_style_describes_its_own_tokens() {
        let style = PreferencesPanelStyle::dark();
        let fields = style.style_fields();

        assert!(fields.iter().any(|field| field.path == "nav_width"));
        assert!(fields.iter().any(|field| field.path == "background"));
        assert!(fields.iter().any(|field| field.path == "button_corner_radius"));
    }
}
