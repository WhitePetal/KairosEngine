//! The three window buttons at the leading edge of a title bar.
//!
//! They are painted by egui, not supplied by the operating system: the editor
//! draws its own window frames on every platform, so the dock and the color
//! picker share one custom control instead of drifting apart. A Windows-native
//! caption bar would be a separate skin (or a real OS window), not a different
//! branch hidden in every widget.

use egui::{Color32, CursorIcon, Id, Rect, Sense, Stroke, Ui, Vec2, pos2};

/// One of the three macOS-style traffic lights, in visual order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WindowControl {
    Close,
    Minimize,
    Maximize,
}

impl WindowControl {
    /// Left-to-right order: close, minimize, maximize.
    pub const ALL: [Self; 3] = [Self::Close, Self::Minimize, Self::Maximize];

    fn color(self, style: &WindowControlsStyle) -> Color32 {
        match self {
            Self::Close => style.close,
            Self::Minimize => style.minimize,
            Self::Maximize => style.maximize,
        }
    }
}

/// Which traffic lights accept input. Disabled lights are still painted, in
/// grey, because a window without a minimize/zoom action keeps the buttons in
/// the Unity/macOS chrome rather than hiding two of the three.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowControlsEnabled {
    pub close: bool,
    pub minimize: bool,
    pub maximize: bool,
}

impl WindowControlsEnabled {
    pub const ALL: Self = Self {
        close: true,
        minimize: true,
        maximize: true,
    };

    /// A floating popup that can only be closed.
    pub const CLOSE_ONLY: Self = Self {
        close: true,
        minimize: false,
        maximize: false,
    };

    pub fn is_enabled(self, control: WindowControl) -> bool {
        match control {
            WindowControl::Close => self.close,
            WindowControl::Minimize => self.minimize,
            WindowControl::Maximize => self.maximize,
        }
    }
}

impl Default for WindowControlsEnabled {
    fn default() -> Self {
        Self::ALL
    }
}

/// Geometry and colours of the traffic lights.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowControlsStyle {
    /// Circle radius.
    pub radius: f32,
    /// Centre-to-centre distance between adjacent lights.
    pub spacing: f32,
    /// Left inset from the title bar's leading edge to the first circle.
    pub margin: f32,
    pub close: Color32,
    pub minimize: Color32,
    pub maximize: Color32,
    /// Fill of a light whose action is unavailable.
    pub disabled: Color32,
    /// Glyph revealed while hovering an enabled light.
    pub glyph: Color32,
}

impl Default for WindowControlsStyle {
    fn default() -> Self {
        Self {
            radius: 6.0,
            spacing: 20.0,
            margin: 12.0,
            close: Color32::from_rgb(0xFF, 0x5F, 0x56),
            minimize: Color32::from_rgb(0xFF, 0xBD, 0x2E),
            maximize: Color32::from_rgb(0x28, 0xC8, 0x40),
            disabled: Color32::from_rgb(0x4A, 0x4A, 0x4A),
            glyph: Color32::from_black_alpha(170),
        }
    }
}

/// What one pass of the controls produced.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowControlsResponse {
    pub close: bool,
    pub minimize: bool,
    pub maximize: bool,
    /// Union of the three hit rects. A title-bar drag area starts to its right.
    pub rect: Rect,
}

impl Default for WindowControlsResponse {
    fn default() -> Self {
        Self {
            close: false,
            minimize: false,
            maximize: false,
            rect: Rect::NOTHING,
        }
    }
}

/// Paint and interact with the three traffic lights inside the title bar.
///
/// The id must be unique per window; the per-control ids are derived from it.
/// Disabled lights still occupy their usual slots and paint grey, but they use
/// hover-only sensing and never report a click.
pub fn window_controls(
    ui: &mut Ui,
    id: Id,
    bar: Rect,
    style: &WindowControlsStyle,
    enabled: WindowControlsEnabled,
) -> WindowControlsResponse {
    let hit = (style.radius * 2.0).max(18.0);
    let first = bar.left() + style.margin + style.radius;
    let mut response = WindowControlsResponse::default();

    for (index, control) in WindowControl::ALL.into_iter().enumerate() {
        let center = pos2(first + style.spacing * index as f32, bar.center().y);
        let rect = Rect::from_center_size(center, Vec2::splat(hit));
        response.rect = response.rect.union(rect);

        let active = enabled.is_enabled(control);
        let sense = if active {
            Sense::click()
        } else {
            Sense::hover()
        };
        let mut control_response = ui.interact(rect, id.with(control), sense);
        if active {
            control_response = control_response.on_hover_cursor(CursorIcon::PointingHand);
        }

        let fill = if active {
            control.color(style)
        } else {
            style.disabled
        };
        ui.painter().circle_filled(center, style.radius, fill);
        if active && control_response.hovered() {
            paint_glyph(ui, control, center, style);
        }

        if active && control_response.clicked() {
            match control {
                WindowControl::Close => response.close = true,
                WindowControl::Minimize => response.minimize = true,
                WindowControl::Maximize => response.maximize = true,
            }
        }
    }

    response
}

