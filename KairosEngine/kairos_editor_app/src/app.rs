use std::sync::Arc;

use eframe::egui;
use kairos_editor_component::{
    ConsoleLine, ConsoleLog, Frame, HostView, PanelHost, PanelInstanceId, PanelRegistry, UiOut,
    panels, toolbar::ToolBar,
};
use kairos_editor_core::{Action, EditorState, LayoutAction, reduce};
use kairos_editor_docking::{
    docker::{DockState, Docker},
    split::Fraction,
    tree::NodeIndex,
};
use kairos_log::SinkHandle;
use kairos_profile::Profiler;

use crate::consts::VERSION;

/// Host-owned resources a panel may display but never own (the prototype's
/// `HostState`). The console history is a `kairos_log` sink: the process logger
/// writes into it, the Console panel reads it through `HostView`, and neither
/// owns it. It is not in `EditorState` because it is a host stream, not
/// serializable state.
///
/// The profiler is the second such resource: the process owns the capture store,
/// the app drives its frame boundary (`kairos_profile::begin_frame` /
/// `end_frame`), and the Profiler panel reads it through `HostView::profiler`.
struct AppHost {
    console: Arc<ConsoleLog>,
    /// Keeps the console attached to the process logger; dropping the app
    /// detaches the sink.
    _console_sink: SinkHandle,
    /// Process-wide capture store. Initialised here, before the first frame, so
    /// the Profiler panel has a sink to read from as soon as it opens.
    profiler: &'static Profiler,
}

impl AppHost {
    fn new() -> Self {
        let console = Arc::new(ConsoleLog::new());
        let console_sink = kairos_log::attach(console.clone());
        Self {
            console,
            _console_sink: console_sink,
            profiler: Profiler::global(),
        }
    }
}

impl HostView for AppHost {
    fn console_lines(&self, f: &mut dyn FnMut(&[ConsoleLine])) {
        self.console.with_lines(f);
    }

    fn profiler(&self) -> Option<&Profiler> {
        Some(self.profiler)
    }
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
}

impl EditorApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // egui's async image loaders back every runtime UI image: icons are
        // plain files under `Preferences/Textures` and panels start loading
        // them in `Panel::init`, not here.
        egui_extras::install_image_loaders(&cc.egui_ctx);
        if let Err(error) = kairos_editor_widgets::fonts::install(&cc.egui_ctx) {
            kairos_log::warn!(target: "kairos_editor", "UI fonts: {error}");
        }

        let mut registry = PanelRegistry::new();
        panels::register_default(&mut registry);

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

        let mut host = PanelHost::new(registry);
        host.sync(&state.layout, &cc.egui_ctx);
        let dock = default_dock(&host);

        let host_view = AppHost::new();

        let tool_bar = ToolBar::new(&cc.egui_ctx);

        // Reaches stderr, the run log and the console sink attached above.
        kairos_log::info!(target: "kairos_editor", "editor booted");

        Self {
            state,
            host,
            dock,
            host_view,
            tool_bar,
        }
    }

    /// Align live instances (open/closed panels) and dock tabs. Both are
    /// derived from `LayoutState`, so closing a panel or restoring a layout
    /// converges without special-case code in the UI.
    fn sync(&mut self, ctx: &egui::Context) {
        kairos_profile::scope!("Kairos.Editor.Sync");
        self.host.sync(&self.state.layout, ctx);

        let ids: Vec<PanelInstanceId> = self.host.ids().collect();
        for stale in self.dock.tabs().into_iter().filter(|id| !ids.contains(id)) {
            self.dock.remove_tab_by_id(stale);
        }
        for id in ids {
            if !self.dock.contains_tab(id) {
                self.dock.append_tab(id);
            }
        }
    }

    /// Toolbar is host chrome, not a panel: it reads state and pushes actions,
    /// exactly like a panel body would.
    fn show_tool_bar(&self, ui: &mut egui::Ui, out: &mut UiOut) {
        kairos_profile::scope!("Kairos.Editor.ToolBar");
        let kinds: Vec<_> = self.host.kinds().collect();

        egui::Panel::top("editor-top-bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Kairos");
                ui.separator();
                for kind in &kinds {
                    let open = self.state.layout.is_open(*kind);
                    if ui.selectable_label(open, kind.0).clicked() {
                        out.push(Action::Layout(LayoutAction::TogglePanel { panel: *kind }));
                    }
                }
                ui.separator();
                // No focus debug label here: it only exists on some passes and
                // would shift egui's auto-id bookkeeping between passes.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("v{VERSION}"));
                });
            });
        });
    }

    /// dispatch the actions collected during the UI pass, then let the host-side resources record what happened.
    ///
    /// Called only for a committed pass (`!Context::will_discard()`): egui may
    /// run the UI pass twice, and a discarded pass must not apply its actions.
    fn pump(&mut self, mut out: UiOut) {
        kairos_profile::scope!("Kairos.Editor.Pump");
        for action in out.take() {
            kairos_log::info!(target: "kairos_editor", "{}", describe(&action));
            reduce(&mut self.state, action);
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
    }
}

