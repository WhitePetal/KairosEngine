//! Scene viewport panel: the egui side of the egui <-> wgpu seam.
//!
//! This file is the UI half of the viewport integration (the host half lives
//! in editor-viewport). It is deliberately thin:
//!
//! * it asks the host for an `egui::TextureId` through `Frame::host`
//!   (`HostView::viewport_texture`); it never sees wgpu, the render graph, or
//!   the camera;
//! * it reports its physical pixel size with `ViewportAction::Ensure`;
//! * it translates pointer gestures into viewport intents.
//!
//! The panel has no domain state, no GPU handle and no camera.

use editor_core_prototype::{Action, LayoutAction, PanelId, ViewportAction, ViewportId};
use egui::{Color32, CursorIcon, PointerButton, Pos2, Rect, Sense, Ui, Vec2};

use crate::frame::{Frame, UiOut};
use crate::panel::Panel;

const INPUT_HINT: &str = "LMB orbit   |   MMB / Shift+LMB pan   |   wheel zoom   |   click to pick";

/// Orbit sensitivity: how many radians one logical pixel of drag rotates.
const ORBIT_RADIANS_PER_POINT: f32 = 0.008;

/// Zoom sensitivity: viewport camera input is unitless; one wheel notch is
/// roughly 50 points of `smooth_scroll_delta`, which maps to 0.1 here.
const ZOOM_PER_SCROLL_POINT: f32 = 1.0 / 500.0;

/// Convert a drag delta in points into an orbit delta in radians `[yaw, pitch]`.
///
/// Drag right rotates the camera left so the scene follows the pointer; drag
/// down raises the camera (grab-the-world orbit).
fn orbit_delta(points: Vec2) -> [f32; 2] {
    [
        -points.x * ORBIT_RADIANS_PER_POINT,
        points.y * ORBIT_RADIANS_PER_POINT,
    ]
}

/// Convert a drag delta in points into a pan delta normalized to viewport height.
fn pan_delta(points: Vec2, viewport_height_points: f32) -> [f32; 2] {
    let height = viewport_height_points.max(1.0);
    [points.x / height, points.y / height]
}

/// Convert `smooth_scroll_delta.y` into the camera's unitless zoom (positive = in).
fn zoom_delta(scroll_points: f32) -> f32 {
    scroll_points * ZOOM_PER_SCROLL_POINT
}

pub struct ViewportPanel {
    viewport: ViewportId,
}

impl ViewportPanel {
    pub fn new(viewport: ViewportId) -> Self {
        Self { viewport }
    }

    pub fn ndc_from_pos(rect: Rect, pos: Pos2) -> [f32; 2] {
        let size = rect.size();
        if size.x <= 0.0 || size.y <= 0.0 {
            return [0.0, 0.0];
        }
        let x = ((pos.x - rect.min.x) / size.x).clamp(0.0, 1.0);
        let y = ((pos.y - rect.min.y) / size.y).clamp(0.0, 1.0);
        [x * 2.0 - 1.0, 1.0 - y * 2.0]
    }
}

