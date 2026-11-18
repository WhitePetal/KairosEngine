use std::sync::Arc;

use kairos_editor_component::{
    ConsoleLine, ConsoleLog, EditorInfo, Frame, HostView, PanelContext, PanelHost, PanelInstanceId,
    PanelSkin, UiOut, panels, toolbar::ToolBar,
};
use kairos_editor_core::{
    Action, AppAction, EditorState, Effect, LayoutAction, StyleAction, reduce,
};
use kairos_editor_docking::{
    docker::{DockState, Docker},
    split::Fraction,
    tree::NodeIndex,
};
use kairos_engine::Engine;
use kairos_log::SinkHandle;
use kairos_profile::{Category, Profiler};

use crate::consts;
use crate::effects::EffectExecutor;

/// Host-owned resources a panel may display but never own (the prototype's
/// `HostState`). The console history is a `kairos_log` sink: the process logger
/// writes into it, the Console panel reads it through `HostView`, and neither
/// owns it. It is not in `EditorState` because it is a host stream, not
/// serializable state.
///
/// The engine is the other, but the app does not own it: the engine's loop
/// (`kairos_engine::runtime`) does, and borrows it into each UI pass. A frame
/// sees it through [`HostScope`], which is the `HostView` handed to the panels.
/// `kairos_profile` has no process-global recorder, so the engine owns the one
/// this process uses; the app never creates a second.
struct AppHost {
    console: Arc<ConsoleLog>,
    /// Keeps the console attached to the process logger; dropping the app
    /// detaches the sink.
    _console_sink: SinkHandle,
}

impl AppHost {
    fn new() -> Self {
        let console = Arc::new(ConsoleLog::new());
        let console_sink = kairos_log::attach(console.clone());
        Self {
            console,
            _console_sink: console_sink,
        }
    }
}

/// The `HostView` one UI pass hands to the panels: the host's console history
/// plus the loop's engine recorder.
///
/// A separate short-lived value rather than a field on the app, because the
/// engine belongs to the loop, not to the app — this is where that borrow is
/// bounded.
struct HostScope<'a> {
    console: Arc<ConsoleLog>,
    profiler: Option<&'a Profiler>,
}

impl HostView for HostScope<'_> {
    fn console_lines(&self, f: &mut dyn FnMut(&[ConsoleLine])) {
        self.console.with_lines(f);
    }

    fn profiler(&self) -> Option<&Profiler> {
        self.profiler
    }
}

/// The panels' construction context, built from this app's product constants.
///
/// The app owns what the editor calls itself; `kairos_editor_component` cannot
/// reach these constants without depending on this crate, so the identity is
/// injected through the `PanelContext` the host hands to every factory (see
/// `panels::CATALOG`). That keeps one source of truth for the version: the
/// title bar and About read the same constant.
fn editor_info() -> EditorInfo {
    EditorInfo {
        name: consts::APP_NAME,
        version: consts::VERSION,
        description: consts::DESCRIPTION,
        license: consts::LICENSE,
        repository: consts::REPOSITORY_URL,
        discussions: consts::DISCUSSIONS_URL,
        credits: consts::CREDITS_URL,
    }
}

/// The first-run open set: the five panels [`default_dock`] builds tabs for.
///
/// Bootstrapping is the only code allowed to call `LayoutState::open` directly;
/// runtime opens go through `LayoutAction` so the reducer can return a
/// placement effect. This is app policy, not panel metadata: which panels a
/// fresh editor shows is a product decision, so it does not live in the
/// catalog.
fn boot_state() -> EditorState {
    let mut state = EditorState::new();
    for kind in [
        panels::SCENE,
        panels::INSPECTOR,
        panels::CONSOLE,
        panels::PROFILER,
        panels::HIERARCHY,
    ] {
        state.layout.open(kind);
    }
    state
}

