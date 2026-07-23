use std::{cell::Cell, fs, ops::DerefMut, path::PathBuf, sync::Arc};

use egui::{
    Vec2,
    menu::{MenuConfig, SubMenuButton},
};
use egui_extras::{Column, TableBuilder};
use parking_lot::Mutex;
use serde::Deserialize;
use strum::IntoEnumIterator;

use crate::{
    asset_loader::assets::{
        AssetHandle, AssetsServer, MaterialAssetsSystem, SerializedMaterialAssetsSystem,
        ShaderAssetsSystem, TextureAssetsSystem,
    },
    graphics::{
        compare_function::CompareFunction,
        material::SerializedMaterial,
        render_state::{
            BlendFactor, BlendOperation, BlendPreset, BlendState, CullMode, PrimitiveTopology,
            RenderState,
        },
    },
    kairos_editor::{
        asset_registry::AssetKind,
        project_path_tree::ProjectPathGraph,
        ui::{
            Message, Messager, UIReader,
            dialog::{ConfirmDialogWindow, Dialog},
            drag::Drag,
            inspector::Inspector,
            paths,
            project_window::ProjectWindow,
        },
    },
    math,
};

// ============================================================
// Style
// ============================================================

#[derive(Debug, Deserialize)]
struct MaterialInspectorStyle {
    shader_selector_height: f32,
    shader_selector_menu_border: f32,
    shader_selector_menu_min_width: f32,
    shader_selector_submenu_width_factor: f32,

    texture_label_height: f32,
    texture_background_color: math::Color32,
    texture_drag_hover_background_color: math::Color32,
    texture_empty_stroke_color: math::Color32,
    texture_fill_stroke_color: math::Color32,
    texture_drag_hover_stroke_color: math::Color32,
    texture_corner_radius: u8,
    texture_stroke_width: f32,

    render_state_row_height: f32,
    render_state_sub_row_indent: f32,

    apply_button_height: f32,
}

impl MaterialInspectorStyle {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let bytes = fs::read(paths::PATH_MATERIAL_INSPECTOR_STYLE).map_err(|error| {
            format!(
                "Load MaterialInspector Style Failed, path: {}, error: {}",
                paths::PATH_MATERIAL_INSPECTOR_STYLE,
                error
            )
        })?;
        let style = toml::from_slice(&bytes)?;
        Ok(style)
    }
}

// ============================================================
// Shader 层级菜单树
// ============================================================

/// Shader 下拉菜单中的节点：目录或 shader 文件。
struct ShaderMenuNode {
    /// 显示名称（目录名或文件名）
    display_name: String,
    /// 完整路径（shader 文件节点有值，目录节点为 None）
    full_path: Option<PathBuf>,
    /// 子节点（目录节点的子目录/文件）
    children: Vec<ShaderMenuNode>,
}

impl ShaderMenuNode {
    /// 将扁平的 (name, path) 列表按目录层级组织为树。
    pub fn build_tree(shader_list: &[(String, PathBuf)]) -> Vec<Self> {
        let mut roots: Vec<ShaderMenuNode> = Vec::new();

        for (name, path) in shader_list {
            let parent = path.parent().unwrap_or(std::path::Path::new(""));
            let components: Vec<&str> = parent
                .components()
                .filter_map(|c| match c {
                    std::path::Component::Normal(s) => s.to_str(),
                    _ => None,
                })
                .collect();
            Self::insert_shader_node(&mut roots, &components, name, path);
        }

        roots
    }

    fn insert_shader_node(
        nodes: &mut Vec<ShaderMenuNode>,
        components: &[&str],
        shader_name: &str,
        shader_path: &PathBuf,
    ) {
        if components.is_empty() {
            // 根级 shader，直接添加为叶子节点
            nodes.push(ShaderMenuNode {
                display_name: shader_name.to_string(),
                full_path: Some(shader_path.clone()),
                children: Vec::new(),
            });
            return;
        }

        let dir_name = components[0];
        let rest = &components[1..];

        let pos = nodes.iter().position(|n| n.display_name == dir_name);
        let pos = pos.unwrap_or_else(|| {
            nodes.push(ShaderMenuNode {
                display_name: dir_name.to_string(),
                full_path: None,
                children: Vec::new(),
            });
            nodes.len() - 1
        });

        Self::insert_shader_node(&mut nodes[pos].children, rest, shader_name, shader_path);
    }
}

// ============================================================
// 纹理缩略图缓存
// ============================================================

/// 缓存最近的纹理缩略图 egui handle。
/// Tuple: (纹理路径, egui::TextureHandle)
struct Thumbnail {
    path: PathBuf,
    handle: egui::TextureHandle,
}

