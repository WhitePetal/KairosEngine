use std::{cell::Cell, fs, ops::Deref, path::PathBuf, sync::Arc};

use egui::{
    Vec2,
    menu::{MenuConfig, SubMenuButton},
};
use egui_extras::{Column, TableBuilder};
use parking_lot::Mutex;
use serde::Deserialize;

use crate::{
    asset_loader::assets::{
        AssetHandle, AssetsServer, MaterialAssetsSystem, SerializedMaterialAssetsSystem,
        ShaderAssetsSystem, TextureAssetsSystem,
    }, kairos_editor::{
        asset_registry::AssetKind, project_path_tree::ProjectPathGraph, ui::{
            Message, Messager, UIReader, dialog::{ConfirmDialogWindow, Dialog}, drag::Drag, inspector::Inspector, paths, project_window::ProjectWindow,
        },
    }, math,
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
    texture_background_color:  math::Color32,
    texture_drag_hover_background_color: math::Color32,
    texture_empty_stroke_color: math::Color32,
    texture_fill_stroke_color: math::Color32,
    texture_drag_hover_stroke_color: math::Color32,
    texture_corner_radius: u8,
    texture_stroke_width: f32,
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

/// 将扁平的 (name, path) 列表按目录层级组织为树。
fn build_shader_menu_tree(shader_list: &[(String, PathBuf)]) -> Vec<ShaderMenuNode> {
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
        insert_shader_node(&mut roots, &components, name, path);
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

    insert_shader_node(&mut nodes[pos].children, rest, shader_name, shader_path);
}

/// 递归渲染 shader 层级菜单。
fn render_shader_menu(
    ui: &mut egui::Ui,
    nodes: &[ShaderMenuNode],
    current_path: &PathBuf,
    messager: &mut Messager,
    mat_path: &PathBuf,
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
                messager.send(Message::MaterialInspectorChangeShader(
                    mat_path.clone(),
                    shader_path.clone(),
                ));
            }
        } else {
            // Directory — 子菜单
            let mut menu = SubMenuButton::new(&node.display_name);
            menu.button = menu.button.min_size(Vec2::new(min_menu_width, menu_height));
            menu.config(MenuConfig::new().close_behavior(egui::PopupCloseBehavior::CloseOnClick))
                .ui(ui, |ui| {
                    render_shader_menu(
                        ui,
                        &node.children,
                        current_path,
                        messager,
                        mat_path,
                        min_menu_width,
                        menu_height,
                    );
                });
        }
    }
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

// ============================================================
// 纹理缩略图缓存
// ============================================================

/// 缓存最近的纹理缩略图 egui handle。
/// Tuple: (纹理路径, egui::TextureHandle)
struct Thumbnail {
    path: PathBuf,
    handle: egui::TextureHandle,
}

/// 为 Texture 创建 48x48 缩略图的 egui handle。
fn build_texture_thumbnail(
    ui: &egui::Ui,
    texture: &crate::graphics::texture::Texture,
) -> egui::TextureHandle {
    let rgba = crate::graphics::texture::format::decode_to_rgba8(
        &texture.data[0],
        texture.width,
        texture.height,
        texture.format,
    );
    let w = texture.width as usize;
    let h = texture.height as usize;
    let color_image = egui::ColorImage::from_rgba_unmultiplied([w, h], &rgba);
    ui.ctx().load_texture(
        "material_tex_thumb",
        color_image,
        egui::TextureOptions::LINEAR,
    )
}

// ============================================================
// Model
// ============================================================

