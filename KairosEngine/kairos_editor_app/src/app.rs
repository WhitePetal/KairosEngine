use std::sync::Arc;

use eframe::egui;
use kairos_editor_component::{
    ConsoleLine, ConsoleLog, Frame, HostView, PanelHost, PanelInstanceId, PanelRegistry, UiOut,
    panels,
};
use kairos_editor_core::{Action, EditorState, LayoutAction, reduce};
use kairos_editor_docking::{
    docker::{DockState, Docker},
    split::Fraction,
    tree::NodeIndex,
};
use kairos_log::SinkHandle;

use crate::consts::VERSION;

/// Host-owned resources a panel may display but never own (the prototype's
/// `HostState`). The console history is a `kairos_log` sink: the process logger
/// writes into it, the Console panel reads it through `HostView`, and neither
/// owns it. It is not in `EditorState` because it is a host stream, not
/// serializable state.
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

impl HostView for AppHost {
    fn console_lines(&self, f: &mut dyn FnMut(&[ConsoleLine])) {
        self.console.with_lines(f);
    }
}

/// Build the first-run layout from the ids the host created for the four
/// default panels. User drags/splits after this are pure dock-state changes.
fn default_dock(host: &PanelHost) -> DockState<PanelInstanceId> {
    let scene = host.id_of(panels::SCENE).expect("scene instance");
    let inspector = host.id_of(panels::INSPECTOR).expect("inspector instance");
    let console = host.id_of(panels::CONSOLE).expect("console instance");
    let hierarchy = host.id_of(panels::HIERARCHY).expect("hierarchy instance");

    let mut dock: DockState<PanelInstanceId> = DockState::new();
    let [scene_leaf, _inspector_leaf] = dock.main_surface_mut().split_right(
        NodeIndex::root(),
        Fraction::new(0.78),
        vec![inspector],
    );
    dock.main_surface_mut()[scene_leaf].append_tab(scene);
    let [top, _bottom] =
        dock.main_surface_mut()
            .split_bottom(scene_leaf, Fraction::new(0.7), vec![console]);
    dock.main_surface_mut()
        .split_left(top, Fraction::new(0.22), vec![hierarchy]);
    dock
}

pub struct EditorApp {
    state: EditorState,
    host: PanelHost,
    dock: DockState<PanelInstanceId>,
    host_view: AppHost,
}

impl EditorApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        kairos_editor_widgets::theme::Theme::dark().install(&cc.egui_ctx);

        let mut registry = PanelRegistry::new();
        panels::register_default(&mut registry);

        let mut state = EditorState::new();
        for kind in [
            panels::SCENE,
            panels::INSPECTOR,
            panels::CONSOLE,
            panels::HIERARCHY,
        ] {
            state.layout.open(kind);
        }

        let mut host = PanelHost::new(registry);
        host.sync(&state.layout);
        let dock = default_dock(&host);

        let host_view = AppHost::new();
        // Reaches stderr, the run log and the console sink attached above.
        kairos_log::info!(target: "kairos_editor", "editor booted");

        Self {
            state,
            host,
            dock,
            host_view,
        }
    }

    /// Align live instances (open/closed panels) and dock tabs. Both are
    /// derived from `LayoutState`, so closing a panel or restoring a layout
    /// converges without special-case code in the UI.
    fn sync(&mut self) {
        self.host.sync(&self.state.layout);

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

    /// Phases 3+4 of the prototype pipeline: dispatch the actions collected
    /// during the UI pass, then let the host-side resources record what happened.
    ///
    /// Called only for a committed pass (`!Context::will_discard()`): egui may
    /// run the UI pass twice, and a discarded pass must not apply its actions.
    fn pump(&mut self, mut out: UiOut) {
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
    fn ui(&mut self, ui: &mut egui::Ui) {
        // Editor chrome is not a text document; dragging tabs must not select
        // labels in the panel bodies.
        ui.style_mut().interaction.selectable_labels = false;

        let mut out = UiOut::new();
        self.show_tool_bar(ui, &mut out);
        self.sync();

        {
            let Self {
                state,
                host,
                dock,
                host_view,
            } = self;
            let frame = Frame::with_host(state, &*host_view);
            let mut viewer = host.viewer(&mut out, frame);
            Docker::new("kairos_editor_docker", dock).show(ui, &mut viewer);
        }

        if !ui.ctx().will_discard() {
            self.pump(out);
            // Focus is instance-keyed in `LayoutState`; the dock owns the live
            // instance ids, so the committed frame mirrors them back.
            self.state.layout.set_focus(self.dock.focused_tab());
        }
    }
}

impl eframe::App for EditorApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.ui(ui);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kairos_editor_docking::viewer::TabViewer;

    fn build() -> EditorApp {
        let mut registry = PanelRegistry::new();
        panels::register_default(&mut registry);
        let mut state = EditorState::new();
        for kind in [
            panels::SCENE,
            panels::INSPECTOR,
            panels::CONSOLE,
            panels::HIERARCHY,
        ] {
            state.layout.open(kind);
        }
        let mut host = PanelHost::new(registry);
        host.sync(&state.layout);
        let dock = default_dock(&host);
        EditorApp {
            state,
            host,
            dock,
            host_view: AppHost::new(),
        }
    }

    #[test]
    fn default_layout_opens_the_four_default_tabs() {
        let app = build();
        assert_eq!(app.host.ids().count(), 4);
        assert_eq!(app.dock.tabs().len(), 4);
    }

    #[test]
    fn draw_pipeline_runs_end_to_end() {
        let mut app = build();
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::Vec2::new(1280.0, 820.0),
        ));

        let _ = ctx.run_ui(input, |ui| app.ui(ui));

        assert_eq!(app.host.ids().count(), 4);
        assert_eq!(app.dock.tabs().len(), 4);
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

        app.sync();
        assert_eq!(app.host.ids().count(), 3);
        assert_eq!(app.dock.tabs().len(), 3);
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

        app.state.layout.close(panels::CONSOLE);
        app.sync();
        assert_eq!(app.host.ids().count(), 3);
        assert_eq!(app.dock.tabs().len(), 3);

        app.state.layout.open(panels::CONSOLE);
        app.sync();
        assert_eq!(app.host.ids().count(), 4);
        assert_eq!(app.dock.tabs().len(), 4);
    }
}
