//! A modal dialog: the editor's confirm / alert frame.
//!
//! The widget wraps [`egui::Modal`] and adds nothing the platform already
//! provides: egui draws the dimmed backdrop, blocks pointer input to the layers
//! below and keeps the dialog centred. What this module owns is the *editor's*
//! look — fill, border, shadow, the title strip, the body typography and the
//! button row — so a dialog is not an egui-default window dropped into the
//! authoring tool — and the editor's dismissal rule: the title strip's red
//! close circle (the project's shared system window button) and Escape both
//! dismiss, a click on the dim backdrop does nothing. A question the user asked
//! the editor to stop at must be answered with a button (or that close circle /
//! Escape), never with a stray click outside it.
//!
//! The widget is pure appearance: it returns a [`ModalChoice`] and never emits a
//! domain action. The caller keeps the "is a dialog open, and for what" state;
//! for the Project panel that is panel view state (a half-open confirmation must
//! not survive closing the tab).
//!
//! Keyboard: Enter activates the first non-[`ModalRole::Danger`] button, so a
//! destructive button is only ever reachable by clicking it. Escape and the
//! title strip's red close circle report [`ModalChoice::Dismissed`]; a click on
//! the backdrop does not (it is blocked like any other click that is not on a
//! button).
//!
//! A dialog is centred by default. [`Modal::show_draggable`] additionally makes
//! the title strip grab the dialog and move it, with the caller owning the
//! remembered offset: a dialog position is view state, not widget state, and a
//! tab that is reopened must not remember a drag it never made.

use egui::{
    Align, Align2, Color32, CornerRadius, Frame, Key, Layout, Margin, Modifiers, Rect, Sense,
    Shadow, Stroke, Ui, Vec2, pos2, vec2,
};

use crate::button::{Button, ButtonAlign, ButtonStyle};
use crate::text::TextStyle;
use crate::window_controls::{WindowControlsEnabled, WindowControlsStyle, window_controls};

/// What a dialog button means, which decides both its paint and whether the
/// Enter key may press it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModalRole {
    /// The safe affirmative ("OK", "Cancel"): Enter presses the first one.
    ///
    /// The role is behavioural, not a fill: the editor's dialogs give every
    /// non-destructive button the same neutral button style (variant C of the
    /// design review), so there is no accent "primary" look to pick between.
    Default,
    /// A destructive action ("Delete"). Never the Enter default, and the only
    /// button that takes the danger fill.
    Danger,
}

/// One button of a dialog, in left-to-right display order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModalButton<'a> {
    pub label: &'a str,
    pub role: ModalRole,
}

impl<'a> ModalButton<'a> {
    pub const fn new(label: &'a str, role: ModalRole) -> Self {
        Self { label, role }
    }
}

/// What the user did with the dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModalChoice {
    /// The button at this index was pressed.
    Button(usize),
    /// Escape or the title strip's red close circle. For a confirmation that
    /// always means "do nothing", so dismissal carries no payload. A backdrop
    /// click is not an answer: see the module docs.
    Dismissed,
}

/// Appearance of the editor's dialogs.
///
/// The runtime token struct, exactly like [`crate::tree::TreeStyle`] and
/// [`crate::menu::MenuStyle`]: a component projects its serializable skin onto
/// this value at the call site. The dark defaults follow the editor's menus and
/// windows (the same Unity/Tuanjie greys), so a dialog reads as one of the
/// tool's own windows rather than a stock egui popup.
#[derive(Clone, Debug)]
pub struct ModalStyle {
    /// Dim over the rest of the editor.
    pub backdrop: Color32,
    /// The dialog body's fill.
    pub fill: Color32,
    /// The 1 px outline that separates the dialog from its own dim.
    pub stroke: Stroke,
    pub corner_radius: CornerRadius,
    pub shadow: Shadow,
    /// Fixed dialog width.
    pub width: f32,
    /// Body inset around the content; the title strip is full-bleed.
    pub padding: Margin,

