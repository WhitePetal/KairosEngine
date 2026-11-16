//! Reusable text styles.
//!
//! egui's [`egui::RichText`] carries its style with every string, so a screen
//! that paints many labels in the same colour and size repeats
//! `.color(..).size(..)` at every call site (the toolbar and the Console panel
//! both did). [`TextStyle`] stores those values once and stamps them onto as
//! many strings as needed:
//!
//! ```ignore
//! let menu_text = TextStyle::new(style.text_color.to_egui()).size(12.0);
//! ui.menu_button(menu_text.rich("World"), |ui| { .. });
//! menu_text.paint(ui.painter(), pos, Align2::LEFT_CENTER, "World");
//! ```
//!
//! A style only describes text *appearance*; it never owns the string, so one
//! instance can be reused for a whole frame.

use std::sync::Arc;

use egui::{Align2, Color32, FontFamily, FontId, Galley, Response, RichText, Ui};

/// A reusable run of text formatting.
///
/// The size is optional: without one, painting falls back to egui's body text
/// size from the active [`egui::Style`], the same default `RichText` uses.
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    color: Color32,
    size: Option<f32>,
    family: FontFamily,
    strong: bool,
    italics: bool,
    underline: bool,
}

impl TextStyle {
    /// A style with the given colour and egui's default body size/family.
    pub fn new(color: Color32) -> Self {
        Self {
            color,
            size: None,
            family: FontFamily::Proportional,
            strong: false,
            italics: false,
            underline: false,
        }
    }

    /// A style with an explicit colour and pixel size: the common case in the
    /// editor chrome, where measurements come from the component style.
    pub fn sized(color: Color32, size: f32) -> Self {
        Self::new(color).size(size)
    }

    pub fn color(mut self, color: Color32) -> Self {
        self.color = color;
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = Some(size);
        self
    }

    pub fn family(mut self, family: FontFamily) -> Self {
        self.family = family;
        self
    }

    pub fn strong(mut self) -> Self {
        self.strong = true;
        self
    }

    pub fn italics(mut self) -> Self {
        self.italics = true;
        self
    }

    pub fn underline(mut self) -> Self {
        self.underline = true;
        self
    }

    pub fn text_color(&self) -> Color32 {
        self.color
    }

    /// Resolve to a concrete font, inheriting the size from egui's body text
    /// when this style does not set one.
    pub fn font_id(&self, style: &egui::Style) -> FontId {
        let mut font = style
            .text_styles
            .get(&egui::TextStyle::Body)
            .cloned()
            .unwrap_or_else(|| FontId::new(14.0, FontFamily::Proportional));
        font.family = self.family.clone();
        if let Some(size) = self.size {
            font.size = size;
        }
        font
    }

    /// A RichText carrying this style: the drop-in replacement for a chain of
    /// `RichText::new(..).size(..).color(..)` calls.
    pub fn rich(&self, text: impl Into<String>) -> RichText {
        let mut rich = RichText::new(text).color(self.color);
        if let Some(size) = self.size {
            rich = rich.size(size);
        }
        rich = rich.family(self.family.clone());
        if self.strong {
            rich = rich.strong();
        }
        if self.italics {
            rich = rich.italics();
        }
        if self.underline {
            rich = rich.underline();
        }
        rich
    }

    /// A plain label in this style.
    pub fn label(&self, ui: &mut Ui, text: impl Into<String>) -> Response {
        ui.label(self.rich(text))
    }

    /// Lay the text out without wrapping.
    pub fn layout(&self, ui: &Ui, text: impl Into<String>) -> Arc<Galley> {
        ui.painter()
            .layout_no_wrap(text.into(), self.font_id(ui.style()), self.color)
    }

    /// Lay the text out with a wrap width (used by the Console rows).
    pub fn layout_wrapped(&self, ui: &Ui, text: impl Into<String>, wrap_width: f32) -> Arc<Galley> {
        ui.painter().layout(
            text.into(),
            self.font_id(ui.style()),
            self.color,
            wrap_width,
        )
    }

    /// Paint the text directly, for chrome that hand-lays-out its own rows.
    pub fn paint(&self, ui: &Ui, pos: egui::Pos2, anchor: Align2, text: impl ToString) {
        ui.painter().text(
            pos,
            anchor,
            text.to_string(),
            self.font_id(ui.style()),
            self.color,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_style_stamps_many_strings_with_the_same_font() {
        let style = TextStyle::sized(Color32::from_rgb(10, 20, 30), 12.0);
        let egui_style = egui::Style::default();

        let first = style.font_id(&egui_style);
        let second = style.font_id(&egui_style);
        assert_eq!(first, second);
        assert_eq!(first.size, 12.0);
        assert_eq!(first.family, FontFamily::Proportional);
        assert_eq!(style.rich("World").text(), "World");
        assert_eq!(style.text_color(), Color32::from_rgb(10, 20, 30));
    }

    #[test]
    fn unspecified_size_falls_back_to_the_body_font() {
        let egui_style = egui::Style::default();
        let expected = egui_style.text_styles[&egui::TextStyle::Body].clone();
        let style = TextStyle::new(Color32::WHITE);
        assert_eq!(style.font_id(&egui_style).size, expected.size);
    }
}
