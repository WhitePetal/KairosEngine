//! Small shared painting primitives for hand-laid-out chrome.
//!
//! Editors draw some controls without an egui widget (Unity-style toolbars pin
//! each button to a measured rect). Those controls still share the same few
//! glyphs and the same accessibility registration, so the drawing lives here
//! instead of being copied into every panel. Components keep owning the layout
//! and the *values* (colours, sizes); this module only knows how to paint.

use egui::{
    Color32, Painter, Pos2, Response, Shape, Stroke, WidgetInfo, WidgetType, pos2, vec2,
};

/// A downward-pointing caret, used by split buttons and dropdown triggers.
///
/// The triangle matches Unity's toolbar caret: 7 px wide, 4 px tall, centred on
/// `center`.
pub fn caret(painter: &Painter, center: Pos2, color: Color32) {
    let points = vec![
        pos2(center.x - 3.5, center.y - 1.5),
        pos2(center.x + 3.5, center.y - 1.5),
        pos2(center.x, center.y + 2.5),
    ];
    painter.add(Shape::convex_polygon(points, color, Stroke::NONE));
}

/// A magnifier glyph for search fields.
pub fn magnifier(painter: &Painter, center: Pos2, color: Color32) {
    painter.circle_stroke(center + vec2(-1.0, -1.0), 4.0, Stroke::new(1.2, color));
    painter.line_segment(
        [center + vec2(1.8, 1.8), center + vec2(4.2, 4.2)],
        Stroke::new(1.2, color),
    );
}

/// Register a hand-painted control with egui's accessibility tree.
///
/// A painter-only control is invisible to screen readers and to label/role
/// queries (headless tests included); built-in widgets report this themselves.
pub fn labelled_widget_info(
    response: &Response,
    widget: WidgetType,
    label: &str,
    selected: Option<bool>,
) {
    response.widget_info(|| match selected {
        Some(selected) => WidgetInfo::selected(widget, true, selected, label),
        None => WidgetInfo::labeled(widget, true, label),
    });
}