impl Thumbnail {
    pub fn new(ui: &egui::Ui, path: &PathBuf, texture: &crate::graphics::texture::Texture) -> Self {
        let rgba = crate::graphics::texture::format::decode_to_rgba8(
            &texture.data[0],
            texture.width,
            texture.height,
            texture.format,
        );
        let w = texture.width as usize;
        let h = texture.height as usize;
        let color_image = egui::ColorImage::from_rgba_unmultiplied([w, h], &rgba);
        let handle = ui.ctx().load_texture(
            "material_tex_thumb",
            color_image,
            egui::TextureOptions::LINEAR,
        );

        Self {
            path: path.clone(),
            handle,
        }
    }
}

// ============================================================
// Model
// ============================================================

struct MaterialInspectorModel {
    style: MaterialInspectorStyle,
    serialized_handle: Arc<AssetHandle<SerializedMaterialAssetsSystem>>,
    /// SerializedMaterial
    serialized_material: Arc<Mutex<Option<SerializedMaterial>>>,
    /// 运行时 Material 句柄（通过 assets_server.load 获取）
    material_handle: Arc<AssetHandle<MaterialAssetsSystem>>,
    /// 按目录层级组织的 shader 菜单树
    shader_menu_tree: Vec<ShaderMenuNode>,
    /// 缩略图缓存
    thumbnail: Arc<Mutex<Option<Thumbnail>>>,
    /// 用户在 blend 下拉中显式选择了 Custom（即使当前值仍命中某个预设也展开子字段）
    blend_custom_expanded: Cell<bool>,
    /// 用户是否修改了（未保存）
    dirty: Cell<bool>,
}

// ============================================================
// MaterialInspector
// ============================================================

pub struct MaterialInspector {
    model: MaterialInspectorModel,
}

impl MaterialInspector {
    // ============================================================
    // Blend 预设映射（issue #33）
    // ============================================================

    /// 从 RenderState 推导当前应显示的 blend 选项。
    /// `blend_mod = None`（不混合）→ None 选项；
    /// `Some(state)` 匹配预设，不匹配任何预设时为 Custom。
    fn current_blend_option(render_state: &RenderState) -> Option<BlendPreset> {
        render_state.blend_mod.map(BlendPreset::from_blend_state)
    }

    /// 将选中的 blend 选项写回 RenderState。
    /// None 选项写回 `blend_mod = None`；预设 / Custom 写回 `Some(BlendState)`。
    fn apply_blend_option(render_state: &mut RenderState, option: Option<BlendPreset>) {
        render_state.blend_mod = option.map(|preset| preset.to_blend_state());
    }

    // ============================================================
    // RenderState 编辑行（issue #33）
    // ============================================================

    /// BlendFactor 下拉行。返回 Some(new_value) 表示用户修改了该字段。
    fn draw_blend_factor_combo_row(
        &self,
        body: &mut egui_extras::TableBody,
        label: &str,
        id_salt: &str,
        current: BlendFactor,
    ) -> Option<BlendFactor> {
        let mut changed_to = None;
        body.row(self.model.style.render_state_row_height, |mut row| {
            row.col(|ui| {
                ui.horizontal(|ui| {
                    ui.add_space(self.model.style.render_state_sub_row_indent);
                    ui.label(label);
                });
            });
            row.col(|ui| {
                let mut selected = current;
                egui::ComboBox::from_id_salt(id_salt)
                    .selected_text(current.label())
                    .show_ui(ui, |ui| {
                        for factor in BlendFactor::iter() {
                            if ui
                                .selectable_value(&mut selected, factor, factor.label())
                                .changed()
                            {
                                changed_to = Some(factor);
                            }
                        }
                    });
            });
        });
        changed_to
    }

    /// BlendOperation 下拉行。返回 Some(new_value) 表示用户修改了该字段。
    fn draw_blend_operation_combo_row(
        &self,
        body: &mut egui_extras::TableBody,
        label: &str,
        id_salt: &str,
        current: BlendOperation,
    ) -> Option<BlendOperation> {
        let mut changed_to = None;
        body.row(self.model.style.render_state_row_height, |mut row| {
            row.col(|ui| {
                ui.horizontal(|ui| {
                    ui.add_space(self.model.style.render_state_sub_row_indent);
                    ui.label(label);
                });
            });
            row.col(|ui| {
                let mut selected = current;
                egui::ComboBox::from_id_salt(id_salt)
                    .selected_text(current.label())
                    .show_ui(ui, |ui| {
                        for operation in BlendOperation::iter() {
                            if ui
                                .selectable_value(&mut selected, operation, operation.label())
                                .changed()
                            {
                                changed_to = Some(operation);
                            }
                        }
                    });
            });
        });
        changed_to
    }

