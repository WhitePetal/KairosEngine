//! Headless tests for the panel framework: registry, instance reconciliation,
//! the dock viewer adapter and the close-as-intent path. No window, no GPU.

use egui::{Context, Pos2, RawInput, Rect, Ui, Vec2, WidgetText};
use kairos_editor_component::{
    ConsoleLine, EditorInfo, Frame, HostView, Panel, PanelCatalog, PanelContext, PanelDef,
    PanelHost, PanelId, PanelInstanceId, UiOut, panels,
};
use kairos_editor_core::{EditorState, LayoutState, StyleValue};
use kairos_editor_docking::viewer::TabViewer;
use kairos_log::Level;
use kairos_profile::profiling;

const FAKE: PanelId = PanelId("fake");

/// The identity the composition root would inject; the real one lives in
/// `kairos_editor_app`, which this crate cannot depend on.
fn test_info() -> EditorInfo {
    EditorInfo::new("KairosEngine", "9.9.9-test")
}

/// The attribute expands to `profiling::function_scope!()`; the re-export above
/// is the only `profiling` path this crate has, so compiling this function
/// proves the documented usage works without a direct facade dependency.
#[kairos_profile::function]
fn profiled_probe() {}

#[test]
fn function_attribute_uses_the_profiling_reexport() {
    profiled_probe();
}

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

fn fake_allow(_context: &PanelContext) -> Box<dyn Panel> {
    Box::new(FakePanel { veto_close: false })
}

fn fake_veto(_context: &PanelContext) -> Box<dyn Panel> {
    Box::new(FakePanel { veto_close: true })
}

static FAKE_ALLOW: [PanelDef; 1] = [PanelDef::new(FAKE, fake_allow)];
static FAKE_VETO: [PanelDef; 1] = [PanelDef::new(FAKE, fake_veto)];

fn host_with_fake(veto_close: bool) -> PanelHost {
    let defs = if veto_close { &FAKE_VETO } else { &FAKE_ALLOW };
    PanelHost::new(PanelCatalog::new(defs), PanelContext::new(test_info()))
}

