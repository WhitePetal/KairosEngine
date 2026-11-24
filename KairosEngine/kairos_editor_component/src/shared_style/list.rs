//! The reorderable list's shared skin.
//!
//! A list is chrome plus a caller's rows: the header, the count box, the
//! add/remove pair, the element bands and the floating drag ghost. The rows
//! themselves are value fields and keep reading `EditorFieldStyle`, so this
//! skin owns only the list around them.
//!
//! It lives here, not in a panel's style, because the same widget appears in
//! Preferences today and in an object inspector later: one file under
//! Preferences/Styles/List.ron makes every list look the same and restyles
//! them in one place. [ListChromeStyle] stays the runtime token struct; the
//! serde skin projects onto it with [ListStyle::tokens], which takes the
//! current [FieldStyle] for the size box and for the font family.

use kairos_color::Color;
use kairos_editor_widgets::color::ColorExt;
use kairos_editor_widgets::field::FieldStyle;
use kairos_editor_widgets::field::ListChromeStyle;
use kairos_editor_widgets::style::Style;
use serde::{Deserialize, Serialize};

use crate::shared_style::SharedStyle;
use crate::style_row::StyleRow;

/// Appearance of a reorderable list: header, element bands, stripes, selection,
/// drag handle, container rules and the add/remove cells.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, StyleRow)]
#[SharedStyle(LIST = "list", title = "List")]
pub struct ListStyle {
    /// Header name.
    pub header_text: Color,
    /// Element name at rest.
    pub band_text: Color,
    /// Element name while its band is selected.
    pub band_text_selected: Color,

    /// Header band at rest and under the pointer.
    pub header_fill: Color,
    pub header_hover: Color,
    /// Element band at rest and under the pointer.
    pub band_fill: Color,
    pub band_hover: Color,
    /// The second grey of an element body: its field rows stripe between
    /// [Self::band_fill] and this.
    pub band_alt: Color,
    /// Selected element band.
    pub selection: Color,
    /// Drag-handle ink at rest.
    pub handle: Color,
    /// Dark rules: the container frame and the block's bottom edges.
    pub container_border: Color,
    /// Outline of the floating drag ghost.
    pub container_bevel: Color,

    /// Header name size.
    pub header_text_size: f32,
    /// Element name size.
    pub band_text_size: f32,
    /// Height of one element field row.
    pub row_height: f32,
    /// Height of the list header and of a foldout band.
    pub group_height: f32,
    /// Indent of one nesting level inside an element.
    pub group_indent: f32,
    /// Left and right content inset inside an element row.
    pub row_inset: f32,
    /// Left gutter of an element, holding the drag handle.
    pub gutter_width: f32,
    /// Width of one add/remove cell; the pair is two cells side by side.
    pub button_cell: f32,
    /// Height of the add/remove pair.
    pub button_height: f32,
    /// Width of the header's inline count box.
    pub count_width: f32,

    /// Add/remove cell at rest and under the pointer.
    pub button_fill: Color,
    pub button_hover: Color,
    /// Cell outline; the list closes its container with the same dark rule.
    pub button_border: Color,
    pub button_border_width: f32,
    pub button_text: Color,
}

