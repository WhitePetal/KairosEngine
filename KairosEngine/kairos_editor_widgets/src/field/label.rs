//! The field's name: Unity's scrub handle for a number.
//!
//! The row lays the name out (the inspector's label column belongs to the
//! panel), so this widget owns only the text and the gesture: clicking selects
//! the field, and holding the name of a number field turns the pointer into the
//! left/right scrub cursor.

use egui::{CursorIcon, Id, Rect, Response, Sense, Ui, pos2};

use super::Scrub;
use super::style::FieldStyle;

/// One field's name column.
pub struct FieldLabel<'a> {
    id: Id,
    text: &'a str,
    style: &'a FieldStyle,
    draggable: bool,
}

impl<'a> FieldLabel<'a> {
    pub fn new(id: Id, text: &'a str, style: &'a FieldStyle) -> Self {
        Self {
            id,
            text,
            style,
            draggable: false,
        }
    }

    /// The field is a number: the name becomes its scrub handle.
    pub fn draggable(mut self, draggable: bool) -> Self {
        self.draggable = draggable;
        self
    }

    /// The gesture half. Call it where the row starts its name column, then
    /// paint after the value so the name turns blue in the same pass as the
    /// selection.
    pub fn interact(&self, ui: &mut Ui, rect: Rect) -> FieldLabelResponse {
        let sense = if self.draggable {
            Sense::click_and_drag()
        } else {
            Sense::click()
        };
        let response = ui.interact(rect, self.id, sense);
        let cursor = if self.draggable {
            CursorIcon::ResizeHorizontal
        } else {
            CursorIcon::PointingHand
        };
        FieldLabelResponse {
            response: response.on_hover_and_drag_cursor(cursor),
        }
    }

    /// Paint the name, left-aligned and vertically centred in its rect.
    pub fn paint(&self, ui: &Ui, rect: Rect, active: bool) {
        let color = self.style.label_color(active);
        let galley = self.style.text.clone().color(color).layout(ui, self.text);
        let text_pos = pos2(rect.left(), rect.center().y - galley.size().y * 0.5);
        ui.painter()
            .with_clip_rect(rect)
            .galley(text_pos, galley, color);
    }
}

/// What the name did this pass.
pub struct FieldLabelResponse {
    pub response: Response,
}

impl FieldLabelResponse {
    pub fn clicked(&self) -> bool {
        self.response.clicked()
    }

    /// The pointer moved while held on the name: one frame of a scrub.
    pub fn scrub(&self) -> Option<Scrub> {
        let dragged = self.response.dragged();
        if !dragged && !self.response.drag_stopped() {
            return None;
        }
        Some(Scrub {
            delta: if dragged {
                self.response.drag_delta().x
            } else {
                0.0
            },
            stopped: self.response.drag_stopped(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::testing::{button, input, label_rect, style};
    use egui::{Context, Event};

    fn draw(style: &FieldStyle, ui: &mut Ui) -> FieldLabelResponse {
        FieldLabel::new(Id::new("kairos-field-label"), "Width", style)
            .draggable(true)
            .interact(ui, label_rect())
    }

    #[test]
    fn a_draggable_name_reports_the_scrub_delta() {
        let ctx = Context::default();
        let style = style();
        let start = label_rect().center();

        let _ = ctx.run_ui(input(), |ui| {
            draw(&style, ui);
        });
        let mut press = input();
        press.events = vec![button(start, true)];
        let _ = ctx.run_ui(press, |ui| {
            draw(&style, ui);
        });
        let mut moved = input();
        moved.events = vec![Event::PointerMoved(start + egui::vec2(20.0, 0.0))];
        let _ = ctx.run_ui(moved, |ui| {
            let label = draw(&style, ui);
            assert_eq!(label.scrub().map(|scrub| scrub.delta), Some(20.0));
        });
    }

    /// Unity's scrub cursor: the name is a handle, so the pointer says so before
    /// the user has to guess.
    #[test]
    fn hovering_a_draggable_name_shows_the_scrub_cursor() {
        let ctx = Context::default();
        let style = style();
        let _ = ctx.run_ui(input(), |ui| {
            draw(&style, ui);
        });
        let mut hover = input();
        hover.events = vec![egui::Event::PointerMoved(label_rect().center())];
        let output = ctx.run_ui(hover, |ui| {
            draw(&style, ui);
        });

        assert_eq!(
            output.platform_output.cursor_icon,
            egui::CursorIcon::ResizeHorizontal
        );
    }

    #[test]
    fn a_plain_name_has_no_scrub() {
        let ctx = Context::default();
        let style = style();
        let _ = ctx.run_ui(input(), |ui| {
            let widget = FieldLabel::new(Id::new("kairos-field-label-plain"), "Visible", &style);
            let label = widget.interact(ui, label_rect());
            assert!(label.scrub().is_none());
            widget.paint(ui, label_rect(), true);
        });
    }
}
