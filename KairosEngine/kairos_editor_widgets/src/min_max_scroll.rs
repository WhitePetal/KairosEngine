//! Min-max scroll bars: Unity's `EditorGUIExt.MinMaxScroller` on egui.
//!
//! A [`MinMaxScroll`] bar shows a window `value .. value + span` of a `bounds`
//! range. The thumb is that window: dragging it pans, and clicking the trough
//! snaps the thumb under the pointer (Unity's min-max slider behaviour). When the
//! bar is [resizable](MinMaxScroll::resizable) a grip at each end of the thumb
//! resizes the window — that is the gesture Unity's Profiler uses to zoom its
//! timeline.
//!
//! egui's own scroll bar is a plain thumb owned by a [`egui::ScrollArea`] and
//! cannot grow a second mode, so a zoomable view draws this widget instead: the
//! timeline uses the vertical form for its rows and the resizable horizontal form
//! for its time axis. Unity's implementation is `EditorGUIExt.MinMaxScroller`
//! and `DoMinMaxSlider` in `Editor/Mono/Animation/EditorGUIExt.cs`.

use egui::{Color32, CornerRadius, CursorIcon, Id, Pos2, Rect, Response, Sense, Ui, pos2, vec2};

/// Which axis a scroll bar runs along.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollOrientation {
    /// Runs left to right; the window moves through the bounds horizontally.
    #[default]
    Horizontal,
    /// Runs top to bottom.
    Vertical,
}

/// Appearance of a [`MinMaxScroll`] bar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MinMaxScrollStyle {
    /// The trough the thumb moves in.
    pub trough: Color32,
    pub thumb: Color32,
    /// Thumb fill while the pointer is over it.
    pub thumb_hover: Color32,
    /// Thumb fill while it is being dragged.
    pub thumb_active: Color32,
    /// The grip lines on a resizable thumb's two ends.
    pub grip: Color32,
    /// Width of each end grip: the drag target for resizing.
    pub handle_width: f32,
}

/// The window a scroll bar shows: `value .. value + span` inside `bounds`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollWindow {
    /// Start of the window, in the bounds' units.
    pub value: f64,
    /// Length of the window, in the bounds' units.
    pub span: f64,
    /// `(min, max)` of the range the window moves through.
    pub bounds: (f64, f64),
    /// Narrowest span a resize may produce; 0 for "no limit".
    pub min_span: f64,
}

impl ScrollWindow {
    /// A window with no resize limit.
    pub fn new(value: f64, span: f64, bounds: (f64, f64)) -> Self {
        Self {
            value,
            span,
            bounds,
            min_span: 0.0,
        }
    }

    /// The narrowest span a resize may produce (a zoom limit).
    pub fn with_min_span(mut self, min_span: f64) -> Self {
        self.min_span = min_span;
        self
    }

    fn bounds_span(&self) -> f64 {
        (self.bounds.1 - self.bounds.0).max(0.0)
    }

    /// Keep the window inside its bounds and no narrower than `min_span`.
    fn clamp(&mut self) {
        let bounds_span = self.bounds_span();
        if bounds_span <= 0.0 {
            self.value = self.bounds.0;
            self.span = 0.0;
            return;
        }
        let min_span = self.min_span.clamp(0.0, bounds_span);
        self.span = self.span.clamp(min_span, bounds_span);
        self.value = self.value.clamp(
            self.bounds.0,
            (self.bounds.1 - self.span).max(self.bounds.0),
        );
    }
}

/// One Unity-style min-max scroll bar.
///
/// The caller owns the [`ScrollWindow`]; the widget owns nothing between passes
/// but the drag it is in the middle of (kept in egui's temporary data, keyed by
/// the bar's id). Besides moving the window it reports
/// [`Response::changed`] on the pass that moved it.
#[derive(Clone, Copy, Debug)]
pub struct MinMaxScroll<'a> {
    style: &'a MinMaxScrollStyle,
    orientation: ScrollOrientation,
    resizable: bool,
}

/// Which part of the bar a drag grabbed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Grab {
    Thumb,
    Min,
    Max,
}

/// The drag in progress: the window as it was when the drag began, so every
/// pass recomputes from the press point instead of accumulating deltas (which
/// drifts when a frame is skipped or the pointer jumps).
#[derive(Clone, Copy, Debug)]
struct Drag {
    grab: Grab,
    /// Pointer position along the bar when the drag began, in points.
    from: f32,
    /// The window when the drag began.
    window: ScrollWindow,
}

