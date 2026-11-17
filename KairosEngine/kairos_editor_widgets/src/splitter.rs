//! A draggable divider between two panes (Unity's split-view handle).
//!
//! egui's `SidePanel` / `TopBottomPanel` each own a `resizable` flag, but a
//! divider *inside* a panel — Unity's module-area/timeline split, or the border
//! between a dock's two panes — has no container to hand the drag to. This
//! widget is that handle: it allocates a thin strip, paints the 1 px rule, and
//! moves the boundary while it is dragged. The caller keeps ownership of the
//! layout and of the size the boundary is expressed in, so the same widget works
//! in a measured layout ([`Splitter::show`]) and over a strip the caller has
//! already laid out ([`Splitter::show_in`]).
//!
//! The gesture is Unity's split view: drag with the primary button, and the
//! strip changes the pointer to a resize cursor. The drag is computed from the
//! press point and the size at the press ([like `MinMaxScroll`](crate::min_max_scroll)),
//! not accumulated per pass, so a skipped pass or a pointer jump cannot drift
//! the boundary. Double-clicking restores a caller-supplied default size.
//!
//! The widget deliberately is **not** a container: it does not know how to lay
//! out two child UIs, it only reports the size the leading pane should get.
//! Panels in this editor paint measured chrome rather than nesting layouts (the
//! profiler's module area and timeline both compute their own rects from the
//! space above them), so a container would have to re-learn both layouts. The
//! strip is the shared seam.

use egui::{Color32, CornerRadius, CursorIcon, Id, Rect, Response, Sense, Ui, pos2, vec2};

/// Direction of a [`Splitter`]'s rule.
///
/// The vocabulary matches [`Divider`](crate::divider::Divider): a horizontal
/// splitter is a horizontal line between panes stacked top to bottom (it is
/// dragged vertically), a vertical one separates panes laid out left to right.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SplitterOrientation {
    /// Runs left to right; separates panes stacked top to bottom.
    #[default]
    Horizontal,
    /// Runs top to bottom; separates panes laid out left to right.
    Vertical,
}

/// A draggable rule between two panes.
///
/// Callers keep the size of the pane *before* the split; [`show`](Self::show)
/// clamps it to the minimums given by [`limits`](Self::limits) and to what the
/// two panes and the strip share (`total`), and changes it while the strip is
/// dragged. The returned [`Response`] reports [`changed`](Response::changed) on
/// every pass that moved the boundary.
#[derive(Clone, Copy, Debug)]
pub struct Splitter {
    orientation: SplitterOrientation,
    line: f32,
    grab: f32,
    colors: Option<[Color32; 3]>,
    length: Option<f32>,
    min_before: f32,
    min_after: f32,
    reset_to: Option<f32>,
}

/// The drag in progress: the boundary as it was when the drag began, so every
/// pass recomputes from the press point instead of accumulating deltas.
#[derive(Clone, Copy, Debug)]
struct Drag {
    /// Pointer position along the drag axis when the drag began, in points.
    from: f32,
    /// The leading pane's size when the drag began.
    size: f32,
}

impl Splitter {
    /// A horizontal rule between panes stacked top to bottom.
    pub fn horizontal() -> Self {
        Self::new(SplitterOrientation::Horizontal)
    }

    /// A vertical rule between panes laid out left to right.
    pub fn vertical() -> Self {
        Self::new(SplitterOrientation::Vertical)
    }

    fn new(orientation: SplitterOrientation) -> Self {
        Self {
            orientation,
            // Unity's chrome rules are 1 px; the strip around it is the hit
            // area and reads as a gutter, like a dock's separator.
            line: 1.0,
            grab: 5.0,
            colors: None,
            length: None,
            min_before: 0.0,
            min_after: 0.0,
            reset_to: None,
        }
    }

    /// Thickness of the painted rule in points (default 1 px, Unity's chrome).
    pub fn line(mut self, line: f32) -> Self {
        self.line = line.max(0.0);
        self
    }

    /// Thickness of the strip the splitter allocates: the drag target. The
    /// painted rule is centred in it. Defaults to 5 pt; never less than
    /// [`line`](Self::line).
    pub fn grab(mut self, grab: f32) -> Self {
        self.grab = grab.max(0.0);
        self
    }