    pub title: Color32,
    pub title_size: f32,
    /// The title strip's traffic lights, shared with the dock windows and the
    /// floating color picker.
    ///
    /// The red close circle is the pointer's half of the Escape rule: both
    /// report [`ModalChoice::Dismissed`], so a dialog is always answerable the
    /// same two ways. The other two lights stay visible but disabled, exactly
    /// like a close-only floating window.
    pub window_controls: WindowControlsStyle,
    /// Height of the full-bleed title strip. 0 drops the strip and draws the
    /// title inline, which is how the compact variant reads.
    pub title_height: f32,
    pub title_bar: Color32,
    /// The rule under the title strip; [`Stroke::NONE`] draws none.
    pub rule: Stroke,

    pub text: Color32,
    pub text_size: f32,
    /// Gap between the title block and the body text.
    pub body_gap: f32,
    /// Gap between the body text and the button row.
    pub footer_gap: f32,

    /// Minimum button size.
    pub button: Vec2,
    pub button_gap: f32,
    pub button_text_size: f32,
    pub button_radius: u8,
    pub button_fill: Color32,
    pub button_hover: Color32,
    pub button_text: Color32,
    pub button_stroke: Stroke,
    pub danger_fill: Color32,
    pub danger_hover: Color32,
    pub danger_text: Color32,
}

impl Default for ModalStyle {
    fn default() -> Self {
        Self::dark()
    }
}

impl ModalStyle {
    /// The editor's dark dialog: window grey, a full-bleed title strip, a 1 px
    /// outline and Unity's highlight blue for the affirmative button.
    pub fn dark() -> Self {
        Self {
            backdrop: Color32::from_black_alpha(120),
            fill: Color32::from_rgb(0x38, 0x38, 0x38),
            stroke: Stroke::new(1.0, Color32::from_rgb(0x1E, 0x1E, 0x1E)),
            corner_radius: CornerRadius::same(3),
            shadow: Shadow {
                offset: [2, 3],
                blur: 12,
                spread: 0,
                color: Color32::from_black_alpha(170),
            },
            width: 380.0,
            padding: Margin {
                left: 14,
                right: 14,
                top: 12,
                bottom: 12,
            },
            title: Color32::from_rgb(0xDE, 0xDE, 0xDE),
            title_size: 13.0,
            window_controls: WindowControlsStyle::default(),
            title_height: 22.0,
            title_bar: Color32::from_rgb(0x3C, 0x3C, 0x3C),
            rule: Stroke::new(1.0, Color32::from_rgb(0x2B, 0x2B, 0x2B)),
            text: Color32::from_rgb(0xB4, 0xB4, 0xB4),
            text_size: 12.0,
            body_gap: 10.0,
            footer_gap: 14.0,
            button: vec2(74.0, 24.0),
            button_gap: 6.0,
            button_text_size: 12.0,
            button_radius: 2,
            button_fill: Color32::from_rgb(0x4A, 0x4A, 0x4A),
            button_hover: Color32::from_rgb(0x5A, 0x5A, 0x5A),
            button_text: Color32::from_rgb(0xDE, 0xDE, 0xDE),
            button_stroke: Stroke::new(1.0, Color32::from_rgb(0x23, 0x23, 0x23)),
            // The one semantic colour: destructive only. The editor fills no
            // other button with an accent, so a dialog's safe button stays grey.
            danger_fill: Color32::from_rgb(0x8A, 0x3B, 0x33),
            danger_hover: Color32::from_rgb(0xA3, 0x46, 0x3D),
            danger_text: Color32::from_rgb(0xFF, 0xFF, 0xFF),
        }
    }