impl<'a> MinMaxScroll<'a> {
    /// A bar with a non-resizable thumb (a plain scrollbar).
    pub fn horizontal(style: &'a MinMaxScrollStyle) -> Self {
        Self {
            style,
            orientation: ScrollOrientation::Horizontal,
            resizable: false,
        }
    }

    /// A vertical bar with a non-resizable thumb (a plain scrollbar).
    pub fn vertical(style: &'a MinMaxScrollStyle) -> Self {
        Self {
            style,
            orientation: ScrollOrientation::Vertical,
            resizable: false,
        }
    }

    /// Give the thumb a resize grip at each end.
    pub fn resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    /// Draw the bar into `rect`, moving `window` with the pointer.
    ///
    /// `rect` is laid out by the caller — the bar is chrome inside a measured
    /// layout, not a widget that allocates space of its own.
    pub fn show(self, ui: &mut Ui, id: Id, rect: Rect, window: &mut ScrollWindow) -> Response {
        window.clamp();
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            return ui.interact(rect, id, Sense::hover());
        }

        let horizontal = self.orientation == ScrollOrientation::Horizontal;
        let length = if horizontal {
            rect.width()
        } else {
            rect.height()
        };
        let bounds_span = window.bounds_span();
        let handle = if self.resizable {
            self.style.handle_width.clamp(0.0, length / 3.0)
        } else {
            0.0
        };
        // The grips live inside the thumb, so the movable range excludes them:
        // at full span the thumb covers the bar, at minimum span the grips stay
        // grabbable.
        let range = (length - 2.0 * handle).max(1.0);
        let points_per_unit = if bounds_span > 0.0 {
            f64::from(range) / bounds_span
        } else {
            0.0
        };
        let start = ((window.value - window.bounds.0) * points_per_unit) as f32;
        let thumb_length = ((window.span * points_per_unit) as f32 + 2.0 * handle).min(length);
        let thumb = strip(rect, horizontal, start, thumb_length);
        let min_grip = strip(rect, horizontal, start, handle);
        let max_grip = strip(rect, horizontal, start + thumb_length - handle, handle);

        let mut response = ui.interact(rect, id, Sense::drag());
        let drag_id = id.with("drag");
        let mut drag: Option<Drag> =
            ui.data_mut(|data| data.get_temp::<Option<Drag>>(drag_id).flatten());
        let before = *window;