    /// Rule colours: idle, under the pointer, and while dragged. The defaults
    /// come from the theme's widget strokes.
    pub fn colors(mut self, idle: Color32, hovered: Color32, dragged: Color32) -> Self {
        self.colors = Some([idle, hovered, dragged]);
        self
    }

    /// Length of the rule along its own axis; defaults to the available width
    /// (horizontal) or height (vertical).
    pub fn length(mut self, length: f32) -> Self {
        self.length = Some(length.max(0.0));
        self
    }

    /// Minimum sizes of the panes before and after the split.
    ///
    /// "Before" is the pane above a horizontal splitter / left of a vertical
    /// one — the pane whose size the caller hands in.
    pub fn limits(mut self, before: f32, after: f32) -> Self {
        self.min_before = before.max(0.0);
        self.min_after = after.max(0.0);
        self
    }

    /// Size double-clicking the strip restores (clamped like a drag); e.g. the
    /// style's default pane size. Without it a double-click does nothing.
    pub fn reset_to(mut self, size: f32) -> Self {
        self.reset_to = Some(size);
        self
    }

    /// The strip thickness this splitter allocates and hit-tests.
    pub fn thickness(&self) -> f32 {
        self.grab.max(self.line)
    }

    /// Allocate the strip in the current layout and drag `size` (the pane
    /// before the split).
    ///
    /// `total` is the space the two panes and the strip share along the drag
    /// axis, measured before the pane before the split is allocated:
    /// `size + thickness + trailing pane`. The layout cursor is advanced by the
    /// strip's thickness (horizontal) or width (vertical). The strip's layout
    /// space counts against the panes' bounds, so a trailing pane stops
    /// `thickness` short of the end.
    pub fn show(self, ui: &mut Ui, id: Id, size: &mut f32, total: f32) -> Response {
        let horizontal = self.orientation == SplitterOrientation::Horizontal;
        let thickness = self.thickness();
        let length = self.length.unwrap_or(if horizontal {
            ui.available_width()
        } else {
            ui.available_height()
        });
        let strip = if horizontal {
            vec2(length, thickness)
        } else {
            vec2(thickness, length)
        };
        let (strip, _) = ui.allocate_exact_size(strip, Sense::hover());
        self.show_strip(ui, id, strip, size, total, thickness)
    }

    /// Paint and drag the splitter over a strip the caller laid out.
    ///
    /// Use this when the caller computes all its rects up front and the strip
    /// overlays the panes instead of taking layout space (the Console's
    /// stack-trace pane, a dock split). `total` is the space the two panes share
    /// along the drag axis, **excluding** the strip.
    pub fn show_in(self, ui: &mut Ui, id: Id, strip: Rect, size: &mut f32, total: f32) -> Response {
        self.show_strip(ui, id, strip, size, total, 0.0)
    }

    /// Both entry points: `reserve` is the layout space the strip takes from
    /// the panes' shared `total` (the whole strip when it is allocated).
    fn show_strip(
        self,
        ui: &mut Ui,
        id: Id,
        strip: Rect,
        size: &mut f32,
        total: f32,
        reserve: f32,
    ) -> Response {
        let horizontal = self.orientation == SplitterOrientation::Horizontal;
        let cursor = if horizontal {
            CursorIcon::ResizeVertical
        } else {
            CursorIcon::ResizeHorizontal
        };
        let mut response = ui
            .interact(strip, id, Sense::click_and_drag())
            .on_hover_and_drag_cursor(cursor);

        let before = *size;
        // The panel can shrink below the size the user last dragged to: clamp
        // here so every caller sees a size both panes can live with.
        *size = self.clamped(*size, total, reserve);

        let drag_id = id.with("drag");
        let mut drag: Option<Drag> =
            ui.data_mut(|data| data.get_temp::<Option<Drag>>(drag_id).flatten());
        let primary_down = ui.input(|i| i.pointer.primary_down());
        if response.is_pointer_button_down_on() && primary_down {
            if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                let at = if horizontal { pos.y } else { pos.x };
                if drag.is_none() {
                    drag = Some(Drag {
                        from: at,
                        size: *size,
                    });
                }
                if let Some(active) = drag {
                    let desired = active.size + (at - active.from);
                    let next = self.clamped(desired, total, reserve);
                    // Past a bound the pointer would have to replay its overshoot
                    // before the pane moved again; re-anchor at the bound so
                    // dragging back responds immediately.
                    if next != desired {
                        drag = Some(Drag {
                            from: at,
                            size: next,
                        });
                    }
                    *size = next;
                }
            }
        } else if !response.is_pointer_button_down_on() {
            drag = None;
        }

