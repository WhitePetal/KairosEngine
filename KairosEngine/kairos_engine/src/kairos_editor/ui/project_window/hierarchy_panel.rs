use egui::{CollapsingHeader, RichText, Vec2};
use petgraph::graph::NodeIndex;
use serde::{Deserialize, Serialize};

use crate::{
    kairos_editor::{
        project_path_tree::{
            ProjectPathGraph,
            tree_node::{ProjectNodeKind, ProjectTreeNode},
        },
        ui::{
            Message, Messager,
            global_styles::GlobalStyles,
            project_window::{
                ProjectWindowStyle,
                context_menu::{ContextMenu, ContextMenuState},
            },
        },
    },
    math,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct HierarchyStyle {
    pub directory_header_color: math::Color32,
    pub file_header_color: math::Color32,
    pub selected_file_background_color: math::Color32,
    pub file_suffix_color: math::Color32,
    pub file_header_size: f32,
    pub file_icon_size: f32,
    pub selected_file_background_corner_radius: u8,
}

pub struct HierarchyPanel {}

// ============================================================
// Hierarchy 面板入口
// ============================================================

impl HierarchyPanel {
    /// 在 egui Ui 中绘制项目目录树（Hierarchy 面板）。
    pub fn draw(
        ui: &mut egui::Ui,
        global_styles: &GlobalStyles,
        graph: &ProjectPathGraph,
        style: &ProjectWindowStyle,
        messager: &mut Messager,
        selected_node: Option<NodeIndex>,
        force_expand_to: Option<NodeIndex>,
    ) {
        let root = graph.get_root_node();
        Self::draw_node(
            ui,
            global_styles,
            graph,
            root,
            style,
            messager,
            selected_node,
            force_expand_to,
        );
    }

    // ============================================================
    // 递归渲染
    // ============================================================

    fn draw_node(
        ui: &mut egui::Ui,
        global_styles: &GlobalStyles,
        graph: &ProjectPathGraph,
        node: NodeIndex,
        style: &ProjectWindowStyle,
        messager: &mut Messager,
        selected_node: Option<NodeIndex>,
        force_expand_to: Option<NodeIndex>,
    ) {
        let Some(node_data) = graph.get_node(node) else {
            return;
        };

        ui.push_id(node_data.guid, |ui| {
            match &node_data.kind {
                ProjectNodeKind::Directory => Self::draw_directory(
                    ui,
                    global_styles,
                    graph,
                    node,
                    node_data,
                    style,
                    messager,
                    selected_node,
                    force_expand_to,
                ),
                _ => Self::draw_file(
                    ui,
                    global_styles,
                    node,
                    node_data,
                    style,
                    messager,
                    selected_node,
                ),
            }
        });
    }

    fn draw_directory(
        ui: &mut egui::Ui,
        global_styles: &GlobalStyles,
        graph: &ProjectPathGraph,
        node: NodeIndex,
        node_data: &ProjectTreeNode,
        style: &ProjectWindowStyle,
        messager: &mut Messager,
        selected_node: Option<NodeIndex>,
        force_expand_to: Option<NodeIndex>,
    ) {
        let is_selected = selected_node == Some(node);

        let name = node_data.name();

        // 仅在一次性标记有效时强制展开：当前节点是目标或其祖先
        let should_force_open = force_expand_to.is_some_and(|target| {
            target == node || graph.get_ancestors(target).contains(&node)
        });

        ui.visuals_mut().collapsing_header_frame = true;

        let header_text = RichText::new(name).color(style.hierachy.directory_header_color);
        let header = CollapsingHeader::new(header_text).open(if should_force_open {
            Some(true)
        } else {
            None
        });
        let has_children = !graph.sorted_children(node).is_empty();

        let mut response = if has_children {
            header.show(ui, |ui| {
                let children = graph.sorted_children(node);
                for (child, _) in children {
                    Self::draw_node(
                        ui,
                        global_styles,
                        graph,
                        child,
                        style,
                        messager,
                        selected_node,
                        force_expand_to,
                    );
                }
            })
        } else {
            header.show(ui, |ui| {
                ui.label(
                    RichText::new("(empty)")
                        .size(style.hierachy.file_header_size)
                        .color(style.hierachy.file_header_color),
                );
            })
        };
        ui.visuals_mut().collapsing_header_frame = false;

        if is_selected {
            response.header_response = response.header_response.highlight();
        }

        if response.header_response.clicked() {
            messager.send(Message::NavigateToProjectDirectory(node));
        }
        if response.header_response.secondary_clicked() {
            messager.send(Message::SelectProjectNode(Some(node)));
        }
        response.header_response.context_menu(|ui| {
            ContextMenu::show(ui, ContextMenuState::new(node), messager);
        });
    }

    fn draw_file(
        ui: &mut egui::Ui,
        global_styles: &GlobalStyles,
        node: NodeIndex,
        node_data: &ProjectTreeNode,
        style: &ProjectWindowStyle,
        messager: &mut Messager,
        selected_node: Option<NodeIndex>,
    ) {
        let is_selected = selected_node == Some(node);

        let icon_size = style.hierachy.file_icon_size;
        let icon_size = Vec2::new(icon_size, icon_size);
        let row_height = ui.spacing().interact_size.y;
        let row_start = ui.cursor().min;
        let row_width = ui.available_width(); // 在 horizontal 消费前保存

        // 1. 选中背景（优先绘制，在内容下方）
        if is_selected {
            let row_rect = egui::Rect::from_min_size(row_start, egui::vec2(row_width, row_height));
            ui.painter().rect_filled(
                row_rect,
                egui::CornerRadius::same(style.hierachy.selected_file_background_corner_radius),
                style.hierachy.selected_file_background_color,
            );
        }

        // 2. 渲染内容（在背景上方）
        ui.horizontal(|ui| {
            let icon_path = format!(
                "file://{}",
                global_styles.project_node_icons.for_kind(node_data)
            );
            let icon = egui::Image::new(egui::ImageSource::Uri(icon_path.into()))
                .fit_to_exact_size(icon_size);
            ui.add(icon);

            let name = node_data.name();
            ui.label(RichText::new(name).color(style.hierachy.file_header_color));

            if let Some(suffix) = node_data.kind.suffix() {
                ui.label(
                    RichText::new(suffix)
                        .size(style.hierachy.file_header_size)
                        .color(style.hierachy.file_suffix_color),
                );
            }
        });

        // 3. 全宽点击覆盖层
        let row_rect = egui::Rect::from_min_size(row_start, egui::vec2(row_width, row_height));
        let response = ui.interact(
            row_rect,
            ui.id().with("row"),
            egui::Sense::click(),
        );
        if response.clicked() || response.secondary_clicked() {
            messager.send(Message::SelectProjectNode(Some(node)));
        }
        response.context_menu(|ui| {
            ContextMenu::show(ui, ContextMenuState::new(node), messager);
        });
    }
}