    /// The light dialog, same structure as [`Self::dark`].
    pub fn light() -> Self {
        Self {
            backdrop: Color32::from_black_alpha(90),
            fill: Color32::from_rgb(0xC8, 0xC8, 0xC8),
            stroke: Stroke::new(1.0, Color32::from_rgb(0x9B, 0x9B, 0x9B)),
            shadow: Shadow {
                offset: [2, 3],
                blur: 12,
                spread: 0,
                color: Color32::from_black_alpha(90),
            },
            title: Color32::from_rgb(0x30, 0x30, 0x30),

            title_bar: Color32::from_rgb(0xBD, 0xBD, 0xBD),
            rule: Stroke::new(1.0, Color32::from_rgb(0x9B, 0x9B, 0x9B)),
            text: Color32::from_rgb(0x3C, 0x3C, 0x3C),
            button_fill: Color32::from_rgb(0xB4, 0xB4, 0xB4),
            button_hover: Color32::from_rgb(0xC4, 0xC4, 0xC4),
            button_text: Color32::from_rgb(0x2B, 0x2B, 0x2B),
            button_stroke: Stroke::new(1.0, Color32::from_rgb(0x90, 0x90, 0x90)),
            ..Self::dark()
        }
    }

    /// The button style for one role.
    pub fn button_style(&self, role: ModalRole) -> ButtonStyle {
        let (fill, hover, text) = match role {
            ModalRole::Default => (self.button_fill, self.button_hover, self.button_text),
            ModalRole::Danger => (self.danger_fill, self.danger_hover, self.danger_text),
        };
        ButtonStyle::new(TextStyle::sized(text, self.button_text_size))
            .fills(fill, hover, hover)
            .stroke(self.button_stroke)
            .corner_radius(CornerRadius::same(self.button_radius))
            .padding(vec2(10.0, 2.0))
            .height(self.button.y.max(1.0))
    }

    fn content_width(&self) -> f32 {
        (self.width - f32::from(self.padding.left) - f32::from(self.padding.right)).max(1.0)
    }
}

/// One dialog: title, one paragraph of body text and a button row.
pub struct Modal<'a> {
    id_salt: &'a str,
    title: &'a str,
    body: &'a str,
    buttons: &'a [ModalButton<'a>],
}

impl<'a> Modal<'a> {
    /// A dialog with no buttons yet.
    pub fn new(id_salt: &'a str, title: &'a str, body: &'a str) -> Self {
        Self {
            id_salt,
            title,
            body,
            buttons: &[],
        }
    }

