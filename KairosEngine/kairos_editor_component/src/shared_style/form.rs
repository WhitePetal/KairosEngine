//! The form rows' shared skin.
//!
//! A form page is drawn by `kairos_editor_widgets::form::FormRows`: one row per
//! token, a foldout band per nested group, a band per list element. The widget
//! owns that layout; this is the serializable skin the editor owns — plain
//! colours and metrics, one file under `Preferences/Styles/EditorForm.ron`,
//! projected onto the widget tokens at the call site.
//!
//! It lives here, not in a panel's style, because the row grid is not a panel:
//! the Preferences editor draws every editable page with it today, an object
//! inspector (a form over `&mut` properties) will draw the same rows tomorrow.
//! One file restyles every property row in the editor. The value box is still
//! `EditorFieldStyle` and a list's element bands are still `ListStyle`; this
//! skin owns the page-level row grid, its two zebra greys and the foldout
//! header around them.

use kairos_color::Color;
use kairos_editor_widgets::color::ColorExt;
use kairos_editor_widgets::form::FormStyle;
use kairos_editor_widgets::style::Style;
use kairos_editor_widgets::text::TextStyle;
use serde::{Deserialize, Serialize};

use crate::shared_style::SharedStyle;
use crate::style_row::StyleRow;

/// Appearance of the form rows: the two zebra greys, the four column metrics,
/// the row height and the foldout header band.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, StyleRow)]
#[SharedStyle(FORM = "form", title = "Form")]
pub struct EditorFormStyle {
    /// Row band fill on the first, third, ... (even, zero-based) rows.
    pub row_odd: Color,
    /// Row band fill on the second, fourth, ... rows.
    pub row_even: Color,

    /// Foldout header text (a group of tokens, named "Icons").
    pub group: Color,
    /// Foldout header band. A band, not a label: the header is clickable.
    pub group_background: Color,
    pub group_hover: Color,
    /// Foldout header text size.
    pub group_size: f32,

    /// Ideal width of a row's label column; the value box starts after it.
    pub label_width: f32,
    /// Minimum width left for the value box; below it the label column
    /// shrinks instead of the box clipping off the right edge.
    pub field_width: f32,
    /// Gap between the label column and the value box.
    pub label_gap: f32,
    /// Height of one field row.
    pub row_height: f32,

    /// Height of a foldout header row.
    pub group_height: f32,
    /// Indent of one nesting level in the token tree.
    pub group_indent: f32,
    /// Left and right content inset inside a row band.
    pub row_inset: f32,
}

impl EditorFormStyle {
    /// Project the serializable tokens onto the widget's runtime style.
    pub fn tokens(&self) -> FormStyle {
        FormStyle {
            row_odd: self.row_odd.to_egui(),
            row_even: self.row_even.to_egui(),
            row_height: self.row_height,
            row_inset: self.row_inset,
            label_width: self.label_width,
            field_width: self.field_width,
            label_gap: self.label_gap,
            group_text: TextStyle::sized(self.group.to_egui(), self.group_size),
            group_fill: self.group_background.to_egui(),
            group_hover: self.group_hover.to_egui(),
            group_height: self.group_height,
            group_indent: self.group_indent,
        }
    }
}

impl Style for EditorFormStyle {
    const FILE: &'static str = "EditorForm.ron";

    fn dark() -> Self {
        Self {
            row_odd: Color::from_srgb8([0x38, 0x38, 0x38, 255]),
            row_even: Color::from_srgb8([0x33, 0x33, 0x33, 255]),

            group: Color::from_srgb8([0xDC, 0xDC, 0xDC, 255]),
            group_background: Color::from_srgb8([0x3C, 0x3C, 0x3C, 255]),
            group_hover: Color::from_srgb8([0x46, 0x46, 0x46, 255]),
            group_size: 12.0,

            label_width: 170.0,
            field_width: 120.0,
            label_gap: 8.0,
            row_height: 20.0,

            group_height: 20.0,
            group_indent: 16.0,
            row_inset: 8.0,
        }
    }

    fn light() -> Self {
        Self {
            row_odd: Color::from_srgb8([0xF6, 0xF6, 0xF6, 255]),
            row_even: Color::from_srgb8([0xED, 0xED, 0xED, 255]),

            group: Color::from_srgb8([0x1E, 0x1E, 0x1E, 255]),
            group_background: Color::from_srgb8([0xDC, 0xDC, 0xDC, 255]),
            group_hover: Color::from_srgb8([0xD2, 0xD2, 0xD2, 255]),

            ..Self::dark()
        }
    }
}

impl SharedStyle for EditorFormStyle {}

#[cfg(test)]
mod tests {
    use kairos_editor_widgets::style::StyleFile;

    use super::*;

    #[test]
    fn the_shared_skin_projects_every_token_onto_the_form() {
        let skin = EditorFormStyle::dark();
        let tokens = skin.tokens();

        assert_eq!(tokens.row_odd, skin.row_odd.to_egui());
        assert_eq!(tokens.row_even, skin.row_even.to_egui());
        assert_eq!(tokens.row_height, skin.row_height);
        assert_eq!(tokens.row_inset, skin.row_inset);
        assert_eq!(tokens.label_width, skin.label_width);
        assert_eq!(tokens.field_width, skin.field_width);
        assert_eq!(tokens.label_gap, skin.label_gap);
        assert_eq!(tokens.group_text.text_color(), skin.group.to_egui());
        assert_eq!(tokens.group_fill, skin.group_background.to_egui());
        assert_eq!(tokens.group_hover, skin.group_hover.to_egui());
        assert_eq!(tokens.group_height, skin.group_height);
        assert_eq!(tokens.group_indent, skin.group_indent);
    }

    /// The property list is a Unity-style zebra: two greys, one per row.
    #[test]
    fn field_rows_alternate_between_two_greys() {
        let skin = EditorFormStyle::dark();
        let tokens = skin.tokens();

        assert_eq!(tokens.row_fill(0), skin.row_odd.to_egui());
        assert_eq!(tokens.row_fill(1), skin.row_even.to_egui());
        assert_eq!(tokens.row_fill(2), skin.row_odd.to_egui());
        assert_ne!(skin.row_odd, skin.row_even);
    }

    /// The checked-in reference skin and the code defaults are the same skin.
    #[test]
    fn shipped_reference_skin_matches_the_code_defaults() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../Preferences/Styles/EditorForm.ron");
        let file =
            StyleFile::<EditorFormStyle>::load_from(&path).expect("the reference skin parses");

        let defaults = StyleFile::<EditorFormStyle>::defaults();
        assert_eq!(file.dark, defaults.dark, "dark skin drifted from code");
        assert_eq!(file.light, defaults.light, "light skin drifted from code");
    }
}
