use std::{cell::RefCell, rc::Rc};

use kairos_editor_docking::{docker::{DockState, Docker}, split::Fraction, tab::{Tab, TabState}, tree::NodeIndex};
use kairos_editor_widgets::theme::Theme;


pub struct EditorState {
    pub dock_state: DockState,
}

impl EditorState {
    pub fn new() -> Self {
        Self {
            dock_state: DockState::new(),
        }
    }
}

pub struct EditorApp {
    state: EditorState,
}

struct InspectorTab {

}
struct SceneTab {

}
struct ConsoleTab {

}
struct HierarchyTab {

}
impl Tab for InspectorTab {
    fn title(&self) -> egui::WidgetText {
        "Inspector".into()
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.label("Inspector");
        });
    }
}
impl Tab for SceneTab {
    fn title(&self) -> egui::WidgetText {
        "Scene".into()
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.label("Scene");
        });
    }
}
impl Tab for ConsoleTab {
    fn title(&self) -> egui::WidgetText {
        "Console".into()
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.label("Console");
        });
    }
}
impl Tab for HierarchyTab {
    fn title(&self) -> egui::WidgetText {
        "Hierarchy".into()
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.label("Hierarchy");
        });
    }
}

impl EditorApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Theme::dark().install(&cc.egui_ctx);

        let mut state = EditorState::new();
        let [left, _] = state.dock_state.main_surface_mut().split_right(NodeIndex::root(), Fraction::new(0.8), vec![TabState::new(Rc::new(RefCell::new(InspectorTab {})))]);
        state.dock_state.main_surface_mut()[left].append_tab(TabState::new(Rc::new(RefCell::new(SceneTab {}))));
        let [top, _] = state.dock_state.main_surface_mut().split_bottom(left, Fraction::new(0.7), vec![TabState::new(Rc::new(RefCell::new(ConsoleTab {})))]);
        state.dock_state.main_surface_mut().split_left(top, Fraction::new(0.2), vec![TabState::new(Rc::new(RefCell::new(HierarchyTab {})))]);

        Self {
            state,
        }
    }
}

impl eframe::App for EditorApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        Docker::new("kairos_editor_docker", &mut self.state.dock_state).show(ui);
    }
}
