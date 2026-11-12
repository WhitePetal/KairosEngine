//! Headless interaction tests for the scene viewport panel.
//!
//! These close the loop that the window demo shows manually: egui pointer input
//! in the viewport body becomes the exact `Action` the host would dispatch
//! (`Pick` with NDC, `Orbit` with radians, `Ensure` with physical pixels, and
//! `FocusPanel`). They are deliberately GPU-free: the panel only sees a fake
//! `HostView` that hands it `TextureId::User(0)`.

use editor_components_prototype::{Frame, HostView, Panel, PanelId, UiOut, ViewportPanel};
use editor_core_prototype::{
    Action, EditorState, LayoutAction, SceneObjectId, ViewportAction, ViewportId, ViewportInfo,
};
use egui::{Pos2, Rect, TextureId, pos2};
use ui_widgets_prototype::testing::Harness;

struct FakeHost;

impl HostView for FakeHost {
    fn viewport_texture(&self, viewport: ViewportId) -> Option<TextureId> {
        (viewport == ViewportId::SCENE).then_some(TextureId::User(0))
    }
}

fn bound_state() -> (EditorState, Rect) {
    let mut state = EditorState::new();
    // The panel reports `rect * pixels_per_point`; the harness uses 1.0 ppp and
    // the viewport is drawn in the root Ui, so the whole screen is the viewport.
    state.viewports.insert(
        ViewportId::SCENE,
        ViewportInfo {
            size: (800, 560),
            bind_size: (800, 560),
            bound: true,
            pending_bind: None,
            pending_pick: None,
            last_error: None,
        },
    );
    (
        state,
        Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 560.0)),
    )
}

#[test]
fn viewport_panel_click_emits_focus_and_pick_ndc() {
    let (state, _rect) = bound_state();
    let host = FakeHost;
    let mut panel = ViewportPanel::new(ViewportId::SCENE);
    let mut harness = Harness::new(800.0, 560.0);
    let mut actions: Vec<Action> = Vec::new();

    harness.click(pos2(400.0, 280.0), |ui| {
        // Last-pass semantics, like the app shell: each pass starts clean.
        let mut out = UiOut::new();
        let frame = Frame::with_host(&state, &host);
        panel.draw(ui, &frame, &mut out);
        actions = out.take();
    });

    assert!(
        actions.contains(&Action::Layout(LayoutAction::FocusPanel {
            panel: PanelId::VIEWPORT
        })),
        "clicking the viewport must focus the panel, got {actions:?}"
    );
    let pick = actions.iter().find_map(|action| match action {
        Action::Viewport(ViewportAction::Pick { viewport, ndc }) => Some((*viewport, *ndc)),
        _ => None,
    });
    let (viewport, ndc) = pick.expect("click must emit ViewportAction::Pick");
    assert_eq!(viewport, ViewportId::SCENE);
    assert!(
        (ndc[0]).abs() < 0.01 && (ndc[1]).abs() < 0.01,
        "center click must map to NDC origin, got {ndc:?}"
    );
}

#[test]
fn viewport_panel_drag_emits_orbit_with_radians() {
    let (state, _rect) = bound_state();
    let host = FakeHost;
    let mut panel = ViewportPanel::new(ViewportId::SCENE);
    let mut harness = Harness::new(800.0, 560.0);
    let mut actions: Vec<Action> = Vec::new();

    harness.drag(pos2(300.0, 250.0), pos2(400.0, 300.0), |ui| {
        let mut out = UiOut::new();
        let frame = Frame::with_host(&state, &host);
        panel.draw(ui, &frame, &mut out);
        actions.extend(out.take());
    });

    let orbit = actions
        .iter()
        .find_map(|action| match action {
            Action::Viewport(ViewportAction::Orbit { viewport, delta }) => {
                Some((*viewport, *delta))
            }
            _ => None,
        })
        .expect("primary drag must emit ViewportAction::Orbit");
    assert_eq!(orbit.0, ViewportId::SCENE);
    // Drag right -> yaw decreases (scene follows the pointer); drag down -> pitch
    // increases (camera goes up, grab-the-world orbit). Units are radians.
    assert!(
        orbit.1[0] < 0.0,
        "yaw delta must be negative: {:?}",
        orbit.1
    );
    assert!(
        orbit.1[1] > 0.0,
        "pitch delta must be positive: {:?}",
        orbit.1
    );
    assert!(
        orbit.1[0].abs() < 1.0,
        "orbit delta must be radians, not pixels: {:?}",
        orbit.1
    );
}

#[test]
fn viewport_panel_never_touches_scene_selection_itself() {
    // The panel emits Pick; only the reducer/host completion may change
    // selection. This test pins that the view layer stays read-only.
    let (state, _rect) = bound_state();
    let host = FakeHost;
    let mut panel = ViewportPanel::new(ViewportId::SCENE);
    let mut harness = Harness::new(800.0, 560.0);
    let mut actions: Vec<Action> = Vec::new();

    harness.click(pos2(400.0, 280.0), |ui| {
        let mut out = UiOut::new();
        let frame = Frame::with_host(&state, &host);
        panel.draw(ui, &frame, &mut out);
        actions = out.take();
    });

    assert!(!actions.iter().any(|action| matches!(
        action,
        Action::Viewport(ViewportAction::PickCompleted { .. })
    )));
    assert_eq!(state.selection.selected_scene_object, None);
    let _ = SceneObjectId(1);
}