    // ============================================================
    // Texture 拖拽相关（跨帧持久化）
    // ============================================================

    /// 在 drop target 区域检查是否发生了拖放操作。
    /// 返回值：Some(texture_path) 表示拖放有效，None 表示无操作。
    fn check_texture_drop(
        ui: &egui::Ui,
        drag: &Drag<PathBuf>,
        target_rect: egui::Rect,
    ) -> Option<PathBuf> {
        let pointer_pos = ui.input(|i| i.pointer.interact_pos())?;
        if !target_rect.contains(pointer_pos) {
            return None;
        }

        match drag {
            Drag::Draging(_) => None,
            Drag::Stoped(path) => Some(path.clone()),
        }
    }

    fn draw_texture_col(
        &self,
        ui: &mut egui::Ui,
        reader: &UIReader,
        messager: &mut Messager,
        assets_server: &AssetsServer,
        current_texture_path: &Option<PathBuf>,
    ) {
        ui.horizontal(|ui| {
            self.draw_texture_rect(ui, reader, messager, assets_server, current_texture_path);

            if let Some(texture_path) = current_texture_path {
                if ui.button("X").clicked() {
                    messager.send(Message::MaterialInspectorClearTexture);
                }
                ui.add(egui::Label::new(texture_path.to_string_lossy()).extend());
            }
        });
    }

    fn draw_texture_rect(
        &self,
        ui: &mut egui::Ui,
        reader: &UIReader,
        messager: &mut Messager,
        assets_server: &AssetsServer,
        current_texture_path: &Option<PathBuf>,
    ) {
        let texture_size = self.model.style.texture_label_height;
        let texture_size = Vec2::new(texture_size, texture_size);
        let texture_rect = egui::Rect::from_min_size(ui.cursor().min, texture_size);

        let dragging = reader
            .get_drawer::<ProjectWindow>()
            .and_then(|d| d.get_dragging().as_ref());
        let mut drop_path = None;
        let is_valid_drag = dragging.is_some_and(|d| {
            let valid =
                d.get().extension().and_then(|e| e.to_str()) == AssetKind::Texture.extension();
            if valid {
                drop_path = Self::check_texture_drop(ui, d, texture_rect);
            }
            valid
        });

        // 处理拖放结果
        if let Some(texture_path) = drop_path {
            // 清除缩略图缓存，下次 draw 会重新生成
            *self.model.thumbnail.lock() = None;
            messager.send(Message::MaterialInspectorDropTexture(texture_path));
        }

        // let pointer_down = ui.input(|i| i.pointer.any_down());
        let pointer_over = ui
            .input(|i| i.pointer.interact_pos())
            .map_or(false, |p| texture_rect.contains(p));
        let is_drag_hover = pointer_over && is_valid_drag;

        // ── 背景 + 边框 ──
        let bg_color = if is_drag_hover {
            self.model.style.texture_drag_hover_background_color
        } else {
            self.model.style.texture_background_color
        };

        let stroke_color = if is_drag_hover {
            self.model.style.texture_drag_hover_stroke_color
        } else if matches!(current_texture_path, Some(_)) {
            self.model.style.texture_fill_stroke_color
        } else {
            self.model.style.texture_empty_stroke_color
        };

        let (_texture_response, painter) = ui.allocate_painter(texture_size, egui::Sense::click());
        painter.rect(
            texture_rect,
            egui::CornerRadius::same(self.model.style.texture_corner_radius),
            bg_color,
            egui::Stroke::new(self.model.style.texture_stroke_width, stroke_color),
            egui::StrokeKind::Middle,
        );

        let padding = 2.0;
        let thumb_size = texture_size.x - padding - padding;
        let thumb_rect = egui::Rect::from_min_size(
            egui::pos2(texture_rect.min.x + padding, texture_rect.min.y + padding),
            Vec2::splat(thumb_size),
        );
        if let Some(texture_path) = current_texture_path {
            // ── 缩略图 ──
            // 尝试从缓存或 assets_server 取缩略图
            let mut thumb_guard = self.model.thumbnail.lock();
            let thumb_mismatch = thumb_guard
                .as_ref()
                .map_or(true, |thumb| thumb.path != *texture_path);

            if thumb_mismatch {
                // 路径变了，重新生成
                *thumb_guard = None;
                // 异步加载 texture 并生成缩略图
                if let Some(mat) =
                    assets_server.get::<MaterialAssetsSystem>(&self.model.material_handle)
                {
                    if let Some(tex_handle) = &mat.texture {
                        if let Some(texture) = assets_server.get::<TextureAssetsSystem>(tex_handle)
                        {
                            *thumb_guard = Some(Thumbnail::new(ui, texture_path, texture));
                        }
                    }
                }
            }

            if let Some(ref thumb) = *thumb_guard {
                // 缩略图可用 → 绘制
                ui.painter().image(
                    thumb.handle.id(),
                    thumb_rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );

                return;
            }
        }

        // 缩略图不可用 → 占位色块
        if !is_drag_hover {
            ui.painter().rect_filled(
                thumb_rect,
                egui::CornerRadius::same(self.model.style.texture_corner_radius),
                egui::Color32::from_gray(10),
            );
        }
    }