impl EditorApp {
    /// One frame, independent of the window backend (also used by smoke tests).
    ///
    /// The profiling frame boundary wraps the whole pass. `begin_frame` flushes
    /// the *previous* pass (puffin reports a thread's stream only after the
    /// pass's outermost scope closes), and `end_frame` carries egui's discard
    /// decision so a multi-pass layout never captures the throwaway pass.
    fn ui(&mut self, ui: &mut egui::Ui) {
        kairos_profile::begin_frame();

        let committed = {
            kairos_profile::scope!("Kairos.Editor.Frame");

            // Editor chrome is not a text document; dragging tabs must not select
            // labels in the panel bodies.
            ui.style_mut().interaction.selectable_labels = false;

            let mut out = UiOut::new();
            // self.show_tool_bar(ui, &mut out);
            self.sync(ui.ctx());

            // Capture is host policy: keep it tied to the visible Profiler tab
            // so closing the tab cannot leave the recorder running with no
            // control surface. Frames already captured stay in the store.
            if !self.state.layout.is_open(panels::PROFILER) && Profiler::global().is_recording() {
                Profiler::global().set_recording(false);
            }

            {
                kairos_profile::scope!("Kairos.Editor.Dock");
                let Self {
                    state,
                    host,
                    dock,
                    host_view,
                    tool_bar,
                } = self;
                let frame = Frame::with_host(state, &*host_view);
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

        kairos_profile::end_frame(committed);
    }
}

impl eframe::App for EditorApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.ui(ui);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use kairos_editor_docking::viewer::TabViewer;

    /// The profiler test drives the real global recorder; only one test may run
    /// a UI pass at a time while it is recording.
    static UI_LOCK: Mutex<()> = Mutex::new(());

    fn raw_input() -> egui::RawInput {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::Vec2::new(1280.0, 820.0),
        ));
        input
    }

    fn build() -> EditorApp {
        let mut registry = PanelRegistry::new();
        panels::register_default(&mut registry);
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
        let mut host = PanelHost::new(registry);
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
        }
    }

    #[test]
    fn default_layout_opens_the_five_default_tabs() {
        let app = build();
        assert_eq!(app.host.ids().count(), 5);
        assert_eq!(app.dock.tabs().len(), 5);
    }

    #[test]
    fn draw_pipeline_runs_end_to_end() {
        let _guard = UI_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let mut app = build();
        let ctx = egui::Context::default();

        let _ = ctx.run_ui(raw_input(), |ui| app.ui(ui));

        assert_eq!(app.host.ids().count(), 5);
        assert_eq!(app.dock.tabs().len(), 5);
    }

    #[test]
    fn editor_frames_are_captured_when_recording() {
        let _guard = UI_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let mut app = build();
        let profiler = Profiler::global();
        profiler.set_recording(true);

        let ctx = egui::Context::default();
        // Frame 1 is recorded between begin_frame and end_frame; the next
        // begin_frame flushes it into the store.
        let _ = ctx.run_ui(raw_input(), |ui| app.ui(ui));
        let _ = ctx.run_ui(raw_input(), |ui| app.ui(ui));

        let session = profiler.session();
        assert!(
            !session.frames.is_empty(),
            "a committed editor pass is flushed at the next frame boundary"
        );
        let has_editor_frame = session.frames.iter().any(|frame| {
            frame.threads.iter().any(|thread| {
                thread
                    .samples
                    .iter()
                    .any(|sample| session.scope_name(sample.scope) == "Kairos.Editor.Frame")
            })
        });
        assert!(has_editor_frame, "Kairos.Editor.Frame is in the capture");

        profiler.set_recording(false);
        profiler.clear();
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
    }

    #[test]
    fn host_view_exposes_the_profiler() {
        let host = AppHost::new();
        assert!(
            host.profiler().is_some(),
            "the app installs the process profiler before the first frame"
        );
    }

    #[test]
    fn host_view_exposes_the_console_history() {
        let host = AppHost::new();
        host.console.push(&kairos_log::Record::new(
            kairos_log::Level::Info,
            "test",
            "hello console",
        ));

        let mut seen = Vec::new();
        host.console_lines(&mut |lines| seen = lines.to_vec());

        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].message, "hello console");
        assert_eq!(seen[0].target, "test");
    }

    #[test]
    fn closing_the_profiler_tab_stops_recording() {
        let _guard = UI_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let mut app = build();
        let profiler = Profiler::global();
        profiler.set_recording(true);

        let ctx = egui::Context::default();
        let _ = ctx.run_ui(raw_input(), |ui| app.ui(ui));
        assert!(profiler.is_recording(), "the first pass starts the capture");

        app.state.layout.close(panels::PROFILER);
        let _ = ctx.run_ui(raw_input(), |ui| app.ui(ui));

        assert!(
            !profiler.is_recording(),
            "closing the Profiler tab stops the host capture"
        );
        profiler.clear();
    }

    #[test]
    fn scene_panel_can_be_detached_into_a_window() {
        let mut app = build();
        let id = app.host.id_of(panels::SCENE).expect("scene instance");

        let mut out = UiOut::new();
        {
            let EditorApp {
                state,
                host,
                host_view,
                ..
            } = &mut app;
            let frame = Frame::with_host(state, &*host_view);
            let mut viewer = host.viewer(&mut out, frame);
            assert!(
                viewer.allowed_in_windows(id),
                "Scene must be draggable into a floating window"
            );
        }
        assert!(out.is_empty());
    }

    #[test]
    fn close_and_reopen_a_panel_converges_instance_and_tab() {
        let mut app = build();
        let ctx = egui::Context::default();

        app.state.layout.close(panels::CONSOLE);
        app.sync(&ctx);
        assert_eq!(app.host.ids().count(), 4);
        assert_eq!(app.dock.tabs().len(), 4);

        app.state.layout.open(panels::CONSOLE);
        app.sync(&ctx);
        assert_eq!(app.host.ids().count(), 5);
        assert_eq!(app.dock.tabs().len(), 5);
    }
}
