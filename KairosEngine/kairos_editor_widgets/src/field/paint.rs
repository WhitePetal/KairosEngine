//! Painting the field widgets share.

use egui::pos2;
use egui::{Color32, CornerRadius, Margin, Rect, Stroke, StrokeKind, Ui};

use super::style::FieldStyle;

pub(super) fn paint_box(ui: &Ui, style: &FieldStyle, rect: Rect, fill: Color32) {
    ui.painter()
        .rect_filled(rect, CornerRadius::same(style.corner_radius), fill);
}

pub(super) fn paint_outline(ui: &Ui, style: &FieldStyle, rect: Rect, edge: Color32) {
    if style.border_width <= 0.0 {
        return;
    }
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(style.corner_radius),
        Stroke::new(style.border_width, edge),
        StrokeKind::Inside,
    );
}

pub(super) fn paint_check(ui: &Ui, style: &FieldStyle, rect: Rect) {
    let stroke = Stroke::new(1.5, style.text.text_color());
    let a = pos2(rect.left() + rect.width() * 0.22, rect.center().y);
    let b = pos2(
        rect.left() + rect.width() * 0.42,
        rect.bottom() - rect.height() * 0.22,
    );
    let c = pos2(
        rect.right() - rect.width() * 0.18,
        rect.top() + rect.height() * 0.22,
    );
    ui.painter().line_segment([a, b], stroke);
    ui.painter().line_segment([b, c], stroke);
}

/// Horizontal text padding plus the vertical padding that centres one line in a
/// box (a TextEdit sizes itself by its font's row height, not by the box, so the
/// difference is split above and below).
pub(super) fn value_padding(ui: &Ui, style: &FieldStyle, rect: Rect) -> Margin {
    let font = style.text.font_id(ui.style());
    let row_height = ui.fonts_mut(|fonts| fonts.row_height(&font));
    let vertical = ((rect.height() - row_height) * 0.5).round().max(0.0) as i8;
    Margin::symmetric(style.padding.round() as i8, vertical)
}
