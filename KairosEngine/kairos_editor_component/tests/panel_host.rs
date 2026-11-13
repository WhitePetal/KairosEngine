//! Headless tests for the panel framework: registry, instance reconciliation,
//! the dock viewer adapter and the close-as-intent path. No window, no GPU.

use egui::{Context, Pos2, RawInput, Rect, Ui, Vec2, WidgetText};
use kairos_editor_component::{
    ConsoleLine, Frame, HostView, Panel, PanelHost, PanelId, PanelInstanceId, PanelRegistry, UiOut,
    panels,
};
use kairos_editor_core::{EditorState, LayoutState};
use kairos_editor_docking::viewer::TabViewer;
use kairos_log::Level;

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

#[test]
fn sync_creates_and_drops_instances_from_layout() {
    let mut host = host_with_fake(false);
    let mut state = EditorState::new();

    host.sync(&state.layout, &Context::default());
    assert_eq!(host.ids().count(), 0);

    state.layout.open(FAKE);
    host.sync(&state.layout, &Context::default());
    assert_eq!(host.ids().count(), 1);

    state.layout.close(FAKE);
    host.sync(&state.layout, &Context::default());
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
fn console_renders_the_host_log_snapshot() {
    struct TestHost {
        lines: Vec<ConsoleLine>,
    }

    impl HostView for TestHost {
        fn console_lines(&self, f: &mut dyn FnMut(&[ConsoleLine])) {
            f(&self.lines);
        }
    }

    let mut state = EditorState::new();
    state.layout.open(panels::CONSOLE);
    let mut host = default_host();
    host.sync(&state.layout, &Context::default());
    let ids: Vec<PanelInstanceId> = host.ids().collect();

    let host_view = TestHost {
        lines: vec![
            ConsoleLine::new(Level::Info, "editor booted"),
            ConsoleLine::new(Level::Warning, "toggle panel: console"),
        ],
    };
    let out = draw_with(&mut host, &host_view, &state, &ids);

    assert!(out.is_empty());
    // The console owns no history: it just rendered the host snapshot.
    assert_eq!(host_view.lines.len(), 2);
}

#[test]
fn default_layout_reconciles_with_the_host() {
    // The app builds its dock from the ids the host created; the host's id set
    // must be exactly the open kinds, not the registry.
    let mut state = EditorState::new();
    state.layout.open(panels::SCENE);
    state.layout.open(panels::INSPECTOR);

    let mut host = default_host();
    host.sync(&state.layout, &Context::default());

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

#[test]
fn viewer_bridges_the_panel_icon_path_to_a_file_uri() {
    struct NoHost;
    impl HostView for NoHost {}

    let mut state = EditorState::new();
    state.layout.open(panels::CONSOLE);
    let mut host = default_host();
    host.sync(&state.layout, &Context::default());
    let id = host.id_of(panels::CONSOLE).expect("console instance");

    let mut out = UiOut::new();
    let frame = Frame::with_host(&state, &NoHost);
    let mut viewer = host.viewer(&mut out, frame);
    let icon = viewer.icon(id).expect("console tab icon");
    let uri = icon.uri().expect("URI image source");

    // The panel owns the editor-relative appearance path; the viewer is the
    // only place that turns it into something egui's loaders accept.
    assert!(uri.starts_with("file://"), "{uri}");
    assert!(
        uri.ends_with("Preferences/Textures/tab_icons/console.png"),
        "{uri}"
    );
}

#[test]
fn every_builtin_panel_returns_an_existing_tab_icon() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut registry = PanelRegistry::new();
    panels::register_default(&mut registry);

    for kind in [
        panels::SCENE,
        panels::INSPECTOR,
        panels::CONSOLE,
        panels::HIERARCHY,
        panels::PROJECT,
    ] {
        let panel = registry.create(kind).expect("registered factory");
        let path = panel
            .icon()
            .unwrap_or_else(|| panic!("{kind} returns no tab icon"));
        assert!(
            workspace.join(path).is_file(),
            "missing tab icon {path} for {kind}"
        );
    }
}
