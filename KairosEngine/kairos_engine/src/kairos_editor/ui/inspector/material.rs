use std::{cell::Cell, fs, path::PathBuf, sync::Arc};

use serde::Deserialize;

use crate::{
    asset_loader::assets::{
        AssetHandle, AssetsServer, MaterialAssetsSystem, ShaderAssetsSystem,
    },
    graphics::material::Material,
    kairos_editor::{
        asset_registry::AssetKind,
        project_path_tree::ProjectPathGraph,
        ui::{
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
    label_width: f32,
    combo_width: f32,
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
// Model
// ============================================================

struct MaterialInspectorModel {
    style: MaterialInspectorStyle,
    /// .mat 文件的磁盘路径
    path: PathBuf,
    /// 运行时 Material 句柄（通过 assets_server.load 获取）
    material_handle: Arc<AssetHandle<MaterialAssetsSystem>>,
    /// 下拉选项中所有可用的 Shader 路径（display_name, full_path）
    shader_paths: Vec<(String, PathBuf)>,
    /// 当前选中的 shader 路径
    current_shader_path: PathBuf,
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

        // 读取 .mat 文件，获取当前 shader_path
        let toml_bytes = fs::read(&mat_path)?;
        let serialized_material: crate::graphics::material::SerializedMaterial =
            toml::from_slice(&toml_bytes)?;
        let current_shader_path = serialized_material.shader_path;

        // 加载运行时 Material（异步，句柄立即返回）
        let material_handle =
            assets_server.load::<MaterialAssetsSystem>(&mat_path);

        // 查询项目图中所有 Shader 节点，用于下拉栏
        let shader_paths: Vec<(String, PathBuf)> = project_graph
            .find_assets_by_kind(AssetKind::Shader)
            .iter()
            .map(|node| {
                let name = node.name();
                (name, node.path.clone())
            })
            .collect();

        let model = MaterialInspectorModel {
            style,
            path: mat_path,
            material_handle,
            shader_paths,
            current_shader_path,
            dirty: Cell::new(false),
        };

        Ok(Self { model })
    }

    fn draw(
        &self,
        ui: &mut egui::Ui,
        _messager: &mut crate::kairos_editor::ui::Messager,
        _assets_server: &AssetsServer,
        _dt: f32,
    ) {
        // ---- Source path ----
        ui.label(format!("Source: {}", self.model.path.display()));
        ui.separator();

        // ---- Shader dropdown ----
        let current_shader_name = self
            .model
            .current_shader_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown");

        // 用 label_width 控制 ComboBox 标签宽度
        let label_width = self.model.style.label_width;
        let combo_width = self.model.style.combo_width;

        ui.horizontal(|ui| {
            // 预留标签宽度确保对齐
            ui.add_sized(
                egui::vec2(label_width, 0.0),
                egui::Label::new("Shader"),
            );
            egui::ComboBox::from_id_salt("shader_combo")
                .selected_text(current_shader_name)
                .width(combo_width)
                .show_ui(ui, |ui| {
                    for (name, shader_path) in &self.model.shader_paths {
                        let is_selected = *shader_path == self.model.current_shader_path;
                        if ui.selectable_label(is_selected, name).clicked() {
                            // 用户选择了不同的 shader —— 通过 messager 异步处理
                            // 但因为 draw(&self)，发送消息让主循环处理
                            _messager.send(
                                crate::kairos_editor::ui::Message::MaterialInspectorChangeShader(
                                    self.model.path.clone(),
                                    shader_path.clone(),
                                ),
                            );
                        }
                    }
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
                None::<crate::kairos_editor::ui::Message>,
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
    pub fn change_shader(
        &mut self,
        assets_server: &mut AssetsServer,
        new_shader_path: PathBuf,
    ) {
        // 1. 加载新 shader（异步，句柄立即返回）
        let new_shader_handle =
            assets_server.load::<ShaderAssetsSystem>(&new_shader_path);

        // 2. 读取现有运行时 Material（保留已有字段如 texture、render_state 等）
        let existing = assets_server
            .get::<MaterialAssetsSystem>(&self.model.material_handle)
            .cloned()
            .unwrap_or_default();

        let material = Material {
            shader: Some(new_shader_handle),
            ..existing
        };

        // 3. 插入运行时 MaterialAssetsSystem
        assets_server.insert::<MaterialAssetsSystem>(material, &self.model.path);

        // 4. 更新模型状态
        self.model.current_shader_path = new_shader_path;
        self.model.dirty.set(true);
    }
}
