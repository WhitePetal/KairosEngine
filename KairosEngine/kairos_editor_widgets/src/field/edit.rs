//! The inline text editor the number box and the text field share.
//!
//! Both are a framed single-line TextEdit that fills the caller's rect, and both
//! show Unity's selected-text wash: a translucent accent behind the digits
//! instead of egui's solid default, which would hide the value being edited.

use egui::{Align, Color32, Id, Rect, Response, Stroke, TextEdit, Ui, UiBuilder, vec2};

use super::paint::value_padding;
use super::style::FieldStyle;

/// Unity's selected-text highlight: the accent at ~62%, so the digits stay
/// readable under it.
pub(super) fn selection_fill(style: &FieldStyle) -> Color32 {
    let accent = style.accent;
    Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 160)
}

/// A framed single-line editor filling the caller's rect.
pub(super) fn text_edit_box(
    ui: &mut Ui,
    rect: Rect,
    id: Id,
    text: &mut String,
    style: &FieldStyle,
) -> Response {
    let padding = value_padding(ui, style, rect);
    ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        ui.visuals_mut().selection.bg_fill = selection_fill(style);
        // egui paints selected glyphs with the selection stroke colour, so a
        // NONE stroke would make the selected value disappear entirely.
        ui.visuals_mut().selection.stroke = Stroke::new(1.0, style.text.text_color());
        let font = style.text.font_id(ui.style());
        ui.add(
            TextEdit::singleline(text)
                .id(id)
                .font(font)
                .text_color(style.text.text_color())
                .frame(egui::Frame::NONE.inner_margin(padding))
                .horizontal_align(Align::Min)
                .vertical_align(Align::Center)
                .min_size(vec2(rect.width(), rect.height()))
                .desired_width(rect.width()),
        )
    })
    .inner
}

/// Select the whole value, the state Unity enters a field in.
pub(super) fn select_all(ui: &Ui, id: Id, chars: usize) {
    let mut state = TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::default(),
            egui::text::CCursor::new(chars),
        )));
    state.store(ui.ctx(), id);
}
