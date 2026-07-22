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
        ShaderAssetsSystem,
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
    /// 用户是否修改了（未保存）
    dirty: Cell<bool>,
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
            dirty: Cell::new(false),
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
        // ---- 异步加载 SerializedMaterial，获取 shader_path ----
        {
            let mut current = self.model.current_shader_path.lock();
            if current.is_none() {
                if let Some(serialized) = assets_server
                    .get::<SerializedMaterialAssetsSystem>(&self.model.serialized_handle)
                {
                    *current = Some(serialized.shader_path.clone());
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

        // 4. 更新数据状态
        *self.model.current_shader_path.lock() = Some(new_shader_path);
        self.model.dirty.set(true);
    }
}