        if let Some(restore) = self.reset_to
            && response.double_clicked()
        {
            *size = self.clamped(restore, total, reserve);
        }

        if drag.is_some() {
            ui.data_mut(|data| data.insert_temp(drag_id, drag));
        } else {
            ui.data_mut(|data| data.remove_temp::<Option<Drag>>(drag_id));
        }

        if *size != before {
            response.mark_changed();
        }
        self.paint(ui, strip, &response, drag.is_some());
        response
    }

    /// Clamp a leading-pane size so both panes keep their minimums. When
    /// `total` is too small for both minimums the leading pane keeps its
    /// minimum and the trailing pane gets the rest (Unity's
    /// `TwoPaneSplitView.UpdateLayout` does the same).
    fn clamped(&self, size: f32, total: f32, reserve: f32) -> f32 {
        let max_before = (total - reserve - self.min_after).max(self.min_before);
        size.clamp(self.min_before, max_before)
    }

    fn paint(&self, ui: &Ui, strip: Rect, response: &Response, dragging: bool) {
        if !ui.is_rect_visible(strip) {
            return;
        }
        let [idle, hovered, dragged] = self.colors.unwrap_or_else(|| {
            let widgets = &ui.visuals().widgets;
            [
                widgets.noninteractive.bg_stroke.color,
                widgets.hovered.bg_stroke.color,
                widgets.active.bg_stroke.color,
            ]
        });
        let color = if dragging {
            dragged
        } else if response.hovered() {
            hovered
        } else {
            idle
        };
        let line = self.line.min(self.thickness());
        if line <= 0.0 {
            return;
        }
        let half = line * 0.5;
        let rect = match self.orientation {
            SplitterOrientation::Horizontal => Rect::from_min_max(
                pos2(strip.left(), strip.center().y - half),
                pos2(strip.right(), strip.center().y + half),
            ),
            SplitterOrientation::Vertical => Rect::from_min_max(
                pos2(strip.center().x - half, strip.top()),
                pos2(strip.center().x + half, strip.bottom()),
            ),
        };
        ui.painter().rect_filled(rect, CornerRadius::ZERO, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Context, Event, FullOutput, Modifiers, PointerButton, Pos2, RawInput};

    const IDLE: Color32 = Color32::from_rgb(0x25, 0x25, 0x25);
    const HOVER: Color32 = Color32::from_rgb(0x70, 0x70, 0x70);
    const DRAGGED: Color32 = Color32::from_rgb(0xC4, 0xC4, 0xC4);

    fn input(events: Vec<Event>) -> RawInput {
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0))),
            events,
            ..Default::default()
        }
    }

    /// The horizontal splitter under test: a 5 pt strip at the top of the UI.
    fn splitter() -> Splitter {
        Splitter::horizontal()
            .colors(IDLE, HOVER, DRAGGED)
            .limits(50.0, 80.0)
            .reset_to(100.0)
    }

    /// One pass; returns the output and whether the boundary moved.
    fn draw(ctx: &Context, size: &mut f32, total: f32, events: Vec<Event>) -> (FullOutput, bool) {
        let mut changed = false;
        let output = ctx.run_ui(input(events), |ui| {
            changed = splitter().show(ui, Id::new("split"), size, total).changed();
        });
        (output, changed)
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

    fn click(pos: Pos2) -> Vec<Event> {
        vec![
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
        ]
    }

    /// The strip's centre, where the line is painted.
    fn strip_centre() -> Pos2 {
        Pos2::new(200.0, 2.5)
    }

    /// The first painted filled rect (the splitter's rule is the only one).
    fn rule(output: &FullOutput) -> (Rect, Color32) {
        output
            .shapes
            .iter()
            .find_map(|clipped| match &clipped.shape {
                egui::Shape::Rect(shape) => Some((shape.rect, shape.fill)),
                _ => None,
            })
            .expect("the splitter paints its rule")
    }

    /// A pass that lays the strip out before an input pass can hit it.
    fn idle_pass(ctx: &Context, size: &mut f32, total: f32) {
        let _ = draw(ctx, size, total, Vec::new());
    }

    #[test]
    fn dragging_grows_the_pane_before_the_split() {
        let ctx = Context::default();
        let mut size = 100.0;
        idle_pass(&ctx, &mut size, 300.0);

        // The press alone does not move the boundary.
        let (_, changed) = draw(&ctx, &mut size, 300.0, press(strip_centre()));
        assert!(!changed);
        assert_eq!(size, 100.0);

        let (_, changed) = draw(&ctx, &mut size, 300.0, move_to(Pos2::new(200.0, 42.5)));
        assert!(changed, "the drag moves the boundary");
        assert_eq!(size, 140.0);
        let _ = draw(&ctx, &mut size, 300.0, release(Pos2::new(200.0, 42.5)));
    }

    #[test]
    fn the_panes_keep_their_minimum_sizes() {
        let ctx = Context::default();
        let mut size = 100.0;
        idle_pass(&ctx, &mut size, 300.0);
        let _ = draw(&ctx, &mut size, 300.0, press(strip_centre()));

        // Far up: the leading pane stops at its minimum.
        let _ = draw(&ctx, &mut size, 300.0, move_to(Pos2::new(200.0, -400.0)));
        assert_eq!(size, 50.0);
        // Far down: the trailing pane stops at its minimum, so the leading pane
        // stops at total - thickness - minimum.
        let _ = draw(&ctx, &mut size, 300.0, move_to(Pos2::new(200.0, 600.0)));
        assert_eq!(size, 300.0 - 5.0 - 80.0);
        let _ = draw(&ctx, &mut size, 300.0, release(Pos2::new(200.0, 600.0)));
    }

    #[test]
    fn dragging_back_from_a_clamp_moves_immediately() {
        let ctx = Context::default();
        let mut size = 100.0;
        idle_pass(&ctx, &mut size, 300.0);
        let _ = draw(&ctx, &mut size, 300.0, press(strip_centre()));

        // Far past the maximum: the leading pane stops at total - 5 - 80.
        let _ = draw(&ctx, &mut size, 300.0, move_to(Pos2::new(200.0, 600.0)));
        assert_eq!(size, 215.0);

        // Back up 10 pt: the pane follows at once instead of replaying the
        // overshoot first.
        let (_, changed) = draw(&ctx, &mut size, 300.0, move_to(Pos2::new(200.0, 590.0)));
        assert!(changed);
        assert_eq!(size, 205.0);
        let _ = draw(&ctx, &mut size, 300.0, release(Pos2::new(200.0, 590.0)));
    }

    /// The overlay form: a strip the caller laid out over the panes. Its
    /// thickness is not deducted from the panes' total, and the rule is painted
    /// where the panes meet (the band's centre), not at the strip's edge.
    #[test]
    fn show_in_overlays_the_panes_without_reserving_space() {
        let ctx = Context::default();
        let strip = Rect::from_min_size(Pos2::new(0.0, 100.0), vec2(400.0, 6.0));
        let mut size = 100.0;
        let draw = |size: &mut f32, events: Vec<Event>| {
            let mut changed = false;
            let output = ctx.run_ui(input(events), |ui| {
                changed = Splitter::horizontal()
                    .colors(IDLE, HOVER, DRAGGED)
                    .limits(20.0, 20.0)
                    .show_in(ui, Id::new("overlay"), strip, size, 200.0)
                    .changed();
            });
            (output, changed)
        };

        let (output, _) = draw(&mut size, Vec::new());
        let (rule_rect, fill) = rule(&output);
        assert_eq!(fill, IDLE);
        assert!(
            (rule_rect.center().y - 103.0).abs() < 1e-6,
            "the line is at the boundary: {rule_rect:?}"
        );

        let _ = draw(&mut size, press(Pos2::new(200.0, 103.0)));
        let (_, changed) = draw(&mut size, move_to(Pos2::new(200.0, 123.0)));
        assert!(changed);
        assert_eq!(size, 120.0);
        // The panes share 200 pt; the 6 pt band does not take any of it, so the
        // maximum leading size is 200 - 20.
        let _ = draw(&mut size, move_to(Pos2::new(200.0, 500.0)));
        assert_eq!(size, 180.0);
        let _ = draw(&mut size, release(Pos2::new(200.0, 500.0)));
    }

    #[test]
    fn the_rule_is_centred_in_the_strip() {
        let ctx = Context::default();
        let mut size = 100.0;
        let (output, _) = draw(&ctx, &mut size, 300.0, Vec::new());

        let (rect, fill) = rule(&output);
        assert_eq!(fill, IDLE);
        assert_eq!(rect.left(), 0.0);
        assert_eq!(rect.right(), 400.0);
        assert!(
            (rect.center().y - 2.5).abs() < 1e-6,
            "line in the strip: {rect:?}"
        );
        assert!((rect.height() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_rule_highlights_under_the_pointer_and_while_dragged() {
        let ctx = Context::default();
        let mut size = 100.0;
        idle_pass(&ctx, &mut size, 300.0);

        let (hovered, changed) = draw(&ctx, &mut size, 300.0, move_to(strip_centre()));
        assert!(!changed, "hovering does not move the boundary");
        assert_eq!(rule(&hovered).1, HOVER);
        assert_eq!(
            hovered.platform_output.cursor_icon,
            CursorIcon::ResizeVertical
        );

        let (dragged, _) = draw(&ctx, &mut size, 300.0, press(strip_centre()));
        assert_eq!(rule(&dragged).1, DRAGGED);
        let _ = draw(&ctx, &mut size, 300.0, release(strip_centre()));

        // The pointer position survives a pass without events, so move it off
        // the strip to see the idle rule again.
        let (away, _) = draw(&ctx, &mut size, 300.0, move_to(Pos2::new(200.0, 200.0)));
        assert_eq!(rule(&away).1, IDLE, "the highlight follows the pointer");
    }

    #[test]
    fn double_clicking_restores_the_default_size() {
        let ctx = Context::default();
        let mut size = 180.0;
        idle_pass(&ctx, &mut size, 300.0);
        let _ = draw(&ctx, &mut size, 300.0, click(strip_centre()));
        assert_eq!(size, 180.0, "one click leaves the boundary alone");

        let (_, _) = draw(&ctx, &mut size, 300.0, click(strip_centre()));
        assert_eq!(size, 100.0, "the double-click restores the default");
    }

    #[test]
    fn a_shrunk_panel_clamps_the_stored_size() {
        let ctx = Context::default();
        let mut size = 280.0;
        // 300 pt total cannot hold a 280 pt leading pane and an 80 pt trailing
        // one: the trailing minimum wins.
        let (_, changed) = draw(&ctx, &mut size, 300.0, Vec::new());
        assert!(changed, "the clamp is reported");
        assert_eq!(size, 300.0 - 5.0 - 80.0);
    }

    #[test]
    fn too_small_for_both_minimums_keeps_the_leading_minimum() {
        let ctx = Context::default();
        let mut size = 100.0;
        let _ = draw(&ctx, &mut size, 60.0, Vec::new());
        assert_eq!(size, 50.0);
    }

    #[test]
    fn a_vertical_splitter_drags_along_the_x_axis() {
        let ctx = Context::default();
        let mut size = 100.0;
        let strip = Rect::from_min_size(Pos2::ZERO, vec2(5.0, 300.0));
        let draw = |size: &mut f32, events: Vec<Event>| {
            let mut changed = false;
            let output = ctx.run_ui(input(events), |ui| {
                changed = Splitter::vertical()
                    .colors(IDLE, HOVER, DRAGGED)
                    .limits(50.0, 80.0)
                    .show_in(ui, Id::new("vsplit"), strip, size, 300.0)
                    .changed();
            });
            (output, changed)
        };

        let (_, _) = draw(&mut size, Vec::new());
        let (hovered, _) = draw(&mut size, move_to(Pos2::new(2.5, 150.0)));
        assert_eq!(
            hovered.platform_output.cursor_icon,
            CursorIcon::ResizeHorizontal
        );
        let _ = draw(&mut size, press(Pos2::new(2.5, 150.0)));
        let (_, changed) = draw(&mut size, move_to(Pos2::new(42.5, 150.0)));
        assert!(changed);
        assert_eq!(size, 140.0);
        let _ = draw(&mut size, release(Pos2::new(42.5, 150.0)));
    }
}
