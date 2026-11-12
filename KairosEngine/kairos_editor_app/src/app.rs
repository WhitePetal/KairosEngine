use eframe::egui;
use kairos_editor_component::{PanelHost, PanelInstanceId, PanelRegistry, panels};
use kairos_editor_core::EditorState;
use kairos_editor_docking::{
    docker::{DockState, Docker},
    split::Fraction,
    tree::NodeIndex,
};

use crate::consts::VERSION;

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

        Self { state, host, dock }
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

    fn show_tool_bar(&mut self, ui: &mut egui::Ui) {
        let kinds: Vec<_> = self.host.kinds().collect();
        let mut toggles = Vec::new();

        egui::Panel::top("editor-top-bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Kairos");
                ui.separator();
                for kind in &kinds {
                    let open = self.state.layout.is_open(*kind);
                    if ui.selectable_label(open, kind.0).clicked() {
                        toggles.push(*kind);
                    }
                }
                ui.separator();
                ui.label(format!("counter = {}", self.state.counter));
                if let Some(focus) = self.state.layout.focus() {
                    ui.label(format!("focus: {}#{}", focus.kind, focus.serial));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("v{VERSION}"));
                });
            });
        });

        for kind in toggles {
            self.state.layout.toggle(kind);
        }
    }
}

impl EditorApp {
    /// One frame, independent of the window backend (also used by smoke tests).
    fn ui(&mut self, ui: &mut egui::Ui) {
        // Editor chrome is not a text document; dragging tabs must not select
        // labels in the panel bodies.
        ui.style_mut().interaction.selectable_labels = false;
        self.show_tool_bar(ui);
        self.sync();

        let Self { state, host, dock } = self;
        {
            let mut viewer = host.viewer(state);
            Docker::new("kairos_editor_docker", dock).show(ui, &mut viewer);
        }

        state.layout.set_focus(dock.focused_tab());
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
        EditorApp { state, host, dock }
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
    fn scene_panel_can_be_detached_into_a_window() {
        let mut app = build();
        let id = app.host.id_of(panels::SCENE).expect("scene instance");
        let EditorApp { state, host, .. } = &mut app;
        let mut viewer = host.viewer(state);
        assert!(
            viewer.allowed_in_windows(id),
            "Scene must be draggable into a floating window"
        );
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
