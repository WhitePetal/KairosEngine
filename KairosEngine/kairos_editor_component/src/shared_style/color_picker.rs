//! The color picker's shared skin.
//!
//! The widget layer (`kairos_editor_widgets::color_picker`) draws from runtime
//! tokens (`Color32`/`TextStyle`/`WindowControlsStyle`); those types are not
//! serde-friendly and cannot derive `StyleFields`. This is the serializable
//! skin the editor owns: plain colors and metrics, one file under
//! `Preferences/Styles/ColorPicker.ron`, projected onto the widget tokens at
//! the call site.
//!
//! The defaults are the values the Preferences panel used to project from its
//! own skin, so moving the picker onto this file keeps the editor's current
//! look (including the 0.7 px border) instead of adopting the widget fallback's
//! 0.5 px dark defaults.

use egui::Context;
use kairos_color::Color;
use kairos_editor_widgets::color::ColorExt;
use kairos_editor_widgets::color_picker::ColorPickerStyle;
use kairos_editor_widgets::style::Style;
use kairos_editor_widgets::text::TextStyle;
use kairos_editor_widgets::ui_image;
use kairos_editor_widgets::window_controls::WindowControlsStyle;
use serde::{Deserialize, Serialize};

use crate::shared_style::SharedStyle;
use crate::skin::StyleFields;

/// Appearance of the color field and its floating picker.
///
/// Field order is the Preferences page order. `text`/`title` are colors, not
/// `TextStyle` values: the sizes are separate tokens so the field editor can
/// reach every piece. The title is not bold in the current editor look,
/// matching the old projection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, StyleFields)]
#[SharedStyle(COLOR_PICKER = "color_picker", title = "Color Picker")]
pub struct ColorPickerSkin {
    /// Text of channel values and menus.
    pub text: Color,
    /// Picker window title.
    pub title: Color,
    pub text_size: f32,
    pub title_size: f32,

    pub background: Color,
    pub header: Color,
    pub border: Color,
    pub label: Color,
    /// Row label while its channel is the active edit target.
    pub label_active: Color,
    pub value: Color,
    pub field: Color,
    pub field_hover: Color,
    pub field_edge: Color,
    pub accent: Color,
    /// Popup-field trigger fill and hover.
    pub menu_button_fill: Color,
    pub menu_button_hover: Color,
    /// Validation text in the swatch group dialog.
    pub error: Color,
    pub checker_light: Color,
    pub checker_dark: Color,
    /// Runtime image under `Preferences/Textures`, tinted by the style.
    pub eyedropper_icon: String,

    pub border_width: f32,
    pub corner_radius: u8,
    pub window_width: f32,
    pub title_height: f32,
    pub padding: f32,
    pub section_gap: f32,
    pub row_height: f32,
    pub row_gap: f32,
    pub field_height: f32,
    pub wheel_size: f32,
    pub ring_thickness: f32,
    pub square_padding: f32,
    pub mode_width: f32,
    pub hex_width: f32,
    pub channel_label_width: f32,
    pub channel_value_width: f32,
    pub channel_track_height: f32,
    pub preview_swatch_width: f32,
    pub preview_swatch_height: f32,
    pub eyedropper_width: f32,
    pub swatch_size: f32,
    pub swatch_gap: f32,
    pub swatch_header_height: f32,
}

impl Style for ColorPickerSkin {
    const FILE: &'static str = "ColorPicker.ron";

