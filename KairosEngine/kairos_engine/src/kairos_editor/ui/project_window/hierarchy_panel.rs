use egui::{CollapsingHeader, RichText, TextEdit, Vec2};
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
                ProjectWindowStyle, RenameOrigin,
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
        expand_node: Option<NodeIndex>,
        scroll_to_node: Option<NodeIndex>,
        renaming_node: Option<NodeIndex>,
        renaming_origin: Option<RenameOrigin>,
        rename_buffer: &mut String,
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
            expand_node,
            scroll_to_node,
            renaming_node,
            renaming_origin,
            rename_buffer,
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
        expand_node: Option<NodeIndex>,
        scroll_to_node: Option<NodeIndex>,
        renaming_node: Option<NodeIndex>,
        renaming_origin: Option<RenameOrigin>,
        rename_buffer: &mut String,
    ) {
        let Some(node_data) = graph.get_node(node) else {
            return;
        };

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
                expand_node,
                scroll_to_node,
                renaming_node,
                renaming_origin,
                rename_buffer,
            ),
            _ => Self::draw_file(
                ui,
                global_styles,
                node,
                node_data,
                style,
                messager,
                selected_node,
                renaming_node,
                renaming_origin,
                rename_buffer,
            ),
        }
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
        expand_node: Option<NodeIndex>,
        scroll_to_node: Option<NodeIndex>,
        renaming_node: Option<NodeIndex>,
        renaming_origin: Option<RenameOrigin>,
        rename_buffer: &mut String,
    ) {
        let is_renaming = renaming_node == Some(node)
            && (renaming_origin.is_none() || renaming_origin == Some(RenameOrigin::Hierarchy));
        let is_selected = selected_node == Some(node);
        let force_open = expand_node == Some(node)
            || scroll_to_node == Some(node)
            || scroll_to_node.is_some_and(|target| graph.get_ancestors(target).contains(&node));

        if is_renaming {
            // ---- 重命名模式：自定义行 + 直接渲染子节点 ----
            let row_height = ui.spacing().interact_size.y;
            let row_start = ui.cursor().min;
            let row_width = ui.available_width();

            // 选中背景
            if is_selected {
                let row_rect =
                    egui::Rect::from_min_size(row_start, egui::vec2(row_width, row_height));
                ui.painter().rect_filled(
                    row_rect,
                    egui::CornerRadius::same(style.hierachy.selected_file_background_corner_radius),
                    style.hierachy.selected_file_background_color,
                );
            }

            // 渲染行：icon + TextEdit
            ui.horizontal(|ui| {
                let icon_path = format!(
                    "file://{}",
                    global_styles.project_node_icons.for_kind(node_data)
                );
                let icon_size =
                    Vec2::new(style.hierachy.file_icon_size, style.hierachy.file_icon_size);
                let icon = egui::Image::new(egui::ImageSource::Uri(icon_path.into()))
                    .fit_to_exact_size(icon_size);
                ui.add(icon);

                let edit_width = 200.0_f32.min(ui.available_width() - icon_size.x - 8.0);
                let text_edit_response = ui.add(
                    TextEdit::singleline(rename_buffer)
                        .desired_width(edit_width)
                        .id_salt(node_data.guid.to_string()),
                );

                // 处理 Enter / Escape / 焦点丢失
                let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));

                if enter {
                    messager.send(Message::RenameProjectNode(
                        node.index(),
                        rename_buffer.clone(),
                    ));
                } else if escape {
                    messager.send(Message::CancelRenameProjectNode);
                } else if text_edit_response.clicked_elsewhere() {
                    messager.send(Message::RenameProjectNode(
                        node.index(),
                        rename_buffer.clone(),
                    ));
                }
            });

            // 直接渲染子节点
            let has_children = !graph.sorted_children(node).is_empty();
            if has_children {
                ui.indent("rename_dir_children", |ui| {
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
                            expand_node,
                            scroll_to_node,
                            renaming_node,
                            renaming_origin,
                            rename_buffer,
                        );
                    }
                });
            } else {
                ui.indent("rename_dir_children", |ui| {
                    ui.label(
                        RichText::new("(empty)")
                            .size(style.hierachy.file_header_size)
                            .color(style.hierachy.file_header_color),
                    );
                });
            }

            // 滚动到可见区域
            if scroll_to_node == Some(node) {
                // Can't easily scroll in rename mode, skip for now
            }
        } else {
            // ---- 正常模式：CollapsingHeader ----
            let name = node_data.name();
            ui.visuals_mut().collapsing_header_frame = true;

            let header_text = RichText::new(name).color(style.hierachy.directory_header_color);
            let mut header = CollapsingHeader::new(header_text).id_salt(node_data.guid.to_string());

            // 如果本节点是需要强制展开的父目录，覆盖 egui 内部折叠状态
            if force_open {
                header = header.open(Some(true));
            }

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
                            expand_node,
                            scroll_to_node,
                            renaming_node,
                            renaming_origin,
                            rename_buffer,
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

            // 如果是新建节点，滚动到可见区域
            if scroll_to_node == Some(node) {
                response
                    .header_response
                    .scroll_to_me(Some(egui::Align::Center));
            }

            let clicked = response.header_response.clicked();

            if clicked {
                messager.send(Message::NavigateToProjectDirectory(node.index()));
            }
            response.header_response.context_menu(|ui| {
                ContextMenu::show(
                    ui,
                    ContextMenuState::new(node, RenameOrigin::Hierarchy),
                    messager,
                );
            });
        }
    }

    fn draw_file(
        ui: &mut egui::Ui,
        global_styles: &GlobalStyles,
        node: NodeIndex,
        node_data: &ProjectTreeNode,
        style: &ProjectWindowStyle,
        messager: &mut Messager,
        selected_node: Option<NodeIndex>,
        renaming_node: Option<NodeIndex>,
        renaming_origin: Option<RenameOrigin>,
        rename_buffer: &mut String,
    ) {
        let is_selected = selected_node == Some(node);
        let is_renaming = renaming_node == Some(node)
            && (renaming_origin.is_none() || renaming_origin == Some(RenameOrigin::Hierarchy));
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

            if is_renaming {
                let edit_width = 150.0_f32.min(ui.available_width() - icon_size.x - 8.0);
                let text_edit_response = ui.add(
                    TextEdit::singleline(rename_buffer)
                        .desired_width(edit_width)
                        .id_salt(node_data.guid.to_string()),
                );

                // 处理 Enter / Escape / 焦点丢失
                let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));

                if enter {
                    messager.send(Message::RenameProjectNode(
                        node.index(),
                        rename_buffer.clone(),
                    ));
                } else if escape {
                    messager.send(Message::CancelRenameProjectNode);
                } else if text_edit_response.clicked_elsewhere() {
                    messager.send(Message::RenameProjectNode(
                        node.index(),
                        rename_buffer.clone(),
                    ));
                }
            } else {
                let name = node_data.name();
                ui.label(RichText::new(name).color(style.hierachy.file_header_color));

                if let Some(suffix) = node_data.kind.suffix() {
                    ui.label(
                        RichText::new(suffix)
                            .size(style.hierachy.file_header_size)
                            .color(style.hierachy.file_suffix_color),
                    );
                }
            }
        });

        // 3. 全宽点击覆盖层（仅在非重命名模式下，重命名时 TextEdit 自己处理交互）
        if !is_renaming {
            let row_rect = egui::Rect::from_min_size(row_start, egui::vec2(row_width, row_height));
            let response = ui.interact(
                row_rect,
                ui.id().with(("row", node.index())),
                egui::Sense::click(),
            );
            if response.clicked() {
                messager.send(Message::SelectProjectNode(Some(node)));
            }
            response.context_menu(|ui| {
                ContextMenu::show(
                    ui,
                    ContextMenuState::new(node, RenameOrigin::Hierarchy),
                    messager,
                );
            });
        }
    }
}
