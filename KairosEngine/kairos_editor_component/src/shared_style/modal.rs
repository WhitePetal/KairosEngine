//! The modal dialog's shared skin.
//!
//! The widget layer (`kairos_editor_widgets::modal`) draws from runtime tokens
//! (`Color32`/`Stroke`/`Shadow`/`Margin`/`Vec2`); those are not serde-friendly and
//! cannot derive `StyleRow`. This is the serializable skin the editor owns:
//! plain colours and metrics, one file under `Preferences/Styles/Modal.ron`,
//! projected onto the widget tokens at the call site.
//!
//! A dialog is shell chrome, not a panel. The Project panel opens one today and
//! an app-level "quit without saving" prompt will reuse the same widget later, so
//! the skin lives with the toolbar instead of on a panel's style: one file makes
//! every dialog look the same, and it shows up in Preferences without a panel to
//! route the edit through. `Frame::modal_style()` is the one read path.
//!
//! Every composite runtime value is split into the scalars the Preferences
//! editor can address and reassembled in [`ModalSkin::tokens`]: a stroke is a
//! colour plus its width, a shadow an offset/blur/spread plus its colour, and
//! the body inset and minimum button size are symmetric pairs. The defaults are
//! the widget's own `ModalStyle::dark()`/`light()`, so adopting this skin does not
//! change a pixel: it only makes the dialog editable.

use egui::{CornerRadius, Margin, Shadow, Stroke, Vec2};
use kairos_color::Color;
use kairos_editor_widgets::color::ColorExt;
use kairos_editor_widgets::modal::ModalStyle;
use kairos_editor_widgets::style::Style;
use kairos_editor_widgets::window_controls::WindowControlsStyle;
use serde::{Deserialize, Serialize};

use crate::shared_style::SharedStyle;
use crate::style_row::StyleRow;

/// Appearance of the editor's dialogs: the dim, the frame, the title strip, the
/// body text and the button row.
///
/// Field order is the Preferences page order, following the dialog from the
/// backdrop behind it down to its buttons.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, StyleRow)]
#[SharedStyle(MODAL = "modal", title = "Modal")]
pub struct ModalSkin {
    /// Dim over the rest of the editor.
    pub backdrop: Color,
    /// The dialog body's fill.
    pub fill: Color,
    /// The 1 px outline that separates the dialog from its own dim.
    pub stroke: Color,
    pub stroke_width: f32,
    /// Rounded corners shared by the title strip and the buttons.
    pub corner_radius: u8,

    /// Drop shadow under the dialog: offset, blur and spread in points.
    pub shadow_offset_x: f32,
    pub shadow_offset_y: f32,
    pub shadow_blur: f32,
    pub shadow_spread: f32,
    pub shadow_color: Color,

    /// Fixed dialog width.
    pub width: f32,
    /// Horizontal and vertical body inset; the title strip is full-bleed.
    pub padding_x: f32,
    pub padding_y: f32,

    pub title: Color,
    pub title_size: f32,

    /// Height of the full-bleed title strip. 0 drops the strip and draws the
    /// title inline, which is how the compact variant reads.
    pub title_height: f32,
    pub title_bar: Color,
    /// The rule under the title strip; a width of 0 draws none.
    pub rule: Color,
    pub rule_width: f32,

    pub text: Color,
    pub text_size: f32,
    /// Gap between the title block and the body text.
    pub body_gap: f32,
    /// Gap between the body text and the button row.
    pub footer_gap: f32,

    /// Minimum button size; a label wider than this stretches its button.
    pub button_width: f32,
    pub button_height: f32,
    pub button_gap: f32,
    pub button_text_size: f32,
    pub button_radius: u8,
    pub button_fill: Color,
    pub button_hover: Color,
    pub button_text: Color,
    pub button_stroke: Color,
    pub button_stroke_width: f32,
    /// The one semantic colour: destructive buttons only, so a dialog's safe
    /// button stays grey.
    pub danger_fill: Color,
    pub danger_hover: Color,
    pub danger_text: Color,
}

