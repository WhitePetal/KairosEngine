//! The field widgets' shared skin.
//!
//! Value boxes, checkboxes and row names are drawn by
//! `kairos_editor_widgets::field`, which reads runtime tokens (`Color32`,
//! `TextStyle`). Those are not serde-friendly, so this is the serializable skin
//! the editor owns: plain colours and metrics, one file under
//! `Preferences/Styles/EditorField.ron`, projected onto the widget tokens at
//! the call site. Every panel that draws fields reads the same file — the
//! Preferences editor today, an object inspector next — so a field looks the
//! same everywhere and can be restyled in one place.

use kairos_color::Color;
use kairos_editor_widgets::color::ColorExt;
use kairos_editor_widgets::field::FieldStyle;
use kairos_editor_widgets::style::Style;
use kairos_editor_widgets::text::TextStyle;
use serde::{Deserialize, Serialize};

use crate::shared_style::SharedStyle;
use crate::skin::StyleFields;

/// Appearance of one value field: its box, its name, and the wash that marks a
/// selected value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, StyleFields)]
#[SharedStyle(EDITOR_FIELD = "editor_field", title = "Field")]
pub struct EditorFieldStyle {
    /// Value text.
    pub text: Color,
    /// Row name at rest.
    pub label: Color,
    /// Row name while the field is selected or being edited.
    pub label_active: Color,
    /// Box fill at rest.
    pub fill: Color,
    /// Box fill under the pointer, before the field is selected.
    pub hover_fill: Color,
    /// Outline at rest.
    pub edge: Color,
    /// Outline under the pointer, before the field is selected.
    pub edge_hover: Color,
    /// Outline of an active field: selected, focused, or being edited.
    pub accent: Color,

    /// Value and name text size.
    pub text_size: f32,
    pub border_width: f32,
    pub corner_radius: u8,
    /// Horizontal inset of the value text.
    pub padding: f32,
    /// Height of a value box; rows centre it in their band.
    pub height: f32,
    /// Side of a boolean checkbox.
    pub checkbox: f32,
}

impl EditorFieldStyle {
    /// Project the serializable tokens onto the widget's runtime style.
    pub fn tokens(&self) -> FieldStyle {
        FieldStyle::new(TextStyle::sized(self.text.to_egui(), self.text_size))
            .labels(self.label.to_egui(), self.label_active.to_egui())
            .fills(self.fill.to_egui(), self.hover_fill.to_egui())
            .edges(
                self.edge.to_egui(),
                self.edge_hover.to_egui(),
                self.accent.to_egui(),
            )
            .border_width(self.border_width)
            .corner_radius(self.corner_radius)
            .padding(self.padding)
            .height(self.height)
            .checkbox(self.checkbox)
    }
}

impl Style for EditorFieldStyle {
    const FILE: &'static str = "EditorField.ron";

    fn dark() -> Self {
        Self {
            text: Color::from_srgb8([0xDC, 0xDC, 0xDC, 255]),
            label: Color::from_srgb8([0xC8, 0xC8, 0xC8, 255]),
            label_active: Color::from_srgb8([0x8F, 0xB4, 0xD9, 255]),
            fill: Color::from_srgb8([0x28, 0x28, 0x28, 255]),
            hover_fill: Color::from_srgb8([0x3E, 0x3E, 0x3E, 255]),
            edge: Color::from_srgb8([0x20, 0x20, 0x20, 255]),
            edge_hover: Color::from_srgb8([0x5A, 0x5A, 0x5A, 255]),
            accent: Color::from_srgb8([0x4A, 0x90, 0xD9, 255]),

            text_size: 12.0,
            border_width: 1.0,
            corner_radius: 0,
            padding: 6.0,
            height: 18.0,
            checkbox: 15.0,
        }
    }

    fn light() -> Self {
        Self {
            text: Color::from_srgb8([0x1E, 0x1E, 0x1E, 255]),
            label: Color::from_srgb8([0x1E, 0x1E, 0x1E, 255]),
            label_active: Color::from_srgb8([0x2C, 0x5D, 0x87, 255]),
            fill: Color::WHITE,
            hover_fill: Color::from_srgb8([0xF0, 0xF0, 0xF0, 255]),
            edge: Color::from_srgb8([0xA8, 0xA8, 0xA8, 255]),
            edge_hover: Color::from_srgb8([0x7A, 0x7A, 0x7A, 255]),
            accent: Color::from_srgb8([0x2C, 0x5D, 0x87, 255]),
            ..Self::dark()
        }
    }
}

impl SharedStyle for EditorFieldStyle {}

#[cfg(test)]
mod tests {
    use super::*;
    use kairos_editor_widgets::style::StyleFile;

    #[test]
    fn the_shared_skin_projects_every_token_onto_the_widget_style() {
        let skin = EditorFieldStyle::dark();
        let tokens = skin.tokens();

        assert_eq!(tokens.text.text_color(), skin.text.to_egui());
        assert_eq!(tokens.label, skin.label.to_egui());
        assert_eq!(tokens.label_active, skin.label_active.to_egui());
        assert_eq!(tokens.fill, skin.fill.to_egui());
        assert_eq!(tokens.hover_fill, skin.hover_fill.to_egui());
        assert_eq!(tokens.edge, skin.edge.to_egui());
        assert_eq!(tokens.edge_hover, skin.edge_hover.to_egui());
        assert_eq!(tokens.accent, skin.accent.to_egui());
        assert_eq!(tokens.height, skin.height);
        assert_eq!(tokens.checkbox, skin.checkbox);
        assert_eq!(tokens.corner_radius, skin.corner_radius);
    }

    #[test]
    fn shipped_reference_skin_matches_the_code_defaults() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../Preferences/Styles/EditorField.ron");
        let file = StyleFile::<EditorFieldStyle>::load_from(&path).expect("the skin parses");

        assert_eq!(file.dark, EditorFieldStyle::dark(), "dark skin drifted");
        assert_eq!(file.light, EditorFieldStyle::light(), "light skin drifted");
    }
}
