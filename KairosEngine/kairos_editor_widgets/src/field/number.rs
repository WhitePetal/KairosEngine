//! The number box: Unity's scrub-to-drag, click-to-type field.

use std::fmt::Display;

use egui::{CursorIcon, Id, Key, Rect, Sense, Ui, WidgetType, pos2, vec2};

use super::edit::{select_all, text_edit_box};
use super::paint::{paint_box, paint_outline};
use super::style::FieldStyle;
use super::{FieldOutcome, Scrub};
use crate::paint::labelled_widget_info;

/// A primitive a number box can edit.
///
/// The box prints through the type's own Display, so an `f32` keeps f32 digits
/// instead of the widened `f64` noise, and rounds through the type's own range,
/// so a `u8` cannot hold 300. Integers step by whole units when dragged;
/// floats scale with their magnitude.
pub trait Number: Copy + PartialEq + Display + 'static {
    /// Whole numbers round and step by one; floats keep their fraction.
    const WHOLE: bool;

    /// The value as f64, for the shared drag and edit maths.
    fn as_f64(self) -> f64;

    /// Round and clamp an f64 into this type (`u8` saturates at 255).
    fn from_f64(value: f64) -> Self;

    /// The displayed text, with a negative zero folded to 0.
    fn text(self) -> String {
        let text = self.to_string();
        if text == "-0" { "0".to_owned() } else { text }
    }
}

macro_rules! float_number {
    ($($ty:ty),* $(,)?) => {
        $(
            impl Number for $ty {
                const WHOLE: bool = false;

                fn as_f64(self) -> f64 {
                    self as f64
                }

                fn from_f64(value: f64) -> Self {
                    value as $ty
                }
            }
        )*
    };
}

macro_rules! whole_number {
    ($($ty:ty),* $(,)?) => {
        $(
            impl Number for $ty {
                const WHOLE: bool = true;

                fn as_f64(self) -> f64 {
                    self as f64
                }

                fn from_f64(value: f64) -> Self {
                    value.round().clamp(<$ty>::MIN as f64, <$ty>::MAX as f64) as $ty
                }
            }
        )*
    };
}

float_number!(f32, f64);
whole_number!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);

/// The open text editor of one number box.
///
/// Owned by the caller between passes: [NumberField::show] fills it on the click
/// that starts an edit and takes it back when the edit ends. Escape restores the
/// value the box had when the edit started.
#[derive(Clone, Debug)]
pub struct NumberEdit {
    text: String,
    /// Value before the edit, as f64: exact for every primitive the box edits.
    original: f64,
    /// Focus the editor and select everything on the first pass.
    focus: bool,
}

impl NumberEdit {
    fn new(text: String, original: f64) -> Self {
        Self {
            text,
            original,
            focus: true,
        }
    }
}

/// A Unity-style number box: the field's name scrubs it, a click types into it.
pub struct NumberField<'a, N: Number> {
    id: Id,
    value: &'a mut N,
    style: &'a FieldStyle,
    range: (f64, f64),
    selected: bool,
    label: &'a str,
    box_drag: bool,
}

impl<'a, N: Number> NumberField<'a, N> {
    pub fn new(id: Id, value: &'a mut N, style: &'a FieldStyle) -> Self {
        Self {
            id,
            value,
            style,
            range: (f64::NEG_INFINITY, f64::INFINITY),
            selected: false,
            label: "",
            box_drag: false,
        }
    }

    /// Clamp to this range on top of the type's own: a u32 token that models a
    /// u8 stops at 255, a list length stops at its maximum.
    pub fn range(mut self, min: f64, max: f64) -> Self {
        self.range = (min, max);
        self
    }

    /// The row that owns this field is selected: draw it active even without
    /// focus.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Accessibility name (the inspector's row label).
    pub fn label(mut self, label: &'a str) -> Self {
        self.label = label;
        self
    }

    /// Let the box itself drag, for a field with no name to grab (a list's size
    /// box). Rows scrub through their label instead.
    pub fn box_drag(mut self, box_drag: bool) -> Self {
        self.box_drag = box_drag;
        self
    }