/// Build the first-run layout from the ids the host created for the five
/// default panels. User drags/splits after this are pure dock-state changes.
fn default_dock(host: &PanelHost) -> DockState<PanelInstanceId> {
    let scene = host.id_of(panels::SCENE).expect("scene instance");
    let inspector = host.id_of(panels::INSPECTOR).expect("inspector instance");
    let console = host.id_of(panels::CONSOLE).expect("console instance");
    let profiler = host.id_of(panels::PROFILER).expect("profiler instance");
    let hierarchy = host.id_of(panels::HIERARCHY).expect("hierarchy instance");

    let mut dock: DockState<PanelInstanceId> = DockState::new();
    let [scene_leaf, _inspector_leaf] = dock.main_surface_mut().split_right(
        NodeIndex::root(),
        Fraction::new(0.78),
        vec![inspector],
    );
    dock.main_surface_mut()[scene_leaf].append_tab(scene);
    let [top, _bottom] = dock.main_surface_mut().split_bottom(
        scene_leaf,
        Fraction::new(0.7),
        vec![console, profiler],
    );
    dock.main_surface_mut()
        .split_left(top, Fraction::new(0.22), vec![hierarchy]);
    dock
}

pub struct EditorApp {
    state: EditorState,
    host: PanelHost,
    dock: DockState<PanelInstanceId>,
    host_view: AppHost,
    tool_bar: ToolBar,
    /// Host work returned by the last committed reduce. Applied at the start of
    /// the next frame, after the panel host has synced its instances.
    pending_effects: Vec<Effect>,
    /// Live panels' skin field tables, rebuilt each frame while the Preferences
    /// editor is open (and empty otherwise).
    skins: Vec<PanelSkin>,
}

impl EditorApp {
    /// Builds the app on an existing egui context.
    ///
    /// The context comes from the engine's egui layer
    /// (`kairos_editor_egui::EguiLayer`), which owns it, installs the image
    /// loaders and wires the repaint callback. The engine belongs to the loop
    /// and is not passed here; it arrives with every UI pass.
    pub fn new(egui_ctx: &egui::Context) -> Self {
        if let Err(error) = kairos_editor_widgets::fonts::install(egui_ctx) {
            kairos_log::warn!(target: "kairos_editor", "UI fonts: {error}");
        }

        // The editor chrome is authored dark-only: the dock's `Style`, the
        // toolbar skin and the widget defaults are all dark, but each panel picks
        // its skin from `ctx.theme()`. Left to follow the desktop, a light system
        // theme paints a white panel inside the dark window (About is where that
        // showed up). Pin the theme before `PanelHost::sync` runs `Panel::init`,
        // which is where every component reads it. The per-component light skins
        // stay in their RON files, dormant until the dock gains one too.
        egui_ctx.set_theme(egui::Theme::Dark);

        let state = boot_state();
        let mut host = PanelHost::new(panels::CATALOG, PanelContext::new(editor_info()));
        host.sync(&state.layout, egui_ctx);
        let dock = default_dock(&host);

        let host_view = AppHost::new();
        let tool_bar = ToolBar::new(egui_ctx);

        // Reaches stderr, the run log and the console sink attached above.
        kairos_log::info!(target: "kairos_editor", "editor booted");

        Self {
            state,
            host,
            dock,
            host_view,
            tool_bar,
            pending_effects: Vec::new(),
            skins: Vec::new(),
        }
    }