    /// 递归渲染 shader 层级菜单。
    fn draw_shader_menu(
        &self,
        ui: &mut egui::Ui,
        nodes: &[ShaderMenuNode],
        current_path: &PathBuf,
        messager: &mut Messager,
        min_menu_width: f32,
        menu_height: f32,
    ) {
        for node in nodes {
            if let Some(ref shader_path) = node.full_path {
                // Leaf — shader 文件
                let is_selected = shader_path == current_path;
                if ui
                    .selectable_label(is_selected, &node.display_name)
                    .clicked()
                {
                    messager.send(Message::MaterialInspectorChangeShader(shader_path.clone()));
                }
            } else {
                // Directory — 子菜单
                let mut menu = SubMenuButton::new(&node.display_name);
                menu.button = menu.button.min_size(Vec2::new(min_menu_width, menu_height));
                menu.config(
                    MenuConfig::new().close_behavior(egui::PopupCloseBehavior::CloseOnClick),
                )
                .ui(ui, |ui| {
                    self.draw_shader_menu(
                        ui,
                        &node.children,
                        current_path,
                        messager,
                        min_menu_width,
                        menu_height,
                    );
                });
            }
        }
    }

    /// 发送 RenderState 变更消息（由 Context::handle 回写运行时 Material + 缓存 + dirty）
    fn send_render_state(&self, messager: &mut Messager, new_state: RenderState) {
        messager.send(Message::MaterialInspectorChangeRenderState(new_state));
    }

    /// 以 `effective_blend` 为底应用一处子字段修改，发送新的 RenderState。
    fn send_blend_edit(
        &self,
        messager: &mut Messager,
        render_state: &RenderState,
        effective_blend: BlendState,
        edit: impl FnOnce(&mut BlendState),
    ) {
        let mut blend = effective_blend;
        edit(&mut blend);
        self.send_render_state(
            messager,
            RenderState {
                blend_mod: Some(blend),
                ..*render_state
            },
        );
    }

