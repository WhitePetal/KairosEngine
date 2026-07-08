use egui::{Button, Vec2};
use petgraph::graph::NodeIndex;

use crate::kairos_editor::{
    project_path_tree::tree_node::ProjectNodeKind,
    ui::{Message, Messager},
};

// ============================================================
// ContextMenuState
// ============================================================

pub struct ContextMenuState {
    pub node: NodeIndex,
}
impl ContextMenuState {
    pub fn new(node: NodeIndex) -> Self {
        Self { node }
    }
}

pub struct ContextMenu {}

// ============================================================
// 渲染
// ============================================================

impl ContextMenu {
    pub fn show(ui: &mut egui::Ui, state: ContextMenuState, messager: &mut Messager) {
        ui.menu_button("Create", |ui| {
            if ui.button("Folder").clicked() {
                messager.send(Message::CreateProjectNode(
                    state.node.index(),
                    "New Folder".into(),
                    ProjectNodeKind::Directory,
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
            todo!()
        }
        if ui
            .add(Button::new("Rename").min_size(btn_min_size))
            .clicked()
        {
            todo!()
        }

        ui.menu_button("TTTTTTEST", |ui| {});
    }
}