    /// Align live instances (open/closed panels), dock tabs and the placement
    /// effects returned by the reducer. Everything is derived from
    /// `LayoutState`, so closing a panel or restoring a layout converges without
    /// special-case code in the UI.
    fn sync(&mut self, ctx: &egui::Context) {
        kairos_profile::scope!(Category::Editor, "Kairos.Editor.Sync");
        self.host.sync(&self.state.layout, ctx);

        let ids: Vec<PanelInstanceId> = self.host.ids().collect();
        for stale in self.dock.tabs().into_iter().filter(|id| !ids.contains(id)) {
            self.dock.remove_tab_by_id(stale);
        }

        // Effects are the only source of new dock tabs: every action that opens
        // a panel returns a placement effect (see LayoutAction::reduce). This
        // replaces the old "append every live instance missing a tab" fallback,
        // which silently invented a placement in the focused leaf.
        {
            let effects = std::mem::take(&mut self.pending_effects);
            let mut executor = EffectExecutor::new(&mut self.host, &mut self.dock);
            for effect in effects {
                executor.apply(effect, ctx);
            }
        }

        // The Preferences editor draws the live panels' field tables. Building
        // them walks every token of every open skin, so snapshots exist only
        // while that panel is open; after the effects, so an edit applied this
        // frame is already visible in the table.
        self.skins = if self.state.layout.is_open(panels::PREFERENCES) {
            self.host.skins()
        } else {
            Vec::new()
        };

        debug_assert!(
            ids.iter().all(|id| self.dock.contains_tab(*id)),
            "a live panel instance has no dock tab; opening a panel must return a layout effect"
        );
    }

    /// dispatch the actions collected during the UI pass, then let the host-side resources record what happened.
    ///
    /// Called only for a committed pass (`!Context::will_discard()`): egui may
    /// run the UI pass twice, and a discarded pass must not apply its actions.
    fn pump(&mut self, mut out: UiOut) {
        kairos_profile::scope!(Category::Editor, "Kairos.Editor.Pump");
        for action in out.take() {
            // A skin drag emits one `SetField` per frame; logging every frame
            // of a gesture would flood the console the way the legacy window
            // did with its per-frame style messages. The commit (the write) is
            // logged; the value itself is visible in the field.
            if !matches!(action, Action::Style(StyleAction::SetField { .. })) {
                kairos_log::info!(target: "kairos_editor", "{}", describe(&action));
            }
            let effects = reduce(&mut self.state, action);
            self.pending_effects.extend(effects);
        }
    }
}

/// Human-readable form of an action, for the editor log (`kairos_log`).
fn describe(action: &Action) -> String {
    match action {
        Action::Layout(action) => match action {
            LayoutAction::TogglePanel { panel } => format!("toggle panel: {panel}"),
            LayoutAction::ShowPanel { panel } => format!("show panel: {panel}"),
            LayoutAction::ClosePanel { panel } => format!("close panel: {panel}"),
            LayoutAction::FocusPanel { panel } => format!("focus panel: {panel}"),
        },
        Action::App(action) => match action {
            AppAction::QuitRequested => "quit editor".to_owned(),
        },
        Action::Style(action) => match action {
            StyleAction::SetField { panel, path, .. } => {
                format!("set {panel} skin: {path}")
            }
            StyleAction::Commit { panel } => format!("save {panel} skin"),
        },
    }
}

