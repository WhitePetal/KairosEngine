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
use toml::{from_str, Table, Value};

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

/// TOML 文件编辑缓存：加载到内存中的可编辑数据。
struct TomlEditCache {
    /// 文件路径，用于选中节点变化时判断缓存是否失效
    path: PathBuf,
    /// 解析后的可编辑表
    table: Table,
    /// 是否有未保存修改
    dirty: bool,
}

struct InspectorWindowModel {
    style: InspectorWindowStyle,
    /// 当前选中的节点信息，None 表示无选中
    selected: Option<InspectorNodeInfo>,
    /// 上一帧指针是否在 Inspector 区域内（用于检测进入/离开）
    pointer_was_inside: Cell<bool>,
    /// TOML 编辑缓存（仅选中 Toml 文件时存在）
    toml_cache: Option<TomlEditCache>,
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
            toml_cache: None,
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
        // 选中节点变化 → 清空旧缓存
        let needs_reload = match (&self.model.selected, &info) {
            (Some(old), Some(new)) => old.path != new.path,
            (None, Some(_)) => true,
            _ => false,
        };
        if needs_reload {
            self.model.toml_cache = None;
        }

        let is_toml = info.as_ref().is_some_and(|i| i.kind == ProjectNodeKind::Toml);
        self.model.selected = info;