    fn dark() -> Self {
        Self {
            text: Color::from_srgb8([0xDC, 0xDC, 0xDC, 255]),
            title: Color::from_srgb8([0xEE, 0xEE, 0xEE, 255]),
            text_size: 12.0,
            title_size: 13.0,

            background: Color::from_srgb8([0x36, 0x36, 0x36, 255]),
            header: Color::from_srgb8([0x3C, 0x3C, 0x3C, 255]),
            border: Color::from_srgb8([0x20, 0x20, 0x20, 255]),
            label: Color::from_srgb8([0xC8, 0xC8, 0xC8, 255]),
            label_active: Color::from_srgb8([0x8F, 0xB4, 0xD9, 255]),
            value: Color::from_srgb8([0xDC, 0xDC, 0xDC, 255]),
            field: Color::from_srgb8([0x28, 0x28, 0x28, 255]),
            field_hover: Color::from_srgb8([0x3E, 0x3E, 0x3E, 255]),
            field_edge: Color::from_srgb8([0x20, 0x20, 0x20, 255]),
            accent: Color::from_srgb8([0x4A, 0x90, 0xD9, 255]),
            menu_button_fill: Color::from_srgb8([0x4A, 0x4A, 0x4A, 255]),
            menu_button_hover: Color::from_srgb8([0x56, 0x56, 0x56, 255]),
            error: Color::from_srgb8([0xFF, 0x5F, 0x56, 255]),
            checker_light: Color::from_srgb8([0xC8, 0xC8, 0xC8, 255]),
            checker_dark: Color::from_srgb8([0x8C, 0x8C, 0x8C, 255]),
            eyedropper_icon: "Preferences/Textures/color_picker/eyedropper.png".to_owned(),

            border_width: 0.7,
            corner_radius: 0,
            window_width: 260.0,
            title_height: 28.0,
            padding: 6.0,
            section_gap: 10.0,
            row_height: 20.0,
            row_gap: 6.0,
            field_height: 18.0,
            wheel_size: 232.0,
            ring_thickness: 26.0,
            square_padding: 6.0,
            mode_width: 94.0,
            hex_width: 94.0,
            channel_label_width: 14.0,
            channel_value_width: 42.0,
            channel_track_height: 14.0,
            preview_swatch_width: 42.0,
            preview_swatch_height: 26.0,
            eyedropper_width: 42.0,
            swatch_size: 14.0,
            swatch_gap: 1.0,
            swatch_header_height: 20.0,
        }
    }

    fn light() -> Self {
        Self {
            text: Color::from_srgb8([0x1E, 0x1E, 0x1E, 255]),
            title: Color::from_srgb8([0x1E, 0x1E, 0x1E, 255]),

            background: Color::from_srgb8([0xEB, 0xEB, 0xEB, 255]),
            header: Color::from_srgb8([0xDC, 0xDC, 0xDC, 255]),
            border: Color::from_srgb8([0xA8, 0xA8, 0xA8, 255]),
            label: Color::from_srgb8([0x1E, 0x1E, 0x1E, 255]),
            label_active: Color::from_srgb8([0x2C, 0x5D, 0x87, 255]),
            value: Color::from_srgb8([0x1E, 0x1E, 0x1E, 255]),
            field: Color::WHITE,
            field_hover: Color::from_srgb8([0xF0, 0xF0, 0xF0, 255]),
            field_edge: Color::from_srgb8([0xA8, 0xA8, 0xA8, 255]),
            accent: Color::from_srgb8([0x2C, 0x5D, 0x87, 255]),
            menu_button_fill: Color::from_srgb8([0xE0, 0xE0, 0xE0, 255]),
            menu_button_hover: Color::WHITE,
            ..Self::dark()
        }
    }
}