impl EditorApp {
    /// One UI pass, independent of the window backend (also used by smoke tests).
    ///
    /// **The frame boundary is not here.** `kairos_engine::runtime::EngineRunner`
    /// ticks the engine once per redraw, before calling the layer; the pass this
    /// function draws belongs to the frame the loop just opened. So multi-pass
    /// egui layouts need no guard: the engine never ticks twice.
    ///
    /// `Profiler::end_frame` carries egui's discard decision — a discarded pass
    /// must not store the frame, and for a second pass the committed one wins.
    ///
    /// The app asks egui for the next repaint immediately: an editor left alone
    /// keeps producing frames at the display rate. That is also what keeps the
    /// engine's wall-clock `Time` in step with the loop — the engine samples its
    /// own delta in `First`, so it only needs the frames to keep coming, never a
    /// timestamp handed in by the host.
    pub(crate) fn ui(&mut self, engine: &Engine, ui: &mut egui::Ui) {
        let committed = {
            kairos_profile::scope!(Category::Editor, "Kairos.Editor.Frame");

            // Editor chrome is not a text document; dragging tabs must not select
            // labels in the panel bodies.
            ui.style_mut().interaction.selectable_labels = false;

            let mut out = UiOut::new();
            self.sync(ui.ctx());

            // Capture is host policy: keep it tied to the visible Profiler tab
            // so closing the tab cannot leave the recorder running with no
            // control surface. Frames already captured stay in the store.
            if !self.state.layout.is_open(panels::PROFILER) && engine.profiler().is_recording() {
                engine.profiler().set_recording(false);
            }

            // Built before the destructure below: it borrows the engine, not the
            // app, so the dock pass can still take `self` apart by field.
            let host_scope = HostScope {
                console: self.host_view.console.clone(),
                profiler: Some(engine.profiler()),
            };

            {
                kairos_profile::scope!(Category::Editor, "Kairos.Editor.Dock");
                let Self {
                    state,
                    host,
                    dock,
                    tool_bar,
                    skins,
                    ..
                } = self;
                let frame = Frame::with_host(state, &host_scope).with_skins(skins);
                tool_bar.ui(ui, &frame, &mut out);
                let mut viewer = host.viewer(&mut out, frame);
                Docker::new("kairos_editor_docker", dock).show(ui, &mut viewer);
            }

            if !ui.ctx().will_discard() {
                self.pump(out);
                // Focus is instance-keyed in `LayoutState`; the dock owns the live
                // instance ids, so the committed frame mirrors them back.
                self.state.layout.set_focus(self.dock.focused_tab());
            }

            !ui.ctx().will_discard()
        };

        engine.profiler().end_frame(committed);

        // Keep the host loop turning whether or not the window receives input:
        // an idle editor must still produce frames (and, when the Profiler tab
        // is recording, record). This is the whole of the frame-pacing policy
        // for now — repaint every frame, capped by vsync.
        ui.ctx().request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use kairos_editor_core::StyleValue;
    use kairos_editor_docking::viewer::TabViewer;

    /// Puffin's frame collector is process-global even though each app owns its
    /// recorder, so only one capture test may run a UI pass while recording.
    static UI_LOCK: Mutex<()> = Mutex::new(());

    fn raw_input() -> egui::RawInput {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::Vec2::new(1280.0, 820.0),
        ));
        input
    }

    /// `PanelHost::sync` and `ToolBar::new` seed missing style files; keep them
    /// in a scratch root instead of the crate directory.
    fn scratch_style_root() -> &'static std::path::Path {
        static DIR: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
        let root = DIR.get_or_init(|| {
            let root = std::env::temp_dir()
                .join(format!("kairos-editor-app-styles-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            let _ = std::fs::create_dir_all(&root);
            root
        });
        kairos_editor_widgets::style::set_style_root(root);
        root
    }

    /// Apply one action the way a committed UI pass does, then run the effects
    /// the way the next frame's `sync` does.
    fn apply(app: &mut EditorApp, action: Action, ctx: &egui::Context) {
        app.pump({
            let mut out = UiOut::new();
            out.push(action);
            out
        });
        app.sync(ctx);
    }

    fn build() -> EditorApp {
        scratch_style_root();

        let state = boot_state();
        let mut host = PanelHost::new(panels::CATALOG, PanelContext::new(editor_info()));
        let ctx = egui::Context::default();
        host.sync(&state.layout, &ctx);
        let dock = default_dock(&host);
        let tool_bar = ToolBar::new(&ctx);
        EditorApp {
            state,
            host,
            dock,
            host_view: AppHost::new(),
            tool_bar,
            pending_effects: Vec::new(),
            skins: Vec::new(),
        }
    }

    /// A light desktop must not paint a white panel inside the dark chrome: the
    /// app pins the egui theme before `PanelHost::sync` runs `Panel::init`, which
    /// is where each panel reads `ctx.theme()` to pick its skin.
    #[test]
    fn a_light_desktop_still_gets_the_dark_editor() {
        scratch_style_root();

        let ctx = egui::Context::default();
        ctx.set_theme(egui::Theme::Light);
        let _app = EditorApp::new(&ctx);

        assert_eq!(ctx.theme(), egui::Theme::Dark);
    }

    #[test]
    fn default_layout_opens_the_five_default_tabs() {
        let app = build();
        assert_eq!(app.host.ids().count(), 5);
        assert_eq!(app.dock.tabs().len(), 5);
    }

    /// The About entry is an ordinary `ShowPanel`: it floats where there is no
    /// dock tab, focuses the same instance when there is one, and closing it
    /// takes the floating surface away again.
    #[test]
    fn about_opens_as_a_floating_window_and_comes_back() {
        let mut app = build();
        let ctx = egui::Context::default();

        let mut out = UiOut::new();
        out.push(Action::Layout(LayoutAction::ShowPanel {
            panel: panels::ABOUT,
        }));
        app.pump(out);
        app.sync(&ctx);

        let about = app.host.id_of(panels::ABOUT).expect("about instance");
        assert!(app.dock.contains_tab(about));
        assert_eq!(
            app.dock.valid_surface_indices().len(),
            2,
            "About floats instead of splitting the dock"
        );
        assert_eq!(
            app.dock.main_surface().tabs_count(),
            5,
            "the workspace layout is untouched"
        );
        assert_eq!(app.dock.focused_tab(), Some(about));

        // A second Show focuses the open instance instead of stacking windows.
        let mut out = UiOut::new();
        out.push(Action::Layout(LayoutAction::ShowPanel {
            panel: panels::ABOUT,
        }));
        app.pump(out);
        app.sync(&ctx);
        assert_eq!(app.dock.valid_surface_indices().len(), 2);
        assert_eq!(app.host.id_of(panels::ABOUT), Some(about));

        // Closing drops the instance's tab; the empty window is removed with it.
        let mut out = UiOut::new();
        out.push(Action::Layout(LayoutAction::ClosePanel {
            panel: panels::ABOUT,
        }));
        app.pump(out);
        app.sync(&ctx);
        assert_eq!(app.host.id_of(panels::ABOUT), None);
        assert_eq!(app.dock.valid_surface_indices().len(), 1);
    }

    #[test]
    fn draw_pipeline_runs_end_to_end() {
        let _guard = UI_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let mut app = build();
        let engine = Engine::headless();
        let ctx = egui::Context::default();

        let _ = ctx.run_ui(raw_input(), |ui| app.ui(&engine, ui));

        assert_eq!(app.host.ids().count(), 5);
        assert_eq!(app.dock.tabs().len(), 5);
    }

    #[test]
    fn editor_frames_are_captured_when_recording() {
        let _guard = UI_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let mut app = build();
        let mut engine = Engine::headless();
        engine.profiler().set_recording(true);

        let ctx = egui::Context::default();
        // The loop ticks the engine and then draws: `update` opens the frame,
        // the UI pass below belongs to it, and the next `update` flushes it
        // into the store.
        engine.update();
        let _ = ctx.run_ui(raw_input(), |ui| app.ui(&engine, ui));
        engine.update();

        let profiler = engine.profiler();
        let session = profiler.session();
        assert!(
            !session.frames.is_empty(),
            "a committed editor pass is flushed at the next frame boundary"
        );
        let has_scope = |name: &str| {
            session.frames.iter().any(|frame| {
                frame.threads.iter().any(|thread| {
                    thread
                        .samples
                        .iter()
                        .any(|sample| session.scope_name(sample.scope) == name)
                })
            })
        };
        assert!(
            has_scope("Kairos.Engine.Frame"),
            "the shell ticks the engine inside the recorded frame"
        );
        assert!(
            has_scope("Kairos.Stage"),
            "the engine's schedule stages are captured too"
        );
        assert!(
            has_scope("Kairos.Editor.Frame"),
            "the UI pass belongs to the same engine frame"
        );

        engine.profiler().set_recording(false);
        engine.profiler().clear();
    }

    #[test]
    fn pump_applies_layout_actions() {
        let mut app = build();

        let mut out = UiOut::new();
        out.push(Action::Layout(LayoutAction::TogglePanel {
            panel: panels::CONSOLE,
        }));
        app.pump(out);

        assert!(!app.state.layout.is_open(panels::CONSOLE));

        app.sync(&egui::Context::default());
        assert_eq!(app.host.ids().count(), 4);
        assert_eq!(app.dock.tabs().len(), 4);
    }

    #[test]
    fn actions_have_human_readable_log_lines() {
        assert_eq!(
            describe(&Action::Layout(LayoutAction::TogglePanel {
                panel: panels::CONSOLE
            })),
            "toggle panel: console"
        );
        assert_eq!(
            describe(&Action::Layout(LayoutAction::ClosePanel {
                panel: panels::SCENE
            })),
            "close panel: scene"
        );
        assert_eq!(
            describe(&Action::App(AppAction::QuitRequested)),
            "quit editor"
        );
        assert_eq!(
            describe(&Action::Style(StyleAction::SetField {
                panel: panels::CONSOLE,
                path: "toolbar_height".to_owned(),
                value: StyleValue::Float(30.0),
            })),
            "set console skin: toolbar_height"
        );
        assert_eq!(
            describe(&Action::Style(StyleAction::Commit {
                panel: panels::CONSOLE
            })),
            "save console skin"
        );
    }

    /// Quit is host work: the toolbar only *requests* it, the reducer answers
    /// with an app effect, and the effect executor turns that into the close
    /// command the backend reads as `FrameOutput::close_requested`.
    #[test]
    fn quitting_asks_the_window_to_close() {
        let _guard = UI_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let mut app = build();
        let engine = Engine::headless();

        let mut out = UiOut::new();
        out.push(Action::App(AppAction::QuitRequested));
        app.pump(out);

        // Effects run at the start of the next pass, inside it, so the close
        // request belongs to that pass's viewport output.
        let ctx = egui::Context::default();
        let output = ctx.run_ui(raw_input(), |ui| app.ui(&engine, ui));

        let close_requested = output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .is_some_and(|viewport| {
                viewport
                    .commands
                    .iter()
                    .any(|command| matches!(command, egui::ViewportCommand::Close))
            });
        assert!(
            close_requested,
            "the quit action must reach the backend as a close request"
        );
    }

    #[test]
    fn host_view_exposes_the_engine_profiler() {
        let engine = Engine::headless();
        let host = AppHost::new();
        let scope = HostScope {
            console: host.console.clone(),
            profiler: Some(engine.profiler()),
        };
        assert!(
            scope.profiler().is_some(),
            "the pass borrows the loop engine's recorder"
        );
    }

    #[test]
    fn host_view_exposes_the_console_history() {
        let host = AppHost::new();
        let scope = HostScope {
            console: host.console.clone(),
            profiler: None,
        };
        host.console.push(&kairos_log::Record::new(
            kairos_log::Level::Info,
            "test",
            "hello console",
        ));

        let mut seen = Vec::new();
        scope.console_lines(&mut |lines| seen = lines.to_vec());

        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].message, "hello console");
        assert_eq!(seen[0].target, "test");
    }

    #[test]
    fn closing_the_profiler_tab_stops_recording() {
        let _guard = UI_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let mut app = build();
        let engine = Engine::headless();
        engine.profiler().set_recording(true);

        let ctx = egui::Context::default();
        let _ = ctx.run_ui(raw_input(), |ui| app.ui(&engine, ui));
        assert!(
            engine.profiler().is_recording(),
            "the first pass starts the capture"
        );

        app.state.layout.close(panels::PROFILER);
        let _ = ctx.run_ui(raw_input(), |ui| app.ui(&engine, ui));

        assert!(
            !engine.profiler().is_recording(),
            "closing the Profiler tab stops the host capture"
        );
        engine.profiler().clear();
    }

    #[test]
    fn scene_panel_can_be_detached_into_a_window() {
        let mut app = build();
        let id = app.host.id_of(panels::SCENE).expect("scene instance");
        let engine = Engine::headless();
        let host_scope = HostScope {
            console: app.host_view.console.clone(),
            profiler: Some(engine.profiler()),
        };

        let mut out = UiOut::new();
        {
            let EditorApp { state, host, .. } = &mut app;
            let frame = Frame::with_host(state, &host_scope);
            let mut viewer = host.viewer(&mut out, frame);
            assert!(
                viewer.allowed_in_windows(id),
                "Scene must be draggable into a floating window"
            );
        }
        assert!(out.is_empty());
    }

    #[test]
    fn showing_a_docked_panel_focuses_its_tab() {
        let mut app = build();
        let ctx = egui::Context::default();
        let scene = app.host.id_of(panels::SCENE).expect("scene instance");
        let console = app.host.id_of(panels::CONSOLE).expect("console instance");

        assert!(app.dock.focus_tab(scene));
        assert_eq!(app.dock.focused_tab(), Some(scene));

        let mut out = UiOut::new();
        out.push(Action::Layout(LayoutAction::ShowPanel {
            panel: panels::CONSOLE,
        }));
        app.pump(out);
        app.sync(&ctx);

        assert_eq!(app.dock.focused_tab(), Some(console));
        assert_eq!(
            app.dock.valid_surface_indices().len(),
            1,
            "a panel that already has a tab must not become a second window"
        );
    }

    #[test]
    fn showing_a_panel_without_a_dock_tab_floats_it() {
        let mut app = build();
        let ctx = egui::Context::default();
        let profiler = app.host.id_of(panels::PROFILER).expect("profiler instance");

        app.state.layout.close(panels::PROFILER);
        app.sync(&ctx);
        assert_eq!(app.dock.tabs().len(), 4);
        assert_eq!(app.dock.valid_surface_indices().len(), 1);

        let mut out = UiOut::new();
        out.push(Action::Layout(LayoutAction::ShowPanel {
            panel: panels::PROFILER,
        }));
        app.pump(out);
        app.sync(&ctx);

        let reopened = app.host.id_of(panels::PROFILER).expect("profiler reopened");
        assert_ne!(reopened, profiler, "reopening creates a new instance");
        assert_eq!(app.dock.tabs().len(), 5);
        assert_eq!(app.dock.valid_surface_indices().len(), 2);
        assert_eq!(app.dock.focused_tab(), Some(reopened));
        assert_eq!(
            app.dock.main_surface().tabs_count(),
            4,
            "the reopened panel is a floating window, not a tab in the main surface"
        );
    }

    #[test]
    fn focusing_a_panel_without_a_dock_tab_docks_it() {
        let mut app = build();
        let ctx = egui::Context::default();

        app.state.layout.close(panels::PROFILER);
        app.sync(&ctx);

        let mut out = UiOut::new();
        out.push(Action::Layout(LayoutAction::FocusPanel {
            panel: panels::PROFILER,
        }));
        app.pump(out);
        app.sync(&ctx);

        let reopened = app.host.id_of(panels::PROFILER).expect("profiler reopened");
        assert_eq!(app.dock.tabs().len(), 5);
        assert_eq!(
            app.dock.valid_surface_indices().len(),
            1,
            "Focus docks a missing panel instead of opening a window"
        );
        assert_eq!(app.dock.focused_tab(), Some(reopened));
        assert_eq!(app.dock.main_surface().tabs_count(), 5);
    }

    #[test]
    fn close_and_reopen_a_panel_converges_instance_and_tab() {
        let mut app = build();
        let ctx = egui::Context::default();

        app.state.layout.close(panels::CONSOLE);
        app.sync(&ctx);
        assert_eq!(app.host.ids().count(), 4);
        assert_eq!(app.dock.tabs().len(), 4);

        let mut out = UiOut::new();
        out.push(Action::Layout(LayoutAction::TogglePanel {
            panel: panels::CONSOLE,
        }));
        app.pump(out);
        app.sync(&ctx);

        assert_eq!(app.host.ids().count(), 5);
        assert_eq!(app.dock.tabs().len(), 5);
        assert_eq!(
            app.dock.focused_tab(),
            app.host.id_of(panels::CONSOLE),
            "opening via Toggle docks and focuses the panel"
        );
    }

    /// Preferences is an ordinary panel: the layout reducer opens it and
    /// ShowPanel floats it, exactly like About.
    #[test]
    fn preferences_opens_as_a_floating_window() {
        let mut app = build();
        let ctx = egui::Context::default();

        apply(
            &mut app,
            Action::Layout(LayoutAction::ShowPanel {
                panel: panels::PREFERENCES,
            }),
            &ctx,
        );

        let id = app
            .host
            .id_of(panels::PREFERENCES)
            .expect("preferences instance");
        assert!(app.dock.contains_tab(id));
        assert_eq!(app.dock.valid_surface_indices().len(), 2);
        assert_eq!(app.dock.focused_tab(), Some(id));
    }

    /// Opening Preferences makes the app snapshot the live panels' field tables;
    /// they are the rows the editor draws.
    #[test]
    fn opening_preferences_snapshots_the_live_skins() {
        let mut app = build();
        let ctx = egui::Context::default();
        assert!(app.skins.is_empty(), "nothing to draw while closed");

        apply(
            &mut app,
            Action::Layout(LayoutAction::ShowPanel {
                panel: panels::PREFERENCES,
            }),
            &ctx,
        );

        assert!(
            app.skins.iter().any(|skin| skin.panel == panels::CONSOLE),
            "the open console is editable"
        );
        assert!(
            app.skins
                .iter()
                .any(|skin| skin.panel == panels::PREFERENCES),
            "the editor can edit its own skin"
        );
        assert!(
            !app.skins.iter().any(|skin| skin.panel == panels::ABOUT),
            "About is not open, so it has no live style to edit"
        );
    }

    /// A field edit is host work: the executor writes it into the owning
    /// panel's live style, and the next snapshot shows the new value.
    #[test]
    fn a_skin_edit_reaches_the_owning_panel() {
        let mut app = build();
        let ctx = egui::Context::default();

        apply(
            &mut app,
            Action::Style(StyleAction::SetField {
                panel: panels::CONSOLE,
                path: "toolbar_height".to_owned(),
                value: StyleValue::Float(30.0),
            }),
            &ctx,
        );

        let console = app
            .host
            .skins()
            .into_iter()
            .find(|skin| skin.panel == panels::CONSOLE)
            .expect("console skin");
        let height = console
            .fields
            .iter()
            .find(|field| field.path == "toolbar_height")
            .expect("toolbar_height");
        assert_eq!(height.value, StyleValue::Float(30.0));
    }

    /// The save boundary: an edit alone writes nothing; the commit writes the
    /// panel's RON file with the new value.
    #[test]
    fn a_skin_commit_writes_the_panel_file() {
        let mut app = build();
        let dir = scratch_style_root().to_path_buf();
        let ctx = egui::Context::default();
        let file = dir.join("ConsoleStyle.ron");

        apply(
            &mut app,
            Action::Style(StyleAction::SetField {
                panel: panels::CONSOLE,
                path: "toolbar_height".to_owned(),
                value: StyleValue::Float(30.0),
            }),
            &ctx,
        );
        let before = std::fs::read_to_string(&file).unwrap_or_default();
        assert!(
            !before.contains("toolbar_height: 30.0"),
            "a field edit must not write the file: {before}"
        );

        apply(
            &mut app,
            Action::Style(StyleAction::Commit {
                panel: panels::CONSOLE,
            }),
            &ctx,
        );

        let text = std::fs::read_to_string(&file).expect("the commit writes the skin");
        assert!(text.contains("toolbar_height: 30.0"), "{text}");
    }
}