impl ListStyle {
    /// Project the serializable tokens onto the widget's runtime chrome.
    ///
    /// The field style supplies the value boxes, their font family and the
    /// count editor's height; the list tokens own every colour and metric
    /// around them.
    pub fn tokens(&self, field: &FieldStyle) -> ListChromeStyle {
        ListChromeStyle {
            field: field.clone(),
            header_text: field
                .text
                .clone()
                .color(self.header_text.to_egui())
                .size(self.header_text_size),
            band_text: field
                .text
                .clone()
                .color(self.band_text.to_egui())
                .size(self.band_text_size),
            band_text_selected: field
                .text
                .clone()
                .color(self.band_text_selected.to_egui())
                .size(self.band_text_size),
            header_fill: self.header_fill.to_egui(),
            header_hover: self.header_hover.to_egui(),
            band_fill: self.band_fill.to_egui(),
            band_hover: self.band_hover.to_egui(),
            band_alt: self.band_alt.to_egui(),
            selection: self.selection.to_egui(),
            handle: self.handle.to_egui(),
            container_border: self.container_border.to_egui(),
            container_bevel: self.container_bevel.to_egui(),
            row_height: self.row_height,
            group_height: self.group_height,
            group_indent: self.group_indent,
            row_inset: self.row_inset,
            gutter_width: self.gutter_width,
            button_cell: self.button_cell,
            button_height: self.button_height,
            count_width: self.count_width,
            button_fill: self.button_fill.to_egui(),
            button_hover: self.button_hover.to_egui(),
            button_border: self.button_border.to_egui(),
            button_border_width: self.button_border_width,
            button_text: self.button_text.to_egui(),
        }
    }
}

impl Style for ListStyle {
    const FILE: &'static str = "List.ron";

    fn dark() -> Self {
        Self {
            header_text: Color::from_srgb8([0xDC, 0xDC, 0xDC, 255]),
            band_text: Color::from_srgb8([0xC8, 0xC8, 0xC8, 255]),
            band_text_selected: Color::from_srgb8([0x8F, 0xB4, 0xD9, 255]),

            header_fill: Color::from_srgb8([0x3C, 0x3C, 0x3C, 255]),
            header_hover: Color::from_srgb8([0x46, 0x46, 0x46, 255]),
            band_fill: Color::from_srgb8([0x46, 0x46, 0x46, 255]),
            band_hover: Color::from_srgb8([0x56, 0x56, 0x56, 255]),
            band_alt: Color::from_srgb8([0x4D, 0x4D, 0x4D, 255]),
            selection: Color::from_srgb8([0x2C, 0x5D, 0x87, 255]),
            handle: Color::from_srgb8([0x5A, 0x5A, 0x5A, 255]),
            container_border: Color::from_srgb8([0x2A, 0x2A, 0x2A, 255]),
            container_bevel: Color::from_srgb8([0x5A, 0x5A, 0x5A, 255]),

            header_text_size: 12.0,
            band_text_size: 12.0,
            row_height: 20.0,
            group_height: 20.0,
            group_indent: 16.0,
            row_inset: 8.0,
            gutter_width: 20.0,
            button_cell: 24.0,
            button_height: 18.0,
            count_width: 48.0,

            button_fill: Color::from_srgb8([0x45, 0x45, 0x45, 255]),
            button_hover: Color::from_srgb8([0x56, 0x56, 0x56, 255]),
            button_border: Color::from_srgb8([0x2A, 0x2A, 0x2A, 255]),
            button_border_width: 1.0,
            button_text: Color::from_srgb8([0xDC, 0xDC, 0xDC, 255]),
        }
    }

    fn light() -> Self {
        Self {
            header_text: Color::from_srgb8([0x1E, 0x1E, 0x1E, 255]),
            band_text: Color::from_srgb8([0x1E, 0x1E, 0x1E, 255]),
            band_text_selected: Color::from_srgb8([0x2C, 0x5D, 0x87, 255]),

            header_fill: Color::from_srgb8([0xDC, 0xDC, 0xDC, 255]),
            header_hover: Color::from_srgb8([0xD2, 0xD2, 0xD2, 255]),
            band_fill: Color::from_srgb8([0xD2, 0xD2, 0xD2, 255]),
            band_hover: Color::WHITE,
            band_alt: Color::from_srgb8([0xD9, 0xD9, 0xD9, 255]),
            selection: Color::from_srgb8([0xB9, 0xD7, 0xF2, 255]),
            handle: Color::from_srgb8([0x7A, 0x7A, 0x7A, 255]),
            container_border: Color::from_srgb8([0xC4, 0xC4, 0xC4, 255]),
            container_bevel: Color::from_srgb8([0x7A, 0x7A, 0x7A, 255]),

            button_fill: Color::from_srgb8([0xF5, 0xF5, 0xF5, 255]),
            button_hover: Color::WHITE,
            button_border: Color::from_srgb8([0xC4, 0xC4, 0xC4, 255]),
            button_text: Color::from_srgb8([0x20, 0x20, 0x20, 255]),
            ..Self::dark()
        }
    }
}

