//! Headless tests for the panel framework: registry, instance reconciliation,
//! the dock viewer adapter and the close-as-intent path. No window, no GPU.

use egui::{Context, Pos2, RawInput, Rect, Vec2};
use kairos_editor_component::panels::ConsoleStyle;
use kairos_editor_component::skin::SkinId;
use kairos_editor_component::{
    ConsoleLine, EditorInfo, Frame, HostView, PanelContext, PanelHost, PanelId, PanelInstanceId,
    UiOut, panels, shared_style::SharedStyles,
};
use kairos_editor_core::style::Skin;
use kairos_editor_core::{EditorState, LayoutState};
use kairos_editor_docking::viewer::TabViewer;
use kairos_log::Level;
use kairos_profile::profiling;

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

    PanelHost::new(panels::catalog(), PanelContext::new(test_info()))
}

/// Run one headless UI pass: exactly what the dock does with its viewer.
fn draw_with(
    host: &mut PanelHost,
    host_resources: &dyn HostView,
    state: &EditorState,
    shared_styles: &SharedStyles,
    ids: &[PanelInstanceId],
) -> UiOut {
    let mut out = UiOut::new();
    let ctx = Context::default();
    let mut input = RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 820.0)));
    {
        let frame = Frame::new(state, host_resources, shared_styles);
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
fn the_catalog_declares_every_builtin_panel_once() {
    let context = PanelContext::new(test_info());
    let mut kinds = Vec::new();

    let catalog = panels::catalog();
    for kind in catalog.kinds() {
        assert!(!kinds.contains(&kind), "duplicate id {kind}");
        kinds.push(kind);
        let panel = catalog.create(kind, &context).expect("catalog factory");
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
    let shared_styles = SharedStyles::default();
    let mut host = default_host();
    host.sync(&state.layout, &Context::default());
    let ids: Vec<PanelInstanceId> = host.ids().collect();

    let host_view = TestHost {
        lines: vec![
            ConsoleLine::new(Level::Info, "editor booted"),
            ConsoleLine::new(Level::Warning, "toggle panel: console"),
        ],
    };
    let out = draw_with(&mut host, &host_view, &state, &shared_styles, &ids);

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
    let shared_styles = SharedStyles::default();
    let mut host = default_host();
    host.sync(&state.layout, &Context::default());
    let ids: Vec<PanelInstanceId> = host.ids().collect();

    let out = draw_with(&mut host, &NoProfilerHost, &state, &shared_styles, &ids);

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
    let shared_styles = SharedStyles::default();
    let mut host = default_host();
    host.sync(&state.layout, &Context::default());
    let ids: Vec<PanelInstanceId> = host.ids().collect();

    let out = draw_with(&mut host, &host_view, &state, &shared_styles, &ids);

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
    let shared_styles = SharedStyles::default();
    let mut host = default_host();
    host.sync(&state.layout, &Context::default());
    let id = host.id_of(panels::CONSOLE).expect("console instance");

    let mut out = UiOut::new();
    let frame = Frame::new(&state, &NoHost, &shared_styles);
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
        let panel = panels::catalog()
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
    let text_only: Vec<PanelId> = panels::catalog()
        .kinds()
        .filter(|kind| {
            panels::catalog()
                .create(*kind, &context)
                .expect("catalog factory")
                .icon()
                .is_none()
        })
        .collect();

    assert_eq!(text_only, vec![panels::PREFERENCES]);
}

/// The Preferences editor reads live panels' self-drawing pages through the
/// host, and the page's whole value lands in the panel that owns the style.
#[test]
fn pages_are_listed_and_replaced_through_the_host() {
    let mut state = EditorState::new();
    state.layout.open(panels::CONSOLE);
    state.layout.open(panels::PROFILER);
    let shared_styles = SharedStyles::default();
    let mut host = default_host();
    host.sync(&state.layout, &Context::default());

    let pages = host.pages(&state, &shared_styles);
    let console = pages
        .iter()
        .find(|page| page.id == SkinId::Panel(panels::CONSOLE))
        .expect("console page");
    assert_eq!(console.file, "ConsoleStyle.ron");
    assert_eq!(
        console.title, "Console",
        "the generated page is titled by Panel::title"
    );
    let style = console
        .page
        .as_any()
        .downcast_ref::<ConsoleStyle>()
        .expect("the page holds the console's own style type");
    assert!(style.toolbar_height > 0.0);

    // The metadata the app compares every frame copies no style value, and the
    // one-page copy it takes on demand is the same page.
    let metas = host.page_metas(&state, &shared_styles);
    let console_meta = metas
        .iter()
        .find(|meta| meta.id == SkinId::Panel(panels::CONSOLE))
        .expect("console page metadata");
    assert_eq!(console_meta.file, "ConsoleStyle.ron");
    assert_eq!(console_meta.title, "Console");
    assert!(
        !metas
            .iter()
            .any(|meta| meta.id == SkinId::Panel(panels::ABOUT)),
        "a panel that is not open has no page"
    );
    let page = host.page(panels::CONSOLE).expect("console page copy");
    assert!(
        page.as_any().downcast_ref::<ConsoleStyle>().is_some(),
        "the per-page copy is the panel's own style type"
    );
    assert!(host.page(panels::ABOUT).is_none());

    // A whole value from the page lands in the owning panel; a box of another
    // type, or a panel that is not open, is a no-op.
    let mut next = style.clone();
    next.toolbar_height = 30.0;
    assert!(host.replace_skin(panels::CONSOLE, Box::new(next.clone())));
    assert!(!host.replace_skin(panels::CONSOLE, Box::new(0u8)));
    assert!(!host.replace_skin(panels::ABOUT, Box::new(next.clone())));

    let updated = host.pages(&state, &shared_styles);
    let console = updated
        .iter()
        .find(|page| page.id == SkinId::Panel(panels::CONSOLE))
        .expect("console page");
    assert_eq!(
        console
            .page
            .as_any()
            .downcast_ref::<ConsoleStyle>()
            .map(|style| style.toolbar_height),
        Some(30.0),
        "the next page is the value the host accepted"
    );

    // The style type also crosses the boundary through `Skin`, the shape an
    // action and an effect carry.
    let skin = Skin::new(next.clone());
    assert!(host.replace_skin(panels::CONSOLE, skin.into_any()));
}