struct MaterialInspectorModel {
    style: MaterialInspectorStyle,
    /// .mat 文件的磁盘路径
    path: PathBuf,
    /// SerializedMaterial 异步句柄（通过 assets_server.load 获取）
    serialized_handle: Arc<AssetHandle<SerializedMaterialAssetsSystem>>,
    /// 运行时 Material 句柄（通过 assets_server.load 获取）
    material_handle: Arc<AssetHandle<MaterialAssetsSystem>>,
    /// 按目录层级组织的 shader 菜单树
    shader_menu_tree: Vec<ShaderMenuNode>,
    /// 当前选中的 shader 路径，首次 draw 时从 SerializedMaterial 异步加载
    current_shader_path: Arc<Mutex<Option<PathBuf>>>,
    /// 当前选中的 texture 路径，首次 draw 时从 SerializedMaterial 异步加载
    current_texture_path: Arc<Mutex<Option<Option<PathBuf>>>>,
    /// 用户是否修改了（未保存）
    dirty: Cell<bool>,
    /// 缩略图缓存
    thumbnail: Arc<Mutex<Option<Thumbnail>>>,
}

// ============================================================
// MaterialInspector
// ============================================================

pub struct MaterialInspector {
    model: MaterialInspectorModel,
}

impl MaterialInspector {
    fn draw_texture_col(&self, ui: &mut egui::Ui, reader: &UIReader, messager: &mut Messager, assets_server: &AssetsServer) {
        ui.horizontal(|ui| {
            self.draw_texture_rect(ui, reader, messager, assets_server);

            let texture_guard = self.model.current_texture_path.lock();
            let current_texture_path = texture_guard.deref();

            if let Some(Some(texture_path)) = current_texture_path {
                ui.label(texture_path.to_string_lossy());
            }

            if ui.button("X").clicked() {
                messager.send(Message::MaterialInspectorClearTexture(
                    self.model.path.clone(),
                ));
            }
        });
    }

