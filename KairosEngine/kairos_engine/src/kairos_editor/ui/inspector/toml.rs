use std::{
    cell::Cell,
    fs,
    ops::{Deref, DerefMut},
    path::PathBuf,
    sync::Arc,
};

use egui::Vec2;
use egui_extras::{Column, TableBuilder, TableRow};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use toml::{Value, map::Map};

use crate::{
    asset_loader::assets::AssetsServer,
    kairos_editor::ui::{
        Messager,
        dialog::{ConfirmDialogWindow, Dialog},
        inspector::Inspector,
        paths,
    },
    math,
};

#[derive(Debug, Serialize, Deserialize)]
struct TomlTableInspectorStyle {
    row_height: f32,      // 20.0
    save_btn_height: f32, // 10.0
}
impl TomlTableInspectorStyle {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style_json =
            fs::read_to_string(paths::PATH_TOML_TABLE_INSPECTOR_STYLE).map_err(|error| {
                format!(
                    "Load TomlTableInspector Style Toml Failed, path: {}, error: {}",
                    paths::PATH_TOML_TABLE_INSPECTOR_STYLE,
                    error
                )
            })?;
        let style: Self = toml::from_str(&style_json).map_err(|error| {
            format!(
                "Deserialize TomlTableInspector Style Toml Failed, error: {}",
                error
            )
        })?;
        Ok(style)
    }
}

struct TomlTableInspectorModle {
    style: TomlTableInspectorStyle,
    table: Arc<Mutex<Map<String, Value>>>,
    path: PathBuf,
    dirty: Cell<bool>,
}

pub struct TomlTableInspector {
    model: TomlTableInspectorModle,
}

impl Inspector for TomlTableInspector {
    fn create(
        path: &std::path::Path,
        _assets_server: &mut AssetsServer,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let style = TomlTableInspectorStyle::new()?;
        let toml = fs::read(path)?;
        let table = toml::from_slice(&toml)?;
        let path = path.to_path_buf();
        let model = TomlTableInspectorModle {
            style,
            table: Arc::new(Mutex::new(table)),
            path,
            dirty: Cell::new(false),
        };

        Ok(Self { model })
    }

    fn draw(&self, ui: &mut egui::Ui, _messager: &mut Messager, _assets_server: &AssetsServer) {
        ui.separator();
        let mut changed = false;

        {
            let table_ref = self.model.table.clone();
            let mut table_mut = table_ref.lock();
            let table_mut = table_mut.deref_mut();

            egui::ScrollArea::vertical()
                .max_height(ui.available_height() - self.model.style.save_btn_height)
                .show(ui, |ui| {
                    Self::render_toml_table(
                        ui,
                        self.model.style.row_height,
                        &[],
                        table_mut,
                        &mut changed,
                    );
                });
        }

        ui.separator();

        if changed {
            self.model.dirty.replace(true);
        }

        ui.vertical_centered(|ui| {
            let save_btn = egui::Button::new("Save").min_size(Vec2::new(
                ui.available_width(),
                self.model.style.save_btn_height,
            ));
            if ui.add_enabled(self.model.dirty.get(), save_btn).clicked() {
                self.save();
                self.model.dirty.replace(false);
            }
            if self.model.dirty.get() {
                ui.label("* unsaved changes");
            }
        });
    }

    fn on_exit(&mut self, _ctx: &egui::Context) -> Option<Box<dyn Dialog>> {
        if self.model.dirty.get() {
            let table = self.model.table.clone();
            let path = self.model.path.clone();
            let dialog = ConfirmDialogWindow::new(
                "have modify not save".into(),
                "save the modify?".into(),
                "save".into(),
                "cancel".into(),
                None,
                None,
                Some(move || {
                    let table = table.lock();
                    Self::save_table(&path, table.deref());
                }),
                None::<fn()>,
            );
            Some(Box::new(dialog))
        } else {
            None
        }
    }
}

impl TomlTableInspector {
    fn save(&self) {
        let table = self.model.table.clone();
        let table = table.lock();
        Self::save_table(&self.model.path, table.deref());
    }

