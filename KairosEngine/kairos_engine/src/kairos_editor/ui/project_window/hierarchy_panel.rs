use egui::{CollapsingHeader, RichText, Vec2};
use petgraph::{graph::NodeIndex, visit::EdgeRef};

use crate::kairos_editor::{
    project_path_tree::{
        ProjectPathGraph,
        tree_node::{ProjectNodeKind, ProjectTreeNode},
    },
    ui::project_window::{ProjectWindowColors, ProjectWindowIcons},
};

// ============================================================
// Hierarchy 面板入口
// ============================================================

/// 在 egui Ui 中绘制项目目录树（Hierarchy 面板）。
pub(super) fn draw(
    ui: &mut egui::Ui,
    graph: &ProjectPathGraph,
    icons: &ProjectWindowIcons,
    colors: &ProjectWindowColors,
) {
    egui::ScrollArea::both().show(ui, |ui| {
        let root = graph.get_root_node();
        draw_node(ui, graph, root, icons, colors);
    });
}

// ============================================================
// 递归渲染
// ============================================================

fn draw_node(
    ui: &mut egui::Ui,
    graph: &ProjectPathGraph,
    node: NodeIndex,
    icons: &ProjectWindowIcons,
    colors: &ProjectWindowColors,
) {
    let Some(node_data) = graph.get_node(node) else {
        return;
    };

    match &node_data.kind {
        ProjectNodeKind::Directory => draw_directory(ui, graph, node, node_data, icons, colors),
        _ => draw_file(ui, node_data, icons, colors),
    }
}

fn draw_directory(
    ui: &mut egui::Ui,
    graph: &ProjectPathGraph,
    node: NodeIndex,
    node_data: &ProjectTreeNode,
    icons: &ProjectWindowIcons,
    colors: &ProjectWindowColors,
) {
    let name = node_name(node_data);
    let header_text = RichText::new(name).color(colors.directory());

    let header =
        CollapsingHeader::new(header_text).id_salt(node_data.guid.to_string());

    let has_children = graph.get_edges(node).count() > 0;

    if has_children {
        header.show(ui, |ui| {
            let children: Vec<_> = graph.get_edges(node).map(|e| e.target()).collect();
            for child in children {
                draw_node(ui, graph, child, icons, colors);
            }
        });
    } else {
        header.show(ui, |_ui| {});
    }
}

fn draw_file(
    ui: &mut egui::Ui,
    node_data: &ProjectTreeNode,
    icons: &ProjectWindowIcons,
    colors: &ProjectWindowColors,
) {
    let [w, h] = icons.size;
    let icon_size = Vec2::new(w, h);

    ui.horizontal(|ui| {
        // 图标
        let icon_path = format!("file://{}", icons.for_kind(node_data));
        let icon = egui::Image::new(egui::ImageSource::Uri(icon_path.into()))
            .fit_to_exact_size(icon_size);
        ui.add(icon);

        // 文件名
        let name = node_name(node_data);
        ui.label(RichText::new(name).color(colors.file()));

        // 类型后缀
        if let Some(suffix) = kind_suffix(&node_data.kind) {
            ui.label(
                RichText::new(suffix)
                    .size(11.0)
                    .color(egui::Color32::from_rgb(120, 120, 120)),
            );
        }
    });
}

// ============================================================
// Helpers
// ============================================================

fn node_name(node_data: &ProjectTreeNode) -> String {
    node_data.name.to_string_lossy().into_owned()
}

fn kind_suffix(kind: &ProjectNodeKind) -> Option<&'static str> {
    match kind {
        ProjectNodeKind::Directory => None,
        ProjectNodeKind::Texture => Some(".texture"),
        ProjectNodeKind::Mesh => Some(".mesh"),
        ProjectNodeKind::Material => Some(".mat"),
        ProjectNodeKind::Audio => Some(".audio"),
        ProjectNodeKind::Shader => Some(".wgsl"),
        ProjectNodeKind::GenericAsset => Some(".asset"),
        ProjectNodeKind::Script => Some(".rs"),
        ProjectNodeKind::Document => None,
        ProjectNodeKind::Toml => Some(".toml"),
        ProjectNodeKind::Unknown => None,
    }
}