    /// Render State 编辑区域（issue #33）：
    /// depth_test（None + CompareFunction）/ depth_write / cull_mode /
    /// blend_mode（预设 + Custom 展开 6 子字段）/ topology。
    /// 每次修改发送 Message 回写运行时 Material 并标记 dirty。
    fn draw_render_state_section(
        &self,
        ui: &mut egui::Ui,
        messager: &mut Messager,
        render_state: &RenderState,
    ) {
        let row_h = self.model.style.render_state_row_height;
        TableBuilder::new(ui)
            .striped(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::LEFT))
            .column(Column::auto())
            .column(Column::remainder())
            .id_salt("render_state")
            .body(|mut body| {
                // ---- Depth Test：None + CompareFunction 各选项 ----
                body.row(row_h, |mut row| {
                    row.col(|ui| {
                        ui.label("Depth Test");
                    });
                    row.col(|ui| {
                        let mut selected = render_state.depth_test;
                        egui::ComboBox::from_id_salt("mat_depth_test")
                            .selected_text(selected.map_or("None", |cf| cf.label()))
                            .show_ui(ui, |ui| {
                                if ui.selectable_value(&mut selected, None, "None").changed() {
                                    self.send_render_state(
                                        messager,
                                        RenderState {
                                            depth_test: None,
                                            ..*render_state
                                        },
                                    );
                                }
                                for cf in CompareFunction::iter() {
                                    if ui
                                        .selectable_value(&mut selected, Some(cf), cf.label())
                                        .changed()
                                    {
                                        self.send_render_state(
                                            messager,
                                            RenderState {
                                                depth_test: Some(cf),
                                                ..*render_state
                                            },
                                        );
                                    }
                                }
                            });
                    });
                });

                // ---- Depth Write（depth_test = None 时 wgpu 不允许 write：禁用并显示生效值 false）----
                body.row(row_h, |mut row| {
                    row.col(|ui| {
                        ui.label("Depth Write");
                    });
                    row.col(|ui| {
                        // 显示生效值：depth_test = None 时强制为 false，
                        // 避免“勾选但不可用”的困惑；存储值保持不变
                        let mut checked = render_state.depth_write_enable();
                        ui.add_enabled_ui(render_state.depth_test.is_some(), |ui| {
                            if ui.checkbox(&mut checked, "").changed() {
                                self.send_render_state(
                                    messager,
                                    RenderState {
                                        depth_write: checked,
                                        ..*render_state
                                    },
                                );
                            }
                        });
                    });
                });

                // ---- Cull Mode ----
                body.row(row_h, |mut row| {
                    row.col(|ui| {
                        ui.label("Cull Mode");
                    });
                    row.col(|ui| {
                        let mut selected = render_state.cull_mod;
                        egui::ComboBox::from_id_salt("mat_cull_mode")
                            .selected_text(selected.label())
                            .show_ui(ui, |ui| {
                                for mode in CullMode::iter() {
                                    if ui
                                        .selectable_value(&mut selected, mode, mode.label())
                                        .changed()
                                    {
                                        self.send_render_state(
                                            messager,
                                            RenderState {
                                                cull_mod: mode,
                                                ..*render_state
                                            },
                                        );
                                    }
                                }
                            });
                    });
                });

                // ---- Blend Mode：None + 预设下拉，Custom 时展开 6 个子字段 ----
                let effective_blend = render_state.blend_mod.unwrap_or(BlendState::REPLACE);
                let current_option = Self::current_blend_option(&render_state);
                let show_custom = matches!(current_option, Some(BlendPreset::Custom(_)))
                    || self.model.blend_custom_expanded.get();
                let option_label =
                    |option: Option<BlendPreset>| option.map_or("None", |preset| preset.label());
                body.row(row_h, |mut row| {
                    row.col(|ui| {
                        ui.label("Blend Mode");
                    });
                    row.col(|ui| {
                        let mut selected = if show_custom {
                            Some(BlendPreset::Custom(effective_blend))
                        } else {
                            current_option
                        };
                        egui::ComboBox::from_id_salt("mat_blend_mode")
                            .selected_text(option_label(selected))
                            .show_ui(ui, |ui| {
                                for option in [
                                    None,
                                    Some(BlendPreset::Replace),
                                    Some(BlendPreset::Add),
                                    Some(BlendPreset::Multiply),
                                    Some(BlendPreset::AlphaBlend),
                                    Some(BlendPreset::Custom(effective_blend)),
                                ] {
                                    if ui
                                        .selectable_value(
                                            &mut selected,
                                            option,
                                            option_label(option),
                                        )
                                        .changed()
                                    {
                                        match option {
                                            Some(BlendPreset::Custom(_)) => {
                                                // 展开子字段编辑；blend 值本身不变
                                                self.model.blend_custom_expanded.set(true);
                                                self.send_render_state(
                                                    messager,
                                                    RenderState {
                                                        blend_mod: Some(effective_blend),
                                                        ..*render_state
                                                    },
                                                );
                                            }
                                            other => {
                                                self.model.blend_custom_expanded.set(false);
                                                let mut new_state = *render_state;
                                                Self::apply_blend_option(&mut new_state, other);
                                                self.send_render_state(messager, new_state);
                                            }
                                        }
                                    }
                                }
                            });
                    });
                });

                // ---- Custom 展开：color/alpha 的 srcFactor / dstFactor / operation（缩进表达附属）----
                if show_custom {
                    if let Some(factor) = self.draw_blend_factor_combo_row(
                        &mut body,
                        "Color Src Factor",
                        "mat_blend_color_src_factor",
                        effective_blend.color.src_factor,
                    ) {
                        self.send_blend_edit(messager, &render_state, effective_blend, |b| {
                            b.color.src_factor = factor
                        });
                    }
                    if let Some(factor) = self.draw_blend_factor_combo_row(
                        &mut body,
                        "Color Dst Factor",
                        "mat_blend_color_dst_factor",
                        effective_blend.color.dst_factor,
                    ) {
                        self.send_blend_edit(messager, &render_state, effective_blend, |b| {
                            b.color.dst_factor = factor
                        });
                    }
                    if let Some(operation) = self.draw_blend_operation_combo_row(
                        &mut body,
                        "Color Operation",
                        "mat_blend_color_operation",
                        effective_blend.color.operation,
                    ) {
                        self.send_blend_edit(messager, &render_state, effective_blend, |b| {
                            b.color.operation = operation
                        });
                    }
                    if let Some(factor) = self.draw_blend_factor_combo_row(
                        &mut body,
                        "Alpha Src Factor",
                        "mat_blend_alpha_src_factor",
                        effective_blend.alpha.src_factor,
                    ) {
                        self.send_blend_edit(messager, &render_state, effective_blend, |b| {
                            b.alpha.src_factor = factor
                        });
                    }
                    if let Some(factor) = self.draw_blend_factor_combo_row(
                        &mut body,
                        "Alpha Dst Factor",
                        "mat_blend_alpha_dst_factor",
                        effective_blend.alpha.dst_factor,
                    ) {
                        self.send_blend_edit(messager, &render_state, effective_blend, |b| {
                            b.alpha.dst_factor = factor
                        });
                    }
                    if let Some(operation) = self.draw_blend_operation_combo_row(
                        &mut body,
                        "Alpha Operation",
                        "mat_blend_alpha_operation",
                        effective_blend.alpha.operation,
                    ) {
                        self.send_blend_edit(messager, &render_state, effective_blend, |b| {
                            b.alpha.operation = operation
                        });
                    }
                }

                // ---- Topology ----
                body.row(row_h, |mut row| {
                    row.col(|ui| {
                        ui.label("Topology");
                    });
                    row.col(|ui| {
                        let mut selected = render_state.topology;
                        egui::ComboBox::from_id_salt("mat_topology")
                            .selected_text(selected.label())
                            .show_ui(ui, |ui| {
                                for topology in PrimitiveTopology::iter() {
                                    if ui
                                        .selectable_value(&mut selected, topology, topology.label())
                                        .changed()
                                    {
                                        self.send_render_state(
                                            messager,
                                            RenderState {
                                                topology,
                                                ..*render_state
                                            },
                                        );
                                    }
                                }
                            });
                    });
                });
            });
    }
}

