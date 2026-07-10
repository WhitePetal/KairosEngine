use std::{any::type_name, cell::Cell, fs, path::PathBuf};

use crate::{
    kairos_editor::{
        Engine,
        asset_registry::Guid,
        project_path_tree::tree_node::ProjectNodeKind,
        ui::{Messager, global_styles::GlobalStyles},
    },
    kairos_game::KairosGame,
    log::Log,
};
use serde::{Deserialize, Serialize};
use toml::from_str;

use crate::kairos_editor::ui::{Drawer, Message, paths};

#[derive(Debug, Serialize, Deserialize)]
struct InspectorWindowStyle {
    pub title: String,
}

/// ProjectWindow 选中节点时传递给 InspectorWindow 的身份信息。
/// 只含路径和类型标识，Inspector 自行从文件读取详细内容。
#[derive(Debug, Clone)]
pub struct InspectorNodeInfo {
    pub name: String,
    pub kind: ProjectNodeKind,
    pub path: PathBuf,
    pub guid: Guid,
}

struct InspectorWindowModel {
    style: InspectorWindowStyle,
    /// 当前选中的节点信息，None 表示无选中
    selected: Option<InspectorNodeInfo>,
    /// 上一帧指针是否在 Inspector 区域内（用于检测进入/离开）
    pointer_was_inside: Cell<bool>,
}

pub struct InspectorWindow {
    model: InspectorWindowModel,
}

impl InspectorWindowStyle {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style_json =
            fs::read_to_string(paths::PATH_INSPECTOR_WINDOW_STYLE).map_err(|error| {
                format!(
                    "Load InspectorWindow Style Json Failed, path: {}, error: {}",
                    paths::PATH_INSPECTOR_WINDOW_STYLE,
                    error
                )
            })?;
        let style = from_str(&style_json).map_err(|error| {
            format!(
                "Deserialize InspectorWindow Style Json Failed, error: {}",
                error
            )
        })?;
        Ok(style)
    }
}

impl InspectorWindowModel {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style = InspectorWindowStyle::new()?;
        Ok(Self {
            style,
            selected: None,
            pointer_was_inside: Cell::new(false),
        })
    }
}

impl InspectorWindow {
    #[inline(always)]
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let model = InspectorWindowModel::new()?;
        Ok(Self { model })
    }

    /// 接收来自 ProjectWindow 的选中节点信息。
    pub fn set_selected(&mut self, info: Option<InspectorNodeInfo>) {
        self.model.selected = info;
    }

    // ---- 各类型专属渲染 ----

    /// 目录：统计子项数量
    fn render_directory_section(ui: &mut egui::Ui, path: &std::path::Path) {
        ui.separator();
        match fs::read_dir(path) {
            Ok(entries) => {
                let count = entries.filter_map(|e| e.ok()).count();
                ui.label(format!("Children: {count}"));
            }
            Err(e) => {
                ui.label(format!("Failed to read directory: {e}"));
            }
        }
    }

    /// Toml：解析为键值表，逐行展示
    fn render_toml_section(ui: &mut egui::Ui, path: &std::path::Path) {
        ui.separator();
        ui.label("Contents:");
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                ui.label(format!("Failed to read file: {e}"));
                return;
            }
        };
        let table: toml::Table = match toml::from_str(&content) {
            Ok(t) => t,
            Err(e) => {
                ui.label(format!("Failed to parse TOML: {e}"));
                return;
            }
        };
        egui::ScrollArea::vertical()
            .id_salt("inspector_toml")
            .max_height(300.0)
            .show(ui, |ui| {
                for (key, value) in &table {
                    ui.label(format!("{} = {}", key, value));
                }
            });
    }

    /// 纯文本文件（Script/Shader/Document）：只读前若干行预览
    fn render_text_section(ui: &mut egui::Ui, path: &std::path::Path) {
        ui.separator();
        ui.label("Preview:");
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                ui.label(format!("Failed to read file: {e}"));
                return;
            }
        };
        let line_count = content.lines().count();
        ui.label(format!("Lines: {line_count}"));
        egui::ScrollArea::vertical()
            .id_salt("inspector_text_preview")
            .max_height(300.0)
            .show(ui, |ui| {
                ui.monospace(&content);
            });
    }

    /// 未实现详细 inspector 的文件类型：显示文件元数据
    fn render_file_meta(ui: &mut egui::Ui, path: &std::path::Path) {
        ui.separator();
        match fs::metadata(path) {
            Ok(meta) => {
                ui.label(format!("Size: {} bytes", meta.len()));
            }
            Err(e) => {
                ui.label(format!("Failed to read metadata: {e}"));
            }
        }
    }
}

impl Drawer for InspectorWindow {
    fn create(
        _assets_server: &mut crate::asset_loader::assets::AssetsServer,
    ) -> Result<Self, Box<dyn std::error::Error>>
    where
        Self: Sized,
    {
        Self::new()
    }

    fn show_window(&self, _state: Option<&mut super::docking_tab::window_state::WindowState>) {}

    fn ui(
        &self,
        ui: &mut egui::Ui,
        _global_styles: &GlobalStyles,
        messager: &mut super::Messager,
        _engine: &Engine,
        _log: &mut Log,
    ) {
        // 检测指针进入/离开 Inspector 区域，发送锁定/解锁消息
        let now_inside = ui.rect_contains_pointer(ui.max_rect());
        let was_inside = self.model.pointer_was_inside.get();
        if now_inside != was_inside {
            messager.send(Message::LockProjectSelection(now_inside));
            self.model.pointer_was_inside.set(now_inside);
        }

        let Some(info) = &self.model.selected else {
            ui.centered_and_justified(|ui| {
                ui.label("Select a node in Project Window");
            });
            return;
        };

        // ---- header ----
        ui.heading(&info.name);
        ui.label(format!("Kind: {:?}", info.kind));
        ui.separator();

        // ---- common ----
        ui.label(format!("Path: {}", info.path.display()));
        ui.label(format!("GUID: {}", info.guid));

        // ---- type-specific ----
        match info.kind {
            ProjectNodeKind::Directory => Self::render_directory_section(ui, &info.path),
            ProjectNodeKind::Toml => Self::render_toml_section(ui, &info.path),
            ProjectNodeKind::Script | ProjectNodeKind::Shader | ProjectNodeKind::Document => {
                Self::render_text_section(ui, &info.path)
            }
            _ => Self::render_file_meta(ui, &info.path),
        }
    }

    fn close(&self, messager: &mut super::Messager) {
        messager.send(Message::CloseInspectorTab);
    }

    fn get_name(&self) -> &'static str {
        type_name::<InspectorWindow>()
    }

    fn get_title(&self) -> egui::WidgetText {
        self.model.style.title.to_owned().into()
    }

    fn get_style_fileds(&self) -> Vec<super::ui_style_fields::StyleField> {
        Vec::new()
    }

    fn update_style(&mut self, _style_fields: &Vec<super::ui_style_fields::StyleField>) {}

    fn render(
        &self,
        _engine: &mut Engine,
        _game: &mut KairosGame,
        _messager: &mut Messager,
    ) -> Option<crate::graphics::graphics_graph::GraphicsCommand> {
        None
    }
}
