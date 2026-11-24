//! The field widgets' shared skin.
//!
//! Value boxes, checkboxes and row names are drawn by
//! kairos_editor_widgets::field, which reads runtime tokens (Color32,
//! TextStyle). Those are not serde-friendly, so this is the serializable skin
//! the editor owns: plain colours and metrics, one file under
//! Preferences/Styles/EditorField.ron, projected onto the widget tokens at
//! the call site. Every panel that draws fields reads the same file - the
//! Preferences editor today, an object inspector next - so a field looks the
//! same everywhere and can be restyled in one place.
//!
//! The enum field's dropdown is part of this skin, not a page of its own. It
//! uses the editor's shared menu tokens (the menu bar's popup) for the popup
//! and a distinct popup-field fill for the trigger, because the color picker's
//! menu buttons read as buttons rather than text boxes.

use egui::{Margin, Vec2};
use kairos_color::Color;
use kairos_editor_widgets::color::ColorExt;
use kairos_editor_widgets::field::FieldStyle;
use kairos_editor_widgets::menu::MenuStyle;
use kairos_editor_widgets::style::Style;
use kairos_editor_widgets::text::TextStyle;
use serde::{Deserialize, Serialize};

use crate::shared_style::SharedStyle;
use crate::style_row::StyleRow;

/// Appearance of one value field: its box, its name, and the wash that marks a
/// selected value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, StyleRow)]
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

    /// The dropdown an enum field opens and the button that opens it. It
    /// lives in this skin, not in a page of its own: it is the field's own
    /// popup, not a second control family.
    #[serde(default)]
    pub menu: EditorFieldMenuStyle,
}

/// Appearance of the enum field's menu button and its dropdown.
///
/// The trigger is Unity's popup field: a fill one shade lighter than a text
/// box, a 1 px outline, the value text at the left and a caret at the right.
/// The popup is the editor's shared menu: a square, dense list with the Unity
/// blue highlight and full-width separators.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, StyleRow)]
pub struct EditorFieldMenuStyle {
    /// Fill of the menu button.
    pub button_fill: Color,
    /// Menu-button fill under the pointer and while its menu is open.
    pub button_hover: Color,
    /// Menu-button outline at rest.
    pub button_edge: Color,
    /// Popup fill.
    pub background: Color,
    /// Item text at rest.
    pub text: Color,
    /// Fill behind the hovered or active item.
    pub highlight: Color,
    /// Full-width separator between menu sections.
    pub separator: Color,
    /// Height of one item row.
    pub item_height: f32,
    /// Vertical margin between the popup edge and the item column.
    pub padding: f32,
    /// Left/right padding of one item.
    pub item_padding: f32,
    /// The popup is at least this wide.
    pub min_width: f32,
}

impl Default for EditorFieldMenuStyle {
    fn default() -> Self {
        Self::dark()
    }
}

impl EditorFieldMenuStyle {
    pub fn dark() -> Self {
        Self {
            button_fill: Color::from_srgb8([0x4A, 0x4A, 0x4A, 255]),
            button_hover: Color::from_srgb8([0x56, 0x56, 0x56, 255]),
            button_edge: Color::from_srgb8([0x16, 0x16, 0x16, 255]),
            background: Color::from_srgb8([0x2D, 0x2D, 0x2D, 255]),
            text: Color::from_srgb8([0xDE, 0xDE, 0xDE, 255]),
            highlight: Color::from_srgb8([0x2C, 0x5D, 0x87, 255]),
            separator: Color::from_srgb8([0x4F, 0x4F, 0x4F, 255]),
            item_height: 22.0,
            padding: 4.0,
            item_padding: 11.0,
            min_width: 160.0,
        }
    }

    pub fn light() -> Self {
        Self {
            button_fill: Color::from_srgb8([0xE0, 0xE0, 0xE0, 255]),
            button_hover: Color::WHITE,
            button_edge: Color::from_srgb8([0xA8, 0xA8, 0xA8, 255]),
            background: Color::from_srgb8([0xF0, 0xF0, 0xF0, 255]),
            text: Color::from_srgb8([0x20, 0x20, 0x20, 255]),
            highlight: Color::from_srgb8([0x3A, 0x72, 0xB0, 255]),
            separator: Color::from_srgb8([0xC0, 0xC0, 0xC0, 255]),
            ..Self::dark()
        }
    }

    /// Project the serializable tokens onto the widget's shared menu style.
    ///
    /// The text size is the field's, not a menu-only token: a field's popup
    /// must not change size from the value it belongs to.
    pub fn tokens(&self, text_size: f32) -> MenuStyle {
        MenuStyle {
            popup_fill: self.background.to_egui(),
            popup_margin: Margin {
                left: 0,
                right: 0,
                top: self.padding.round() as i8,
                bottom: self.padding.round() as i8,
            },
            popup_min_width: self.min_width,
            text: self.text.to_egui(),
            text_size,
            item_hover_fill: self.highlight.to_egui(),
            item_active_fill: self.highlight.to_egui(),
            item_padding: Vec2::new(self.item_padding, 3.0),
            item_height: self.item_height,
            separator: self.separator.to_egui(),
            ..MenuStyle::default()
        }
    }
}

impl EditorFieldStyle {
    /// Project the serializable tokens onto the widget's runtime style.
    pub fn tokens(&self) -> FieldStyle {
        let mut tokens = FieldStyle::new(TextStyle::sized(self.text.to_egui(), self.text_size))
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
            .menu(self.menu.tokens(self.text_size));
        tokens.menu_button_fill = self.menu.button_fill.to_egui();
        tokens.menu_button_hover = self.menu.button_hover.to_egui();
        tokens.menu_button_edge = self.menu.button_edge.to_egui();
        tokens
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
            menu: EditorFieldMenuStyle::dark(),
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
            menu: EditorFieldMenuStyle::light(),
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
        assert_eq!(tokens.menu_button_fill, skin.menu.button_fill.to_egui());
        assert_eq!(tokens.menu_button_hover, skin.menu.button_hover.to_egui());
        assert_eq!(tokens.menu_button_edge, skin.menu.button_edge.to_egui());

        let menu = &skin.menu;
        assert_eq!(tokens.menu.popup_fill, menu.background.to_egui());
        assert_eq!(tokens.menu.text, menu.text.to_egui());
        assert_eq!(tokens.menu.text_size, skin.text_size);
        assert_eq!(tokens.menu.item_hover_fill, menu.highlight.to_egui());
        assert_eq!(tokens.menu.item_active_fill, menu.highlight.to_egui());
        assert_eq!(tokens.menu.item_height, menu.item_height);
        assert_eq!(tokens.menu.item_padding, Vec2::new(menu.item_padding, 3.0));
        assert_eq!(tokens.menu.popup_min_width, menu.min_width);
        assert_eq!(tokens.menu.separator, menu.separator.to_egui());
        assert_eq!(tokens.menu.popup_margin.top as f32, menu.padding);
    }

    /// The enum dropdown belongs to the field page, not a style of its own: a
    /// field's own popup must not look like a second control family.
    #[test]
    fn the_enum_menu_defaults_to_the_dark_field_skin() {
        assert_eq!(
            EditorFieldMenuStyle::default(),
            EditorFieldMenuStyle::dark()
        );
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
