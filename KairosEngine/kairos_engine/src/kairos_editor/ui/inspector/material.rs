use std::{cell::Cell, fs, path::PathBuf, sync::Arc};

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
    },
    kairos_editor::{
        asset_registry::AssetKind,
        project_path_tree::ProjectPathGraph,
        ui::{
            Message, Messager,
            dialog::{ConfirmDialogWindow, Dialog},
            inspector::Inspector,
            paths,
        },
    },
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
    texture_drop_target_height: f32,
    texture_clear_button_width: f32,
    texture_path_font_size: f32,
    texture_drag_prompt: String,
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
            .to_str()
            .unwrap_or("")
            .split('/')
            .filter(|s| !s.is_empty() && *s != ".")
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
            let is_selected = *shader_path == *current_path;
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

/// 跨帧持久化的 drag payload 键。使用 `insert_persisted` / `get_persisted`
/// 因为 `insert_temp` 在每帧开始被清除，无法跨帧传递。
const DRAG_PAYLOAD_KEY: &str = "__kairos_drag_payload";
const DROP_HOVER_KEY: &str = "__material_texture_drop_hover";

/// 从持久化存储读取拖拽 payload。
fn read_drag_payload(ui: &egui::Ui) -> Option<PathBuf> {
    let payload: Option<String> = ui
        .ctx()
        .data_mut(|d| d.get_persisted(egui::Id::new(DRAG_PAYLOAD_KEY)));
    let payload = payload.filter(|s| !s.is_empty());
    payload.map(PathBuf::from)
}

/// 检查 payload 是否为有效的 .texture 文件。
fn is_valid_texture_drag(payload: &Option<PathBuf>) -> bool {
    payload
        .as_ref()
        .is_some_and(|p| p.extension().and_then(|e| e.to_str()) == Some("texture"))
}

/// 清除跨帧持久化的拖拽 payload（在拖拽结束或 drop 消费后调用）。
fn clear_drag_payload(ui: &egui::Ui) {
    ui.ctx().data_mut(|d| {
        d.insert_persisted(egui::Id::new(DRAG_PAYLOAD_KEY), String::new());
    });
}

/// 在 drop target 区域检查是否发生了拖放操作。
/// 返回值：Some(texture_path) 表示拖放有效，None 表示无操作。
fn check_texture_drop(ui: &egui::Ui, target_rect: egui::Rect) -> Option<PathBuf> {
    let pointer_pos = ui.input(|i| i.pointer.interact_pos())?;
    if !target_rect.contains(pointer_pos) {
        // 指针不在目标区域，清除悬停状态
        ui.ctx().data_mut(|d| {
            d.insert_persisted(egui::Id::new(DROP_HOVER_KEY), false);
        });
        return None;
    }

    let payload = read_drag_payload(ui)?;
    if !is_valid_texture_drag(&Some(payload.clone())) {
        return None;
    }

    let pointer_down = ui.input(|i| i.pointer.any_down());

    if pointer_down {
        // 正在拖拽悬停
        ui.ctx().data_mut(|d| {
            d.insert_persisted(egui::Id::new(DROP_HOVER_KEY), true);
        });
        None
    } else {
        // 按钮已释放
        let was_hovering = ui
            .ctx()
            .data_mut(|d| d.get_persisted::<bool>(egui::Id::new(DROP_HOVER_KEY)))
            .unwrap_or(false);
        ui.ctx().data_mut(|d| {
            d.insert_persisted(egui::Id::new(DROP_HOVER_KEY), false);
        });
        if was_hovering {
            clear_drag_payload(ui);
            Some(payload)
        } else {
            None
        }
    }
}

// ============================================================
// 纹理缩略图缓存
// ============================================================

/// 缓存最近的纹理缩略图 egui handle。
/// Tuple: (纹理路径, egui::TextureHandle)
struct ThumbnailCache {
    path: PathBuf,
    handle: egui::TextureHandle,
}

/// 为 Texture 创建 48x48 缩略图的 egui handle。
fn build_texture_thumbnail(ui: &egui::Ui, texture: &crate::graphics::texture::Texture) -> egui::TextureHandle {
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
    thumbnail: Arc<Mutex<Option<ThumbnailCache>>>,
}