    /// The button row, in left-to-right display order.
    pub fn buttons(mut self, buttons: &'a [ModalButton<'a>]) -> Self {
        self.buttons = buttons;
        self
    }

    /// Draw the dialog and report what the user did this pass.
    ///
    /// Returns [`None`] while the dialog is only being shown. The caller keeps
    /// drawing it (with the same state) until it reports a choice, then applies
    /// the choice and drops its own dialog state. A click outside the dialog is
    /// swallowed by the backdrop and is not a choice; Escape is.
    ///
    /// The dialog is centred and cannot be moved; use
    /// [`Self::show_draggable`] when it should behave like a window.
    pub fn show(self, ui: &mut Ui, style: &ModalStyle) -> Option<ModalChoice> {
        self.show_with(ui, style, None)
    }

    /// Draw the dialog and let its title strip drag it away from the centre.
    ///
    /// `offset` is the caller's remembered displacement from the screen centre:
    /// the caller owns it (a dialog position is view state, and a tab that is
    /// reopened must not remember a drag it never made), this widget applies the
    /// drag and clamps it so the dialog always stays on screen. A dialog with no
    /// title strip (`title_height == 0`) has nowhere to grab and stays centred.
    pub fn show_draggable(
        self,
        ui: &mut Ui,
        style: &ModalStyle,
        offset: &mut Vec2,
    ) -> Option<ModalChoice> {
        self.show_with(ui, style, Some(offset))
    }

    fn show_with(
        self,
        ui: &mut Ui,
        style: &ModalStyle,
        mut drag_offset: Option<&mut Vec2>,
    ) -> Option<ModalChoice> {
        let ctx = ui.ctx().clone();
        let id = ui.id().with(("kairos.modal", self.id_salt));
        // The area is centred by default; a dragged dialog anchors the same
        // centre plus the remembered offset.
        let offset_now = drag_offset.as_deref().copied().unwrap_or_default();
        let area = egui::Modal::default_area(id).anchor(Align2::CENTER_CENTER, offset_now);
        let frame = Frame::NONE
            .fill(style.fill)
            .stroke(style.stroke)
            .corner_radius(style.corner_radius)
            .shadow(style.shadow)
            .inner_margin(Margin::ZERO);

        let mut choice = None;
        let mut close_clicked = false;
        let response = egui::Modal::new(id)
            .area(area)
            .backdrop_color(style.backdrop)
            .frame(frame)
            .show(&ctx, |ui| {
                ui.set_min_width(style.width);
                ui.set_max_width(style.width);
                ui.spacing_mut().item_spacing = Vec2::ZERO;

                // Full-bleed title strip; the compact variant draws the title
                // inside the body instead (title_height == 0).
                if style.title_height > 0.0 {
                    // The strip is the window's title bar: the traffic lights
                    // sit at its leading edge, the title follows them, and the
                    // rest of the strip is the drag handle.
                    let (rect, _) = ui
                        .allocate_exact_size(vec2(style.width, style.title_height), Sense::hover());
                    ui.painter().rect_filled(
                        rect,
                        CornerRadius {
                            nw: style.corner_radius.nw,
                            ne: style.corner_radius.ne,
                            sw: 0,
                            se: 0,
                        },
                        style.title_bar,
                    );
                    // The red close circle is the shared system button widget;
                    // the other two are painted grey and do not react, exactly
                    // like the close-only color picker window.
                    let controls = window_controls(
                        ui,
                        id.with("window-controls"),
                        rect,
                        &style.window_controls,
                        WindowControlsEnabled::CLOSE_ONLY,
                    );
                    close_clicked = controls.close;
                    // The title is centred in the strip (like the color
                    // picker's), not squeezed between the traffic lights and
                    // the right edge.
                    TextStyle::sized(style.title, style.title_size).paint(
                        ui,
                        rect.center(),
                        Align2::CENTER_CENTER,
                        self.title,
                    );
                    if let Some(offset) = drag_offset.as_deref_mut() {
                        let drag_rect = Rect::from_min_max(
                            pos2(controls.rect.right() + 4.0, rect.top()),
                            rect.max,
                        );
                        let drag =
                            ui.interact(drag_rect, id.with("title-drag"), Sense::click_and_drag());
                        if drag.hovered() || drag.dragged() {
                            let cursor = if drag.dragged() {
                                egui::CursorIcon::Grabbing
                            } else {
                                egui::CursorIcon::Grab
                            };
                            ui.ctx().set_cursor_icon(cursor);
                        }
                        if drag.dragged() {
                            let delta = drag.drag_delta();
                            offset.x += delta.x;
                            offset.y += delta.y;
                        }
                    }
                    if style.rule.width > 0.0 {
                        ui.painter().rect_filled(
                            Rect::from_min_size(
                                pos2(rect.left(), rect.bottom() - style.rule.width),
                                vec2(rect.width(), style.rule.width),
                            ),
                            CornerRadius::ZERO,
                            style.rule.color,
                        );
                    }
                }

                let top = if style.title_height > 0.0 {
                    10
                } else {
                    style.padding.top
                };
                Frame::NONE
                    .inner_margin(Margin {
                        left: style.padding.left,
                        right: style.padding.right,
                        top,
                        bottom: style.padding.bottom,
                    })
                    .show(ui, |ui| {
                        let width = style.content_width();
                        ui.set_min_width(width);
                        ui.set_max_width(width);

                        if style.title_height <= 0.0 {
                            TextStyle::sized(style.title, style.title_size).label(ui, self.title);
                            ui.add_space(style.body_gap);
                        }

                        let text = TextStyle::sized(style.text, style.text_size);
                        let galley = text.layout_wrapped(ui, self.body, width);
                        let (rect, _) = ui.allocate_exact_size(galley.size(), Sense::hover());
                        ui.painter().galley(rect.min, galley, style.text);
                        ui.add_space(style.footer_gap);

                        choice = self.button_row(ui, style);
                    });
            });

        if choice.is_none() && close_clicked {
            choice = Some(ModalChoice::Dismissed);
        }
        // Escape is the other dismissal; a click on the dim backdrop is ignored.
        // `ModalResponse::should_close` would also report a backdrop click, so
        // the rule is spelled out here instead of taken from egui.
        let escape = response.is_top_modal
            && ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape));
        if choice.is_none() && (escape || response.response.should_close()) {
            choice = Some(ModalChoice::Dismissed);
        }

        // A drag must not park the dialog off screen where its buttons cannot be
        // reached. The rect is this frame's, so the clamp takes effect on the
        // next pass, like the drag itself.
        if let Some(offset) = drag_offset.as_deref_mut() {
            let screen = ctx.content_rect().size();
            let dialog = response.response.rect.size();
            let limit = ((screen - dialog) * 0.5).max(Vec2::ZERO);
            offset.x = offset.x.clamp(-limit.x, limit.x);
            offset.y = offset.y.clamp(-limit.y, limit.y);
        }
        choice
    }

    /// The right-aligned button row. Enter presses the first non-danger button.
    fn button_row(&self, ui: &mut Ui, style: &ModalStyle) -> Option<ModalChoice> {
        if self.buttons.is_empty() {
            return None;
        }
        let mut choice = None;
        let width = style.content_width();
        ui.allocate_ui_with_layout(
            vec2(width, style.button.y),
            Layout::right_to_left(Align::Center),
            |ui| {
                for (index, button) in self.buttons.iter().enumerate().rev() {
                    let label = TextStyle::sized(style.button_text, style.button_text_size);
                    let text_width = label.layout(ui, button.label).size().x;
                    let button_width = style.button.x.max(text_width + 20.0);
                    let button_style = style.button_style(button.role);
                    let response = ui.add(
                        Button::new(button.label, &button_style)
                            .width(button_width)
                            .height(style.button.y)
                            .align(ButtonAlign::Center),
                    );
                    if response.clicked() {
                        choice = Some(ModalChoice::Button(index));
                    }
                    if index > 0 {
                        ui.add_space(style.button_gap);
                    }
                }
            },
        );

        if choice.is_none() && ui.input(|input| input.key_pressed(Key::Enter)) {
            choice = self
                .buttons
                .iter()
                .position(|button| button.role != ModalRole::Danger)
                .map(ModalChoice::Button);
        }
        choice
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Context, Event, FullOutput, Modifiers, PointerButton, Pos2, RawInput, Vec2};

    const CANVAS: Vec2 = vec2(640.0, 420.0);
    const BUTTONS: [ModalButton<'static>; 2] = [
        ModalButton::new("Cancel", ModalRole::Default),
        ModalButton::new("Delete", ModalRole::Danger),
    ];

    struct Harness {
        ctx: Context,
        style: ModalStyle,
        time: f64,
    }

    impl Harness {
        fn new() -> Self {
            let mut harness = Self {
                ctx: Context::default(),
                style: ModalStyle::dark(),
                // egui fades a fresh area in over `animation_time`; jumping the
                // clock keeps every frame past the fade so painted fills are the
                // settled colours (the menu tests do the same).
                time: 10.0,
            };
            // The first pass registers the dialog's widgets for hit-testing.
            let _ = harness.frame_output(Vec::new());
            harness
        }

        fn frame_output(&mut self, events: Vec<Event>) -> (FullOutput, Option<ModalChoice>) {
            self.run(events, None)
        }

        fn draggable_output(
            &mut self,
            events: Vec<Event>,
            offset: &mut Vec2,
        ) -> (FullOutput, Option<ModalChoice>) {
            self.run(events, Some(offset))
        }

        fn run(
            &mut self,
            events: Vec<Event>,
            mut drag: Option<&mut Vec2>,
        ) -> (FullOutput, Option<ModalChoice>) {
            self.time += 1.0;
            let mut input = RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, CANVAS));
            input.time = Some(self.time);
            input.events = events;
            let style = self.style.clone();
            let mut choice = None;
            let output = self.ctx.run_ui(input, |ui| {
                let dialog = Modal::new("test", "Delete", "Delete 2 items?").buttons(&BUTTONS);
                choice = match drag.as_deref_mut() {
                    Some(offset) => dialog.show_draggable(ui, &style, offset),
                    None => dialog.show(ui, &style),
                };
            });
            (output, choice)
        }

        fn draggable_painted(&mut self, offset: &mut Vec2) -> Vec<(Rect, Color32)> {
            fn walk(shape: &egui::Shape, out: &mut Vec<(Rect, Color32)>) {
                match shape {
                    egui::Shape::Rect(rect) => out.push((rect.rect, rect.fill)),
                    egui::Shape::Vec(shapes) => {
                        for shape in shapes {
                            walk(shape, out);
                        }
                    }
                    _ => {}
                }
            }
            let (output, _) = self.draggable_output(Vec::new(), offset);
            let mut out = Vec::new();
            for clipped in &output.shapes {
                walk(&clipped.shape, &mut out);
            }
            out
        }

        /// Press and release inside one frame, like the tree widget's tests.
        fn click(&mut self, pos: Pos2) -> Option<ModalChoice> {
            let events = vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                },
            ];
            self.frame_output(events).1
        }

        fn key(&mut self, key: Key) -> Option<ModalChoice> {
            let (_, choice) = self.frame_output(vec![Event::Key {
                key,
                physical_key: Some(key),
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }]);
            choice
        }

        /// One filled rect of the painted frame, for finding the buttons.
        fn painted_rects(&mut self) -> Vec<(Rect, Color32)> {
            fn walk(shape: &egui::Shape, out: &mut Vec<(Rect, Color32)>) {
                match shape {
                    egui::Shape::Rect(rect) => out.push((rect.rect, rect.fill)),
                    egui::Shape::Vec(shapes) => {
                        for shape in shapes {
                            walk(shape, out);
                        }
                    }
                    _ => {}
                }
            }
            let (output, _) = self.frame_output(Vec::new());
            let mut out = Vec::new();
            for clipped in &output.shapes {
                walk(&clipped.shape, &mut out);
            }
            out
        }
    }

    /// The dialog is shown and reports nothing until the user acts.
    #[test]
    fn a_shown_dialog_reports_no_choice() {
        let mut harness = Harness::new();
        assert_eq!(harness.frame_output(Vec::new()).1, None);
    }

    #[test]
    fn escape_dismisses_the_dialog() {
        let mut harness = Harness::new();
        assert_eq!(harness.key(Key::Escape), Some(ModalChoice::Dismissed));
    }

    /// The title strip's red close circle is the shared system button widget,
    /// and the pointer's half of the Escape rule: both dismiss the dialog.
    #[test]
    fn the_title_close_circle_dismisses_the_dialog() {
        let mut harness = Harness::new();
        let style = harness.style.clone();
        let title = harness
            .painted_rects()
            .into_iter()
            .find(|(_, fill)| *fill == style.title_bar)
            .map(|(rect, _)| rect)
            .expect("the title strip is painted");
        let controls = &style.window_controls;
        let close = pos2(
            title.left() + controls.margin + controls.radius,
            title.center().y,
        );

        // The light is painted with the shared traffic-light colour, not a
        // dialog-specific token.
        let (output, _) = harness.frame_output(Vec::new());
        assert!(painted_circle(&output, controls.close, close));
        assert_eq!(harness.click(close), Some(ModalChoice::Dismissed));
    }

    /// The title is centred in the strip (like the color picker's), not
    /// squeezed between the traffic lights and the right edge.
    #[test]
    fn the_title_is_centred_in_the_title_strip() {
        let mut harness = Harness::new();
        let style = harness.style.clone();
        let (output, _) = harness.frame_output(Vec::new());
        let title_bar = harness
            .painted_rects()
            .into_iter()
            .find(|(_, fill)| *fill == style.title_bar)
            .map(|(rect, _)| rect)
            .expect("the title strip is painted");
        let title = painted_text_rect(&output, "Delete").expect("the title is painted");
        assert!(
            (title.center().x - title_bar.center().x).abs() < 1.0,
            "the title must be centred in the strip: {title:?} vs {title_bar:?}"
        );
    }

    /// The rect of the painted text run equal to `label`.
    fn painted_text_rect(output: &FullOutput, label: &str) -> Option<Rect> {
        fn walk(shape: &egui::Shape, label: &str, out: &mut Option<Rect>) {
            match shape {
                egui::Shape::Text(text) => {
                    // The first run wins: the title is painted before the
                    // button row, and both may say "Delete".
                    if text.galley.text() == label && out.is_none() {
                        *out = Some(Rect::from_min_size(text.pos, text.galley.size()));
                    }
                }
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        walk(shape, label, out);
                    }
                }
                _ => {}
            }
        }
        let mut out = None;
        for clipped in &output.shapes {
            walk(&clipped.shape, label, &mut out);
        }
        out
    }

    /// A painted circle with `fill`, centred at `at` (the traffic-light test
    /// helper the color picker uses).
    fn painted_circle(output: &FullOutput, fill: Color32, at: Pos2) -> bool {
        fn walk(shape: &egui::Shape, fill: Color32, at: Pos2) -> bool {
            match shape {
                egui::Shape::Circle(circle) => {
                    circle.fill == fill && circle.center.distance(at) <= circle.radius
                }
                egui::Shape::Vec(shapes) => shapes.iter().any(|shape| walk(shape, fill, at)),
                _ => false,
            }
        }
        output
            .shapes
            .iter()
            .any(|clipped| walk(&clipped.shape, fill, at))
    }

    /// A confirmation is a question: clicking the dim backdrop must not answer
    /// "no". The dialog stays up and Escape still dismisses it.
    #[test]
    fn clicking_the_backdrop_does_not_dismiss_the_dialog() {
        let mut harness = Harness::new();
        assert_eq!(
            harness.click(pos2(2.0, 2.0)),
            None,
            "a backdrop click is swallowed, not answered"
        );
        assert_eq!(harness.frame_output(Vec::new()).1, None);
        assert_eq!(harness.key(Key::Escape), Some(ModalChoice::Dismissed));
    }

    /// Enter is safe: it presses the first non-danger button, never Delete.
    #[test]
    fn enter_presses_the_safe_button() {
        let mut harness = Harness::new();
        assert_eq!(harness.key(Key::Enter), Some(ModalChoice::Button(0)));
    }

    /// Each button reports its own index, and the dialog paints both roles.
    #[test]
    fn the_buttons_report_their_own_indexes() {
        let mut harness = Harness::new();
        let style = harness.style.clone();
        let painted = harness.painted_rects();
        let cancel = painted
            .iter()
            .find(|(_, fill)| *fill == style.button_fill)
            .map(|(rect, _)| *rect)
            .expect("the safe button is painted in the button fill");
        let delete = painted
            .iter()
            .find(|(_, fill)| *fill == style.danger_fill)
            .map(|(rect, _)| *rect)
            .expect("the danger button is painted in the danger fill");

        assert_eq!(harness.click(cancel.center()), Some(ModalChoice::Button(0)));
        assert_eq!(harness.click(delete.center()), Some(ModalChoice::Button(1)));
    }

    /// The dialog is a dialog, not a floating panel: the backdrop covers the
    /// whole screen and the body draws on top of it.
    #[test]
    fn the_backdrop_covers_the_screen_and_the_body_is_opaque() {
        let mut harness = Harness::new();
        let style = harness.style.clone();
        let painted = harness.painted_rects();
        let backdrop = painted
            .iter()
            .find(|(_, fill)| *fill == style.backdrop)
            .map(|(rect, _)| *rect)
            .expect("the dim layer is painted");
        assert_eq!(backdrop.size(), CANVAS);

        let body = painted
            .iter()
            .find(|(_, fill)| *fill == style.fill)
            .map(|(rect, _)| *rect)
            .expect("the dialog body is painted");
        assert!(backdrop.contains_rect(body));
        assert!(body.width() <= style.width + 2.0);
    }

    /// The title strip is a window title bar: dragging it moves the dialog, and
    /// the caller's offset is the position that survives the pass.
    #[test]
    fn dragging_the_title_strip_moves_the_dialog() {
        let mut harness = Harness::new();
        let style = harness.style.clone();
        let mut offset = Vec2::ZERO;

        let painted = harness.draggable_painted(&mut offset);
        let body = painted
            .iter()
            .find(|(_, fill)| *fill == style.fill)
            .map(|(rect, _)| *rect)
            .expect("the dialog body is painted");
        let title = painted
            .iter()
            .find(|(_, fill)| *fill == style.title_bar)
            .map(|(rect, _)| *rect)
            .expect("the title strip is painted");

        // Press the title bar, then move while the button is held: the dialog
        // follows on the next pass (the area is anchored before the drag lands).
        let start = title.center();
        let _ = harness.draggable_output(
            vec![
                Event::PointerMoved(start),
                Event::PointerButton {
                    pos: start,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
            &mut offset,
        );
        let moved = start + egui::vec2(40.0, 20.0);
        let _ = harness.draggable_output(vec![Event::PointerMoved(moved)], &mut offset);
        let _ = harness.draggable_output(
            vec![Event::PointerButton {
                pos: moved,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
            &mut offset,
        );

        let painted = harness.draggable_painted(&mut offset);
        let now = painted
            .iter()
            .find(|(_, fill)| *fill == style.fill)
            .map(|(rect, _)| *rect)
            .expect("the dialog body is painted");
        let follow = now.center() - body.center();
        assert!(
            (follow - egui::vec2(40.0, 20.0)).length() < 1.0,
            "the dialog must follow the title drag, moved {follow:?}"
        );

        // A drag past the screen edge is clamped: the dialog must stay reachable.
        let title = painted
            .iter()
            .find(|(_, fill)| *fill == style.title_bar)
            .map(|(rect, _)| *rect)
            .expect("the title strip is painted");
        let start = title.center();
        let _ = harness.draggable_output(
            vec![
                Event::PointerMoved(start),
                Event::PointerButton {
                    pos: start,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
            &mut offset,
        );
        let far = start + egui::vec2(5_000.0, 5_000.0);
        let _ = harness.draggable_output(vec![Event::PointerMoved(far)], &mut offset);
        let _ = harness.draggable_output(
            vec![Event::PointerButton {
                pos: far,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
            &mut offset,
        );
        let painted = harness.draggable_painted(&mut offset);
        let body = painted
            .iter()
            .find(|(_, fill)| *fill == style.fill)
            .map(|(rect, _)| *rect)
            .expect("the dialog body is painted");
        assert!(
            Rect::from_min_size(Pos2::ZERO, CANVAS).contains_rect(body),
            "the dragged dialog must stay on screen: {body:?}"
        );
    }

    /// The title strip is full-bleed: it spans the dialog's width so it reads as
    /// a window title bar rather than a second paragraph.
    #[test]
    fn the_title_strip_spans_the_dialog() {
        let mut harness = Harness::new();
        let style = harness.style.clone();
        let painted = harness.painted_rects();
        let strip = painted
            .iter()
            .find(|(_, fill)| *fill == style.title_bar)
            .map(|(rect, _)| *rect)
            .expect("the title strip is painted");
        let body = painted
            .iter()
            .find(|(_, fill)| *fill == style.fill)
            .map(|(rect, _)| *rect)
            .expect("the dialog body is painted");
        // The body fill spans the dialog plus its 1 px outline on each side; the
        // strip is the content width, so it reads as a title bar inside the frame.
        assert!(
            (strip.width() - style.width).abs() < 0.5,
            "strip {strip:?} vs token width {}",
            style.width
        );
        assert!(body.contains_rect(strip) || (body.width() - strip.width()).abs() < 2.5);
    }
}