impl ModalSkin {
    /// Project the serializable tokens onto the widget's runtime style.
    ///
    /// The composite values are assembled here so the Preferences page never
    /// has to know about egui types: they are plain colours and numbers on
    /// disk, and `Stroke`/`Shadow`/`Margin`/`Vec2`/`CornerRadius` only exist for
    /// the duration of the draw.
    pub fn tokens(&self) -> ModalStyle {
        ModalStyle {
            backdrop: self.backdrop.to_egui(),
            fill: self.fill.to_egui(),
            stroke: Stroke::new(self.stroke_width, self.stroke.to_egui()),
            corner_radius: CornerRadius::same(self.corner_radius),
            shadow: Shadow {
                offset: [
                    self.shadow_offset_x.round() as i8,
                    self.shadow_offset_y.round() as i8,
                ],
                blur: self.shadow_blur.round() as u8,
                spread: self.shadow_spread.round() as u8,
                color: self.shadow_color.to_egui(),
            },
            width: self.width,
            padding: Margin {
                left: self.padding_x.round() as i8,
                right: self.padding_x.round() as i8,
                top: self.padding_y.round() as i8,
                bottom: self.padding_y.round() as i8,
            },
            title: self.title.to_egui(),
            title_size: self.title_size,
            // The traffic lights stay on the shared system style, like the
            // color picker's: the dialog does not fork the editor's window
            // buttons, so they are not a skin field.
            window_controls: WindowControlsStyle::default(),

            title_height: self.title_height,
            title_bar: self.title_bar.to_egui(),
            rule: Stroke::new(self.rule_width, self.rule.to_egui()),
            text: self.text.to_egui(),
            text_size: self.text_size,
            body_gap: self.body_gap,
            footer_gap: self.footer_gap,
            button: Vec2::new(self.button_width, self.button_height),
            button_gap: self.button_gap,
            button_text_size: self.button_text_size,
            button_radius: self.button_radius,
            button_fill: self.button_fill.to_egui(),
            button_hover: self.button_hover.to_egui(),
            button_text: self.button_text.to_egui(),
            button_stroke: Stroke::new(self.button_stroke_width, self.button_stroke.to_egui()),
            danger_fill: self.danger_fill.to_egui(),
            danger_hover: self.danger_hover.to_egui(),
            danger_text: self.danger_text.to_egui(),
        }
    }
}

impl Style for ModalSkin {
    const FILE: &'static str = "Modal.ron";

    fn dark() -> Self {
        Self {
            backdrop: Color::from_srgb8([0, 0, 0, 120]),
            fill: Color::from_srgb8([0x38, 0x38, 0x38, 255]),
            stroke: Color::from_srgb8([0x1E, 0x1E, 0x1E, 255]),
            stroke_width: 1.0,
            corner_radius: 3,

            shadow_offset_x: 2.0,
            shadow_offset_y: 3.0,
            shadow_blur: 12.0,
            shadow_spread: 0.0,
            shadow_color: Color::from_srgb8([0, 0, 0, 170]),

            width: 380.0,
            padding_x: 14.0,
            padding_y: 12.0,

            title: Color::from_srgb8([0xDE, 0xDE, 0xDE, 255]),
            title_size: 13.0,

            title_height: 22.0,
            title_bar: Color::from_srgb8([0x3C, 0x3C, 0x3C, 255]),
            rule: Color::from_srgb8([0x2B, 0x2B, 0x2B, 255]),
            rule_width: 1.0,

            text: Color::from_srgb8([0xB4, 0xB4, 0xB4, 255]),
            text_size: 12.0,
            body_gap: 10.0,
            footer_gap: 14.0,

            button_width: 74.0,
            button_height: 24.0,
            button_gap: 6.0,
            button_text_size: 12.0,
            button_radius: 2,
            button_fill: Color::from_srgb8([0x4A, 0x4A, 0x4A, 255]),
            button_hover: Color::from_srgb8([0x5A, 0x5A, 0x5A, 255]),
            button_text: Color::from_srgb8([0xDE, 0xDE, 0xDE, 255]),
            button_stroke: Color::from_srgb8([0x23, 0x23, 0x23, 255]),
            button_stroke_width: 1.0,
            danger_fill: Color::from_srgb8([0x8A, 0x3B, 0x33, 255]),
            danger_hover: Color::from_srgb8([0xA3, 0x46, 0x3D, 255]),
            danger_text: Color::from_srgb8([0xFF, 0xFF, 0xFF, 255]),
        }
    }