// ============================================================
// MaterialInspector
// ============================================================

pub struct MaterialInspector {
    model: MaterialInspectorModel,
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

        let current_texture_path = {
            let guard = self.model.current_texture_path.lock();
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
            .column(Column::remainder());
        shader_selecter.body(|mut body| {
            body.row(self.model.style.shader_selector_height, |mut row| {
                row.col(|ui| {
                    ui.label("Shader: ");
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

        // ---- Texture 区域（Unity 风格缩略图 + 路径 + 清除 ×）----
        let thumbbail_size = 48.0;
        let clear_btn_w = self.model.style.texture_clear_button_width;
        let padding = 6.0;
        let total_height = self.model.style.texture_drop_target_height;
        let available_width = ui.available_width();

        // 读完拖拽状态之后再清理非活跃拖拽
        let drag_payload = read_drag_payload(ui);
        let is_valid_drag = is_valid_texture_drag(&drag_payload);
        let pointer_down = ui.input(|i| i.pointer.any_down());
        let pointer_over = ui
            .input(|i| i.pointer.interact_pos())
            .map_or(false, |p| {
                let full_rect = egui::Rect::from_min_size(
                    ui.cursor().min,
                    Vec2::new(available_width, total_height),
                );
                full_rect.contains(p)
            });

        let is_drag_hover = pointer_down && pointer_over && is_valid_drag;

        // 检查拖放（必须在清理 payload 之前，因为 check_texture_drop 需要读取 payload）
        let full_target_rect = egui::Rect::from_min_size(
            ui.cursor().min,
            Vec2::new(available_width, total_height),
        );
        let drop_path = check_texture_drop(ui, full_target_rect);

        // 拖拽已结束（按钮弹起）且 drop 没有被消费 → 清理残留 payload
        if drop_path.is_none() && !pointer_down {
            let payload_exists: bool = ui
                .ctx()
                .data_mut(|d| d.get_persisted::<String>(egui::Id::new(DRAG_PAYLOAD_KEY)))
                .map_or(false, |s| !s.is_empty());
            if payload_exists {
                clear_drag_payload(ui);
            }
        }

        // ── 背景 + 边框 ──
        let bg_color = if is_drag_hover {
            egui::Color32::from_rgba_premultiplied(0, 80, 200, 50)
        } else {
            egui::Color32::from_rgba_premultiplied(50, 50, 50, 120)
        };
        let stroke_color = if is_drag_hover {
            egui::Color32::from_rgb(70, 150, 255)
        } else if current_texture_path.is_some() {
            egui::Color32::from_gray(90)
        } else {
            egui::Color32::from_gray(70)
        };

        // 分配空间
        let (_id, _response) = ui.allocate_painter(
            Vec2::new(available_width, total_height),
            egui::Sense::click(),
        );

        ui.painter().rect(
            full_target_rect,
            egui::CornerRadius::same(4),
            bg_color,
            egui::Stroke::new(1.0, stroke_color),
            egui::StrokeKind::Middle,
        );

        if let Some(ref tex_path) = current_texture_path {
            // ═══ 有纹理：缩略图 | 路径文字 | × 清除 ═══

            // ── 缩略图 ──
            let thumb_rect = egui::Rect::from_min_size(
                egui::pos2(full_target_rect.min.x + padding, full_target_rect.min.y + padding),
                Vec2::splat(thumbbail_size),
            );

            // 尝试从缓存或 assets_server 取缩略图
            let mut thumb_guard = self.model.thumbnail.lock();
            let thumb_mismatch = thumb_guard
                .as_ref()
                .map_or(true, |cached| cached.path != *tex_path);

            if thumb_mismatch {
                // 路径变了，重新生成
                *thumb_guard = None;
                // 异步加载 texture 并生成缩略图
                if let Some(mat) = assets_server.get::<MaterialAssetsSystem>(&self.model.material_handle) {
                    if let Some(tex_handle) = &mat.texture {
                        if let Some(texture) = assets_server.get::<TextureAssetsSystem>(tex_handle) {
                            let handle = build_texture_thumbnail(ui, texture);
                            *thumb_guard = Some(ThumbnailCache {
                                path: tex_path.clone(),
                                handle,
                            });
                        }
                    }
                }
            }

            if let Some(ref cache) = *thumb_guard {
                // 缩略图可用 → 绘制
                ui.painter().image(
                    cache.handle.id(),
                    thumb_rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            } else {
                // 缩略图不可用 → 占位色块
                ui.painter().rect_filled(
                    thumb_rect,
                    egui::CornerRadius::same(2),
                    egui::Color32::from_gray(40),
                );
            }

            // ── 路径文字 ──
            let text_x = thumb_rect.max.x + padding;
            let text_w = full_target_rect.max.x - clear_btn_w - text_x - padding;
            let text_rect = egui::Rect::from_min_size(
                egui::pos2(text_x, full_target_rect.min.y),
                Vec2::new(text_w, total_height),
            );
            let display_path = tex_path.to_string_lossy();
            ui.painter().text(
                egui::pos2(text_rect.min.x + 2.0, text_rect.center().y),
                egui::Align2::LEFT_CENTER,
                &*display_path,
                egui::FontId::proportional(self.model.style.texture_path_font_size),
                egui::Color32::from_gray(200),
            );

            // ── 清除 × 按钮 ──
            let clear_btn_rect = egui::Rect::from_min_size(
                egui::pos2(full_target_rect.max.x - clear_btn_w, full_target_rect.min.y),
                Vec2::new(clear_btn_w, total_height),
            );
            let clear_resp = ui.interact(
                clear_btn_rect,
                egui::Id::new("material_tex_clear"),
                egui::Sense::click(),
            );
            if clear_resp.clicked() {
                messager.send(Message::MaterialInspectorClearTexture(
                    self.model.path.clone(),
                ));
            }
            if clear_resp.hovered() {
                ui.painter().rect_filled(
                    clear_btn_rect,
                    egui::CornerRadius::same(4),
                    egui::Color32::from_rgba_premultiplied(200, 50, 50, 120),
                );
            }
            ui.painter().text(
                clear_btn_rect.center(),
                egui::Align2::CENTER_CENTER,
                "×",
                egui::FontId::proportional(18.0),
                egui::Color32::WHITE,
            );

            // ── "Texture" label 在左上方 ──
            let label_pos = egui::pos2(full_target_rect.min.x + 2.0, full_target_rect.min.y - 14.0);
            ui.painter().text(
                label_pos,
                egui::Align2::LEFT_BOTTOM,
                "Texture",
                egui::FontId::proportional(12.0),
                egui::Color32::from_gray(140),
            );
        } else {
            // ═══ 无纹理：居中占位文字 ═══
            let prompt = if is_drag_hover {
                "Drop .texture here"
            } else {
                &self.model.style.texture_drag_prompt
            };
            ui.painter().text(
                full_target_rect.center(),
                egui::Align2::CENTER_CENTER,
                prompt,
                egui::FontId::proportional(self.model.style.texture_path_font_size),
                if is_drag_hover {
                    egui::Color32::WHITE
                } else {
                    egui::Color32::from_gray(120)
                },
            );

            // label
            let label_pos = egui::pos2(full_target_rect.min.x + 2.0, full_target_rect.min.y - 14.0);
            ui.painter().text(
                label_pos,
                egui::Align2::LEFT_BOTTOM,
                "Texture",
                egui::FontId::proportional(12.0),
                egui::Color32::from_gray(140),
            );
        }

        // 处理拖放结果
        if let Some(texture_path) = drop_path {
            // 清除缩略图缓存，下次 draw 会重新生成
            *self.model.thumbnail.lock() = None;
            messager.send(Message::MaterialInspectorDropTexture(
                self.model.path.clone(),
                texture_path,
            ));
        }

        // 留空隙，让下一个 separator 不贴边
        ui.allocate_space(Vec2::new(available_width, 4.0 + 14.0));

        ui.separator();

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
    pub fn drop_texture(
        &mut self,
        assets_server: &mut AssetsServer,
        texture_path: PathBuf,
    ) {
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
