//! The boolean field: Unity's square checkbox.

use egui::{Id, Rect, Sense, Ui, WidgetType, pos2, vec2};

use super::FieldOutcome;
use super::paint::{paint_box, paint_check, paint_outline};
use super::style::FieldStyle;
use crate::paint::labelled_widget_info;

/// A boolean token, drawn as the square checkbox at the start of the value
/// column.
pub struct ToggleField<'a> {
    id: Id,
    value: &'a mut bool,
    style: &'a FieldStyle,
    selected: bool,
    label: &'a str,
}

impl<'a> ToggleField<'a> {
    pub fn new(id: Id, value: &'a mut bool, style: &'a FieldStyle) -> Self {
        Self {
            id,
            value,
            style,
            selected: false,
            label: "",
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Accessibility name (the inspector's row label).
    pub fn label(mut self, label: &'a str) -> Self {
        self.label = label;
        self
    }

    pub fn show(self, ui: &mut Ui, rect: Rect) -> FieldOutcome {
        let Self {
            id,
            value,
            style,
            selected,
            label,
        } = self;
        let on = *value;
        let side = style.checkbox;
        let box_rect = Rect::from_center_size(
            pos2(rect.left() + side * 0.5, rect.center().y),
            vec2(side, side),
        );
        let mut response = ui.interact(box_rect, id, Sense::click());
        labelled_widget_info(&response, WidgetType::Checkbox, label, Some(on));

        let active = selected || ui.memory(|memory| memory.has_focus(id));
        paint_box(ui, style, box_rect, style.fill);
        paint_outline(
            ui,
            style,
            box_rect,
            style.outline(active, response.hovered()),
        );
        if on {
            paint_check(ui, style, box_rect);
        }

        if response.clicked() {
            *value = !on;
            response.mark_changed();
        }

        FieldOutcome {
            response,
            finished: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::testing::{click, input, rect, style};
    use egui::Context;

    #[test]
    fn a_checkbox_toggles_on_click() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-field-toggle");
        let mut value = false;

        let mut draw = |ui: &mut Ui| {
            ToggleField::new(id, &mut value, &style)
                .label("Visible")
                .show(ui, rect());
        };
        let _ = ctx.run_ui(input(), &mut draw);
        click(
            &ctx,
            pos2(rect().left() + 7.0, rect().center().y),
            &mut draw,
        );
        assert!(value);
    }
}