    /// Draw the box, or its text editor while the edit buffer belongs here.
    pub fn show(self, ui: &mut Ui, rect: Rect, edit: &mut Option<NumberEdit>) -> FieldOutcome {
        if let Some(mut editor) = edit.take() {
            let outcome = self.draw_editor(ui, rect, &mut editor);
            if !outcome.finished {
                *edit = Some(editor);
            }
            return outcome;
        }
        self.draw_at_rest(ui, rect, edit)
    }

    /// One frame of the label scrub. Returns whether the value changed.
    pub fn scrub(&mut self, ui: &Ui, scrub: Scrub) -> bool {
        let mut changed = false;
        if scrub.delta != 0.0 {
            changed = self.drag(ui, scrub.delta);
        }
        if scrub.stopped {
            ui.ctx()
                .data_mut(|data| data.insert_temp(drag_key(self.id), 0.0_f32));
        }
        changed
    }

    fn draw_at_rest(
        mut self,
        ui: &mut Ui,
        rect: Rect,
        edit: &mut Option<NumberEdit>,
    ) -> FieldOutcome {
        let hovered = ui.rect_contains_pointer(rect);
        let focused = ui.memory(|memory| memory.has_focus(self.id));
        let active = self.selected || focused;
        let fill = if hovered && !active {
            self.style.hover_fill
        } else {
            self.style.fill
        };
        paint_box(ui, self.style, rect, fill);

        let galley = self.style.text.layout(ui, self.value.text());
        let text_pos = pos2(
            rect.left() + self.style.padding,
            rect.center().y - galley.size().y * 0.5,
        );
        ui.painter()
            .with_clip_rect(rect.shrink2(vec2(self.style.padding, 0.0)))
            .galley(text_pos, galley, self.style.text.text_color());

        let sense = if self.box_drag {
            Sense::click_and_drag()
        } else {
            Sense::click()
        };
        let mut response = ui.interact(rect, self.id, sense);
        if self.box_drag {
            response = response.on_hover_cursor(CursorIcon::ResizeHorizontal);
        }
        labelled_widget_info(&response, WidgetType::DragValue, self.label, None);

        if response.clicked() {
            *edit = Some(NumberEdit::new(self.value.text(), (*self.value).as_f64()));
            response.request_focus();
        }

        if self.box_drag {
            let delta = response.drag_delta().x - response.drag_delta().y;
            if response.dragged() && delta != 0.0 && self.drag(ui, delta) {
                response.mark_changed();
            }
            if response.drag_stopped() {
                ui.ctx()
                    .data_mut(|data| data.insert_temp(drag_key(self.id), 0.0_f32));
            }
        }

        paint_outline(ui, self.style, rect, self.style.outline(active, hovered));
        FieldOutcome {
            response,
            finished: false,
        }
    }

    fn draw_editor(mut self, ui: &mut Ui, rect: Rect, editor: &mut NumberEdit) -> FieldOutcome {
        let hovered = ui.rect_contains_pointer(rect);
        paint_box(ui, self.style, rect, self.style.fill);
        let mut response = text_edit_box(ui, rect, self.id, &mut editor.text, self.style);

        if editor.focus {
            response.request_focus();
            select_all(ui, self.id, editor.text.chars().count());
            editor.focus = false;
        }

        let mut changed = false;
        if response.changed() {
            changed = self.parse(&editor.text);
        }

        let escape = ui.input(|input| input.key_pressed(Key::Escape));
        let enter = ui.input(|input| input.key_pressed(Key::Enter));
        let mut finished = false;
        if escape {
            changed |= self.write(editor.original);
            response.surrender_focus();
            finished = true;
        } else if enter || response.lost_focus() {
            changed |= self.parse(&editor.text);
            finished = true;
        }
        if changed {
            response.mark_changed();
        }

        paint_outline(ui, self.style, rect, self.style.outline(true, hovered));
        FieldOutcome { response, finished }
    }

    fn drag(&mut self, ui: &Ui, delta: f32) -> bool {
        let current = (*self.value).as_f64();
        let raw = if N::WHOLE {
            let key = drag_key(self.id);
            let carried = ui
                .ctx()
                .data(|data| data.get_temp::<f32>(key).unwrap_or(0.0));
            let total = carried + delta;
            let steps = total.trunc();
            ui.ctx()
                .data_mut(|data| data.insert_temp(key, total - steps));
            current + f64::from(steps)
        } else {
            let moved = current as f32 + delta * drag_speed(current as f32) as f32;
            f64::from(round_decimal(moved, 3))
        };
        self.write(raw)
    }