fn paint_glyph(ui: &Ui, control: WindowControl, center: egui::Pos2, style: &WindowControlsStyle) {
    let stroke = Stroke::new(1.2, style.glyph);
    let arm = style.radius * 0.48;
    match control {
        WindowControl::Close => {
            ui.painter().line_segment(
                [
                    pos2(center.x - arm, center.y - arm),
                    pos2(center.x + arm, center.y + arm),
                ],
                stroke,
            );
            ui.painter().line_segment(
                [
                    pos2(center.x + arm, center.y - arm),
                    pos2(center.x - arm, center.y + arm),
                ],
                stroke,
            );
        }
        WindowControl::Minimize => {
            ui.painter()
                .hline((center.x - arm)..=(center.x + arm), center.y, stroke);
        }
        WindowControl::Maximize => {
            ui.painter().line_segment(
                [
                    pos2(center.x - arm, center.y),
                    pos2(center.x + arm, center.y),
                ],
                stroke,
            );
            ui.painter().line_segment(
                [
                    pos2(center.x, center.y - arm),
                    pos2(center.x, center.y + arm),
                ],
                stroke,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Context, Event, Modifiers, PointerButton, Pos2, RawInput, vec2};

    struct Harness {
        ctx: Context,
        enabled: WindowControlsEnabled,
        response: Option<WindowControlsResponse>,
    }

    impl Harness {
        fn new(enabled: WindowControlsEnabled) -> Self {
            Self {
                ctx: Context::default(),
                enabled,
                response: None,
            }
        }

        fn frame(&mut self, events: Vec<Event>) -> egui::FullOutput {
            let input = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(300.0, 100.0))),
                events,
                ..Default::default()
            };
            let enabled = self.enabled;
            let mut captured = None;
            let output = self.ctx.run_ui(input, |ui| {
                let (bar, _) = ui.allocate_exact_size(vec2(260.0, 28.0), Sense::hover());
                captured = Some(window_controls(
                    ui,
                    Id::new("controls"),
                    bar,
                    &WindowControlsStyle::default(),
                    enabled,
                ));
            });
            self.response = captured;
            output
        }

        fn click_at(&mut self, pos: Pos2) {
            self.frame(vec![Event::PointerMoved(pos)]);
            self.frame(vec![pointer(pos, true)]);
            self.frame(vec![pointer(pos, false)]);
        }
    }

    fn pointer(pos: Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        }
    }

    fn painted_circle(output: &egui::FullOutput, fill: Color32, at: Pos2) -> bool {
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

    #[test]
    fn the_three_lights_keep_macos_order_and_disabled_ones_are_grey() {
        let mut harness = Harness::new(WindowControlsEnabled::CLOSE_ONLY);
        let output = harness.frame(Vec::new());
        let id = Id::new("controls");
        let close = harness
            .ctx
            .read_response(id.with(WindowControl::Close))
            .unwrap()
            .rect;
        let minimize = harness
            .ctx
            .read_response(id.with(WindowControl::Minimize))
            .unwrap()
            .rect;
        let maximize = harness
            .ctx
            .read_response(id.with(WindowControl::Maximize))
            .unwrap()
            .rect;

        assert!(close.center().x < minimize.center().x);
        assert!(minimize.center().x < maximize.center().x);
        assert_eq!(close.size(), vec2(18.0, 18.0));
        let style = WindowControlsStyle::default();
        assert!(painted_circle(&output, style.close, close.center()));
        assert!(painted_circle(&output, style.disabled, minimize.center()));
        assert!(painted_circle(&output, style.disabled, maximize.center()));

        harness.click_at(minimize.center());
        assert!(!harness.response.unwrap().minimize);
        harness.click_at(maximize.center());
        assert!(!harness.response.unwrap().maximize);
        harness.click_at(close.center());
        assert!(harness.response.unwrap().close);
    }

    #[test]
    fn enabled_lights_each_report_their_click() {
        let mut harness = Harness::new(WindowControlsEnabled::ALL);
        harness.frame(Vec::new());
        let id = Id::new("controls");
        let close = harness
            .ctx
            .read_response(id.with(WindowControl::Close))
            .unwrap()
            .rect
            .center();
        let minimize = harness
            .ctx
            .read_response(id.with(WindowControl::Minimize))
            .unwrap()
            .rect
            .center();
        let maximize = harness
            .ctx
            .read_response(id.with(WindowControl::Maximize))
            .unwrap()
            .rect
            .center();

        harness.click_at(close);
        assert!(harness.response.unwrap().close);
        harness.click_at(minimize);
        assert!(harness.response.unwrap().minimize);
        harness.click_at(maximize);
        assert!(harness.response.unwrap().maximize);
    }
}