impl SharedStyle for ListStyle {}

#[cfg(test)]
mod tests {
    use egui::Color32;
    use kairos_editor_widgets::style::StyleFile;

    use super::*;

    #[test]
    fn the_shared_skin_projects_every_token_onto_the_chrome() {
        let skin = ListStyle::dark();
        let field = FieldStyle::default();
        let chrome = skin.tokens(&field);

        assert_eq!(chrome.header_text.text_color(), skin.header_text.to_egui());
        assert_eq!(chrome.band_text.text_color(), skin.band_text.to_egui());
        assert_eq!(
            chrome.band_text_selected.text_color(),
            skin.band_text_selected.to_egui()
        );
        assert_eq!(chrome.header_fill, skin.header_fill.to_egui());
        assert_eq!(chrome.header_hover, skin.header_hover.to_egui());
        assert_eq!(chrome.band_fill, skin.band_fill.to_egui());
        assert_eq!(chrome.band_hover, skin.band_hover.to_egui());
        assert_eq!(chrome.band_alt, skin.band_alt.to_egui());
        assert_eq!(chrome.selection, skin.selection.to_egui());
        assert_eq!(chrome.handle, skin.handle.to_egui());
        assert_eq!(chrome.container_border, skin.container_border.to_egui());
        assert_eq!(chrome.container_bevel, skin.container_bevel.to_egui());
        assert_eq!(chrome.row_height, skin.row_height);
        assert_eq!(chrome.group_height, skin.group_height);
        assert_eq!(chrome.group_indent, skin.group_indent);
        assert_eq!(chrome.row_inset, skin.row_inset);
        assert_eq!(chrome.gutter_width, skin.gutter_width);
        assert_eq!(chrome.button_cell, skin.button_cell);
        assert_eq!(chrome.button_height, skin.button_height);
        assert_eq!(chrome.count_width, skin.count_width);
        assert_eq!(chrome.button_fill, skin.button_fill.to_egui());
        assert_eq!(chrome.button_hover, skin.button_hover.to_egui());
        assert_eq!(chrome.button_border, skin.button_border.to_egui());
        assert_eq!(chrome.button_border_width, skin.button_border_width);
        assert_eq!(chrome.button_text, skin.button_text.to_egui());
        assert_eq!(
            chrome.field.height, field.height,
            "the value boxes keep their skin"
        );
    }

    /// The list page must be able to restyle without moving the field page:
    /// changing the field label ink changes a row's name, never the list band.
    #[test]
    fn the_list_skin_owns_the_band_ink_not_the_field_skin() {
        let skin = ListStyle::dark();
        let field = FieldStyle {
            label: Color32::RED,
            label_active: Color32::GREEN,
            ..FieldStyle::default()
        };

        let chrome = skin.tokens(&field);

        assert_eq!(chrome.band_text.text_color(), skin.band_text.to_egui());
        assert_eq!(
            chrome.band_text_selected.text_color(),
            skin.band_text_selected.to_egui()
        );
    }

    #[test]
    fn shipped_reference_skin_matches_the_code_defaults() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../Preferences/Styles/List.ron");
        let file = StyleFile::<ListStyle>::load_from(&path).expect("the skin parses");

        assert_eq!(file.dark, ListStyle::dark(), "dark skin drifted");
        assert_eq!(file.light, ListStyle::light(), "light skin drifted");
    }
}