        // Only the primary button drags the bar (Unity's scrollers ignore the
        // others), but a press on another button must not leave a stale grab.
        let primary_down = ui.input(|i| i.pointer.primary_down());
        if response.is_pointer_button_down_on() && primary_down && points_per_unit > 0.0 {
            if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                let at = along(pos, rect, horizontal);
                if drag.is_none() {
                    // Classify the grab at the press point. A grip wins over the
                    // thumb it sits on; a press in the trough snaps the thumb
                    // under the pointer and keeps dragging it.
                    let mut grabbed = *window;
                    let grab = if self.resizable && min_grip.contains(pos) {
                        Grab::Min
                    } else if self.resizable && max_grip.contains(pos) {
                        Grab::Max
                    } else if thumb.contains(pos) {
                        Grab::Thumb
                    } else {
                        grabbed.value = window.bounds.0
                            + (f64::from(at - thumb_length * 0.5) / points_per_unit);
                        grabbed.clamp();
                        Grab::Thumb
                    };
                    drag = Some(Drag {
                        grab,
                        from: at,
                        window: grabbed,
                    });
                    *window = grabbed;
                }

                if let Some(drag) = drag {
                    let delta = f64::from(at - drag.from) / points_per_unit;
                    *window = moved(drag, delta, bounds_span);
                }
            }
        } else if !response.is_pointer_button_down_on() {
            drag = None;
        }

        if drag.is_some() {
            ui.data_mut(|data| data.insert_temp(drag_id, drag));
        } else {
            ui.data_mut(|data| data.remove_temp::<Option<Drag>>(drag_id));
        }

        if *window != before {
            response.mark_changed();
        }
        self.paint(
            ui,
            rect,
            &response,
            thumb,
            min_grip,
            max_grip,
            drag.is_some(),
        );
        response
    }

    fn paint(
        &self,
        ui: &Ui,
        rect: Rect,
        response: &Response,
        thumb: Rect,
        min_grip: Rect,
        max_grip: Rect,
        dragging: bool,
    ) {
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::ZERO, self.style.trough);

        let horizontal = self.orientation == ScrollOrientation::Horizontal;
        let pointer = ui.input(|i| i.pointer.hover_pos());
        let over_thumb = pointer.is_some_and(|pos| thumb.contains(pos));
        let fill = if dragging {
            self.style.thumb_active
        } else if over_thumb || response.hovered() {
            self.style.thumb_hover
        } else {
            self.style.thumb
        };
        painter.rect_filled(thumb, CornerRadius::same(2), fill);

        if self.resizable {
            // Unity's min-max thumb carries a grip at each end: the resize
            // target, drawn as two thin lines.
            let inset = 3.0;
            for grip in [min_grip, max_grip] {
                if horizontal {
                    let line = Rect::from_min_size(
                        pos2(grip.center().x - 0.5, thumb.top() + inset),
                        vec2(1.0, (thumb.height() - 2.0 * inset).max(1.0)),
                    );
                    painter.rect_filled(line, CornerRadius::ZERO, self.style.grip);
                    painter.rect_filled(
                        line.translate(vec2(2.0, 0.0)),
                        CornerRadius::ZERO,
                        self.style.grip,
                    );
                } else {
                    let line = Rect::from_min_size(
                        pos2(thumb.left() + inset, grip.center().y - 0.5),
                        vec2((thumb.width() - 2.0 * inset).max(1.0), 1.0),
                    );
                    painter.rect_filled(line, CornerRadius::ZERO, self.style.grip);
                    painter.rect_filled(
                        line.translate(vec2(0.0, 2.0)),
                        CornerRadius::ZERO,
                        self.style.grip,
                    );
                }
            }
        }

        if response.hovered() || dragging {
            let over_grip = self.resizable
                && pointer.is_some_and(|pos| min_grip.contains(pos) || max_grip.contains(pos));
            let icon = if over_grip {
                if horizontal {
                    CursorIcon::ResizeHorizontal
                } else {
                    CursorIcon::ResizeVertical
                }
            } else if dragging {
                CursorIcon::Grabbing
            } else {
                CursorIcon::Grab
            };
            ui.ctx().set_cursor_icon(icon);
        }
    }
}

/// Apply one drag step to the window the drag started from.
fn moved(drag: Drag, delta: f64, bounds_span: f64) -> ScrollWindow {
    let bounds = drag.window.bounds;
    let min_span = drag.window.min_span.clamp(0.0, bounds_span);
    let mut next = drag.window;
    match drag.grab {
        Grab::Thumb => next.value += delta,
        Grab::Min => {
            // The maximum edge stays put while the minimum edge follows the
            // pointer; hitting either limit pushes the other edge instead.
            let mut value = drag.window.value + delta;
            let mut span = drag.window.span - delta;
            if value < bounds.0 {
                span -= bounds.0 - value;
                value = bounds.0;
            }
            if span < min_span {
                value -= min_span - span;
                span = min_span;
            }
            next.value = value;
            next.span = span;
        }
        Grab::Max => {
            let mut span = drag.window.span + delta;
            if span < min_span {
                span = min_span;
            }
            if drag.window.value + span > bounds.1 {
                span = (bounds.1 - drag.window.value).max(min_span);
            }
            next.span = span;
        }
    }
    next.clamp();
    next
}

/// The sub-strip of `rect` starting `start` points along it.
fn strip(rect: Rect, horizontal: bool, start: f32, length: f32) -> Rect {
    if horizontal {
        Rect::from_min_size(
            pos2(rect.left() + start, rect.top()),
            vec2(length, rect.height()),
        )
    } else {
        Rect::from_min_size(
            pos2(rect.left(), rect.top() + start),
            vec2(rect.width(), length),
        )
    }
}