    fn write(&mut self, raw: f64) -> bool {
        let clamped = raw.clamp(self.range.0, self.range.1);
        let next = N::from_f64(clamped);
        if next != *self.value {
            *self.value = next;
            true
        } else {
            false
        }
    }

    fn parse(&mut self, text: &str) -> bool {
        match text.trim().parse::<f64>() {
            Ok(raw) if raw.is_finite() => self.write(raw),
            _ => false,
        }
    }
}

fn drag_key(id: Id) -> Id {
    id.with("drag")
}

/// Units per point of drag: a 3 px padding needs a finer hand than a 130 px
/// pane height, and the value itself says how fine it is.
fn drag_speed(value: f32) -> f64 {
    (value.abs() / 50.0).clamp(0.01, 1.0) as f64
}

fn round_decimal(value: f32, places: i32) -> f32 {
    let factor = 10f32.powi(places);
    (value * factor).round() / factor
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::testing::{button, click, enter, input, rect, style};
    use egui::{Context, Event};

    fn field<'a>(value: &'a mut f32, style: &'a FieldStyle) -> NumberField<'a, f32> {
        NumberField::new(Id::new("kairos-field-number"), value, style).label("Width")
    }

    #[test]
    fn a_number_box_types_a_value_and_finishes_on_enter() {
        let ctx = Context::default();
        let style = style();
        let mut value = 1.0_f32;
        let mut edit: Option<NumberEdit> = None;

        let _ = ctx.run_ui(input(), |ui| {
            field(&mut value, &style).show(ui, rect(), &mut edit);
        });
        click(&ctx, rect().center(), |ui| {
            field(&mut value, &style).show(ui, rect(), &mut edit);
        });
        assert!(edit.is_some(), "a click opens the text editor");

        // One pass with no events so the editor can focus and select its text.
        let _ = ctx.run_ui(input(), |ui| {
            field(&mut value, &style).show(ui, rect(), &mut edit);
        });
        let mut typed = input();
        typed.events = vec![Event::Text("7".to_owned()), enter()];
        let _ = ctx.run_ui(typed, |ui| {
            field(&mut value, &style).show(ui, rect(), &mut edit);
        });

        assert_eq!(value, 7.0);
        assert!(edit.is_none(), "Enter ends the edit");
    }

    #[test]
    fn escape_puts_the_original_value_back() {
        let ctx = Context::default();
        let style = style();
        let mut value = 2.5_f32;
        let mut edit: Option<NumberEdit> = None;

        let _ = ctx.run_ui(input(), |ui| {
            field(&mut value, &style).show(ui, rect(), &mut edit);
        });
        click(&ctx, rect().center(), |ui| {
            field(&mut value, &style).show(ui, rect(), &mut edit);
        });
        let _ = ctx.run_ui(input(), |ui| {
            field(&mut value, &style).show(ui, rect(), &mut edit);
        });
        let mut typed = input();
        typed.events = vec![Event::Text("9".to_owned())];
        let _ = ctx.run_ui(typed, |ui| {
            field(&mut value, &style).show(ui, rect(), &mut edit);
        });
        assert_eq!(value, 9.0, "typing writes the value as it goes");

        let mut escape = input();
        escape.events = vec![Event::Key {
            key: Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        }];
        let _ = ctx.run_ui(escape, |ui| {
            field(&mut value, &style).show(ui, rect(), &mut edit);
        });
        assert_eq!(
            value, 2.5,
            "Escape restores the value the edit started from"
        );
        assert!(edit.is_none());
    }

    #[test]
    fn a_whole_number_type_rounds_and_clamps_its_typed_value() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-field-u8");
        let mut value = 3_u8;
        let mut edit: Option<NumberEdit> = None;
        let mut draw = |ui: &mut Ui| {
            NumberField::new(id, &mut value, &style).show(ui, rect(), &mut edit);
        };

        let _ = ctx.run_ui(input(), &mut draw);
        click(&ctx, rect().center(), &mut draw);
        let _ = ctx.run_ui(input(), &mut draw);
        let mut typed = input();
        typed.events = vec![Event::Text("999".to_owned()), enter()];
        let _ = ctx.run_ui(typed, &mut draw);

        assert_eq!(value, 255, "a u8 saturates instead of wrapping");
    }

    #[test]
    fn a_range_narrows_the_value_a_u32_token_may_hold() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-field-range");
        let mut value = 3_u32;
        let mut edit: Option<NumberEdit> = None;
        let mut draw = |ui: &mut Ui| {
            NumberField::new(id, &mut value, &style)
                .range(0.0, 64.0)
                .show(ui, rect(), &mut edit);
        };

        let _ = ctx.run_ui(input(), &mut draw);
        click(&ctx, rect().center(), &mut draw);
        let _ = ctx.run_ui(input(), &mut draw);
        let mut typed = input();
        typed.events = vec![Event::Text("900".to_owned()), enter()];
        let _ = ctx.run_ui(typed, &mut draw);

        assert_eq!(value, 64);
    }

    #[test]
    fn scrubbing_the_label_moves_a_float_and_steps_a_whole_number() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-field-scrub");
        let mut float = 10.0_f32;
        let mut whole = 10_u32;
        let mut edit: Option<NumberEdit> = None;

        let _ = ctx.run_ui(input(), |ui| {
            let mut number = NumberField::new(id, &mut float, &style);
            number.scrub(
                ui,
                Scrub {
                    delta: 10.0,
                    stopped: false,
                },
            );
            number.show(ui, rect(), &mut edit);

            let mut number = NumberField::new(id.with("whole"), &mut whole, &style);
            number.scrub(
                ui,
                Scrub {
                    delta: 0.4,
                    stopped: false,
                },
            );
            number.show(ui, rect(), &mut edit);
        });
        assert!(
            float > 10.0,
            "a rightward scrub raises a float, got {float}"
        );
        assert_eq!(whole, 10, "a fourth of a step does not move a whole number");

        let _ = ctx.run_ui(input(), |ui| {
            let mut number = NumberField::new(id.with("whole"), &mut whole, &style);
            number.scrub(
                ui,
                Scrub {
                    delta: 0.7,
                    stopped: true,
                },
            );
            number.show(ui, rect(), &mut edit);
        });
        assert_eq!(whole, 11, "the carried fraction crosses a unit");
    }

    #[test]
    fn the_box_only_drags_when_it_is_asked_to() {
        let ctx = Context::default();
        let style = style();
        let mut value = 10.0_f32;
        let mut edit: Option<NumberEdit> = None;

        let _ = ctx.run_ui(input(), |ui| {
            field(&mut value, &style).show(ui, rect(), &mut edit);
        });
        let start = rect().center();
        let mut press = input();
        press.events = vec![button(start, true)];
        let _ = ctx.run_ui(press, |ui| {
            field(&mut value, &style).show(ui, rect(), &mut edit);
        });
        let mut moved = input();
        moved.events = vec![Event::PointerMoved(start + egui::vec2(20.0, 0.0))];
        let _ = ctx.run_ui(moved, |ui| {
            field(&mut value, &style).show(ui, rect(), &mut edit);
        });

        assert_eq!(value, 10.0, "a row scrubs through its name, not its box");
        assert!(edit.is_none(), "a drag does not open the text editor");
    }

    #[test]
    fn a_field_box_paints_its_fill_and_outline() {
        let ctx = Context::default();
        let style = style();
        let mut value = 1.0_f32;
        let mut edit: Option<NumberEdit> = None;
        let output = ctx.run_ui(input(), |ui| {
            field(&mut value, &style).show(ui, rect(), &mut edit);
        });

        let fills: Vec<Rect> = output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Rect(shape) if shape.fill != egui::Color32::TRANSPARENT => {
                    Some(shape.rect)
                }
                _ => None,
            })
            .collect();
        let outlines: Vec<Rect> = output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Rect(shape) if shape.stroke.width > 0.0 => Some(shape.rect),
                _ => None,
            })
            .collect();

        assert!(fills.contains(&rect()), "painted fills {fills:?}");
        assert!(outlines.contains(&rect()), "painted outlines {outlines:?}");
    }
}