    fn light() -> Self {
        Self {
            backdrop: Color::from_srgb8([0, 0, 0, 90]),
            fill: Color::from_srgb8([0xC8, 0xC8, 0xC8, 255]),
            stroke: Color::from_srgb8([0x9B, 0x9B, 0x9B, 255]),
            shadow_color: Color::from_srgb8([0, 0, 0, 90]),
            title: Color::from_srgb8([0x30, 0x30, 0x30, 255]),

            title_bar: Color::from_srgb8([0xBD, 0xBD, 0xBD, 255]),
            rule: Color::from_srgb8([0x9B, 0x9B, 0x9B, 255]),
            text: Color::from_srgb8([0x3C, 0x3C, 0x3C, 255]),
            button_fill: Color::from_srgb8([0xB4, 0xB4, 0xB4, 255]),
            button_hover: Color::from_srgb8([0xC4, 0xC4, 0xC4, 255]),
            button_text: Color::from_srgb8([0x2B, 0x2B, 0x2B, 255]),
            button_stroke: Color::from_srgb8([0x90, 0x90, 0x90, 255]),
            ..Self::dark()
        }
    }
}

impl SharedStyle for ModalSkin {}

#[cfg(test)]
mod tests {
    use kairos_editor_widgets::style::StyleFile;

    use super::*;

    /// Each runtime token comes from its own serializable counterpart, so the
    /// page edits what the dialog draws.
    #[test]
    fn the_shared_skin_projects_every_token_onto_the_runtime_style() {
        let skin = ModalSkin::dark();
        let tokens = skin.tokens();

        assert_eq!(tokens.backdrop, skin.backdrop.to_egui());
        assert_eq!(tokens.fill, skin.fill.to_egui());
        assert_eq!(
            tokens.stroke,
            Stroke::new(skin.stroke_width, skin.stroke.to_egui())
        );
        assert_eq!(tokens.corner_radius, CornerRadius::same(skin.corner_radius));
        assert_eq!(
            tokens.shadow,
            Shadow {
                offset: [
                    skin.shadow_offset_x.round() as i8,
                    skin.shadow_offset_y.round() as i8,
                ],
                blur: skin.shadow_blur.round() as u8,
                spread: skin.shadow_spread.round() as u8,
                color: skin.shadow_color.to_egui(),
            }
        );
        assert_eq!(tokens.width, skin.width);
        assert_eq!(
            tokens.padding,
            Margin {
                left: skin.padding_x.round() as i8,
                right: skin.padding_x.round() as i8,
                top: skin.padding_y.round() as i8,
                bottom: skin.padding_y.round() as i8,
            }
        );
        assert_eq!(tokens.title, skin.title.to_egui());
        assert_eq!(tokens.title_size, skin.title_size);
        assert_eq!(
            tokens.window_controls,
            WindowControlsStyle::default(),
            "the traffic lights are the editor's shared system buttons"
        );

        assert_eq!(tokens.title_height, skin.title_height);
        assert_eq!(tokens.title_bar, skin.title_bar.to_egui());
        assert_eq!(
            tokens.rule,
            Stroke::new(skin.rule_width, skin.rule.to_egui())
        );
        assert_eq!(tokens.text, skin.text.to_egui());
        assert_eq!(tokens.text_size, skin.text_size);
        assert_eq!(tokens.body_gap, skin.body_gap);
        assert_eq!(tokens.footer_gap, skin.footer_gap);
        assert_eq!(
            tokens.button,
            Vec2::new(skin.button_width, skin.button_height)
        );
        assert_eq!(tokens.button_gap, skin.button_gap);
        assert_eq!(tokens.button_text_size, skin.button_text_size);
        assert_eq!(tokens.button_radius, skin.button_radius);
        assert_eq!(tokens.button_fill, skin.button_fill.to_egui());
        assert_eq!(tokens.button_hover, skin.button_hover.to_egui());
        assert_eq!(tokens.button_text, skin.button_text.to_egui());
        assert_eq!(
            tokens.button_stroke,
            Stroke::new(skin.button_stroke_width, skin.button_stroke.to_egui())
        );
        assert_eq!(tokens.danger_fill, skin.danger_fill.to_egui());
        assert_eq!(tokens.danger_hover, skin.danger_hover.to_egui());
        assert_eq!(tokens.danger_text, skin.danger_text.to_egui());
    }