/// Where `pos` sits along the bar, in points from its start.
fn along(pos: Pos2, rect: Rect, horizontal: bool) -> f32 {
    if horizontal {
        pos.x - rect.left()
    } else {
        pos.y - rect.top()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Context, Event, Modifiers, PointerButton, Pos2, RawInput};

    /// A 200 × 13 bar at (20, 20); with a 6 pt grip the movable range is 188 pt.
    fn bar() -> Rect {
        Rect::from_min_size(pos2(20.0, 20.0), vec2(200.0, 13.0))
    }

    const STYLE: MinMaxScrollStyle = MinMaxScrollStyle {
        trough: Color32::from_rgb(0x10, 0x10, 0x10),
        thumb: Color32::from_rgb(0x40, 0x40, 0x40),
        thumb_hover: Color32::from_rgb(0x50, 0x50, 0x50),
        thumb_active: Color32::from_rgb(0x60, 0x60, 0x60),
        grip: Color32::from_rgb(0xC0, 0xC0, 0xC0),
        handle_width: 6.0,
    };

    fn input(events: Vec<Event>) -> RawInput {
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 120.0))),
            events,
            ..Default::default()
        }
    }

    /// One pass of the bar; returns whether it reported a changed window.
    fn draw(ctx: &Context, window: &mut ScrollWindow, resizable: bool, events: Vec<Event>) -> bool {
        let mut changed = false;
        let _ = ctx.run_ui(input(events), |ui| {
            changed = MinMaxScroll::horizontal(&STYLE)
                .resizable(resizable)
                .show(ui, Id::new("bar"), bar(), window)
                .changed();
        });
        changed
    }

    fn press(pos: Pos2) -> Vec<Event> {
        vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ]
    }

    fn move_to(pos: Pos2) -> Vec<Event> {
        vec![Event::PointerMoved(pos)]
    }

    fn release(pos: Pos2) -> Vec<Event> {
        vec![Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }]
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-6,
            "expected {expected}, got {actual}"
        );
    }

    /// A window showing the middle half of the range: thumb starts 47 pt in and
    /// is 106 pt long, i.e. x = 67..173 on screen. Draws one quiet pass first,
    /// because egui hit-tests against the previous pass's widget rects.
    fn middle_window(ctx: &Context, resizable: bool) -> ScrollWindow {
        let mut window = ScrollWindow::new(0.25, 0.5, (0.0, 1.0)).with_min_span(0.05);
        draw(ctx, &mut window, resizable, Vec::new());
        window
    }

    #[test]
    fn dragging_the_thumb_pans_the_window() {
        let ctx = Context::default();
        let mut window = middle_window(&ctx, true);

        draw(&ctx, &mut window, true, press(Pos2::new(120.0, 26.5)));
        assert_close(window.value, 0.25);
        let changed = draw(&ctx, &mut window, true, move_to(Pos2::new(140.0, 26.5)));
        assert!(changed, "the drag moves the window");
        assert_close(window.value, 0.25 + 20.0 / 188.0);
        assert_close(window.span, 0.5);
        draw(&ctx, &mut window, true, release(Pos2::new(140.0, 26.5)));
    }

    #[test]
    fn dragging_the_right_grip_resizes_from_that_end() {
        let ctx = Context::default();
        let mut window = middle_window(&ctx, true);

        // The right grip is the last 6 pt of the thumb: x = 167..173.
        draw(&ctx, &mut window, true, press(Pos2::new(170.0, 26.5)));
        draw(&ctx, &mut window, true, move_to(Pos2::new(120.0, 26.5)));

        assert_close(window.value, 0.25);
        assert_close(window.span, 0.5 - 50.0 / 188.0);
        draw(&ctx, &mut window, true, release(Pos2::new(120.0, 26.5)));
    }

    #[test]
    fn dragging_the_left_grip_resizes_from_that_end() {
        let ctx = Context::default();
        let mut window = middle_window(&ctx, true);

        // The left grip is the first 6 pt of the thumb.
        draw(&ctx, &mut window, true, press(Pos2::new(70.0, 26.5)));
        draw(&ctx, &mut window, true, move_to(Pos2::new(90.0, 26.5)));

        assert_close(window.value, 0.25 + 20.0 / 188.0);
        assert_close(window.span, 0.5 - 20.0 / 188.0);
        // The window's far edge did not move.
        assert_close(window.value + window.span, 0.75);
        draw(&ctx, &mut window, true, release(Pos2::new(90.0, 26.5)));
    }

    #[test]
    fn clicking_the_trough_snaps_the_thumb_under_the_pointer() {
        let ctx = Context::default();
        let mut window = middle_window(&ctx, true);

        // Far left of the trough: the window clamps to its start.
        let changed = draw(&ctx, &mut window, true, press(Pos2::new(30.0, 26.5)));
        assert!(changed);
        assert_close(window.value, 0.0);
        assert_close(window.span, 0.5);
    }

    #[test]
    fn the_window_stays_inside_its_bounds() {
        let ctx = Context::default();
        let mut window = middle_window(&ctx, true);

        draw(&ctx, &mut window, true, press(Pos2::new(120.0, 26.5)));
        draw(&ctx, &mut window, true, move_to(Pos2::new(400.0, 26.5)));
        assert_close(window.value, 0.5);
        draw(&ctx, &mut window, true, move_to(Pos2::new(-400.0, 26.5)));
        assert_close(window.value, 0.0);
        draw(&ctx, &mut window, true, release(Pos2::new(0.0, 26.5)));
    }

    #[test]
    fn the_zoom_limit_caps_the_resize() {
        let ctx = Context::default();
        let mut window = middle_window(&ctx, true);

        // Pull the left grip far right: the window stops at the minimum span.
        draw(&ctx, &mut window, true, press(Pos2::new(70.0, 26.5)));
        draw(&ctx, &mut window, true, move_to(Pos2::new(400.0, 26.5)));
        assert_close(window.span, 0.05);
        draw(&ctx, &mut window, true, release(Pos2::new(400.0, 26.5)));
    }

    #[test]
    fn a_plain_scrollbar_never_resizes_the_window() {
        let ctx = Context::default();
        let mut window = middle_window(&ctx, false);

        // Its thumb spans the window without the grip padding, so with value
        // 0.25 and span 0.5 it starts at x = 70 and ends at x = 170.
        draw(&ctx, &mut window, false, press(Pos2::new(165.0, 26.5)));
        draw(&ctx, &mut window, false, move_to(Pos2::new(125.0, 26.5)));

        assert_close(window.span, 0.5);
        assert_close(window.value, 0.25 - 40.0 / 200.0);
        draw(&ctx, &mut window, false, release(Pos2::new(125.0, 26.5)));
    }

    #[test]
    fn a_window_beyond_its_bounds_is_clamped_to_them() {
        let ctx = Context::default();
        let mut window = ScrollWindow::new(0.9, 0.5, (0.0, 1.0));
        draw(&ctx, &mut window, true, Vec::new());
        assert_close(window.value, 0.5);
        assert_close(window.span, 0.5);

        let mut full = ScrollWindow::new(0.2, 2.0, (0.0, 1.0));
        draw(&ctx, &mut full, true, Vec::new());
        assert_close(full.value, 0.0);
        assert_close(full.span, 1.0);
    }

    /// The vertical form maps the pointer's y and fills the bar's width.
    #[test]
    fn the_vertical_bar_uses_the_y_axis() {
        let ctx = Context::default();
        let mut window = ScrollWindow::new(0.0, 0.25, (0.0, 1.0)).with_min_span(0.25);
        let bar = Rect::from_min_size(pos2(10.0, 10.0), vec2(13.0, 200.0));
        let mut changed = false;
        let _ = ctx.run_ui(input(Vec::new()), |ui| {
            let _ = MinMaxScroll::vertical(&STYLE).show(ui, Id::new("vbar"), bar, &mut window);
        });
        // 25 % of the bar is the thumb at the top: y = 10..60.
        let _ = ctx.run_ui(
            input(vec![
                Event::PointerMoved(pos2(16.5, 30.0)),
                Event::PointerButton {
                    pos: pos2(16.5, 30.0),
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ]),
            |ui| {
                changed = MinMaxScroll::vertical(&STYLE)
                    .show(ui, Id::new("vbar"), bar, &mut window)
                    .changed();
            },
        );
        assert!(!changed, "the press alone does not move the window");
        let _ = ctx.run_ui(input(vec![Event::PointerMoved(pos2(16.5, 110.0))]), |ui| {
            let _ = MinMaxScroll::vertical(&STYLE).show(ui, Id::new("vbar"), bar, &mut window);
        });
        // 80 pt down on a 200 pt range is 40 % of the range.
        assert_close(window.value, 0.4);
    }
}