    pub fn save_table(path: &PathBuf, table: &toml::Table) {
        let content = match toml::to_string_pretty(table) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("Failed to serialize TOML: {e}");
                return;
            }
        };
        if let Err(e) = fs::write(&path, &content) {
            log::warn!("Failed to write TOML '{}': {e}", path.display());
        }
    }

    /// 递归渲染 TOML Table
    fn render_toml_table(
        ui: &mut egui::Ui,
        row_height: f32,
        path: &[String],
        table: &mut toml::Table,
        changed: &mut bool,
    ) {
        let inspector_table = TableBuilder::new(ui)
            .resizable(true)
            .striped(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::auto())
            .column(Column::remainder());

        inspector_table.body(|mut body| {
            for (key, value) in table {
                let mut full_path = path.to_vec();
                full_path.push(key.clone());
                body.row(row_height, |mut row| {
                    Self::render_toml_field(&mut row, row_height, &full_path, key, value, changed);
                });
            }
        });
    }

    /// 渲染单个 TOML 字段（值类型分发）
    fn render_toml_field(
        row: &mut TableRow,
        row_height: f32,
        path: &[String],
        key: &str,
        value: &mut Value,
        changed: &mut bool,
    ) {
        row.set_overline(true);

        row.col(|ui| {
            ui.label(key);
        });

        row.col(|ui| {
            match value {
                Value::String(s) => {
                    if let Ok(color) = math::Color32::from_hex(s) {
                        let mut color_ar = color.to_rgb_array();
                        let resp = ui.color_edit_button_srgb(&mut color_ar);
                        if resp.changed() {
                            *s = math::Color32::from(color_ar).to_hex();
                            *changed = true;
                        }
                    } else {
                        let text_edit = egui::text_edit::TextEdit::singleline(s).clip_text(false);
                        let resp = ui.add(text_edit);
                        if resp.changed() {
                            *changed = true;
                        }
                    }
                }
                Value::Integer(i) => {
                    if ui.add(egui::DragValue::new(i)).changed() {
                        *changed = true;
                    }
                }
                Value::Float(f) => {
                    if ui.add(egui::DragValue::new(f).speed(0.01)).changed() {
                        *changed = true;
                    }
                }
                Value::Boolean(b) => {
                    if ui.checkbox(b, "").changed() {
                        *changed = true;
                    }
                }
                Value::Datetime(dt) => {
                    let mut buf = dt.to_string();
                    let resp = ui.text_edit_singleline(&mut buf);
                    if resp.lost_focus() {
                        // 尝试解析回 Datetime
                        if let Ok(parsed) = buf.parse::<toml::value::Datetime>() {
                            *dt = parsed;
                            *changed = true;
                        }
                    }
                }
                Value::Array(arr) => {
                    let collapsed = egui::CollapsingHeader::new(format!("[{}]", arr.len()))
                        .default_open(false)
                        .show(ui, |ui| {
                            for (i, elem) in arr.iter_mut().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.label(format!("[{i}]"));
                                    Self::render_toml_value(ui, elem, changed);
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
                            Self::render_toml_table(ui, row_height, path, t, changed);
                        });
                }
            }
        });
    }

    /// 渲染单个 TOML 值（无 key 标签，用于数组元素）
    fn render_toml_value(ui: &mut egui::Ui, value: &mut Value, changed: &mut bool) {
        match value {
            Value::String(s) => {
                if ui.text_edit_singleline(s).changed() {
                    *changed = true;
                }
            }
            Value::Integer(i) => {
                if ui.add(egui::DragValue::new(i)).changed() {
                    *changed = true;
                }
            }
            Value::Float(f) => {
                if ui.add(egui::DragValue::new(f).speed(0.01)).changed() {
                    *changed = true;
                }
            }
            Value::Boolean(b) => {
                if ui.checkbox(b, "").changed() {
                    *changed = true;
                }
            }
            Value::Datetime(dt) => {
                let mut buf = dt.to_string();
                if ui.text_edit_singleline(&mut buf).lost_focus() {
                    if let Ok(parsed) = buf.parse::<toml::value::Datetime>() {
                        *dt = parsed;
                        *changed = true;
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
}