impl ViewportPanel {
    /// Draw the viewport body: size reporting, image, input, overlay.
    fn draw_body(&mut self, ui: &mut Ui, frame: &Frame, out: &mut UiOut) {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        if rect.width() < 1.0 || rect.height() < 1.0 {
            return;
        }
        if !ui.is_rect_visible(rect) {
            return;
        }

        let pixels_per_point = ui.ctx().pixels_per_point();
        let width = (rect.width() * pixels_per_point).round().max(1.0) as u32;
        let height = (rect.height() * pixels_per_point).round().max(1.0) as u32;
        let recorded = frame
            .state
            .viewports
            .get(self.viewport)
            .map(|info| info.size);
        if recorded != Some((width, height)) {
            out.push(Action::Viewport(ViewportAction::Ensure {
                viewport: self.viewport,
                width,
                height,
            }));
        }

        match frame.host.viewport_texture(self.viewport) {
            Some(texture) => {
                ui.painter().image(
                    texture,
                    rect,
                    Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
                self.handle_input(ui, rect, &response, out);
                self.draw_overlay(ui, rect, frame);
            }
            None => self.draw_unbound(ui, rect, frame),
        }
    }

    fn handle_input(&self, ui: &Ui, rect: Rect, response: &egui::Response, out: &mut UiOut) {
        if response.clicked() || response.drag_started() {
            out.push(Action::Layout(LayoutAction::FocusPanel {
                panel: PanelId::VIEWPORT,
            }));
        }

        let shift = ui.input(|input| input.modifiers.shift);
        let orbit = response.dragged_by(PointerButton::Primary) && !shift;
        let pan = response.dragged_by(PointerButton::Middle)
            || (response.dragged_by(PointerButton::Primary) && shift);

        if orbit {
            let delta = response.drag_delta();
            if delta != Vec2::ZERO {
                out.push(Action::Viewport(ViewportAction::Orbit {
                    viewport: self.viewport,
                    delta: orbit_delta(delta),
                }));
            }
        }
        if pan {
            let delta = response.drag_delta();
            if delta != Vec2::ZERO {
                // Normalized to viewport height: the same gesture pans the same
                // fraction of the scene at any panel size.
                out.push(Action::Viewport(ViewportAction::Pan {
                    viewport: self.viewport,
                    delta: pan_delta(delta, rect.height()),
                }));
            }
        }

        if response.hovered() {
            let scroll = ui.input(|input| input.smooth_scroll_delta.y);
            if scroll != 0.0 {
                out.push(Action::Viewport(ViewportAction::Zoom {
                    viewport: self.viewport,
                    delta: zoom_delta(scroll),
                }));
            }
            ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
        }

        if response.clicked()
            && let Some(pos) = response.interact_pointer_pos()
        {
            out.push(Action::Viewport(ViewportAction::Pick {
                viewport: self.viewport,
                ndc: Self::ndc_from_pos(rect, pos),
            }));
        }
    }

    fn draw_overlay(&self, ui: &Ui, rect: Rect, frame: &Frame) {
        let visuals = ui.visuals();
        let painter = ui.painter();
        painter.text(
            rect.left_top() + Vec2::new(8.0, 6.0),
            egui::Align2::LEFT_TOP,
            INPUT_HINT,
            egui::FontId::proportional(11.0),
            visuals.weak_text_color(),
        );
        let selected = frame
            .state
            .selection
            .selected_scene_object
            .map(|object| format!("picked object {}", object.0))
            .unwrap_or_else(|| "nothing picked".to_owned());
        painter.text(
            rect.right_bottom() - Vec2::new(8.0, 6.0),
            egui::Align2::RIGHT_BOTTOM,
            selected,
            egui::FontId::proportional(11.0),
            visuals.weak_text_color(),
        );
    }

    fn draw_unbound(&self, ui: &Ui, rect: Rect, frame: &Frame) {
        let visuals = ui.visuals();
        ui.painter()
            .rect_filled(rect, 0.0, visuals.extreme_bg_color);
        let error = frame
            .state
            .viewports
            .get(self.viewport)
            .and_then(|info| info.last_error.clone());
        let message = match error {
            Some(error) => format!("3D viewport unavailable: {error}"),
            None => "Binding 3D viewport...".to_owned(),
        };
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            message,
            egui::FontId::proportional(14.0),
            visuals.weak_text_color(),
        );
    }
}

impl Panel for ViewportPanel {
    fn kind(&self) -> PanelId {
        PanelId::VIEWPORT
    }

    fn title(&self, _frame: &Frame) -> String {
        "Scene".to_string()
    }

    fn draw(&mut self, ui: &mut Ui, frame: &Frame, out: &mut UiOut) {
        self.draw_body(ui, frame, out);
    }

    fn wants_continuous_repaint(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::ViewportPanel;
    use egui::{Rect, pos2};

    fn rect() -> Rect {
        Rect::from_min_max(pos2(10.0, 20.0), pos2(210.0, 120.0))
    }

    #[test]
    fn center_maps_to_ndc_origin() {
        assert_eq!(
            ViewportPanel::ndc_from_pos(rect(), pos2(110.0, 70.0)),
            [0.0, 0.0]
        );
    }

    #[test]
    fn corners_map_to_ndc_corners_with_y_up() {
        assert_eq!(
            ViewportPanel::ndc_from_pos(rect(), pos2(10.0, 20.0)),
            [-1.0, 1.0]
        );
        assert_eq!(
            ViewportPanel::ndc_from_pos(rect(), pos2(210.0, 120.0)),
            [1.0, -1.0]
        );
    }

    #[test]
    fn outside_points_are_clamped() {
        assert_eq!(
            ViewportPanel::ndc_from_pos(rect(), pos2(-100.0, 500.0)),
            [-1.0, -1.0]
        );
    }

    #[test]
    fn zero_sized_rect_is_origin() {
        let empty = Rect::from_min_max(pos2(0.0, 0.0), pos2(0.0, 0.0));
        assert_eq!(
            ViewportPanel::ndc_from_pos(empty, pos2(1.0, 1.0)),
            [0.0, 0.0]
        );
    }

    #[test]
    fn orbit_drag_right_turns_yaw_negative_and_drag_down_raises_pitch() {
        let delta = super::orbit_delta(egui::vec2(100.0, 50.0));
        assert!(delta[0] < 0.0, "drag right must rotate yaw negative");
        assert!(delta[1] > 0.0, "drag down must raise pitch");
        assert_eq!(delta, [-0.8, 0.4]);
    }

    #[test]
    fn pan_is_normalized_to_viewport_height() {
        let delta = super::pan_delta(egui::vec2(100.0, 50.0), 500.0);
        assert_eq!(delta, [0.2, 0.1]);
        // A zero-height rect must not divide by zero.
        assert_eq!(super::pan_delta(egui::vec2(10.0, 10.0), 0.0), [10.0, 10.0]);
    }

    #[test]
    fn scroll_up_maps_to_positive_zoom_in() {
        assert!(super::zoom_delta(50.0) > 0.0);
        assert!(super::zoom_delta(-50.0) < 0.0);
        assert_eq!(super::zoom_delta(0.0), 0.0);
    }
}
