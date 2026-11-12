//! Headless tests for the panel framework: registry, instance reconciliation
//! and the dock viewer adapter. No window, no GPU.

use egui::{Context, Pos2, RawInput, Rect, Ui, Vec2, WidgetText};
use kairos_editor_component::{
    Panel, PanelContext, PanelHost, PanelId, PanelInstanceId, PanelRegistry, panels,
};
use kairos_editor_core::{EditorState, LayoutState};
use kairos_editor_docking::viewer::{CloseResponse, TabViewer};

struct FakePanel;

impl Panel for FakePanel {
    fn kind(&self) -> PanelId {
        PanelId("fake")
    }

    fn title(&self, _state: &EditorState) -> WidgetText {
        "Fake".into()
    }

    fn ui(&mut self, _ui: &mut Ui, _ctx: &mut PanelContext) {}
}

fn host_with_fake() -> PanelHost {
    let mut registry = PanelRegistry::new();
    registry.register(PanelId("fake"), || Box::new(FakePanel));
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

fn run_frame(host: &mut PanelHost, state: &mut EditorState, ids: &[PanelInstanceId]) {
    let ctx = Context::default();
    let mut input = RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 820.0)));
    let _ = ctx.run_ui(input, |ui| {
        let mut viewer = host.viewer(state);
        for id in ids {
            let _ = viewer.title(*id);
            viewer.ui(ui, *id);
        }
    });
}

#[test]
fn sync_creates_and_drops_instances_from_layout() {
    let mut host = host_with_fake();
    let mut state = EditorState::new();

    host.sync(&state.layout);
    assert_eq!(host.ids().count(), 0);

    state.layout.open(PanelId("fake"));
    host.sync(&state.layout);
    assert_eq!(host.ids().count(), 1);

    state.layout.close(PanelId("fake"));
    host.sync(&state.layout);
    assert_eq!(host.ids().count(), 0);
}

#[test]
fn viewer_close_updates_layout_and_drops_instance() {
    let mut host = host_with_fake();
    let mut state = EditorState::new();
    state.layout.open(PanelId("fake"));
    host.sync(&state.layout);
    let id = host.id_of(PanelId("fake")).expect("instance");

    {
        let mut viewer = host.viewer(&mut state);
        assert_eq!(viewer.on_close(id), CloseResponse::Close);
    }

    assert!(!state.layout.is_open(PanelId("fake")));
    assert_eq!(host.ids().count(), 0);
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
    let mut state = default_state();
    let mut registry = PanelRegistry::new();
    panels::register_default(&mut registry);
    let mut host = PanelHost::new(registry);
    host.sync(&state.layout);

    let ids: Vec<PanelInstanceId> = host.ids().collect();
    assert_eq!(ids.len(), 5);
    run_frame(&mut host, &mut state, &ids);
}

#[test]
fn viewer_unknown_id_uses_fallback_title_and_tolerates_missing_panel() {
    let mut state = EditorState::new();
    let mut host = PanelHost::new(PanelRegistry::new());
    host.sync(&state.layout);
    let unknown = PanelInstanceId::new(PanelId("missing"), 7);

    let ctx = Context::default();
    let mut input = RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0)));
    let _ = ctx.run_ui(input, |ui| {
        let mut viewer = host.viewer(&mut state);
        assert_eq!(viewer.title(unknown).text(), "missing#7");
        assert!(!viewer.closeable(unknown));
        viewer.ui(ui, unknown);
    });
}

#[test]
fn default_layout_reconciles_with_the_host() {
    // The app builds its dock from the ids the host created; the host's id set
    // must be exactly the open kinds, not the registry.
    let mut state = EditorState::new();
    state.layout.open(panels::SCENE);
    state.layout.open(panels::INSPECTOR);

    let mut registry = PanelRegistry::new();
    panels::register_default(&mut registry);
    let mut host = PanelHost::new(registry);
    host.sync(&state.layout);

    let ids: Vec<PanelInstanceId> = host.ids().collect();
    assert_eq!(ids.len(), 2);
    assert!(ids.iter().all(|id| state.layout.is_open(id.kind)));
    assert!(host.kinds().count() == 5);
    assert_eq!(
        host.id_of(panels::SCENE).map(|id| id.kind),
        Some(panels::SCENE)
    );
    assert_eq!(host.id_of(panels::CONSOLE), None);
    assert_eq!(LayoutState::default().focus(), None);
}
