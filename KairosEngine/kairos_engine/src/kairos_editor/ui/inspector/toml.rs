use std::sync::Arc;

use toml::Value;

use crate::{
    asset_loader::assets::{AssetHandle, AssetsServer, TomlTableAssetsSystem},
    kairos_editor::ui::{Message, Messager, inspector::Inspector},
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
                egui::ScrollArea::vertical()
                    .id_salt("inspector_toml_edit")
                    .max_height(400.0)
                    .show(ui, |ui| {
                        render_toml_table(ui, messager, &[], &table);
                    });

                ui.separator();

                ui.horizontal(|ui| {
                    let save_btn = egui::Button::new("Save");
                    if ui.add_enabled(self.dirty, save_btn).clicked() {
                        messager.send(Message::SaveInspectorToml);
                    }
                    if self.dirty {
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

/// 递归渲染 TOML Table
fn render_toml_table(
    ui: &mut egui::Ui,
    messager: &mut Messager,
    path: &[String],
    table: &toml::Table,
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
                render_toml_field(ui, messager, &full_path, key, value);
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
                            render_toml_value(ui, messager, &elem_path, elem);
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
                    render_toml_table(ui, messager, path, t);
                });
            ui.end_row();
            return; // CollapsingHeader 已经占了整行，跳过 ui.end_row()
        }
    }
    ui.end_row();
}

/// 渲染单个 TOML 值（无 key 标签，用于数组元素）
fn render_toml_value(ui: &mut egui::Ui, messager: &mut Messager, path: &[String], value: &Value) {
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
