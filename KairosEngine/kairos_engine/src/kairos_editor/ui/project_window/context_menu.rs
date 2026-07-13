use egui::{Button, Vec2};
use petgraph::graph::NodeIndex;

use crate::kairos_editor::{
    asset_registry::AssetKind,
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
                messager.send(Message::OpenProjectNode(state.node));
                messager.send(Message::CreateProjectNode(
                    state.node,
                    "New Folder".into(),
                    AssetKind::Directory,
                ));
            }

            ui.separator();

            if ui.button("Rust").clicked() {
                messager.send(Message::CreateProjectNode(
                    state.node,
                    "new_rust_script".into(),
                    AssetKind::Script,
                ));
            }
            if ui.button("Shader").clicked() {
                messager.send(Message::CreateProjectNode(
                    state.node,
                    "New Shader".into(),
                    AssetKind::Shader,
                ));
            }
            if ui.button("Toml").clicked() {
                messager.send(Message::CreateProjectNode(
                    state.node,
                    "New Toml".into(),
                    AssetKind::Toml,
                ));
            }
            if ui.button("Document").clicked() {
                messager.send(Message::CreateProjectNode(
                    state.node,
                    "New Document".into(),
                    AssetKind::Document,
                ));
            }
        });
        let btn_min_size = Vec2::new(ui.min_size().x, 0.0);
        if ui.add(Button::new("Open").min_size(btn_min_size)).clicked() {
            messager.send(Message::OpenProjectNode(state.node));
        }
        if ui
            .add(Button::new("Delete").min_size(btn_min_size))
            .clicked()
        {
            messager.send(Message::DeleteProjectNode(state.node));
        }
        if ui
            .add(Button::new("Rename").min_size(btn_min_size))
            .clicked()
        {
            messager.send(Message::StartRenameProjectNode(state.node));
        }
    }
}