    fn draw_texture_rect(&self, ui: &mut egui::Ui, reader: &UIReader, messager: &mut Messager, assets_server: &AssetsServer) {
        let texture_size = self.model.style.texture_label_height;
        let texture_size = Vec2::new(texture_size, texture_size);
        let texture_rect = egui::Rect::from_min_size(ui.cursor().min, texture_size);

        let dragging = reader
            .get_drawer::<ProjectWindow>()
            .and_then(|d| d.get_dragging().as_ref());
        let mut drop_path = None;
        let is_valid_drag = dragging.is_some_and(|d| {
            let valid = d.get().extension().and_then(|e| e.to_str()) == AssetKind::Texture.extension();
            if valid {
                drop_path = check_texture_drop(ui, d, texture_rect);
            }
            valid
        });

        // 处理拖放结果
        if let Some(texture_path) = drop_path {
            // 清除缩略图缓存，下次 draw 会重新生成
            *self.model.thumbnail.lock() = None;
            messager.send(Message::MaterialInspectorDropTexture(
                self.model.path.clone(),
                texture_path,
            ));
        }

        // let pointer_down = ui.input(|i| i.pointer.any_down());
        let pointer_over = ui.input(|i| i.pointer.interact_pos()).map_or(false, |p| {
            texture_rect.contains(p)
        });
        let is_drag_hover = pointer_over && is_valid_drag;

        // ── 背景 + 边框 ──
        let bg_color = if is_drag_hover {
            self.model.style.texture_drag_hover_background_color
        } else {
            self.model.style.texture_background_color
        };

        let texture_guard = self.model.current_texture_path.lock();
        let current_texture_path = texture_guard.deref();

        let stroke_color = if is_drag_hover {
            self.model.style.texture_drag_hover_stroke_color
        } else if current_texture_path.is_some() {
            self.model.style.texture_fill_stroke_color
        } else {
            self.model.style.texture_empty_stroke_color
        };

        let (_texture_response, painter) = ui.allocate_painter(
            texture_size,
            egui::Sense::click(),
        );
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
            egui::pos2(
                texture_rect.min.x + padding,
                texture_rect.min.y + padding,
            ),
            Vec2::splat(thumb_size),
        );
        if let Some(Some(texture_path)) = current_texture_path {
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
                            let handle = build_texture_thumbnail(ui, texture);
                            *thumb_guard = Some(Thumbnail {
                                path: texture_path.clone(),
                                handle,
                            });
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
        let shader_menu_tree = build_shader_menu_tree(&shader_list);

        let model = MaterialInspectorModel {
            style,
            path: mat_path,
            serialized_handle,
            material_handle,
            shader_menu_tree,
            current_shader_path: Arc::new(Mutex::new(None)),
            current_texture_path: Arc::new(Mutex::new(None)),
            dirty: Cell::new(false),
            thumbnail: Arc::new(Mutex::new(None)),
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
        // ---- 异步加载 SerializedMaterial，获取 shader_path 和 texture_path ----
        {
            let mut current_shader = self.model.current_shader_path.lock();
            let mut current_texture = self.model.current_texture_path.lock();
            if current_shader.is_none() || current_texture.is_none() {
                if let Some(serialized) = assets_server
                    .get::<SerializedMaterialAssetsSystem>(&self.model.serialized_handle)
                {
                    if current_shader.is_none() {
                        *current_shader = Some(serialized.shader_path.clone());
                    }
                    if current_texture.is_none() {
                        *current_texture = Some(serialized.texture_path.clone());
                    }
                } else {
                    ui.label("Material is loading...");
                    return;
                }
            }
        }

        let current_shader_path = {
            let guard = self.model.current_shader_path.lock();
            guard.clone().unwrap()
        };

        // ---- Source path ----
        ui.label(format!("Source: {}", self.model.path.display()));
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
                            render_shader_menu(
                                ui,
                                &self.model.shader_menu_tree,
                                &current_shader_path,
                                messager,
                                &self.model.path,
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
            .resizable(true)
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
                    self.draw_texture_col(ui, reader, messager, assets_server);
                });
            });
        });

        // ---- Dirty indicator ----
        if self.model.dirty.get() {
            ui.label("* unsaved changes");
        }
    }

    fn on_exit(&mut self, _ctx: &egui::Context) -> Option<Box<dyn Dialog>> {
        if self.model.dirty.get() {
            let _path = self.model.path.clone();
            // TODO: MaterialInspector save will be implemented in a future sprint.
            let dialog = ConfirmDialogWindow::new(
                "Unsaved Material Changes".into(),
                "Material has unsaved changes. Discard?".into(),
                "Discard".into(),
                "Cancel".into(),
                None::<Message>,
                None,
                None::<fn()>,
                None::<fn()>,
            );
            Some(Box::new(dialog))
        } else {
            None
        }
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
        *self.model.current_shader_path.lock() = Some(new_shader_path);
        self.model.dirty.set(true);
    }

    /// 拖入 / 设置纹理：加载 texture → 更新运行时 Material → 标记 dirty
    pub fn drop_texture(&mut self, assets_server: &mut AssetsServer, texture_path: PathBuf) {
        // 1. 加载 texture（异步，句柄立即返回）
        let texture_handle = assets_server.load::<TextureAssetsSystem>(&texture_path);

        // 2. 更新运行时 Material
        let material = assets_server.get_mut(&self.model.material_handle);
        if let Some(material) = material {
            material.texture = Some(texture_handle);
        }

        // 3. 清除缩略图缓存（下次 draw 重新生成）
        *self.model.thumbnail.lock() = None;

        // 4. 更新数据状态
        *self.model.current_texture_path.lock() = Some(Some(texture_path));
        self.model.dirty.set(true);
    }

    /// 清除纹理：清空运行时 Material 的 texture → 标记 dirty
    pub fn clear_texture(&mut self, assets_server: &mut AssetsServer) {
        // 1. 更新运行时 Material
        let material = assets_server.get_mut(&self.model.material_handle);
        if let Some(material) = material {
            material.texture = None;
        }

        // 2. 清除缩略图缓存
        *self.model.thumbnail.lock() = None;

        // 3. 更新数据状态
        *self.model.current_texture_path.lock() = Some(None);
        self.model.dirty.set(true);
    }
}
