use std::sync::Arc;

use egui_extras::{Column, TableBuilder, TableRow};
use toml::Value;

use crate::{
    asset_loader::assets::{AssetHandle, AssetsServer, TomlTableAssetsSystem}, kairos_editor::ui::{Message, Messager, inspector::Inspector}, math,
};

pub struct TomlTableInspector {
    toml_handle: Arc<AssetHandle<TomlTableAssetsSystem>>,
    dirty: bool,
}
impl Inspector for TomlTableInspector {
    fn create(path: &std::path::Path, assets_server: &mut AssetsServer) -> Self {
        Self {
            toml_handle: assets_server.load::<TomlTableAssetsSystem>(path.to_path_buf()),
            dirty: false,
        }
    }

    fn draw(&self, ui: &mut egui::Ui, messager: &mut Messager, assets_server: &AssetsServer) {
        ui.separator();

        let table = assets_server.get(&self.toml_handle);
        match table {
            Some(table) => {
                let mut table = table.clone();
                let mut changed = false;
                egui::ScrollArea::vertical()
                    .id_salt("inspector_toml_edit")
                    .max_height(400.0)
                    .show(ui, |ui| {
                        Self::render_toml_table(ui, messager, &[], &mut table, &mut changed);
                    });

                ui.separator();

                ui.horizontal(|ui| {
                    let save_btn = egui::Button::new("Save");
                    if ui.add_enabled(changed, save_btn).clicked() {
                        messager.send(Message::SaveInspectorToml);
                    }
                    if changed {
                        ui.label("* unsaved changes");
                    }
                });
            }
            None => {
                ui.label("loading toml");
            }
        }        
    }

    fn dirty(&self) -> bool {
        self.dirty
    }

    fn set_dirty(&mut self, dirty: bool) {
        self.dirty = dirty
    }
}

impl TomlTableInspector {
    /// 递归渲染 TOML Table
    fn render_toml_table(
        ui: &mut egui::Ui,
        messager: &mut Messager,
        path: &[String],
        table: &mut toml::Table,
        changed: &mut bool,
    ) {
        let inspector_table = TableBuilder::new(ui)
            .striped(false)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::auto())
            .column(Column::auto());

        inspector_table
            .body(|mut body| {
                for (key, value) in table {
                    let mut full_path = path.to_vec();
                    full_path.push(key.clone());
                    body.row(20.0, |mut row| {
                        Self::render_toml_field(&mut row, messager, &full_path, key, value, changed);
                    });
                }
            });
    }
    
    /// 渲染单个 TOML 字段（值类型分发）
    fn render_toml_field(
        row: &mut TableRow,
        messager: &mut Messager,
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
                        if resp.changed() || resp.lost_focus() {
                            *s = math::Color32::from(color_ar).to_hex();
                            *changed = true;
                        }
                    } else {                        
                        let text_edit = egui::text_edit::TextEdit::singleline(s).clip_text(false);
                        let resp = ui.add(text_edit);
                        if resp.changed() || resp.lost_focus() {
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
                                    let mut elem_path = path.to_vec();
                                    elem_path.push(i.to_string());
                                    Self::render_toml_value(ui, messager, &elem_path, elem, changed);
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
                            Self::render_toml_table(ui, messager, path, t, changed);
                        });
                }
            }
        });
    }
    
    /// 渲染单个 TOML 值（无 key 标签，用于数组元素）
    fn render_toml_value(ui: &mut egui::Ui, messager: &mut Messager, path: &[String], value: &mut Value, changed: &mut bool) {
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