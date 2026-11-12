use egui::{Ui, WidgetText};

use crate::{
    panel::{Panel, PanelContext, PanelId, PanelRegistry},
    state::EditorState,
};

pub const SCENE: PanelId = PanelId("scene");
pub const INSPECTOR: PanelId = PanelId("inspector");
pub const CONSOLE: PanelId = PanelId("console");
pub const HIERARCHY: PanelId = PanelId("hierarchy");
pub const PROJECT: PanelId = PanelId("project");

pub fn register_all(registry: &mut PanelRegistry) {
    registry.register(SCENE, || Box::new(ScenePanel));
    registry.register(INSPECTOR, || Box::new(InspectorPanel));
    registry.register(CONSOLE, || Box::new(ConsolePanel));
    registry.register(HIERARCHY, || Box::new(HierarchyPanel));
    registry.register(PROJECT, || Box::new(ProjectPanel));
}

struct ScenePanel;

impl Panel for ScenePanel {
    fn kind(&self) -> PanelId {
        SCENE
    }

    fn title(&self, _state: &EditorState) -> WidgetText {
        "Scene".into()
    }

    fn ui(&mut self, ui: &mut Ui, _ctx: &mut PanelContext) {
        ui.centered_and_justified(|ui| {
            ui.label("3D viewport placeholder");
        });
    }
}

struct InspectorPanel;

impl Panel for InspectorPanel {
    fn kind(&self) -> PanelId {
        INSPECTOR
    }

    fn title(&self, _state: &EditorState) -> WidgetText {
        "Inspector".into()
    }

    fn ui(&mut self, ui: &mut Ui, ctx: &mut PanelContext) {
        ui.heading("Inspector");
        ui.label(format!("selected project: {}", ctx.state.project_name));
        ui.separator();
        if ui.button("Increment shared counter").clicked() {
            ctx.state.counter += 1;
        }
        ui.label(format!("counter = {}", ctx.state.counter));
    }
}

struct ConsolePanel;

impl Panel for ConsolePanel {
    fn kind(&self) -> PanelId {
        CONSOLE
    }

    fn title(&self, _state: &EditorState) -> WidgetText {
        "Console".into()
    }

    fn ui(&mut self, ui: &mut Ui, ctx: &mut PanelContext) {
        ui.heading("Console");
        for line in &ctx.state.log {
            ui.monospace(line);
        }
        if ui.button("Push log line").clicked() {
            let n = ctx.state.log.len() + 1;
            ctx.state.log.push(format!("log line {n}"));
        }
    }
}

struct HierarchyPanel;

impl Panel for HierarchyPanel {
    fn kind(&self) -> PanelId {
        HIERARCHY
    }

    fn title(&self, _state: &EditorState) -> WidgetText {
        "Hierarchy".into()
    }

    fn ui(&mut self, ui: &mut Ui, _ctx: &mut PanelContext) {
        ui.heading("Hierarchy");
        for item in ["World", "Main Camera", "Editor Camera", "Sun"] {
            ui.label(item);
        }
    }
}

struct ProjectPanel;

impl Panel for ProjectPanel {
    fn kind(&self) -> PanelId {
        PROJECT
    }

    fn title(&self, state: &EditorState) -> WidgetText {
        format!("Project — {}", state.project_name).into()
    }

    fn ui(&mut self, ui: &mut Ui, ctx: &mut PanelContext) {
        ui.heading("Project");
        ui.label(format!("name: {}", ctx.state.project_name));
        if ui.button("Rename project").clicked() {
            ctx.state.project_name = format!("{}-renamed", ctx.state.project_name);
        }
    }
}
