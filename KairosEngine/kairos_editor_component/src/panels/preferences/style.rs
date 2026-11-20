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
use kairos_color::Color;
use kairos_editor_widgets::button::ButtonStyle;
use kairos_editor_widgets::color::ColorExt;
use kairos_editor_widgets::style::Style;
use kairos_editor_widgets::text::TextStyle;
use serde::{Deserialize, Serialize};

use crate::skin::StyleFields;

/// Appearance tokens for the Preferences panel: palette, type sizes, the
/// two-column geometry and the three button shapes it paints.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, StyleFields)]
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
    /// Foldout header text (a group of tokens, named "Icons").
    pub group: Color,
    /// Foldout header band. A band, not a label: the header is clickable.
    pub group_background: Color,
    pub group_hover: Color,
    /// Row name (the token).
    pub label: Color,
    /// Row name while its value is selected: Unity's blue label.
    pub label_active: Color,
    /// Rules between the heading, the rows and the footer.
    pub divider: Color,

    /// The two field-row greys, alternating row by row. Unity's inspector
    /// stripes its property list to keep long pages readable.
    pub row_odd: Color,
    pub row_even: Color,

    /// Background of a value box.
    pub control_rail: Color,
    pub control_hover: Color,
    pub control_active: Color,
    /// Value-box outline.
    pub control_edge: Color,
    /// Value-box outline under the pointer, before it is selected.
    pub control_edge_hover: Color,
    /// Text of a control (numbers, token values).
    pub control_value: Color,
    /// Selected outline, active slider fill, and the page-list stripe.
    pub accent: Color,

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
    /// Space between rows. The default is zero: rows are contiguous bands.
    pub row_gap: f32,
    /// Ideal width of a row's label column; the value box starts after it.
    pub label_width: f32,
    /// Minimum width left for the value box; below it the label column
    /// shrinks instead of the box clipping off the right edge.
    pub field_width: f32,
    /// Gap between the label column and the value box.
    pub label_gap: f32,
    /// Height of one field row.
    pub row_height: f32,
    /// Height of a value box (centred in its row).
    pub field_height: f32,
    /// Horizontal inner padding of a value box.
    pub field_padding: f32,
    pub control_border_width: f32,
    pub control_corner_radius: f32,

    /// Height of a foldout header row.
    pub group_height: f32,
    /// Indent of one nesting level in the token tree.
    pub group_indent: f32,
    /// Left and right content inset inside a row band.
    pub row_inset: f32,
    /// Right-hand gutter kept clear of the page scroll bar.
    pub scrollbar_gap: f32,
    /// Side of the square boolean checkbox.
    pub checkbox_size: f32,

    /// Band of a selected list element (Unity's inspector blue).
    pub list_selection: Color,
    /// Height of the list header's add/remove control.
    pub list_button_height: f32,
    /// Width of one cell of that control; it is two cells side by side.
    pub list_button_cell_width: f32,
    /// Left gutter of a list element, holding the drag handle. The divider that
    /// separates it from the element's content sits at its right edge.
    pub list_gutter_width: f32,
    /// Width of the list header's size box.
    pub list_count_width: f32,

    /// Space between the last row and the rule above the footer.
    pub footer_gap: f32,
    pub footer_height: f32,

    /// Dialog buttons (Close): filled and outlined, so they do not read as
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
            group: Color::from_srgb8([0xDC, 0xDC, 0xDC, 255]),
            group_background: Color::from_srgb8([0x3C, 0x3C, 0x3C, 255]),
            group_hover: Color::from_srgb8([0x46, 0x46, 0x46, 255]),
            label: Color::from_srgb8([0xC8, 0xC8, 0xC8, 255]),
            label_active: Color::from_srgb8([0x8F, 0xB4, 0xD9, 255]),
            divider: Color::from_srgb8([0x2A, 0x2A, 0x2A, 255]),

            row_odd: Color::from_srgb8([0x38, 0x38, 0x38, 255]),
            row_even: Color::from_srgb8([0x33, 0x33, 0x33, 255]),

            control_rail: Color::from_srgb8([0x28, 0x28, 0x28, 255]),
            control_hover: Color::from_srgb8([0x3E, 0x3E, 0x3E, 255]),
            control_active: Color::from_srgb8([0x4A, 0x4A, 0x4A, 255]),
            control_edge: Color::from_srgb8([0x20, 0x20, 0x20, 255]),
            control_edge_hover: Color::from_srgb8([0x5A, 0x5A, 0x5A, 255]),
            control_value: Color::from_srgb8([0xDC, 0xDC, 0xDC, 255]),
            accent: Color::from_srgb8([0x4A, 0x90, 0xD9, 255]),

            title_size: 15.0,
            body_size: 12.0,
            group_size: 12.0,

            padding: 14.0,
            nav_width: 118.0,
            nav_row_height: 24.0,
            nav_corner_radius: 2.0,
            column_gap: 14.0,
            section_spacing: 8.0,
            row_gap: 0.0,
            label_width: 170.0,
            field_width: 120.0,
            label_gap: 8.0,
            row_height: 20.0,
            field_height: 18.0,
            field_padding: 6.0,
            control_border_width: 1.0,
            control_corner_radius: 0.0,

            group_height: 20.0,
            group_indent: 16.0,
            row_inset: 8.0,
            scrollbar_gap: 8.0,
            checkbox_size: 15.0,

            list_selection: Color::from_srgb8([0x2C, 0x5D, 0x87, 255]),
            list_button_height: 18.0,
            list_button_cell_width: 24.0,
            list_gutter_width: 20.0,
            list_count_width: 48.0,

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
            group: Color::from_srgb8([0x1E, 0x1E, 0x1E, 255]),
            group_background: Color::from_srgb8([0xDC, 0xDC, 0xDC, 255]),
            group_hover: Color::from_srgb8([0xD2, 0xD2, 0xD2, 255]),
            label: Color::from_srgb8([0x1E, 0x1E, 0x1E, 255]),
            label_active: Color::from_srgb8([0x2C, 0x5D, 0x87, 255]),
            divider: Color::from_srgb8([0xC4, 0xC4, 0xC4, 255]),

            row_odd: Color::from_srgb8([0xF6, 0xF6, 0xF6, 255]),
            row_even: Color::from_srgb8([0xED, 0xED, 0xED, 255]),

            control_rail: Color::WHITE,
            control_hover: Color::from_srgb8([0xF0, 0xF0, 0xF0, 255]),
            control_active: Color::from_srgb8([0xE0, 0xE0, 0xE0, 255]),
            control_edge: Color::from_srgb8([0xA8, 0xA8, 0xA8, 255]),
            control_edge_hover: Color::from_srgb8([0x7A, 0x7A, 0x7A, 255]),
            control_value: Color::from_srgb8([0x1E, 0x1E, 0x1E, 255]),
            accent: Color::from_srgb8([0x2C, 0x5D, 0x87, 255]),
            list_selection: Color::from_srgb8([0xB9, 0xD7, 0xF2, 255]),

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

    /// The band behind a field row: two greys, alternating row by row.
    pub fn row_fill(&self, row: usize) -> Color {
        if row.is_multiple_of(2) {
            self.row_odd
        } else {
            self.row_even
        }
    }

    /// Outline of a value box: blue while selected, a lighter edge under the
    /// pointer, the quiet edge otherwise.
    pub fn value_edge(&self, selected: bool, hovered: bool) -> Color {
        if selected {
            self.accent
        } else if hovered {
            self.control_edge_hover
        } else {
            self.control_edge
        }
    }

    /// Row name colour: Unity turns the label blue while the field is active.
    pub fn label_color(&self, active: bool) -> Color {
        if active {
            self.label_active
        } else {
            self.label
        }
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

    /// The property list is a Unity-style Zebra: two greys, one per row, and a
    /// blue outline only once the field is selected.
    #[test]
    fn field_rows_alternate_and_selected_values_get_the_accent_edge() {
        let style = PreferencesPanelStyle::dark();

        assert_eq!(style.row_fill(0), style.row_odd);
        assert_eq!(style.row_fill(1), style.row_even);
        assert_eq!(style.row_fill(2), style.row_odd);
        assert_ne!(style.row_odd, style.row_even);

        assert_eq!(style.value_edge(false, false), style.control_edge);
        assert_eq!(
            style.value_edge(false, true),
            style.control_edge_hover,
            "hover lightens the edge before the field is selected"
        );
        assert_eq!(
            style.value_edge(true, false),
            style.accent,
            "a selected field keeps its blue outline regardless of hover"
        );
        assert_eq!(style.label_color(true), style.label_active);
        assert_eq!(style.label_color(false), style.label);
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
        assert!(
            fields
                .iter()
                .any(|field| field.path == "button_corner_radius")
        );
    }
}
