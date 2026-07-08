use egui::{RichText, Vec2};
use petgraph::{graph::NodeIndex, visit::EdgeRef};
use serde::{Deserialize, Serialize};

use crate::{
    kairos_editor::{
        project_path_tree::{
            ProjectPathGraph,
            tree_node::{ProjectNodeKind, ProjectTreeNode},
        },
        ui::{
            Message, Messager,
            egui_ext::UiExt,
            global_styles::GlobalStyles,
            project_window::{ContextMenuState, ProjectWindowStyle, context_menu::ContextMenu},
        },
    },
    math,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct ContentStyle {
    pub label_color: math::Color32,
    pub selected_background_color: math::Color32,
    pub selected_background_corner_radius: u8,
    pub icon_size: f32,
    pub label_font_size: f32,
    /// 固定 label 高度（超出部分截断/裁剪），避免行间重叠
    pub label_height: f32,
    /// cell 之间的水平间隔
    pub cell_spacing_x: f32,
    /// cell 之间的垂直间隔
    pub cell_spacing_y: f32,
}

pub struct ContentPanel {}

// ============================================================
// Content Panel 入口
// ============================================================

impl ContentPanel {
    /// `active_directory` — 当前正在浏览的目录（双击目录时更新）
    /// `selected_node`   — 当前高亮选中的节点（单击时更新）
    pub fn draw(
        ui: &mut egui::Ui,
        global_styles: &GlobalStyles,
        graph: &ProjectPathGraph,
        style: &ProjectWindowStyle,
        messager: &mut Messager,
        active_directory: Option<NodeIndex>,
        selected_node: Option<NodeIndex>,
    ) {
        let target = active_directory.unwrap_or_else(|| graph.get_root_node());

        // 背景右键区域（先注册，cell 的 interact 后注册会优先响应）
        let panel_rect = egui::Rect::from_min_size(ui.cursor().min, ui.available_size());
        let bg_response = ui.interact(
            panel_rect,
            ui.id().with("content_panel_bg"),
            egui::Sense::all(),
        );
        bg_response.context_menu(|ui| {
            ContextMenu::show(ui, ContextMenuState::new(target), messager);
        });

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
                ui.label(RichText::new("empty").color(style.content.label_color));
            });
            return;
        }

        // 根据可用宽度计算列数（icon + 水平间隔 = 单列占用宽度）
        let cell_total_width = style.content.icon_size + style.content.cell_spacing_x;
        let cols = (ui.available_width() / cell_total_width).floor().max(1.0) as usize;

        ui.columns(cols, |columns| {
            for (i, child) in children.iter().enumerate() {
                let Some(node_data) = graph.get_node(*child) else {
                    continue;
                };
                Self::draw_cell(
                    &mut columns[i % cols],
                    global_styles,
                    *child,
                    node_data,
                    style,
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
        global_styles: &GlobalStyles,
        node: NodeIndex,
        node_data: &ProjectTreeNode,
        style: &ProjectWindowStyle,
        messager: &mut Messager,
        selected_node: Option<NodeIndex>,
    ) {
        let is_selected = selected_node == Some(node);
        let icon_size = style.content.icon_size;
        let label_height = style.content.label_height;

        // 1. 分配空间：icon + label 固定高度 + 垂直间隔
        let cell_content_height = icon_size + label_height;
        let alloc_height = cell_content_height + style.content.cell_spacing_y;
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(icon_size, alloc_height), egui::Sense::all());

        // 2. 内容区域（不包含底部间隔）
        let content_rect =
            egui::Rect::from_min_size(rect.min, Vec2::new(icon_size, cell_content_height));

        // 3. 选中背景
        if is_selected {
            ui.painter().rect_filled(
                content_rect,
                egui::CornerRadius::same(style.content.selected_background_corner_radius),
                style.content.selected_background_color,
            );
        }

        // 4. 渲染内容，clip 在 content_rect 内防止溢出
        let mut cell_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(content_rect)
                .layout(egui::Layout::top_down(egui::Align::Center)),
        );
        cell_ui.set_clip_rect(content_rect);

        // Icon
        let icon_path = format!(
            "file://{}",
            global_styles.project_node_icons.for_kind(node_data)
        );
        let icon = egui::Image::new(egui::ImageSource::Uri(icon_path.into()))
            .fit_to_exact_size(Vec2::new(icon_size, icon_size))
            .show_loading_spinner(true);
        cell_ui.add(icon);

        // Label — 预截断到固定高度内
        let name = node_data.name.to_string_lossy();
        let truncated_name = ui.truncate_text_to_height(
            &name,
            style.content.label_font_size,
            icon_size,
            label_height,
        );
        let label = RichText::new(truncated_name)
            .size(style.content.label_font_size)
            .color(style.content.label_color);
        cell_ui.label(label);

        if response.clicked() {
            messager.send(Message::SelectProjectDirectoryNode(node.index()));
        }
        if response.double_clicked() && node_data.kind == ProjectNodeKind::Directory {
            messager.send(Message::NavigateToProjectDirectory(node.index()));
        }
        response.context_menu(|ui| {
            ContextMenu::show(ui, ContextMenuState::new(node), messager);
        });
    }
}
