//! The number box: Unity's scrub-to-drag, click-to-type field.

use std::fmt::Display;

use egui::{CursorIcon, Id, Key, Rect, Sense, Ui, WidgetType, pos2, vec2};

use super::edit::{select_all, text_edit_box};
use super::label::FieldLabelResponse;
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

    /// The value as f64, for the shared drag and range maths.
    fn as_f64(self) -> f64;

    /// Round and clamp an f64 into this type (`u8` saturates at 255).
    fn from_f64(value: f64) -> Self;

    /// Parse the type's own text. Exact for `u64`/`i64`, unlike the f64
    /// fallback the box uses for out-of-range input. A custom number can keep
    /// the f64 default; the primitive impls override it.
    fn parse(text: &str) -> Option<Self> {
        text.parse::<f64>().ok().map(Self::from_f64)
    }

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

                fn parse(text: &str) -> Option<Self> {
                    text.parse::<$ty>().ok()
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

                fn parse(text: &str) -> Option<Self> {
                    text.parse::<$ty>().ok()
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
    /// The text the box showed before the edit. Keeping the text (rather than
    /// an f64) preserves a `u64`/`i64` that is larger than 2^53 on Escape.
    original: String,
    /// Focus the editor and select everything on the first pass.
    focus: bool,
}

impl NumberEdit {
    fn new(text: String) -> Self {
        Self {
            original: text.clone(),
            text,
            focus: true,
        }
    }
}

/// A Unity-style number box: dragging the field's name scrubs it, a click types
/// into it.
///
/// The name is a separate widget ([FieldLabel]) because the row owns the label
/// column; [NumberField::dragged_by] hands this pass's name gesture to the box,
/// so the scrub itself (speed, remainder, clamping) stays here and the row only
/// wires two widgets together.
pub struct NumberField<'a, N: Number> {
    id: Id,
    value: &'a mut N,
    style: &'a FieldStyle,
    range: (f64, f64),
    selected: bool,
    label: &'a str,
    /// The name's gesture this pass.
    drag: Option<Scrub>,
    /// The value box itself drags. True until a name supplies the handle.
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
            drag: None,
            box_drag: true,
        }
    }

    /// Clamp to this UI range on top of the type's own: a list length stops at
    /// its maximum, a padding at its design limit. Type bounds stay with `N`.
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

    /// Take this pass's name gesture: Unity scrubs a number by dragging the
    /// row's name, so the box itself stops being a handle.
    pub fn dragged_by(mut self, label: &FieldLabelResponse) -> Self {
        self.drag = label.scrub();
        self.box_drag = false;
        self
    }

    /// Draw the box, or its text editor while the edit buffer belongs here.
    pub fn show(mut self, ui: &mut Ui, rect: Rect, edit: &mut Option<NumberEdit>) -> FieldOutcome {
        // The name's scrub lands before the box is drawn, so the value and the
        // file follow the pointer on the same pass.
        let mut scrubbed = false;
        if let Some(scrub) = self.drag.take() {
            scrubbed = self.apply_scrub(ui, scrub);
        }
        let mut outcome = if let Some(mut editor) = edit.take() {
            let outcome = self.draw_editor(ui, rect, &mut editor);
            if !outcome.finished {
                *edit = Some(editor);
            }
            outcome
        } else {
            self.draw_at_rest(ui, rect, edit)
        };
        if scrubbed {
            outcome.response.mark_changed();
        }
        outcome
    }

    fn apply_scrub(&mut self, ui: &Ui, scrub: Scrub) -> bool {
        let mut changed = false;
        if scrub.delta != 0.0 {
            changed = self.drag_value(ui, scrub.delta);
        }
        if scrub.stopped {
            clear_drag(ui.ctx(), self.id);
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
        let mut response = ui
            .interact(rect, self.id, sense)
            .on_hover_and_drag_cursor(CursorIcon::Text);
        labelled_widget_info(&response, WidgetType::DragValue, self.label, None);

        if response.clicked() {
            *edit = Some(NumberEdit::new(self.value.text()));
            response.request_focus();
        }

        if self.box_drag {
            if response.drag_started() {
                // A new gesture starts from the value it grabbed, not from a
                // step left behind by an interrupted one.
                clear_drag(ui.ctx(), self.id);
            }
            let delta = response.drag_delta().x - response.drag_delta().y;
            if response.dragged() && delta != 0.0 && self.drag_value(ui, delta) {
                response.mark_changed();
            }
            if response.drag_stopped() {
                clear_drag(ui.ctx(), self.id);
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
            changed |= self.parse(&editor.original);
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

    fn drag_value(&mut self, ui: &Ui, delta: f32) -> bool {
        // egui may run a pass twice in one frame (a layout asked for a
        // discard). The second pass sees the value the first one wrote, so
        // adding this frame's delta again would move the field twice. Keep the
        // value and the delta of the frame's first pass and recompute: the
        // same pixels give the same value no matter how often the pass runs.
        let frame = ui.ctx().cumulative_frame_nr();
        let key = drag_key(self.id);
        let mut track = ui
            .ctx()
            .data(|data| data.get_temp::<DragTrack>(key))
            .unwrap_or(DragTrack {
                frame,
                base: (*self.value).as_f64(),
                moved: delta,
                carry: 0.0,
            });
        if track.frame != frame {
            track.base = (*self.value).as_f64();
            track.moved = delta;
            track.frame = frame;
        }

        let raw = if N::WHOLE {
            let total = track.carry + track.moved;
            let steps = total.trunc();
            track.carry = total - steps;
            track.base + f64::from(steps)
        } else {
            let moved = track.base as f32 + track.moved * drag_speed(track.base as f32) as f32;
            f64::from(round_decimal(moved, 3))
        };
        ui.ctx().data_mut(|data| data.insert_temp(key, track));
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
        let text = text.trim();
        // Parse the field's own type first: a u64 above 2^53 must not be
        // round-tripped through f64. Out-of-range or fractional input falls
        // back to the f64 path, which rounds and clamps like the type's own
        // bounds.
        if let Some(value) = N::parse(text) {
            let raw = value.as_f64();
            if !raw.is_finite() {
                return false;
            }
            if raw < self.range.0 || raw > self.range.1 {
                return self.write(raw);
            }
            if value != *self.value {
                *self.value = value;
                return true;
            }
            return false;
        }
        match text.parse::<f64>() {
            Ok(raw) if raw.is_finite() => self.write(raw),
            _ => false,
        }
    }
}

/// One scrub's step within a frame: the value it started at, the pixels this
/// frame moved, and the whole-unit remainder carried between frames.
#[derive(Clone, Copy, Debug)]
struct DragTrack {
    frame: u64,
    base: f64,
    moved: f32,
    carry: f32,
}

fn drag_key(id: Id) -> Id {
    id.with("drag")
}

fn clear_drag(ctx: &egui::Context, id: Id) {
    ctx.data_mut(|data| data.remove::<DragTrack>(drag_key(id)));
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
    use crate::field::FieldLabel;
    use crate::field::testing::{button, click, enter, input, label_rect, rect, style};
    use egui::{Context, Event};

    /// One row, wired the way a page wires it: the name owns the gesture, the
    /// box owns the value.
    fn row<N: Number>(
        ui: &mut Ui,
        style: &FieldStyle,
        value: &mut N,
        edit: &mut Option<NumberEdit>,
        range: Option<(f64, f64)>,
    ) {
        let label = FieldLabel::new(Id::new("kairos-number-name"), "Width", style).draggable(true);
        let gesture = label.interact(ui, label_rect());
        let mut field =
            NumberField::new(Id::new("kairos-number-box"), value, style).dragged_by(&gesture);
        if let Some((min, max)) = range {
            field = field.range(min, max);
        }
        field.show(ui, rect(), edit);
    }

    fn drag(from: egui::Pos2, by: f32, ctx: &Context, mut draw: impl FnMut(&mut Ui)) {
        let mut hover = input();
        hover.events = vec![Event::PointerMoved(from)];
        let _ = ctx.run_ui(hover, &mut draw);
        let mut press = input();
        press.events = vec![button(from, true)];
        let _ = ctx.run_ui(press, &mut draw);
        let mut moved = input();
        moved.events = vec![Event::PointerMoved(from + egui::vec2(by, 0.0))];
        let _ = ctx.run_ui(moved, &mut draw);
    }

    #[test]
    fn a_number_box_types_a_value_and_finishes_on_enter() {
        let ctx = Context::default();
        let style = style();
        let mut value = 1.0_f32;
        let mut edit: Option<NumberEdit> = None;

        let _ = ctx.run_ui(input(), |ui| row(ui, &style, &mut value, &mut edit, None));
        click(&ctx, rect().center(), |ui| {
            row(ui, &style, &mut value, &mut edit, None)
        });
        assert!(edit.is_some(), "a click opens the text editor");

        // One pass with no events so the editor can focus and select its text.
        let _ = ctx.run_ui(input(), |ui| row(ui, &style, &mut value, &mut edit, None));
        let mut typed = input();
        typed.events = vec![Event::Text("7".to_owned()), enter()];
        let _ = ctx.run_ui(typed, |ui| row(ui, &style, &mut value, &mut edit, None));

        assert_eq!(value, 7.0);
        assert!(edit.is_none(), "Enter ends the edit");
    }

    #[test]
    fn escape_puts_the_original_value_back() {
        let ctx = Context::default();
        let style = style();
        let mut value = 2.5_f32;
        let mut edit: Option<NumberEdit> = None;

        let _ = ctx.run_ui(input(), |ui| row(ui, &style, &mut value, &mut edit, None));
        click(&ctx, rect().center(), |ui| {
            row(ui, &style, &mut value, &mut edit, None)
        });
        let _ = ctx.run_ui(input(), |ui| row(ui, &style, &mut value, &mut edit, None));
        let mut typed = input();
        typed.events = vec![Event::Text("9".to_owned())];
        let _ = ctx.run_ui(typed, |ui| row(ui, &style, &mut value, &mut edit, None));
        assert_eq!(value, 9.0, "typing writes the value as it goes");

        let mut escape = input();
        escape.events = vec![Event::Key {
            key: Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        }];
        let _ = ctx.run_ui(escape, |ui| row(ui, &style, &mut value, &mut edit, None));
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
        let mut value = 3_u8;
        let mut edit: Option<NumberEdit> = None;

        let _ = ctx.run_ui(input(), |ui| row(ui, &style, &mut value, &mut edit, None));
        click(&ctx, rect().center(), |ui| {
            row(ui, &style, &mut value, &mut edit, None)
        });
        let _ = ctx.run_ui(input(), |ui| row(ui, &style, &mut value, &mut edit, None));
        let mut typed = input();
        typed.events = vec![Event::Text("999".to_owned()), enter()];
        let _ = ctx.run_ui(typed, |ui| row(ui, &style, &mut value, &mut edit, None));

        assert_eq!(value, 255, "a u8 saturates instead of wrapping");
    }

    #[test]
    fn a_u64_above_f64_precision_round_trips_exactly() {
        let ctx = Context::default();
        let style = style();
        let mut value = 0_u64;
        let mut edit: Option<NumberEdit> = None;

        let _ = ctx.run_ui(input(), |ui| row(ui, &style, &mut value, &mut edit, None));
        click(&ctx, rect().center(), |ui| {
            row(ui, &style, &mut value, &mut edit, None)
        });
        let _ = ctx.run_ui(input(), |ui| row(ui, &style, &mut value, &mut edit, None));
        let mut typed = input();
        typed.events = vec![Event::Text(u64::MAX.to_string()), enter()];
        let _ = ctx.run_ui(typed, |ui| row(ui, &style, &mut value, &mut edit, None));
        assert_eq!(value, u64::MAX);

        // Escape restores the exact original text, not an f64 approximation.
        click(&ctx, rect().center(), |ui| {
            row(ui, &style, &mut value, &mut edit, None)
        });
        let _ = ctx.run_ui(input(), |ui| row(ui, &style, &mut value, &mut edit, None));
        let mut typed = input();
        typed.events = vec![
            Event::Text("1".to_owned()),
            Event::Key {
                key: Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::default(),
            },
        ];
        let _ = ctx.run_ui(typed, |ui| row(ui, &style, &mut value, &mut edit, None));
        assert_eq!(value, u64::MAX);
    }

    #[test]
    fn a_range_narrows_the_value_a_u32_token_may_hold() {
        let ctx = Context::default();
        let style = style();
        let mut value = 3_u32;
        let mut edit: Option<NumberEdit> = None;

        let _ = ctx.run_ui(input(), |ui| {
            row(ui, &style, &mut value, &mut edit, Some((0.0, 64.0)))
        });
        click(&ctx, rect().center(), |ui| {
            row(ui, &style, &mut value, &mut edit, Some((0.0, 64.0)))
        });
        let _ = ctx.run_ui(input(), |ui| {
            row(ui, &style, &mut value, &mut edit, Some((0.0, 64.0)))
        });
        let mut typed = input();
        typed.events = vec![Event::Text("900".to_owned()), enter()];
        let _ = ctx.run_ui(typed, |ui| {
            row(ui, &style, &mut value, &mut edit, Some((0.0, 64.0)))
        });

        assert_eq!(value, 64);
    }

    /// Unity's inspector scours a number by dragging its name: the box itself
    /// is a text field, not a handle.
    #[test]
    fn dragging_the_name_scrubs_the_value() {
        let ctx = Context::default();
        let style = style();
        let mut value = 10.0_f32;
        let mut edit: Option<NumberEdit> = None;

        let _ = ctx.run_ui(input(), |ui| row(ui, &style, &mut value, &mut edit, None));
        drag(label_rect().center(), 20.0, &ctx, |ui| {
            row(ui, &style, &mut value, &mut edit, None)
        });

        assert_eq!(value, 14.0, "20 px on a 10 px value adds 10 * 0.2");
        assert!(edit.is_none(), "a scrub does not open the text editor");
    }

    #[test]
    fn the_box_does_not_scrub_when_the_name_supplies_the_gesture() {
        let ctx = Context::default();
        let style = style();
        let mut value = 10.0_f32;
        let mut edit: Option<NumberEdit> = None;

        let _ = ctx.run_ui(input(), |ui| row(ui, &style, &mut value, &mut edit, None));
        drag(rect().center(), 20.0, &ctx, |ui| {
            row(ui, &style, &mut value, &mut edit, None)
        });

        assert_eq!(value, 10.0, "the box only clicks into its editor");
    }

    #[test]
    fn a_name_less_box_drags_itself() {
        let ctx = Context::default();
        let style = style();
        let mut value = 10.0_f32;
        let mut edit: Option<NumberEdit> = None;

        let mut draw = |ui: &mut Ui| {
            NumberField::new(Id::new("kairos-number-box"), &mut value, &style).show(
                ui,
                rect(),
                &mut edit,
            );
        };
        let _ = ctx.run_ui(input(), &mut draw);
        drag(rect().center(), 20.0, &ctx, |ui| {
            NumberField::new(Id::new("kairos-number-box"), &mut value, &style).show(
                ui,
                rect(),
                &mut edit,
            );
        });

        assert_eq!(value, 14.0, "a size box with no name is its own handle");
    }

    #[test]
    fn hovering_the_box_shows_the_text_cursor() {
        let ctx = Context::default();
        let style = style();
        let mut value = 1.0_f32;
        let mut edit: Option<NumberEdit> = None;

        let _ = ctx.run_ui(input(), |ui| row(ui, &style, &mut value, &mut edit, None));
        let mut hover = input();
        hover.events = vec![Event::PointerMoved(rect().center())];
        let output = ctx.run_ui(hover, |ui| row(ui, &style, &mut value, &mut edit, None));

        assert_eq!(
            output.platform_output.cursor_icon,
            egui::CursorIcon::Text,
            "a click types into the box, so the pointer reads as a caret"
        );
    }

    #[test]
    fn a_field_box_paints_its_fill_and_outline() {
        let ctx = Context::default();
        let style = style();
        let mut value = 1.0_f32;
        let mut edit: Option<NumberEdit> = None;
        let output = ctx.run_ui(input(), |ui| row(ui, &style, &mut value, &mut edit, None));

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

    /// egui may run a pass twice in one frame (a layout requested a discard).
    /// The field must move by this frame's pointer delta once, not once per
    /// pass: the second pass sees the value the first one wrote.
    #[test]
    fn a_discarded_pass_does_not_apply_the_drag_twice() {
        let ctx = Context::default();
        let style = style();
        let mut value = 10.0_f32;
        let mut edit: Option<NumberEdit> = None;

        let from = label_rect().center();
        let _ = ctx.run_ui(input(), |ui| row(ui, &style, &mut value, &mut edit, None));
        let mut hover = input();
        hover.events = vec![Event::PointerMoved(from)];
        let _ = ctx.run_ui(hover, |ui| row(ui, &style, &mut value, &mut edit, None));
        let mut press = input();
        press.events = vec![button(from, true)];
        let _ = ctx.run_ui(press, |ui| row(ui, &style, &mut value, &mut edit, None));

        let mut moved = input();
        moved.events = vec![Event::PointerMoved(from + egui::vec2(20.0, 0.0))];
        let passes = std::cell::Cell::new(0);
        let _ = ctx.run_ui(moved, |ui| {
            passes.set(passes.get() + 1);
            row(ui, &style, &mut value, &mut edit, None);
            ui.ctx().request_discard("two-pass number drag");
        });

        assert!(
            passes.get() >= 2,
            "the frame must run twice: {} pass(es)",
            passes.get()
        );
        assert_eq!(value, 14.0, "10 + 20 px at 0.2 per px, applied once");
    }
}
