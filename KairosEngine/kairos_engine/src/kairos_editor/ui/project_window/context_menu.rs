use egui::{Button, Vec2};
use petgraph::graph::NodeIndex;

use crate::kairos_editor::{
    project_path_tree::tree_node::ProjectNodeKind,
    ui::{Message, Messager, project_window::RenameOrigin},
};

// ============================================================
// ContextMenuState
// ============================================================

pub struct ContextMenuState {
    pub node: NodeIndex,
    pub origin: RenameOrigin,
}
impl ContextMenuState {
    pub fn new(node: NodeIndex, origin: RenameOrigin) -> Self {
        Self { node, origin }
    }
}

pub struct ContextMenu {}

// ============================================================
// 渲染
// ============================================================

impl ContextMenu {
    pub fn show(ui: &mut egui::Ui, state: ContextMenuState, messager: &mut Messager) {
        let origin = Some(state.origin);
        ui.menu_button("Create", |ui| {
            if ui.button("Folder").clicked() {
                messager.send(Message::CreateProjectNode(
                    state.node.index(),
                    "New Folder".into(),
                    ProjectNodeKind::Directory,
                    origin,
                ));
            }

            ui.separator();

            if ui.button("Rust").clicked() {
                messager.send(Message::CreateProjectNode(
                    state.node.index(),
                    "new_rust_script".into(),
                    ProjectNodeKind::Script,
                    origin,
                ));
            }
            if ui.button("Shader").clicked() {
                messager.send(Message::CreateProjectNode(
                    state.node.index(),
                    "New Shader".into(),
                    ProjectNodeKind::Shader,
                    origin,
                ));
            }
            if ui.button("Toml").clicked() {
                messager.send(Message::CreateProjectNode(
                    state.node.index(),
                    "New Toml".into(),
                    ProjectNodeKind::Toml,
                    origin,
                ));
            }
            if ui.button("Document").clicked() {
                messager.send(Message::CreateProjectNode(
                    state.node.index(),
                    "New Document".into(),
                    ProjectNodeKind::Document,
                    origin,
                ));
            }
        });
        let btn_min_size = Vec2::new(ui.min_size().x, 0.0);
        if ui.add(Button::new("Open").min_size(btn_min_size)).clicked() {
            todo!()
        }
        if ui
            .add(Button::new("Delete").min_size(btn_min_size))
            .clicked()
        {
            messager.send(Message::DeleteProjectNode(state.node.index()));
        }
        if ui
            .add(Button::new("Rename").min_size(btn_min_size))
            .clicked()
        {
            messager.send(Message::StartRenameProjectNode(
                state.node.index(),
                Some(state.origin),
            ));
        }
    }
}
