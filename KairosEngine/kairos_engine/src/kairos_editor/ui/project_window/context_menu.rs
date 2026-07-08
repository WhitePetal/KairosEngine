use egui::{Button, PopupAnchor, Vec2};
use petgraph::graph::NodeIndex;

use crate::kairos_editor::{
    project_path_tree::tree_node::ProjectNodeKind,
    ui::{Message, Messager},
};

// ============================================================
// ContextMenuState
// ============================================================

pub(super) struct ContextMenuState {
    pub node: NodeIndex,
    pub position: egui::Pos2,
}

// ============================================================
// 渲染
// ============================================================

/// 渲染级联右键菜单（Create > Folder）。
pub(super) fn draw(ui: &mut egui::Ui, state: &ContextMenuState, messager: &mut Messager) {
    let popup_id = ui.id().with(("ctx_menu", state.node.index()));

    let response = egui::containers::Popup::new(
        popup_id,
        ui.ctx().clone(),
        PopupAnchor::Position(state.position),
        ui.layer_id(),
    )
    .show(|ui| {
        // ui.set_min_width(120.0);
        ui.style_mut().visuals.widgets.inactive.bg_fill = egui::Color32::TRANSPARENT;
        ui.style_mut().visuals.widgets.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;

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
        if ui
            .add(Button::new("Open").min_size(btn_min_size))
            .clicked()
        {
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

        ui.menu_button("TTTTTTEST", |ui| {

        })
    });

    // Popup 不自动关闭，手动检测
    let should_close = if let Some(ref inner) = response {
        inner.response.clicked_elsewhere()
    } else {
        false
    };
    let should_close = should_close || ui.input(|i| i.key_pressed(egui::Key::Escape));
    if should_close {
        messager.send(Message::CloseProjectContextMenu);
    }
}