impl Inspector for MaterialInspector {
    fn create(
        path: &std::path::Path,
        assets_server: &mut AssetsServer,
        project_graph: &ProjectPathGraph,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let style = MaterialInspectorStyle::new()?;
        let mat_path = path.to_path_buf();

        // 通过 SerializedMaterialAssetsSystem 异步加载 .mat 文件
        let serialized_handle = assets_server.load::<SerializedMaterialAssetsSystem>(&mat_path);

        // 加载运行时 Material（异步，句柄立即返回）
        let material_handle = assets_server.load::<MaterialAssetsSystem>(&mat_path);

        // 查询项目图中所有 Shader 节点，按目录层级组织
        let shader_list: Vec<(String, PathBuf)> = project_graph
            .find_assets_by_kind(AssetKind::Shader)
            .iter()
            .map(|node| (node.name(), node.path.clone()))
            .collect();
        let shader_menu_tree = ShaderMenuNode::build_tree(&shader_list);

        let model = MaterialInspectorModel {
            style,
            serialized_handle,
            serialized_material: Arc::new(Mutex::new(None)),
            material_handle,
            shader_menu_tree,
            thumbnail: Arc::new(Mutex::new(None)),
            blend_custom_expanded: Cell::new(false),
            dirty: Cell::new(false),
        };

        Ok(Self { model })
    }

