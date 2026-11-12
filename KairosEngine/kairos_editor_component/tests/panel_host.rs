//! Headless tests for the panel framework: registry, instance reconciliation,
//! the dock viewer adapter and the close-as-intent path. No window, no GPU.

use egui::{Context, Pos2, RawInput, Rect, Ui, Vec2, WidgetText};
use kairos_editor_component::{
    EmptyHost, Frame, HostView, Panel, PanelHost, PanelId, PanelInstanceId, PanelRegistry, UiOut,
    panels,
};
use kairos_editor_core::{Action, EditorState, LayoutAction, LayoutState, reduce};
use kairos_editor_docking::viewer::{CloseResponse, TabViewer};

const FAKE: PanelId = PanelId("fake");

struct FakePanel {
    veto_close: bool,
}

impl Panel for FakePanel {
    fn kind(&self) -> PanelId {
        FAKE
    }

    fn title(&self, _frame: &Frame) -> WidgetText {
        "Fake".into()
    }

    fn ui(&mut self, _ui: &mut Ui, _frame: &Frame, _out: &mut UiOut) {}

    fn on_close(&mut self, _frame: &Frame, _out: &mut UiOut) -> bool {
        !self.veto_close
    }
}

struct TogglePanel;

impl Panel for TogglePanel {
    fn kind(&self) -> PanelId {
        PanelId("toggle")
    }

    fn title(&self, _frame: &Frame) -> WidgetText {
        "Toggle".into()
    }

    fn ui(&mut self, _ui: &mut Ui, _frame: &Frame, out: &mut UiOut) {
        out.push(Action::Layout(LayoutAction::TogglePanel {
            panel: panels::CONSOLE,
        }));
    }
}

fn host_with_fake(veto_close: bool) -> PanelHost {
    let mut registry = PanelRegistry::new();
    registry.register(FAKE, move || Box::new(FakePanel { veto_close }));
    PanelHost::new(registry)
}

fn default_host() -> PanelHost {
    let mut registry = PanelRegistry::new();
    panels::register_default(&mut registry);
    PanelHost::new(registry)
}

fn default_state() -> EditorState {
    let mut state = EditorState::new();
    for kind in [
        panels::SCENE,
        panels::INSPECTOR,
        panels::CONSOLE,
        panels::HIERARCHY,
        panels::PROJECT,
    ] {
        state.layout.open(kind);
    }
    state
}

/// Run one headless UI pass: exactly what the dock does with its viewer.
fn draw_with(
    host: &mut PanelHost,
    host_resources: &dyn HostView,
    state: &EditorState,
    ids: &[PanelInstanceId],
) -> UiOut {
    let mut out = UiOut::new();
    let ctx = Context::default();
    let mut input = RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 820.0)));
    {
        let frame = Frame::with_host(state, host_resources);
        let mut viewer = host.viewer(&mut out, frame);
        let _ = ctx.run_ui(input, |ui| {
            for id in ids {
                let _ = viewer.title(*id);
                viewer.ui(ui, *id);
            }
        });
    }
    out
}

fn draw(host: &mut PanelHost, state: &EditorState, ids: &[PanelInstanceId]) -> UiOut {
    draw_with(host, &EmptyHost, state, ids)
}

#[test]
fn sync_creates_and_drops_instances_from_layout() {
    let mut host = host_with_fake(false);
    let mut state = EditorState::new();

    host.sync(&state.layout);
    assert_eq!(host.ids().count(), 0);

    state.layout.open(FAKE);
    host.sync(&state.layout);
    assert_eq!(host.ids().count(), 1);

    state.layout.close(FAKE);
    host.sync(&state.layout);
    assert_eq!(host.ids().count(), 0);
}

#[test]
fn viewer_close_defers_layout_change_to_the_reducer() {
    let mut host = host_with_fake(false);
    let mut state = EditorState::new();
    state.layout.open(FAKE);
    host.sync(&state.layout);
    let id = host.id_of(FAKE).expect("instance");

    let mut out = UiOut::new();
    {
        let frame = Frame::new(&state);
        let mut viewer = host.viewer(&mut out, frame);
        assert_eq!(viewer.on_close(id), CloseResponse::Close);
    }

    // The viewer only reports the intent; it must not touch the state tree.
    assert!(
        state.layout.is_open(FAKE),
        "on_close must not mutate layout"
    );
    assert_eq!(
        out.as_slice(),
        &[Action::Layout(LayoutAction::ClosePanel { panel: FAKE })]
    );

    // The app half: dispatch the action, then the host drops the instance.
    for action in out.take() {
        reduce(&mut state, action);
    }
    assert!(!state.layout.is_open(FAKE));
    host.sync(&state.layout);
    assert_eq!(host.ids().count(), 0);
}

