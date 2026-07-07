use egui::{RichText, Vec2};
use petgraph::{graph::NodeIndex, visit::EdgeRef};

use crate::kairos_editor::{
    project_path_tree::{
        ProjectPathGraph,
        tree_node::{ProjectNodeKind, ProjectTreeNode},
    },
    ui::project_window::{ProjectWindowColors, ProjectWindowIcons},
};

/// 单个缩略图单元宽度
const CELL_WIDTH: f32 = 80.0;

// ============================================================
// Content Panel 入口
// ============================================================

/// 在 egui Ui 中绘制缩略图网格。
///
/// `selected_node` 为 `Some(dir)` 时展示该目录的子节点；
/// 为 `None` 时默认展示根目录的子节点。
pub(super) fn draw(
    ui: &mut egui::Ui,
    graph: &ProjectPathGraph,
    icons: &ProjectWindowIcons,
    colors: &ProjectWindowColors,
    selected_node: Option<NodeIndex>,
) {
    let target = selected_node.unwrap_or_else(|| graph.get_root_node());

    // 收集子节点：目录优先，文件按名称排序
    let mut children: Vec<NodeIndex> = graph.get_edges(target).map(|e| e.target()).collect();
    children.sort_by(|&a, &b| {
        let na = graph.get_node(a);
        let nb = graph.get_node(b);
        match (na, nb) {
            (Some(a), Some(b)) => {
                // 目录优先
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

    // 自动换行的缩略图布局
    ui.horizontal_wrapped(|ui| {
        for child in children {
            let Some(node_data) = graph.get_node(child) else {
                continue;
            };
            draw_cell(ui, node_data, icons, colors);
        }
    });
}

// ============================================================
// 单个缩略图单元
// ============================================================

fn draw_cell(
    ui: &mut egui::Ui,
    node_data: &ProjectTreeNode,
    icons: &ProjectWindowIcons,
    colors: &ProjectWindowColors,
) {
    ui.scope(|ui| {
        ui.set_width(CELL_WIDTH);
        ui.vertical_centered(|ui| {
            // 图标
            let [iw, ih] = icons.size;
            let icon_size = Vec2::new(iw.max(CELL_WIDTH - 8.0), ih);
            let icon_path = format!("file://{}", icons.for_kind(node_data));
            let icon = egui::Image::new(egui::ImageSource::Uri(icon_path.into()))
                .fit_to_exact_size(icon_size);
            ui.add(icon);

            // 文件名
            let name = node_data.name.to_string_lossy();
            let label = RichText::new(name.as_ref()).size(11.0).color(colors.file());
            ui.label(label);
        });
    });
}