    fn draw(
        &self,
        ui: &mut egui::Ui,
        reader: &UIReader,
        messager: &mut Messager,
        assets_server: &AssetsServer,
        _dt: f32,
    ) {
        // ---- Cmd/Ctrl+S 快捷键触发 Apply（issue #36，ADR §4.5.2）----
        // modifiers.command：macOS = ⌘、Windows/Linux = Ctrl，跨平台保存习惯一致
        // 与 Apply 按钮走同一条消息路径，仅在有未保存修改时触发
        if self.model.dirty.get()
            && ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S))
        {
            messager.send(self.apply_message());
        }

        let mut serialize_mat = self.model.serialized_material.lock();
        let Some(serialize_mat) = serialize_mat.deref_mut() else {
            if let Some(serialized) = assets_server.get(&self.model.serialized_handle) {
                *serialize_mat = Some(serialized.clone());
            }
            ui.label("Material is loading...");
            return;
        };

        let current_shader_path = &serialize_mat.shader_path;

        // ---- Source path ----
        ui.label(format!("Source: {:?}", serialize_mat.source_path));
        ui.separator();

        // ---- Shader 层级下拉菜单 ----
        let current_shader_name = current_shader_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown");

        let shader_selecter = TableBuilder::new(ui)
            .resizable(false)
            .striped(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::LEFT))
            .column(Column::auto())
            .column(Column::remainder())
            .id_salt("shader");
        shader_selecter.body(|mut body| {
            body.row(self.model.style.shader_selector_height, |mut row| {
                row.col(|ui| {
                    ui.label("Shader");
                });
                row.col(|ui| {
                    let mut menu = SubMenuButton::new(current_shader_name);
                    let menu_width = (ui.available_width()
                        - self.model.style.shader_selector_menu_border)
                        .max(self.model.style.shader_selector_menu_min_width);
                    let menu_height = ui.available_height();
                    menu.button = menu.button.min_size(Vec2::new(menu_width, menu_height));
                    let (shader_response, _) = menu
                        .config(
                            MenuConfig::new()
                                .close_behavior(egui::PopupCloseBehavior::CloseOnClick),
                        )
                        .ui(ui, |_ui| {});
                    egui::Popup::menu(&shader_response)
                        .gap(4.0)
                        .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
                        .show(|ui| {
                            self.draw_shader_menu(
                                ui,
                                &self.model.shader_menu_tree,
                                &current_shader_path,
                                messager,
                                (menu_width
                                    * self.model.style.shader_selector_submenu_width_factor)
                                    .max(self.model.style.shader_selector_menu_min_width),
                                menu_height,
                            );
                        });
                });
            });
        });

        ui.separator();

        // ---- Table 区域----
        let table = TableBuilder::new(ui)
            .striped(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::LEFT))
            .column(Column::auto())
            .column(Column::remainder())
            .id_salt("table");
        table.body(|mut body| {
            let texture_size = self.model.style.texture_label_height;
            body.row(texture_size, |mut row| {
                row.col(|ui| {
                    ui.label("Texture");
                });
                row.col(|ui| {
                    self.draw_texture_col(
                        ui,
                        reader,
                        messager,
                        assets_server,
                        &serialize_mat.texture_path,
                    );
                });
            });
        });

        ui.separator();

        // ---- Render State 编辑区域（issue #33）----
        self.draw_render_state_section(ui, messager, &serialize_mat.render_state);

        ui.separator();

        // ---- Apply 按钮（issue #36）----
        let changed = self.model.dirty.get();
        ui.vertical_centered(|ui| {
            // Tag for test harness: widget rect collection
            ui.push_id("apply_button", |ui| {
                let apply_btn = egui::Button::new("Apply").min_size(Vec2::new(
                    ui.available_width(),
                    self.model.style.apply_button_height,
                ));
                let resp = ui.add_enabled(changed, apply_btn);

                if resp.clicked() {
                    messager.send(self.apply_message());
                }
                if changed {
                    ui.label("* unsaved changes");
                }
            }); // push_id("apply_button")
        });
    }

    fn on_exit(&mut self, _ctx: &egui::Context) -> Option<Box<dyn Dialog>> {
        if !self.model.dirty.get() {
            return None;
        }

        // 同 TextureInspector 模式：确认 = Apply（保存后关闭），取消 = Discard（丢弃修改）
        // Discard 同时还原运行时 Material 的内存改动（编辑是 edit-in-place）
        let dialog = ConfirmDialogWindow::new(
            "Unsaved material changes".into(),
            "Apply the changes before leaving?".into(),
            "Apply".into(),
            "Discard".into(),
            Some(self.apply_message()),
            Some(self.discard_message()),
            None::<fn()>,
            None::<fn()>,
        );
        Some(Box::new(dialog))
    }
}

impl MaterialInspector {
    /// 切换 shader：加载新的 shader → 合并现有 Material → 插入运行时 → 标记 dirty
    pub fn change_shader(&mut self, assets_server: &mut AssetsServer, new_shader_path: PathBuf) {
        // 1. 加载新 shader（异步，句柄立即返回）
        let new_shader_handle = assets_server.load::<ShaderAssetsSystem>(&new_shader_path);

        // 2. 修改现有运行时 Material
        let material = assets_server.get_mut(&self.model.material_handle);
        if let Some(material) = material {
            material.shader = Some(new_shader_handle);
        }

        // 3. 更新数据状态
        if let Some(serialized) = self.model.serialized_material.lock().deref_mut() {
            serialized.shader_path = new_shader_path;
        }
        self.model.dirty.set(true);
    }

    /// 拖入 / 设置纹理：加载 texture → 更新运行时 Material → 标记 dirty
    pub fn drop_texture(&mut self, assets_server: &mut AssetsServer, texture_path: PathBuf) {
        // 加载 texture（异步，句柄立即返回）
        let texture_handle = assets_server.load::<TextureAssetsSystem>(&texture_path);

        // 更新运行时 Material
        let material = assets_server.get_mut(&self.model.material_handle);
        if let Some(material) = material {
            material.texture = Some(texture_handle);
        }

        // 清除缩略图缓存（下次 draw 重新生成）
        *self.model.thumbnail.lock() = None;

        // 更新数据状态
        if let Some(serialized) = self.model.serialized_material.lock().deref_mut() {
            serialized.texture_path = Some(texture_path);
        }
        self.model.dirty.set(true);
    }

