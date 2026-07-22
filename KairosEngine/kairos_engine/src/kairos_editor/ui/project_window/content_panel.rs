use std::{ops::DerefMut, sync::Arc};

use egui::{RichText, TextEdit, Vec2};
use parking_lot::Mutex;
use petgraph::graph::NodeIndex;
use serde::{Deserialize, Serialize};

use crate::{
    kairos_editor::{
        asset_registry::AssetKind,
        project_path_tree::{ProjectPathGraph, tree_node::ProjectTreeNode},
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
    // ---- 底部栏（选中文件路径）----
    pub bottom_bar_height: f32,
    pub bottom_bar_background_color: math::Color32,
    pub bottom_bar_text_color: math::Color32,
    pub bottom_bar_font_size: f32,
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
        renaming_node: Option<NodeIndex>,
        renaming_buffer: &Option<Arc<Mutex<String>>>,
    ) {
        let target = active_directory.unwrap_or_else(|| graph.get_root_node());

        // 收集排序后的子节点
        let sorted = graph.sorted_children(target);
        let children: Vec<NodeIndex> = sorted.iter().map(|(idx, _)| *idx).collect();

        // 根据可用宽度计算列数（icon + 水平间隔 = 单列占用宽度）
        let cell_total_width = style.content.icon_size + style.content.cell_spacing_x;
        let cols = (ui.available_width() / cell_total_width).floor().max(1.0) as usize;
        let rows = (children.len() + cols - 1) / cols;
        let cell_h =
            style.content.icon_size + style.content.label_height + style.content.cell_spacing_y;
        let total_h = rows as f32 * cell_h;

        let panel_rect = egui::Rect::from_min_max(ui.cursor().min, ui.max_rect().max).union(
            egui::Rect::from_min_size(ui.cursor().min, Vec2::new(ui.available_width(), total_h)),
        );

        let bg_response = ui.interact(
            panel_rect,
            ui.id().with("content_panel_bg"),
            egui::Sense::all(),
        );

        bg_response.context_menu(|ui| {
            ContextMenu::show(ui, ContextMenuState::new(target), messager);
        });

        if bg_response.clicked() {
            messager.send(Message::SelectProjectNode(None));
        }

        ui.columns(cols, |columns| {
            for (i, child) in children.iter().enumerate() {
                let Some(node_data) = graph.get_node(*child) else {
                    continue;
                };
                let ui = &mut columns[i % cols];
                ui.push_id(node_data.guid, |ui| {
                    Self::draw_cell(
                        ui,
                        global_styles,
                        *child,
                        node_data,
                        graph.has_child(*child),
                        style,
                        messager,
                        selected_node,
                        renaming_node,
                        renaming_buffer,
                    );
                });
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
        has_child: bool,
        style: &ProjectWindowStyle,
        messager: &mut Messager,
        selected_node: Option<NodeIndex>,
        renaming_node: Option<NodeIndex>,
        rename_buffer: &Option<Arc<Mutex<String>>>,
    ) {
        let is_selected = selected_node == Some(node);
        let is_renaming = renaming_node == Some(node);
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

        // 4. 渲染内容

        // Icon
        let icon = egui::Image::new(
            global_styles
                .project_node_icons
                .uri_for_kind(node_data, has_child),
        )
        .fit_to_exact_size(Vec2::new(icon_size, icon_size))
        .show_loading_spinner(true);

        // 渲染内容，clip 在 content_rect 内防止溢出
        let mut cell_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(content_rect)
                .layout(egui::Layout::top_down(egui::Align::Center)),
        );

        cell_ui.add_sized(Vec2::new(icon_size, icon_size), icon);

        if is_renaming {
            if let Some(renaming_buffer) = rename_buffer {
                let renaming_buffer = renaming_buffer.clone();
                let mut renaming_buffer_mut = renaming_buffer.lock();
                let renaming_buffer_mut = renaming_buffer_mut.deref_mut();
                let text_edit = TextEdit::singleline(renaming_buffer_mut).font(egui::FontId::new(
                    style.content.label_font_size,
                    egui::FontFamily::Proportional,
                ));
                let text_edit = cell_ui.add_sized(
                    Vec2::new(icon_size + style.content.cell_spacing_x, label_height),
                    text_edit,
                );
                text_edit.request_focus();
                if text_edit.clicked_elsewhere()
                    || cell_ui.input(|i| i.key_pressed(egui::Key::Enter))
                {
                    messager.send(Message::RenameProjectNode);
                }
            }
        } else {
            // Label — 预截断到固定高度内
            let name = node_data.name.to_string_lossy();
            let truncated_name = cell_ui.truncate_text_to_height(
                &name,
                style.content.label_font_size,
                icon_size,
                label_height,
            );
            let label = RichText::new(truncated_name)
                .size(style.content.label_font_size)
                .color(style.content.label_color);
            cell_ui.label(label);
        }

        if !is_renaming {
                if response.clicked() || response.secondary_clicked() {
                    messager.send(Message::SelectProjectNode(Some(node)));
                }
                if response.double_clicked() {
                    messager.send(Message::OpenProjectNode(node));
                }
                response.context_menu(|ui| {
                    ContextMenu::show(ui, ContextMenuState::new(node), messager);
                });

                // ---- Drag source: 将文件节点注册为可拖拽（仅非目录节点） ----
                if node_data.kind != AssetKind::Directory {
                    if response.drag_started() {
                        let file_path = node_data.path.to_string_lossy().to_string();
                        ui.ctx().data_mut(|d| {
                            d.insert_persisted(
                                egui::Id::new("__kairos_drag_payload"),
                                file_path,
                            );
                        });
                    }

                    // 拖拽视觉反馈：光标 + 幽灵图
                    if ui.ctx().is_being_dragged(response.id) {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                        if let Some(pos) = ui.ctx().pointer_interact_pos() {
                            let ghost_id = ui.id().with("drag_ghost");
                            let ghost_pos = pos + egui::vec2(12.0, 12.0);
                            egui::Area::new(ghost_id)
                                .fixed_pos(ghost_pos)
                                .order(egui::Order::Foreground)
                                .show(ui.ctx(), |ui| {
                                    let ghost_frame = egui::Frame {
                                        fill: egui::Color32::from_black_alpha(180),
                                        corner_radius: egui::CornerRadius::same(4),
                                        inner_margin: egui::Margin::symmetric(6, 3).into(),
                                        ..Default::default()
                                    };
                                    ghost_frame.show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            let ghost_icon = egui::Image::new(
                                                global_styles
                                                    .project_node_icons
                                                    .uri_for_kind(node_data, false),
                                            )
                                            .fit_to_exact_size(Vec2::new(16.0, 16.0));
                                            ui.add(ghost_icon);
                                            ui.label(
                                                RichText::new(node_data.name())
                                                    .size(style.content.label_font_size)
                                                    .color(egui::Color32::WHITE),
                                            );
                                        });
                                    });
                                });
                        }
                    }
                }
            }
    }
}
