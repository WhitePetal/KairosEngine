//! Boxed value fields: the right-hand column of an inspector.
//!
//! Unity's inspector is a list of properties: a name on the left, and a value
//! control drawn as a fixed-height box. The Preferences editor reproduces that
//! layout, and it is not the last place that will — an inspector panel for
//! scene objects draws the same shapes. So the controls live here, over plain
//! values (f32, u32, String, bool), and know nothing about style tokens,
//! dotted paths or domain actions. A component maps its own field table onto
//! them and decides what an edit means.
//!
//! Every control has the same shape:
//!
//!     let outcome = NumberField::float(id, &mut value, &style)
//!         .label("Width")
//!         .show(ui, rect, &mut edit);
//!     if outcome.changed() { .. }
//!
//! show takes the rect the caller laid out (an inspector's two columns belong
//! to the panel, not to egui's layout) and returns a [FieldOutcome]: the egui
//! [Response] plus whether a typed edit ended. That flag is what lets a panel
//! write its RON file when the user presses Enter, while a drag is written when
//! the pointer comes up.
//!
//! # State the caller owns
//!
//! A number box switches into a text editor on click. The editor's buffer is
//! [NumberEdit], which the caller keeps next to its own field state and hands
//! back every pass. That way a panel can drop a half-typed edit when the row
//! disappears, and a test can drive the control without knowing egui's memory
//! keys. Everything else — cursor, selection, focus — is the TextEdit's own
//! state.

use egui::{
    Align, Color32, CornerRadius, CursorIcon, Id, Key, Margin, Rect, Response, Sense, Stroke,
    StrokeKind, TextEdit, Ui, UiBuilder, WidgetType, pos2, vec2,
};

use crate::paint::labelled_widget_info;
use crate::text::TextStyle;

/// Visual tokens for one value field.
///
/// Components build one from their serializable style and reuse it for every
/// field on a page; a widget never reads the theme itself.
#[derive(Clone, Debug)]
pub struct FieldStyle {
    /// Value text.
    pub text: TextStyle,
    /// Box fill at rest.
    pub fill: Color32,
    /// Box fill under the pointer, before the field is selected.
    pub hover_fill: Color32,
    /// Outline at rest.
    pub edge: Color32,
    /// Outline under the pointer, before the field is selected.
    pub edge_hover: Color32,
    /// Outline of an active field: selected, focused, or being edited.
    pub accent: Color32,
    /// Outline thickness. Zero paints no outline.
    pub border_width: f32,
    pub corner_radius: u8,
    /// Horizontal inset of the value text.
    pub padding: f32,
    /// Side of a boolean checkbox.
    pub checkbox: f32,
}

impl FieldStyle {
    /// Neutral dark defaults; override the colours a component owns.
    pub fn new(text: TextStyle) -> Self {
        Self {
            text,
            fill: Color32::from_rgb(0x28, 0x28, 0x28),
            hover_fill: Color32::from_rgb(0x3E, 0x3E, 0x3E),
            edge: Color32::from_rgb(0x20, 0x20, 0x20),
            edge_hover: Color32::from_rgb(0x5A, 0x5A, 0x5A),
            accent: Color32::from_rgb(0x4A, 0x90, 0xD9),
            border_width: 1.0,
            corner_radius: 0,
            padding: 6.0,
            checkbox: 15.0,
        }
    }

    /// Box fill at rest and under the pointer.
    pub fn fills(mut self, fill: Color32, hover_fill: Color32) -> Self {
        self.fill = fill;
        self.hover_fill = hover_fill;
        self
    }

    /// Outline at rest, under the pointer, and while active.
    pub fn edges(mut self, edge: Color32, edge_hover: Color32, accent: Color32) -> Self {
        self.edge = edge;
        self.edge_hover = edge_hover;
        self.accent = accent;
        self
    }

    pub fn border_width(mut self, border_width: f32) -> Self {
        self.border_width = border_width;
        self
    }

    pub fn corner_radius(mut self, corner_radius: u8) -> Self {
        self.corner_radius = corner_radius;
        self
    }

    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }

    pub fn checkbox(mut self, checkbox: f32) -> Self {
        self.checkbox = checkbox;
        self
    }

    /// Outline colour of a field in this state.
    ///
    /// Active wins over hover: a selected field keeps its accent edge while the
    /// pointer is over it.
    pub fn outline(&self, active: bool, hovered: bool) -> Color32 {
        if active {
            self.accent
        } else if hovered {
            self.edge_hover
        } else {
            self.edge
        }
    }
}

