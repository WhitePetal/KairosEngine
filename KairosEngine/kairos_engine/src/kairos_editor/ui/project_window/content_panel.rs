use egui::{RichText, Vec2};
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

/// 单个缩略图单元宽度
const CELL_WIDTH: f32 = 80.0;

// ============================================================
// Content Panel 入口
// ============================================================

/// `active_directory` — 当前正在浏览的目录（双击目录时更新）
/// `selected_node`   — 当前高亮选中的节点（单击时更新）
pub(super) fn draw(
    ui: &mut egui::Ui,
    graph: &ProjectPathGraph,
    icons: &ProjectWindowIcons,
    colors: &ProjectWindowColors,
    messager: &mut Messager,
    active_directory: Option<NodeIndex>,
    selected_node: Option<NodeIndex>,
) {
    let target = active_directory.unwrap_or_else(|| graph.get_root_node());

    // 收集子节点：目录优先，文件按名称排序
    let mut children: Vec<NodeIndex> = graph.get_edges(target).map(|e| e.target()).collect();
    children.sort_by(|&a, &b| {
        let na = graph.get_node(a);
        let nb = graph.get_node(b);
        match (na, nb) {
            (Some(a), Some(b)) => {
                let a_is_dir = a.kind == ProjectNodeKind::Directory;
                let b_is_dir = b.kind == ProjectNodeKind::Directory;
                if a_is_dir != b_is_dir {
                    b_is_dir.cmp(&a_is_dir)
                } else {
                    a.name.cmp(&b.name)
                }
            }
            _ => std::cmp::Ordering::Equal,
        }
    });

    if children.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label(RichText::new("empty").color(colors.file()));
        });
        return;
    }

    // 根据可用宽度计算列数
    let cols = (ui.available_width() / CELL_WIDTH).floor().max(1.0) as usize;

    ui.columns(cols, |columns| {
        for (i, child) in children.iter().enumerate() {
            let Some(node_data) = graph.get_node(*child) else {
                continue;
            };
            draw_cell(
                &mut columns[i % cols],
                *child,
                node_data,
                icons,
                colors,
                messager,
                selected_node,
            );
        }
    });
}

// ============================================================
// 单个缩略图单元
// ============================================================

fn draw_cell(
    ui: &mut egui::Ui,
    node: NodeIndex,
    node_data: &ProjectTreeNode,
    icons: &ProjectWindowIcons,
    colors: &ProjectWindowColors,
    messager: &mut Messager,
    selected_node: Option<NodeIndex>,
) {
    let is_selected = selected_node == Some(node);

    // 1. 分配空间
    let (rect, _) = ui.allocate_exact_size(Vec2::new(CELL_WIDTH, 90.0), egui::Sense::hover());

    // 2. 选中背景（在内容下方）
    if is_selected {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(4), colors.selection());
    }

    // 3. 渲染内容（在背景上方）
    let mut cell_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Center)),
    );

    let [iw, ih] = icons.size;
    let icon_size = Vec2::new(iw.max(CELL_WIDTH - 8.0), ih);
    let icon_path = format!("file://{}", icons.for_kind(node_data));
    let icon =
        egui::Image::new(egui::ImageSource::Uri(icon_path.into())).fit_to_exact_size(icon_size);
    cell_ui.add(icon);

    let name = node_data.name.to_string_lossy();
    let label = RichText::new(name.as_ref()).size(11.0).color(colors.file());
    cell_ui.label(label);

    // 4. 点击覆盖层（后注册 = 优先响应）
    let response = ui.interact(
        rect,
        ui.id().with(("cell", node.index())),
        egui::Sense::click(),
    );
    if response.clicked() {
        messager.send(Message::SelectProjectDirectoryNode(node.index()));
    }
    if response.double_clicked() && node_data.kind == ProjectNodeKind::Directory {
        messager.send(Message::NavigateToProjectDirectory(node.index()));
    }
}