        // 新选中节点是 Toml → 加载文件到缓存
        if is_toml {
            let path = self.model.selected.as_ref().unwrap().path.clone();
            self.load_toml_cache(&path);
        }
    }

    /// 从磁盘加载 TOML 文件到编辑缓存。
    fn load_toml_cache(&mut self, path: &std::path::Path) {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("Failed to load TOML '{}': {e}", path.display());
                return;
            }
        };
        let table: Table = match toml::from_str(&content) {
            Ok(t) => t,
            Err(e) => {
                log::warn!("Failed to parse TOML '{}': {e}", path.display());
                return;
            }
        };
        self.model.toml_cache = Some(TomlEditCache {
            path: path.to_path_buf(),
            table,
            dirty: false,
        });
    }

    /// 更新缓存中指定路径的字段值。
    pub fn update_toml_field(&mut self, path: &[String], value: Value) {
        let Some(cache) = &mut self.model.toml_cache else {
            return;
        };
        let mut current: &mut Table = &mut cache.table;
        for (i, key) in path.iter().enumerate() {
            if i == path.len() - 1 {
                current.insert(key.clone(), value.clone());
            } else {
                let next = current.get_mut(key).and_then(|v| v.as_table_mut());
                current = match next {
                    Some(t) => t,
                    None => return,
                };
            }
        }
        cache.dirty = true;
    }

    /// 将缓存中的 TOML 数据写回磁盘。
    pub fn save_toml(&mut self) {
        let Some(cache) = &self.model.toml_cache else {
            return;
        };
        let content = match toml::to_string_pretty(&cache.table) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("Failed to serialize TOML: {e}");
                return;
            }
        };
        if let Err(e) = fs::write(&cache.path, &content) {
            log::warn!("Failed to write TOML '{}': {e}", cache.path.display());
        }
        // 更新缓存中的 dirty 标记
        if let Some(cache) = &mut self.model.toml_cache {
            cache.dirty = false;
        }
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

    /// Toml：交互式编辑
    fn render_toml_section(&self, ui: &mut egui::Ui, messager: &mut Messager) {
        ui.separator();
        let Some(cache) = &self.model.toml_cache else {
            ui.label("Failed to load TOML.");
            return;
        };

        // 克隆一份用于渲染（避免借用冲突）
        let table = cache.table.clone();

        egui::ScrollArea::vertical()
            .id_salt("inspector_toml_edit")
            .max_height(400.0)
            .show(ui, |ui| {
                Self::render_toml_table(ui, messager, &[], &table);
            });

        ui.separator();

        ui.horizontal(|ui| {
            let save_btn = egui::Button::new("Save");
            if ui.add_enabled(cache.dirty, save_btn).clicked() {
                messager.send(Message::SaveInspectorToml);
            }
            if cache.dirty {
                ui.label("* unsaved changes");
            }
        });
    }

    /// 递归渲染 TOML Table
    fn render_toml_table(
        ui: &mut egui::Ui,
        messager: &mut Messager,
        path: &[String],
        table: &Table,
    ) {
        egui::Grid::new(ui.next_auto_id())
            .striped(true)
            .show(ui, |ui| {
                // 排序 key 保证稳定性
                let mut keys: Vec<&String> = table.keys().collect();
                keys.sort();
                for key in keys {
                    let value = &table[key];
                    let mut full_path = path.to_vec();
                    full_path.push(key.clone());
                    Self::render_toml_field(ui, messager, &full_path, key, value);
                }
            });
    }

    /// 渲染单个 TOML 字段（值类型分发）
    fn render_toml_field(
        ui: &mut egui::Ui,
        messager: &mut Messager,
        path: &[String],
        key: &str,
        value: &Value,
    ) {
        ui.label(key);
        match value {
            Value::String(s) => {
                let mut buf = s.clone();
                let resp = ui.text_edit_singleline(&mut buf);
                if resp.changed() || resp.lost_focus() {
                    messager.send(Message::UpdateInspectorTomlValue(
                        path.to_vec(),
                        Value::String(buf),
                    ));
                }
            }
            Value::Integer(i) => {
                let mut buf = *i;
                if ui.add(egui::DragValue::new(&mut buf)).changed() {
                    messager.send(Message::UpdateInspectorTomlValue(
                        path.to_vec(),
                        Value::Integer(buf),
                    ));
                }
            }
            Value::Float(f) => {
                let mut buf = *f;
                if ui.add(egui::DragValue::new(&mut buf).speed(0.01)).changed() {
                    messager.send(Message::UpdateInspectorTomlValue(
                        path.to_vec(),
                        Value::Float(buf),
                    ));
                }
            }
            Value::Boolean(b) => {
                let mut buf = *b;
                if ui.checkbox(&mut buf, "").changed() {
                    messager.send(Message::UpdateInspectorTomlValue(
                        path.to_vec(),
                        Value::Boolean(buf),
                    ));
                }
            }
            Value::Datetime(dt) => {
                let mut buf = dt.to_string();
                let resp = ui.text_edit_singleline(&mut buf);
                if resp.lost_focus() {
                    // 尝试解析回 Datetime
                    if let Ok(parsed) = buf.parse::<toml::value::Datetime>() {
                        messager.send(Message::UpdateInspectorTomlValue(
                            path.to_vec(),
                            Value::Datetime(parsed),
                        ));
                    }
                }
            }
            Value::Array(arr) => {
                let collapsed = egui::CollapsingHeader::new(format!("[{}]", arr.len()))
                    .default_open(false)
                    .show(ui, |ui| {
                        for (i, elem) in arr.iter().enumerate() {
                            ui.horizontal(|ui| {
                                ui.label(format!("[{i}]"));
                                let mut elem_path = path.to_vec();
                                elem_path.push(i.to_string());
                                Self::render_toml_value(ui, messager, &elem_path, elem);
                            });
                        }
                    });
                // 点击 header 区域不触发其他交互
                let _ = collapsed;
            }
            Value::Table(t) => {
                egui::CollapsingHeader::new(key)
                    .default_open(false)
                    .show(ui, |ui| {
                        Self::render_toml_table(ui, messager, path, t);
                    });
                ui.end_row();
                return; // CollapsingHeader 已经占了整行，跳过 ui.end_row()
            }
        }
        ui.end_row();
    }

    /// 渲染单个 TOML 值（无 key 标签，用于数组元素）
    fn render_toml_value(
        ui: &mut egui::Ui,
        messager: &mut Messager,
        path: &[String],
        value: &Value,
    ) {
        match value {
            Value::String(s) => {
                let mut buf = s.clone();
                if ui.text_edit_singleline(&mut buf).changed() {
                    messager.send(Message::UpdateInspectorTomlValue(
                        path.to_vec(),
                        Value::String(buf),
                    ));
                }
            }
            Value::Integer(i) => {
                let mut buf = *i;
                if ui.add(egui::DragValue::new(&mut buf)).changed() {
                    messager.send(Message::UpdateInspectorTomlValue(
                        path.to_vec(),
                        Value::Integer(buf),
                    ));
                }
            }
            Value::Float(f) => {
                let mut buf = *f;
                if ui
                    .add(egui::DragValue::new(&mut buf).speed(0.01))
                    .changed()
                {
                    messager.send(Message::UpdateInspectorTomlValue(
                        path.to_vec(),
                        Value::Float(buf),
                    ));
                }
            }
            Value::Boolean(b) => {
                let mut buf = *b;
                if ui.checkbox(&mut buf, "").changed() {
                    messager.send(Message::UpdateInspectorTomlValue(
                        path.to_vec(),
                        Value::Boolean(buf),
                    ));
                }
            }
            Value::Datetime(dt) => {
                let mut buf = dt.to_string();
                if ui.text_edit_singleline(&mut buf).lost_focus() {
                    if let Ok(parsed) = buf.parse::<toml::value::Datetime>() {
                        messager.send(Message::UpdateInspectorTomlValue(
                            path.to_vec(),
                            Value::Datetime(parsed),
                        ));
                    }
                }
            }
            Value::Array(arr) => {
                ui.label(format!("[{}]", arr.len()));
            }
            Value::Table(_) => {
                ui.label("{...}");
            }
        }
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
            ProjectNodeKind::Toml => self.render_toml_section(ui, messager),
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
