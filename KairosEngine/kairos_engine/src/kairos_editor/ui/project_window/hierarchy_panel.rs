use egui::{CollapsingHeader, RichText, Vec2};
use petgraph::{graph::NodeIndex, visit::EdgeRef};

use crate::kairos_editor::{
    project_path_tree::{
        ProjectPathGraph,
        tree_node::{ProjectNodeKind, ProjectTreeNode},
    },
    ui::{
        Message, Messager,
        project_window::{ProjectWindowColors, ProjectWindowIcons},
    },
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
    messager: &mut Messager,
    selected_node: Option<NodeIndex>,
) {
    let root = graph.get_root_node();
    draw_node(ui, graph, root, icons, colors, messager, selected_node);
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
    messager: &mut Messager,
    selected_node: Option<NodeIndex>,
) {
    let Some(node_data) = graph.get_node(node) else {
        return;
    };

    match &node_data.kind {
        ProjectNodeKind::Directory => draw_directory(
            ui,
            graph,
            node,
            node_data,
            icons,
            colors,
            messager,
            selected_node,
        ),
        _ => draw_file(ui, node, node_data, icons, colors, messager, selected_node),
    }
}

fn draw_directory(
    ui: &mut egui::Ui,
    graph: &ProjectPathGraph,
    node: NodeIndex,
    node_data: &ProjectTreeNode,
    _icons: &ProjectWindowIcons,
    colors: &ProjectWindowColors,
    messager: &mut Messager,
    selected_node: Option<NodeIndex>,
) {
    let name = node_name(node_data);
    let is_selected = selected_node == Some(node);
    let row_height = ui.spacing().interact_size.y;
    let row_y = ui.cursor().min.y;
    let row_width = ui.available_width();

    // 选中背景（纯绘制，不推进 cursor）
    if is_selected {
        let bg_rect = egui::Rect::from_min_size(
            egui::pos2(ui.cursor().min.x, row_y),
            egui::vec2(row_width, row_height),
        );
        ui.painter()
            .rect_filled(bg_rect, egui::CornerRadius::same(4), colors.selection());
    }

    let header_text = RichText::new(name).color(colors.directory());
    let header = CollapsingHeader::new(header_text).id_salt(node_data.guid.to_string());

    let has_children = graph.get_edges(node).count() > 0;

    let response = if has_children {
        header.show(ui, |ui| {
            let children: Vec<_> = graph.get_edges(node).map(|e| e.target()).collect();
            for child in children {
                draw_node(ui, graph, child, _icons, colors, messager, selected_node);
            }
        })
    } else {
        header.show(ui, |ui| {
            ui.label(
                RichText::new("(empty)")
                    .size(11.0)
                    .color(egui::Color32::from_rgb(120, 120, 120)),
            );
        })
    };

    // 全宽点击：label 区域 + 右侧空白区域
    let mut clicked = response.header_response.clicked();
    if !clicked {
        let header_right = response.header_response.rect.max.x;
        if header_right < row_width {
            let right_rect = egui::Rect::from_min_max(
                egui::pos2(header_right, row_y),
                egui::pos2(row_width, row_y + row_height),
            );
            let right_click = ui.interact(
                right_rect,
                ui.id().with(("right", node.index())),
                egui::Sense::click(),
            );
            clicked = right_click.clicked();
        }
    }

    if clicked {
        messager.send(Message::NavigateToProjectDirectory(node.index()));
    }
}

fn draw_file(
    ui: &mut egui::Ui,
    node: NodeIndex,
    node_data: &ProjectTreeNode,
    icons: &ProjectWindowIcons,
    colors: &ProjectWindowColors,
    messager: &mut Messager,
    selected_node: Option<NodeIndex>,
) {
    let is_selected = selected_node == Some(node);
    let [w, h] = icons.size;
    let icon_size = Vec2::new(w, h);
    let row_height = ui.spacing().interact_size.y;
    let row_start = ui.cursor().min;
    let row_width = ui.available_width(); // 在 horizontal 消费前保存

    // 1. 选中背景（优先绘制，在内容下方）
    if is_selected {
        let row_rect = egui::Rect::from_min_size(row_start, egui::vec2(row_width, row_height));
        ui.painter()
            .rect_filled(row_rect, egui::CornerRadius::same(4), colors.selection());
    }

    // 2. 渲染内容（在背景上方）
    ui.horizontal(|ui| {
        let icon_path = format!("file://{}", icons.for_kind(node_data));
        let icon =
            egui::Image::new(egui::ImageSource::Uri(icon_path.into())).fit_to_exact_size(icon_size);
        ui.add(icon);

        let name = node_name(node_data);
        ui.label(RichText::new(name).color(colors.file()));

        if let Some(suffix) = kind_suffix(&node_data.kind) {
            ui.label(
                RichText::new(suffix)
                    .size(11.0)
                    .color(egui::Color32::from_rgb(120, 120, 120)),
            );
        }
    });

    // 3. 全宽点击覆盖层（后注册 = 优先响应点击）
    let row_rect = egui::Rect::from_min_size(row_start, egui::vec2(row_width, row_height));
    let response = ui.interact(
        row_rect,
        ui.id().with(("row", node.index())),
        egui::Sense::click(),
    );
    if response.clicked() {
        messager.send(Message::SelectProjectDirectoryNode(node.index()));
    }
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