impl ColorPickerSkin {
    /// Project the serializable tokens onto the widget's runtime style.
    ///
    /// The traffic lights stay on the shared `WindowControlsStyle` default: the
    /// picker's title bar should match the dock's floating windows, and that
    /// widget owns its own look until it grows a shared skin of its own.
    pub fn tokens(&self) -> ColorPickerStyle {
        ColorPickerStyle {
            text: TextStyle::sized(self.text.to_egui(), self.text_size),
            title: TextStyle::sized(self.title.to_egui(), self.title_size),
            background: self.background.to_egui(),
            header: self.header.to_egui(),
            border: self.border.to_egui(),
            label: self.label.to_egui(),
            label_active: self.label_active.to_egui(),
            value: self.value.to_egui(),
            field: self.field.to_egui(),
            field_hover: self.field_hover.to_egui(),
            field_edge: self.field_edge.to_egui(),
            accent: self.accent.to_egui(),
            menu_button_fill: self.menu_button_fill.to_egui(),
            menu_button_hover: self.menu_button_hover.to_egui(),
            error: self.error.to_egui(),
            window_controls: WindowControlsStyle::default(),
            checker_light: self.checker_light.to_egui(),
            checker_dark: self.checker_dark.to_egui(),
            eyedropper_icon: self.eyedropper_icon.clone(),
            border_width: self.border_width,
            corner_radius: self.corner_radius,
            window_width: self.window_width,
            title_height: self.title_height,
            padding: self.padding,
            section_gap: self.section_gap,
            row_height: self.row_height,
            row_gap: self.row_gap,
            field_height: self.field_height,
            wheel_size: self.wheel_size,
            ring_thickness: self.ring_thickness,
            square_padding: self.square_padding,
            mode_width: self.mode_width,
            hex_width: self.hex_width,
            channel_label_width: self.channel_label_width,
            channel_value_width: self.channel_value_width,
            channel_track_height: self.channel_track_height,
            preview_swatch_width: self.preview_swatch_width,
            preview_swatch_height: self.preview_swatch_height,
            eyedropper_width: self.eyedropper_width,
            swatch_size: self.swatch_size,
            swatch_gap: self.swatch_gap,
            swatch_header_height: self.swatch_header_height,
        }
    }
}

impl SharedStyle for ColorPickerSkin {
    fn preload(&self, ctx: &Context) {
        ui_image::preload(ctx, &self.eyedropper_icon);
    }
}

#[cfg(test)]
mod tests {
    use egui::{Color32, Style as EguiStyle};
    use kairos_editor_widgets::style::StyleFile;

    use super::*;
    use crate::skin::StyleKind;

    /// The checked-in reference skin and the code defaults are one skin.
    #[test]
    fn shipped_reference_skin_matches_the_code_defaults() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../Preferences/Styles/ColorPicker.ron");
        let file =
            StyleFile::<ColorPickerSkin>::load_from(&path).expect("the reference skin parses");
        let defaults = StyleFile::<ColorPickerSkin>::defaults();

        assert_eq!(file.dark, defaults.dark, "dark skin drifted from code");
        assert_eq!(file.light, defaults.light, "light skin drifted from code");
    }

    /// The file keeps the look the Preferences projection used to produce:
    /// adopting the widget fallback's darker defaults would be a regression.
    #[test]
    fn the_projection_keeps_the_pre_edit_preferences_look() {
        let tokens = ColorPickerSkin::dark().tokens();

        assert_eq!(tokens.background, Color32::from_rgb(0x36, 0x36, 0x36));
        assert_eq!(tokens.field, Color32::from_rgb(0x28, 0x28, 0x28));
        assert_eq!(tokens.border_width, 0.7);
        assert_eq!(
            tokens.text.font_id(&EguiStyle::default()).size,
            12.0,
            "the picker keeps the panel body text size it had before"
        );
        assert_eq!(tokens.title.font_id(&EguiStyle::default()).size, 13.0);
    }

    /// The traffic lights stay the shared widget default; the dock's floating
    /// windows use the same value until the window-controls widget gets a skin.
    #[test]
    fn the_projection_keeps_window_controls_shared_with_the_dock() {
        assert_eq!(
            ColorPickerSkin::dark().tokens().window_controls,
            WindowControlsStyle::default()
        );
    }

    /// Every picker token is reachable through the field editor, including the
    /// runtime icon path; the window controls are deliberately not a token.
    #[test]
    fn the_skin_describes_its_own_tokens() {
        let fields = ColorPickerSkin::dark().style_fields();

        assert!(fields.iter().any(|field| field.path == "accent"));
        assert!(fields.iter().any(|field| field.path == "text_size"));
        assert!(
            fields
                .iter()
                .any(|field| field.path == "eyedropper_icon" && field.kind == StyleKind::Text),
            "the icon path is a text token, like the Console tab icons"
        );
        assert!(
            !fields.iter().any(|field| field.path == "window_controls"),
            "the picker does not own the shared traffic-light widget"
        );
    }
}