    /// The code defaults are the widget's own dark/light values: this skin only
    /// makes them editable, it does not restyle the dialog.
    #[test]
    fn the_code_defaults_match_the_widgets_own_styles() {
        assert_runtime_eq(&ModalSkin::dark().tokens(), &ModalStyle::dark());
        assert_runtime_eq(&ModalSkin::light().tokens(), &ModalStyle::light());
    }

    /// Field-by-field because the widget's `ModalStyle` is not `PartialEq`
    /// (it owns egui paint values, not an editable value type).
    fn assert_runtime_eq(actual: &ModalStyle, expected: &ModalStyle) {
        assert_eq!(actual.backdrop, expected.backdrop);
        assert_eq!(actual.fill, expected.fill);
        assert_eq!(actual.stroke, expected.stroke);
        assert_eq!(actual.corner_radius, expected.corner_radius);
        assert_eq!(actual.shadow, expected.shadow);
        assert_eq!(actual.width, expected.width);
        assert_eq!(actual.padding, expected.padding);
        assert_eq!(actual.title, expected.title);
        assert_eq!(actual.title_size, expected.title_size);
        assert_eq!(actual.window_controls, expected.window_controls);
        assert_eq!(actual.title_height, expected.title_height);
        assert_eq!(actual.title_bar, expected.title_bar);
        assert_eq!(actual.rule, expected.rule);
        assert_eq!(actual.text, expected.text);
        assert_eq!(actual.text_size, expected.text_size);
        assert_eq!(actual.body_gap, expected.body_gap);
        assert_eq!(actual.footer_gap, expected.footer_gap);
        assert_eq!(actual.button, expected.button);
        assert_eq!(actual.button_gap, expected.button_gap);
        assert_eq!(actual.button_text_size, expected.button_text_size);
        assert_eq!(actual.button_radius, expected.button_radius);
        assert_eq!(actual.button_fill, expected.button_fill);
        assert_eq!(actual.button_hover, expected.button_hover);
        assert_eq!(actual.button_text, expected.button_text);
        assert_eq!(actual.button_stroke, expected.button_stroke);
        assert_eq!(actual.danger_fill, expected.danger_fill);
        assert_eq!(actual.danger_hover, expected.danger_hover);
        assert_eq!(actual.danger_text, expected.danger_text);
    }

    /// The checked-in reference skin and the code defaults are one skin.
    #[test]
    fn shipped_reference_skin_matches_the_code_defaults() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../Preferences/Styles/Modal.ron");
        let file = StyleFile::<ModalSkin>::load_from(&path).expect("the skin parses");

        assert_eq!(file.dark, ModalSkin::dark(), "dark skin drifted");
        assert_eq!(file.light, ModalSkin::light(), "light skin drifted");
    }
}
