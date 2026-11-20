//! The string field: a box whose whole interior is an editable value.

use egui::{Id, Rect, Ui};

use super::FieldOutcome;
use super::edit::{select_all, text_edit_box};
use super::paint::{paint_box, paint_outline};
use super::style::FieldStyle;

/// A string token (an icon path, a font family), or any other line of text.
///
/// Clicking the value selects all of it, the state Unity enters a field in, so
/// typing replaces the old text instead of appending to it.
pub struct TextField<'a> {
    id: Id,
    value: &'a mut String,
    style: &'a FieldStyle,
    selected: bool,
}

impl<'a> TextField<'a> {
    pub fn new(id: Id, value: &'a mut String, style: &'a FieldStyle) -> Self {
        Self {
            id,
            value,
            style,
            selected: false,
        }
    }

    /// The row that owns this field is selected: draw it active even without
    /// focus.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn show(self, ui: &mut Ui, rect: Rect) -> FieldOutcome {
        let Self {
            id,
            value,
            style,
            selected,
        } = self;
        let hovered = ui.rect_contains_pointer(rect);
        let focused = ui.memory(|memory| memory.has_focus(id));
        let active = selected || focused;
        let fill = if hovered && !active {
            style.hover_fill
        } else {
            style.fill
        };
        paint_box(ui, style, rect, fill);

        let chars = value.chars().count();
        let response = text_edit_box(ui, rect, id, value, style);
        if !ui.input(|input| input.pointer.any_down()) && ui.rect_contains_pointer(rect) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
        }
        if response.gained_focus() {
            // Unity enters a field with its whole value selected, so typing
            // replaces the old text instead of landing in the middle of it.
            select_all(ui, id, chars);
        }

        let finished = response.lost_focus();
        let active_now = active || response.has_focus();
        paint_outline(ui, style, rect, style.outline(active_now, hovered));
        FieldOutcome { response, finished }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::edit::selection_fill;
    use crate::field::testing::{click, input, rect, style};
    use egui::{Color32, Context, Event};

    #[test]
    fn typing_writes_the_string_as_it_goes() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-field-text");
        let mut value = "icons/".to_owned();

        let mut draw = |ui: &mut Ui| {
            TextField::new(id, &mut value, &style).show(ui, rect());
        };
        let _ = ctx.run_ui(input(), &mut draw);
        click(&ctx, rect().center(), &mut draw);
        let mut typed = input();
        typed.events = vec![Event::Text("info.png".to_owned())];
        let _ = ctx.run_ui(typed, &mut draw);

        assert_eq!(value, "info.png", "the click selected, so typing replaced");
    }

    /// The click that enters a field selects its whole value, so the next key
    /// replaces it instead of landing in the middle.
    #[test]
    fn clicking_the_value_selects_all_of_it() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-field-text-select");
        let mut value = "Preferences/Textures/info.png".to_owned();

        let mut draw = |ui: &mut Ui| {
            TextField::new(id, &mut value, &style).show(ui, rect());
        };
        let _ = ctx.run_ui(input(), &mut draw);
        click(&ctx, rect().center(), &mut draw);
        let _ = ctx.run_ui(input(), &mut draw);

        let state = egui::TextEdit::load_state(&ctx, id).expect("the editor state");
        let [min, max] = state
            .cursor
            .char_range()
            .expect("a selection")
            .sorted_cursors();
        assert_eq!(min.index, egui::text::CharIndex(0));
        assert_eq!(max.index, egui::text::CharIndex(value.chars().count()));
    }

    /// The value is an editable line, so the pointer reads as a caret before the
    /// click lands.
    #[test]
    fn hovering_the_value_shows_the_text_cursor() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-field-text-cursor");
        let mut value = "icons/".to_owned();

        let mut draw = |ui: &mut Ui| {
            TextField::new(id, &mut value, &style).show(ui, rect());
        };
        let _ = ctx.run_ui(input(), &mut draw);
        let mut hover = input();
        hover.events = vec![Event::PointerMoved(rect().center())];
        let output = ctx.run_ui(hover, &mut draw);

        assert_eq!(output.platform_output.cursor_icon, egui::CursorIcon::Text);
    }

    /// The wash is the picker's channel-value blue at ~62%: a solid block would
    /// hide the string being edited.
    #[test]
    fn the_selection_wash_is_the_translucent_accent() {
        let style = style();
        assert_eq!(selection_fill(&style).a(), 160);
        assert_eq!(
            selection_fill(&style),
            Color32::from_rgba_unmultiplied(0x4A, 0x90, 0xD9, 160)
        );
    }
}