/// What a field control did this pass.
#[derive(Debug)]
pub struct FieldOutcome {
    /// The control's own interaction response; its rect is the field's rect.
    pub response: Response,
    /// A typed edit or a picker gesture ended. A caller that batches writes
    /// should flush now instead of waiting for the pointer to come up.
    pub finished: bool,
}

impl FieldOutcome {
    pub fn changed(&self) -> bool {
        self.response.changed()
    }

    pub fn clicked(&self) -> bool {
        self.response.clicked()
    }
}

/// The open text editor of one number box.
///
/// Owned by the caller between passes: [NumberField::show] fills it on the click
/// that starts an edit and takes it back when the edit ends. Escape restores the
/// value the box had when the edit started.
#[derive(Clone, Debug)]
pub struct NumberEdit {
    text: String,
    /// Value before the edit, as f64: exact for both f32 and u32 fields.
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

/// A Unity-style number box: drag to scrub, click to type.
///
/// [NumberField::float] edits a continuous value, [NumberField::integer] a whole
/// number in 0..=max (a u8 corner radius, a list length).
pub struct NumberField<'a> {
    id: Id,
    value: Value<'a>,
    style: &'a FieldStyle,
    selected: bool,
    label: &'a str,
}

enum Value<'a> {
    Float(&'a mut f32),
    Integer { value: &'a mut u32, max: u32 },
}

impl<'a> NumberField<'a> {
    pub fn float(id: Id, value: &'a mut f32, style: &'a FieldStyle) -> Self {
        Self {
            id,
            value: Value::Float(value),
            style,
            selected: false,
            label: "",
        }
    }

    /// A whole-number field clamped to 0..=max, both by drag and by typing.
    pub fn integer(id: Id, value: &'a mut u32, max: u32, style: &'a FieldStyle) -> Self {
        Self {
            id,
            value: Value::Integer { value, max },
            style,
            selected: false,
            label: "",
        }
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

    fn draw_at_rest(self, ui: &mut Ui, rect: Rect, edit: &mut Option<NumberEdit>) -> FieldOutcome {
        let Self {
            id,
            mut value,
            style,
            selected,
            label,
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

        let galley = style.text.layout(ui, value.text());
        let text_pos = pos2(
            rect.left() + style.padding,
            rect.center().y - galley.size().y * 0.5,
        );
        ui.painter()
            .with_clip_rect(rect.shrink2(vec2(style.padding, 0.0)))
            .galley(text_pos, galley, style.text.text_color());

        let mut response = ui
            .interact(rect, id, Sense::click_and_drag())
            .on_hover_cursor(CursorIcon::ResizeHorizontal);
        labelled_widget_info(&response, WidgetType::DragValue, label, None);

        if response.clicked() {
            *edit = Some(NumberEdit::new(value.text(), value.read()));
            response.request_focus();
        }

        let delta = response.drag_delta().x - response.drag_delta().y;
        if response.dragged() && delta != 0.0 && value.drag(delta, ui, id) {
            response.mark_changed();
        }
        if response.drag_stopped() {
            ui.ctx()
                .data_mut(|data| data.insert_temp(drag_key(id), 0.0_f32));
        }

        paint_outline(ui, style, rect, style.outline(active, hovered));
        FieldOutcome {
            response,
            finished: false,
        }
    }

    fn draw_editor(self, ui: &mut Ui, rect: Rect, editor: &mut NumberEdit) -> FieldOutcome {
        let Self {
            id,
            mut value,
            style,
            ..
        } = self;
        let hovered = ui.rect_contains_pointer(rect);
        paint_box(ui, style, rect, style.fill);

        let padding = value_padding(ui, style, rect);
        let mut response = ui
            .scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                ui.visuals_mut().selection.bg_fill = style.accent;
                ui.visuals_mut().selection.stroke = Stroke::NONE;
                let font = style.text.font_id(ui.style());
                ui.add(
                    TextEdit::singleline(&mut editor.text)
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
            .inner;

        if editor.focus {
            response.request_focus();
            select_all_text(ui, id, editor.text.chars().count());
            editor.focus = false;
        }

        let mut changed = false;
        if response.changed() {
            changed = value.set_text(&editor.text);
        }

        let escape = ui.input(|input| input.key_pressed(Key::Escape));
        let enter = ui.input(|input| input.key_pressed(Key::Enter));
        let mut finished = false;
        if escape {
            changed |= value.restore(editor.original);
            response.surrender_focus();
            finished = true;
        } else if enter || response.lost_focus() {
            changed |= value.set_text(&editor.text);
            finished = true;
        }
        if changed {
            response.mark_changed();
        }

        paint_outline(ui, style, rect, style.outline(true, hovered));
        FieldOutcome { response, finished }
    }
}

impl Value<'_> {
    fn read(&self) -> f64 {
        match self {
            Self::Float(value) => **value as f64,
            Self::Integer { value, .. } => **value as f64,
        }
    }

    /// The displayed form: floats as f32 prints them, integers without a
    /// decimal point.
    fn text(&self) -> String {
        match self {
            Self::Float(value) => float_text(**value),
            Self::Integer { value, .. } => value.to_string(),
        }
    }

    /// Write the text if it parses; returns whether the value changed. An
    /// integer field rounds and clamps, so typing 999 into a u8 token lands on
    /// 255.
    fn set_text(&mut self, text: &str) -> bool {
        let text = text.trim();
        match self {
            Self::Float(current) => match text.parse::<f32>() {
                Ok(next) if next.is_finite() && next != **current => {
                    **current = next;
                    true
                }
                _ => false,
            },
            Self::Integer { value, max } => match text.parse::<f32>() {
                Ok(next) if next.is_finite() => {
                    let next = next.round().clamp(0.0, *max as f32) as u32;
                    if next != **value {
                        **value = next;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
        }
    }

    /// Put back the value the edit started from (Escape).
    fn restore(&mut self, original: f64) -> bool {
        match self {
            Self::Float(current) => {
                let next = original as f32;
                if next != **current {
                    **current = next;
                    true
                } else {
                    false
                }
            }
            Self::Integer { value, .. } => {
                let next = original as u32;
                if next != **value {
                    **value = next;
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Apply one pointer delta. Floats move by a magnitude-aware speed; whole
    /// numbers accumulate the fraction in egui's memory so a slow drag still
    /// crosses a unit.
    fn drag(&mut self, delta: f32, ui: &Ui, id: Id) -> bool {
        match self {
            Self::Float(current) => {
                let next = round_decimal(**current + delta * drag_speed(**current) as f32, 3);
                if next != **current {
                    **current = next;
                    true
                } else {
                    false
                }
            }
            Self::Integer { value, max } => {
                let key = drag_key(id);
                let carried = ui
                    .ctx()
                    .data(|data| data.get_temp::<f32>(key).unwrap_or(0.0));
                let total = carried + delta;
                let steps = total.trunc();
                ui.ctx()
                    .data_mut(|data| data.insert_temp(key, total - steps));
                if steps == 0.0 {
                    return false;
                }
                let next = ((**value as i64) + steps as i64).clamp(0, *max as i64) as u32;
                if next != **value {
                    **value = next;
                    true
                } else {
                    false
                }
            }
        }
    }
}

/// A string token (an icon path, a font family): a box whose whole interior is
/// an editable field.
pub struct TextField<'a> {
    id: Id,
    value: &'a mut String,
    style: &'a FieldStyle,
    selected: bool,
    label: &'a str,
}

impl<'a> TextField<'a> {
    pub fn new(id: Id, value: &'a mut String, style: &'a FieldStyle) -> Self {
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
            ..
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

        let padding = value_padding(ui, style, rect);
        let response = ui
            .scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                ui.visuals_mut().selection.bg_fill = style.accent;
                ui.visuals_mut().selection.stroke = Stroke::NONE;
                let font = style.text.font_id(ui.style());
                ui.add(
                    TextEdit::singleline(value)
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
            .inner;

        let finished = response.lost_focus();
        let active_now = active || response.has_focus();
        paint_outline(ui, style, rect, style.outline(active_now, hovered));
        FieldOutcome { response, finished }
    }
}

/// A boolean token: Unity's square checkbox at the start of the value column.
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

fn drag_key(id: Id) -> Id {
    id.with("drag")
}

fn float_text(value: f32) -> String {
    let mut text = format!("{value}");
    if text == "-0" {
        text = "0".to_owned();
    }
    text
}

fn round_decimal(value: f32, places: i32) -> f32 {
    let factor = 10f32.powi(places);
    (value * factor).round() / factor
}

/// Units per point of drag: a 3 px padding needs a finer hand than a 130 px
/// pane height, and the value itself says how fine it is.
fn drag_speed(value: f32) -> f64 {
    (value.abs() / 50.0).clamp(0.01, 1.0) as f64
}

/// Horizontal text padding plus the vertical padding that centres one line in a
/// box (a TextEdit sizes itself by its font's row height, not by the box, so the
/// difference is split above and below).
fn value_padding(ui: &Ui, style: &FieldStyle, rect: Rect) -> Margin {
    let font = style.text.font_id(ui.style());
    let row_height = ui.fonts_mut(|fonts| fonts.row_height(&font));
    let vertical = ((rect.height() - row_height) * 0.5).round().max(0.0) as i8;
    Margin::symmetric(style.padding.round() as i8, vertical)
}

fn paint_box(ui: &Ui, style: &FieldStyle, rect: Rect, fill: Color32) {
    ui.painter()
        .rect_filled(rect, CornerRadius::same(style.corner_radius), fill);
}

fn paint_outline(ui: &Ui, style: &FieldStyle, rect: Rect, edge: Color32) {
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

fn paint_check(ui: &Ui, style: &FieldStyle, rect: Rect) {
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

fn select_all_text(ui: &Ui, id: Id, chars: usize) {
    let mut state = TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::default(),
            egui::text::CCursor::new(chars),
        )));
    state.store(ui.ctx(), id);
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Context, Event, Modifiers, PointerButton, Pos2, RawInput};

    fn input() -> RawInput {
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(240.0, 80.0))),
            ..Default::default()
        }
    }

    fn style() -> FieldStyle {
        FieldStyle::new(TextStyle::sized(Color32::from_rgb(220, 220, 220), 12.0))
            .fills(
                Color32::from_rgb(0x28, 0x28, 0x28),
                Color32::from_rgb(0x3E, 0x3E, 0x3E),
            )
            .edges(
                Color32::from_rgb(0x20, 0x20, 0x20),
                Color32::from_rgb(0x5A, 0x5A, 0x5A),
                Color32::from_rgb(0x4A, 0x90, 0xD9),
            )
    }

    fn rect() -> Rect {
        Rect::from_min_size(Pos2::ZERO, vec2(120.0, 18.0))
    }

    fn button(pos: Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::default(),
        }
    }

    fn enter() -> Event {
        Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::default(),
        }
    }

    fn click(ctx: &Context, at: Pos2, mut draw: impl FnMut(&mut Ui)) {
        let mut press = input();
        press.events = vec![button(at, true)];
        let _ = ctx.run_ui(press, &mut draw);
        let mut release = input();
        release.events = vec![button(at, false)];
        let _ = ctx.run_ui(release, &mut draw);
    }

    fn number(
        ui: &mut Ui,
        id: Id,
        style: &FieldStyle,
        value: &mut f32,
        edit: &mut Option<NumberEdit>,
    ) {
        NumberField::float(id, value, style)
            .label("Width")
            .show(ui, rect(), edit);
    }

    fn whole(
        ui: &mut Ui,
        id: Id,
        style: &FieldStyle,
        value: &mut u32,
        edit: &mut Option<NumberEdit>,
    ) {
        NumberField::integer(id, value, 255, style).show(ui, rect(), edit);
    }

    /// The click that opens the editor, then typing replaces the selection.
    #[test]
    fn a_number_box_types_a_value_and_finishes_on_enter() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-field-number");
        let mut value = 1.0_f32;
        let mut edit: Option<NumberEdit> = None;

        let _ = ctx.run_ui(input(), |ui| number(ui, id, &style, &mut value, &mut edit));
        click(&ctx, rect().center(), |ui| {
            number(ui, id, &style, &mut value, &mut edit)
        });
        assert!(edit.is_some(), "a click opens the text editor");

        // One pass with no events so the editor can focus and select its text.
        let _ = ctx.run_ui(input(), |ui| number(ui, id, &style, &mut value, &mut edit));
        let mut typed = input();
        typed.events = vec![Event::Text("7".to_owned()), enter()];
        let _ = ctx.run_ui(typed, |ui| number(ui, id, &style, &mut value, &mut edit));

        assert_eq!(value, 7.0);
        assert!(edit.is_none(), "Enter ends the edit");
    }

    #[test]
    fn escape_puts_the_original_value_back() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-field-escape");
        let mut value = 2.5_f32;
        let mut edit: Option<NumberEdit> = None;

        let _ = ctx.run_ui(input(), |ui| number(ui, id, &style, &mut value, &mut edit));
        click(&ctx, rect().center(), |ui| {
            number(ui, id, &style, &mut value, &mut edit)
        });
        let _ = ctx.run_ui(input(), |ui| number(ui, id, &style, &mut value, &mut edit));
        let mut typed = input();
        typed.events = vec![Event::Text("9".to_owned())];
        let _ = ctx.run_ui(typed, |ui| number(ui, id, &style, &mut value, &mut edit));
        assert_eq!(value, 9.0, "typing writes the value as it goes");

        let mut escape = input();
        escape.events = vec![Event::Key {
            key: Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::default(),
        }];
        let _ = ctx.run_ui(escape, |ui| number(ui, id, &style, &mut value, &mut edit));
        assert_eq!(
            value, 2.5,
            "Escape restores the value the edit started from"
        );
        assert!(edit.is_none());
    }

    #[test]
    fn integer_fields_round_and_clamp_a_typed_value() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-field-integer");
        let mut value = 3_u32;
        let mut edit: Option<NumberEdit> = None;

        let _ = ctx.run_ui(input(), |ui| whole(ui, id, &style, &mut value, &mut edit));
        click(&ctx, rect().center(), |ui| {
            whole(ui, id, &style, &mut value, &mut edit)
        });
        let _ = ctx.run_ui(input(), |ui| whole(ui, id, &style, &mut value, &mut edit));
        let mut typed = input();
        typed.events = vec![Event::Text("999".to_owned()), enter()];
        let _ = ctx.run_ui(typed, |ui| whole(ui, id, &style, &mut value, &mut edit));

        assert_eq!(value, 255, "u8 tokens clamp instead of wrapping");
    }

    #[test]
    fn dragging_a_float_moves_it_by_the_pointer_delta() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-field-drag");
        let mut value = 10.0_f32;
        let mut edit: Option<NumberEdit> = None;

        let _ = ctx.run_ui(input(), |ui| number(ui, id, &style, &mut value, &mut edit));
        let start = rect().center();
        let mut press = input();
        press.events = vec![button(start, true)];
        let _ = ctx.run_ui(press, |ui| number(ui, id, &style, &mut value, &mut edit));

        let mut moved = input();
        moved.events = vec![Event::PointerMoved(start + vec2(10.0, 0.0))];
        let _ = ctx.run_ui(moved, |ui| number(ui, id, &style, &mut value, &mut edit));

        assert!(
            value > 10.0,
            "a rightward drag raises the value, got {value}"
        );
        assert!(edit.is_none(), "a drag does not open the text editor");
    }

    #[test]
    fn a_checkbox_toggles_on_click() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-field-toggle");
        let mut value = false;

        let _ = ctx.run_ui(input(), |ui| {
            ToggleField::new(id, &mut value, &style)
                .label("Visible")
                .show(ui, rect());
        });
        click(&ctx, pos2(rect().left() + 7.0, rect().center().y), |ui| {
            ToggleField::new(id, &mut value, &style)
                .label("Visible")
                .show(ui, rect());
        });
        assert!(value);
    }

    /// A field box paints its fill and its outline over the rect the caller
    /// laid out: the inspector columns belong to the panel, not to egui.
    #[test]
    fn a_field_box_paints_its_fill_and_outline() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-field-paint");
        let mut value = 1.0_f32;
        let mut edit: Option<NumberEdit> = None;
        let output = ctx.run_ui(input(), |ui| {
            NumberField::float(id, &mut value, &style).show(ui, rect(), &mut edit);
        });

        let fills: Vec<Rect> = output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Rect(shape) if shape.fill != Color32::TRANSPARENT => Some(shape.rect),
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