#[test]
fn close_veto_keeps_the_panel_open_and_emits_nothing() {
    let mut host = host_with_fake(true);
    let mut state = EditorState::new();
    state.layout.open(FAKE);
    host.sync(&state.layout);
    let id = host.id_of(FAKE).expect("instance");

    let mut out = UiOut::new();
    {
        let frame = Frame::new(&state);
        let mut viewer = host.viewer(&mut out, frame);
        assert_eq!(viewer.on_close(id), CloseResponse::Ignore);
    }

    assert!(out.is_empty());
    assert!(state.layout.is_open(FAKE));
    assert_eq!(host.ids().count(), 1);
}

#[test]
fn panel_actions_flow_into_ui_out() {
    let mut registry = PanelRegistry::new();
    registry.register(PanelId("toggle"), || Box::new(TogglePanel));
    let mut host = PanelHost::new(registry);
    let mut state = EditorState::new();
    state.layout.open(PanelId("toggle"));

    host.sync(&state.layout);
    let ids: Vec<PanelInstanceId> = host.ids().collect();
    let out = draw(&mut host, &state, &ids);

    assert_eq!(
        out.as_slice(),
        &[Action::Layout(LayoutAction::TogglePanel {
            panel: panels::CONSOLE
        })]
    );
}

#[test]
fn register_default_registers_every_default_kind() {
    let mut registry = PanelRegistry::new();
    panels::register_default(&mut registry);

    let kinds: Vec<PanelId> = registry.kinds().collect();
    assert_eq!(kinds.len(), 5);

    for kind in [
        panels::SCENE,
        panels::INSPECTOR,
        panels::CONSOLE,
        panels::HIERARCHY,
        panels::PROJECT,
    ] {
        assert!(kinds.contains(&kind), "missing {kind}");
        let panel = registry.create(kind).expect("registered factory");
        assert_eq!(panel.kind(), kind);
    }
}

#[test]
fn every_default_panel_draws_one_frame_headless() {
    let state = default_state();
    let mut host = default_host();
    host.sync(&state.layout);

    let ids: Vec<PanelInstanceId> = host.ids().collect();
    assert_eq!(ids.len(), 5);

    let out = draw(&mut host, &state, &ids);
    assert!(out.is_empty(), "read-only demo panels push no actions");
}

#[test]
fn console_renders_the_host_log_snapshot() {
    struct TestHost {
        lines: Vec<String>,
    }

    impl HostView for TestHost {
        fn console_lines(&self) -> &[String] {
            &self.lines
        }
    }

    let mut state = EditorState::new();
    state.layout.open(panels::CONSOLE);
    let mut host = default_host();
    host.sync(&state.layout);
    let ids: Vec<PanelInstanceId> = host.ids().collect();

    let host_view = TestHost {
        lines: vec![
            "editor booted".to_owned(),
            "toggle panel: console".to_owned(),
        ],
    };
    let out = draw_with(&mut host, &host_view, &state, &ids);

    assert!(out.is_empty());
    // The console owns no history: it just consumed the host snapshot.
    assert_eq!(host_view.lines.len(), 2);
}

#[test]
fn viewer_unknown_id_uses_fallback_title_and_tolerates_missing_panel() {
    let state = EditorState::new();
    let mut host = PanelHost::new(PanelRegistry::new());
    host.sync(&state.layout);
    let unknown = PanelInstanceId::new(PanelId("missing"), 7);

    let ctx = Context::default();
    let mut input = RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0)));
    let mut out = UiOut::new();
    {
        let frame = Frame::new(&state);
        let mut viewer = host.viewer(&mut out, frame);
        let _ = ctx.run_ui(input, |ui| {
            assert_eq!(viewer.title(unknown).text(), "missing#7");
            assert!(!viewer.closeable(unknown));
            viewer.ui(ui, unknown);
        });
    }
    assert!(out.is_empty());
}

#[test]
fn default_layout_reconciles_with_the_host() {
    // The app builds its dock from the ids the host created; the host's id set
    // must be exactly the open kinds, not the registry.
    let mut state = EditorState::new();
    state.layout.open(panels::SCENE);
    state.layout.open(panels::INSPECTOR);

    let mut host = default_host();
    host.sync(&state.layout);

    let ids: Vec<PanelInstanceId> = host.ids().collect();
    assert_eq!(ids.len(), 2);
    assert!(ids.iter().all(|id| state.layout.is_open(id.kind)));
    assert_eq!(host.kinds().count(), 5);
    assert_eq!(
        host.id_of(panels::SCENE).map(|id| id.kind),
        Some(panels::SCENE)
    );
    assert_eq!(host.id_of(panels::CONSOLE), None);
    assert_eq!(LayoutState::default().focus(), None);
}