    /// 修改 RenderState：更新运行时 Material → 更新缓存 → 标记 dirty（issue #33）
    pub fn change_render_state(
        &mut self,
        assets_server: &mut AssetsServer,
        new_render_state: RenderState,
    ) {
        // 1. 更新运行时 Material（edit-in-place，立即反映到渲染）
        let material = assets_server.get_mut(&self.model.material_handle);
        if let Some(material) = material {
            material.render_state = new_render_state;
        }

        // 2. 更新数据状态
        if let Some(serialized) = self.model.serialized_material.lock().deref_mut() {
            serialized.render_state = new_render_state;
        }
        self.model.dirty.set(true);
    }

    /// 清除纹理
    pub fn clear_texture(&mut self, assets_server: &mut AssetsServer) {
        let material = assets_server.get_mut(&self.model.material_handle);
        if let Some(material) = material {
            material.texture = None;
        }

        // 清除缩略图缓存
        *self.model.thumbnail.lock() = None;

        // 更新数据状态
        if let Some(serialized) = self.model.serialized_material.lock().deref_mut() {
            serialized.texture_path = None;
        }
        self.model.dirty.set(true);
    }

    /// 构造 Apply 消息（Apply 按钮 / Ctrl+S / 关闭确认对话框共用）。
    /// 携带共享状态快照与 serialized 句柄：即使 Inspector 之后被替换，
    /// 保存仍作用于发起时的数据（同 TextureInspectorApply 模式）。
    fn apply_message(&self) -> Message {
        Message::MaterialInspectorApply(
            self.model.serialized_handle.clone(),
            self.model.serialized_material.clone(),
        )
    }

    /// 构造 Discard 消息（关闭确认对话框的 Discard 按钮）。
    /// 携带 serialized / material 句柄：即使 Inspector 之后被替换，
    /// 还原仍作用于发起时的资产。
    fn discard_message(&self) -> Message {
        Message::MaterialInspectorDiscard(
            self.model.serialized_handle.clone(),
            self.model.material_handle.clone(),
        )
    }

    /// Discard：将运行时 Material 还原为磁盘持久化状态。
    /// Inspector 的编辑是 edit-in-place（立即写入运行时 Material 以实时预览），
    /// 用户点击 Discard 后必须撤销这些内存改动。SerializedMaterial 缓存仅在
    /// 保存成功时同步、始终与磁盘一致，作为还原源；句柄按路径缓存加载，
    /// 与初始加载语义一致（None 纹理槽由渲染管线走 white.texture 降级）。
    pub fn discard_changes(
        assets_server: &mut AssetsServer,
        serialized_handle: &Arc<AssetHandle<SerializedMaterialAssetsSystem>>,
        material_handle: &Arc<AssetHandle<MaterialAssetsSystem>>,
    ) {
        // 读取持久化状态（clone 出数据后再可变借用 assets_server）
        let Some(persisted) = assets_server
            .get::<SerializedMaterialAssetsSystem>(serialized_handle)
            .cloned()
        else {
            return;
        };

        let shader_handle = assets_server.load::<ShaderAssetsSystem>(&persisted.shader_path);
        let texture_handle = persisted
            .texture_path
            .as_ref()
            .map(|p| assets_server.load::<TextureAssetsSystem>(p));

        if let Some(material) = assets_server.get_mut::<MaterialAssetsSystem>(material_handle) {
            material.shader = Some(shader_handle);
            material.texture = texture_handle;
            material.render_state = persisted.render_state;
        }
    }

    pub fn apply(&mut self) {
        self.model.dirty.set(false);
    }

    /// 将当前编辑状态写回 .mat 文件（issue #36）。
    /// 返回 true 表示保存成功；状态未加载完成或写盘失败时返回 false，
    /// 失败静默打 log、不崩溃。成功后同步更新资产系统缓存的
    /// SerializedMaterial，避免重新打开 Inspector 时读到过期数据。
    pub fn save_material(
        assets_server: &mut AssetsServer,
        serialized_handle: &Arc<AssetHandle<SerializedMaterialAssetsSystem>>,
        serizlied_mat: &Arc<Mutex<Option<SerializedMaterial>>>,
    ) {
        let mut guard = serizlied_mat.lock();
        let Some(serialized) = guard.take() else {
            return;
        };

        // 写回磁盘；失败静默打 log（不崩溃）
        if let Err(err) = serialized.save_to_file() {
            log::error!(
                "Failed to save material, error: {}, material_path: {:?}",
                err,
                serialized.source_path
            );
            return;
        }

        // 同步内存缓存，保持与磁盘一致
        if let Some(asset) =
            assets_server.get_mut::<SerializedMaterialAssetsSystem>(serialized_handle)
        {
            *asset = serialized;
        }
    }
}