fn default_host() -> PanelHost {
    // `PanelHost::sync` calls `Panel::init`, which seeds missing style files.
    // Keep that write out of the crate directory: tests use a scratch root.
    static DIR: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    let root = DIR.get_or_init(|| {
        let root =
            std::env::temp_dir().join(format!("kairos-panel-host-styles-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::create_dir_all(&root);
        root
    });
    kairos_editor_widgets::style::set_style_root(root);

    PanelHost::new(panels::CATALOG, PanelContext::new(test_info()))
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
fn the_catalog_declares_every_builtin_panel_once() {
    let context = PanelContext::new(test_info());
    let mut kinds = Vec::new();

    for kind in panels::CATALOG.kinds() {
        assert!(!kinds.contains(&kind), "duplicate id {kind}");
        kinds.push(kind);
        let panel = panels::CATALOG
            .create(kind, &context)
            .expect("catalog factory");
        assert_eq!(panel.kind(), kind);
    }

    assert_eq!(kinds.len(), 8);
    for kind in [
        panels::SCENE,
        panels::INSPECTOR,
        panels::CONSOLE,
        panels::HIERARCHY,
        panels::PROJECT,
        panels::PROFILER,
        panels::ABOUT,
        panels::PREFERENCES,
    ] {
        assert!(kinds.contains(&kind), "missing {kind}");
    }
}

/// The catalog is compile-time data, not a startup pass: a panel object exists
/// only while its kind is open, and later frames reuse it instead of rebuilding.
#[test]
fn a_panel_is_constructed_only_while_it_is_open() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    static BUILT: AtomicUsize = AtomicUsize::new(0);

    fn counting(_context: &PanelContext) -> Box<dyn Panel> {
        BUILT.fetch_add(1, Ordering::SeqCst);
        Box::new(FakePanel { veto_close: false })
    }

    static COUNTING: [PanelDef; 1] = [PanelDef::new(FAKE, counting)];

    let mut host = PanelHost::new(PanelCatalog::new(&COUNTING), PanelContext::new(test_info()));
    let mut state = EditorState::new();
    let ctx = Context::default();

    host.sync(&state.layout, &ctx);
    assert_eq!(
        BUILT.load(Ordering::SeqCst),
        0,
        "a closed panel is never built"
    );

    state.layout.open(FAKE);
    host.sync(&state.layout, &ctx);
    host.sync(&state.layout, &ctx);
    assert_eq!(
        BUILT.load(Ordering::SeqCst),
        1,
        "opening builds one instance; later syncs reuse it"
    );
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
    assert_eq!(host.kinds().count(), 8);
    assert_eq!(
        host.id_of(panels::SCENE).map(|id| id.kind),
        Some(panels::SCENE)
    );
    assert_eq!(host.id_of(panels::CONSOLE), None);
    assert_eq!(LayoutState::default().focus(), None);
}

#[test]
fn profiler_panel_renders_without_a_host_capture() {
    struct NoProfilerHost;
    impl HostView for NoProfilerHost {}

    let mut state = EditorState::new();
    state.layout.open(panels::PROFILER);
    let mut host = default_host();
    host.sync(&state.layout, &Context::default());
    let ids: Vec<PanelInstanceId> = host.ids().collect();

    let out = draw_with(&mut host, &NoProfilerHost, &state, &ids);

    // The panel explains that the host is missing instead of panicking; it
    // never owns a capture of its own.
    assert!(out.is_empty());
}

#[test]
fn profiler_panel_renders_a_captured_frame() {
    // The host owns the recorder; the panel only borrows it (there is no
    // global profiler to reach for any more).
    struct ProfilingHost {
        profiler: kairos_profile::Profiler,
    }
    impl HostView for ProfilingHost {
        fn profiler(&self) -> Option<&kairos_profile::Profiler> {
            Some(&self.profiler)
        }
    }

    let host_view = ProfilingHost {
        profiler: kairos_profile::Profiler::new(),
    };
    let profiler = &host_view.profiler;
    profiler.set_recording(true);
    profiler.begin_frame();
    {
        kairos_profile::scope!(kairos_profile::Category::Others, "panel.test.outer");
        {
            kairos_profile::scope!(kairos_profile::Category::Others, "panel.test.inner", "data");
        }
    }
    profiler.end_frame(true);
    profiler.begin_frame();
    assert!(
        !profiler.session().is_empty(),
        "the fixture captured one committed frame"
    );

    let mut state = EditorState::new();
    state.layout.open(panels::PROFILER);
    let mut host = default_host();
    host.sync(&state.layout, &Context::default());
    let ids: Vec<PanelInstanceId> = host.ids().collect();

    let out = draw_with(&mut host, &host_view, &state, &ids);

    assert!(out.is_empty());
    profiler.set_recording(false);
    profiler.clear();
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
fn every_builtin_panel_with_a_tab_icon_points_at_an_existing_file() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let context = PanelContext::new(test_info());

    for kind in [
        panels::SCENE,
        panels::INSPECTOR,
        panels::CONSOLE,
        panels::HIERARCHY,
        panels::PROJECT,
        panels::PROFILER,
        panels::ABOUT,
    ] {
        let panel = panels::CATALOG
            .create(kind, &context)
            .expect("catalog factory");
        let path = panel
            .icon()
            .unwrap_or_else(|| panic!("{kind} returns no tab icon"));
        assert!(
            workspace.join(path).is_file(),
            "missing tab icon {path} for {kind}"
        );
    }
}

/// A text-only tab is allowed by the trait, but it has to be a decision, not a
/// forgotten asset: this test names the only panel without artwork.
#[test]
fn preferences_is_the_only_text_only_tab() {
    let context = PanelContext::new(test_info());
    let text_only: Vec<PanelId> = panels::CATALOG
        .kinds()
        .filter(|kind| {
            panels::CATALOG
                .create(*kind, &context)
                .expect("catalog factory")
                .icon()
                .is_none()
        })
        .collect();

    assert_eq!(text_only, vec![panels::PREFERENCES]);
}

/// The Preferences editor reads live panels' skins through the host, and a
/// write lands in the panel that owns the style.
#[test]
fn skin_fields_are_listed_and_written_through_the_host() {
    let mut state = EditorState::new();
    state.layout.open(panels::CONSOLE);
    state.layout.open(panels::PROFILER);
    let mut host = default_host();
    host.sync(&state.layout, &Context::default());

    let skins = host.skins();
    let console = skins
        .iter()
        .find(|skin| skin.panel == panels::CONSOLE)
        .expect("console skin");
    assert_eq!(console.file, "ConsoleStyle.ron");
    assert!(
        console
            .fields
            .iter()
            .any(|field| field.path == "toolbar_height"),
        "the console exposes its tokens"
    );
    assert!(
        console
            .fields
            .iter()
            .any(|field| field.path == "icons.info"),
        "nested icon paths are editable too"
    );

    // Unknown paths and wrong kinds are rejected by the panel, not applied.
    assert!(!host.set_skin_field(panels::CONSOLE, "nope", &StyleValue::Float(1.0)));
    assert!(!host.set_skin_field(panels::CONSOLE, "toolbar_height", &StyleValue::Toggle(true)));

    assert!(host.set_skin_field(panels::CONSOLE, "toolbar_height", &StyleValue::Float(30.0)));
    let updated = host.skins();
    let console = updated
        .iter()
        .find(|skin| skin.panel == panels::CONSOLE)
        .expect("console skin");
    let height = console
        .fields
        .iter()
        .find(|field| field.path == "toolbar_height")
        .expect("toolbar_height");
    assert_eq!(height.value, StyleValue::Float(30.0));

    // A panel that is not open has no instance, so it has no skin to write.
    assert!(
        host.set_skin_field(panels::ABOUT, "title_size", &StyleValue::Float(1.0))
            .eq(&false)
    );
}
