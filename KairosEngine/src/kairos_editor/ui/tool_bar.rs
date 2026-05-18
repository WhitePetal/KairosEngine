use std::{any::type_name, fs::{self, File}, io::Write};

use eframe::egui::{self, TopBottomPanel, containers::menu};
use kairos_engine::math;
use serde::{Deserialize, Serialize};
use sonic_rs::from_str;

use crate::{kairos_dialog, kairos_editor::ui::{Drawer, Message, paths, ui_style_fields::{ColorStyleField, FloatFieldEditViewType, FloatStyleField, StyleField}}};

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolBarStyle {
    pub height: f32,
    pub button_width: f32,
    pub corner_radius: f32,
    pub fill_color: math::Color32,
    pub button_text_color: math::Color32,
}

pub struct ToolBarModel {
    style: ToolBarStyle,
}

impl ToolBarStyle {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style_json = fs::read_to_string(paths::PATH_EDITOR_WINDO_TOOL_BAR_STYLE)
            .map_err(|error| format!("Load MainWindow TitleBar Json Failed, path: {}, error: {}", paths::PATH_EDITOR_WINDO_TOOL_BAR_STYLE, error))?;
        let style = from_str(&style_json)
            .map_err(|error| format!("Deserialize MainWindow TitleBar Json Failed, error: {}", error))?;

        Ok(style)
    }
}

impl ToolBarModel {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style = ToolBarStyle::new()?;        

        Ok(Self { 
            style,
        })
    }
}

pub struct ToolBar{
    model: ToolBarModel
}

impl ToolBar {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let model = ToolBarModel::new()?;

        Ok(
            Self{
                model
            }   
        )
    }
}

impl Drawer for ToolBar {
    fn show(&self, _state: Option<&mut super::docking_tab::window_state::WindowState>) {

    }

    fn update(&self, _ui: Option<&mut egui::Ui>, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame, messager: &mut super::Messager) {
        let model = &self.model;
        TopBottomPanel::top("toolbar")
            .default_height(model.style.height)
            .show(ctx, |ui|{
            ui.visuals_mut().override_text_color = Some(model.style.button_text_color.into());
            menu::MenuBar::new().ui(ui, |ui| {
                ui.set_height(model.style.height);
                // 工具栏背景
                ui.painter().rect_filled(
                    ui.available_rect_before_wrap(), 
                    model.style.corner_radius, 
                    model.style.fill_color
                );
                
                // Icon
                let icon = egui::Image::new(paths::URI_ENGINE_ICON);
                ui.menu_image_button(icon, |ui| {
                    if ui.button("About Kairos").clicked() {
                        messager.send(Message::OpenAboutWindow);
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        messager.send(Message::QuitEngine);
                    }
                });

                // File
                ui.menu_button("File", |ui| {
                    if ui.button("New Scene").clicked() {
                        todo!()
                    }
                });
                
                // Editor
                ui.menu_button("Edit", |ui| {
                    if ui.button("Preferences").clicked() {
                        messager.send(Message::OpenPreferenceWindow);
                    }
                });

                // Window
                ui.menu_button("Window", |ui| {
                    // General
                    ui.menu_button("General", |ui| {
                        if ui.button("Console").clicked() {
                            messager.send(Message::OpenConsoleWindow);
                        }
                    })
                })
            });
        });
    }
    
    fn get_style_fileds(&self) -> Vec<StyleField> {
        let mut fields = Vec::new();
        let style = &self.model.style;

        fields.push(StyleField::FloatStyleField(FloatStyleField::new("height", style.height, 0.0, f32::MAX, FloatFieldEditViewType::Field)));
        fields.push(StyleField::FloatStyleField(FloatStyleField::new("button_width", style.button_width, 0.0, f32::MAX, FloatFieldEditViewType::Field)));
        fields.push(StyleField::FloatStyleField(FloatStyleField::new("corrner_radius", style.corner_radius, 0.0, f32::MAX, FloatFieldEditViewType::Field)));
        fields.push(StyleField::ColorStyleField(ColorStyleField::new("fill_color", style.fill_color)));
        fields.push(StyleField::ColorStyleField(ColorStyleField::new("button_text_color", style.button_text_color)));

        fields
    }
    
    fn get_name(&self) -> &'static str {
        type_name::<ToolBar>()
    }
    
    fn update_style(&mut self, style_fields: &Vec<StyleField>) {
        if let StyleField::FloatStyleField(field) = &style_fields[0] {
            self.model.style.height = field.value;
        }
        if let StyleField::FloatStyleField(field) = &style_fields[1] {
            self.model.style.button_width = field.value;
        }
        if let StyleField::FloatStyleField(field) = &style_fields[2] {
            self.model.style.corner_radius = field.value;
        }
        if let StyleField::ColorStyleField(field) = &style_fields[3] {
            self.model.style.fill_color = field.color;
        }
        if let StyleField::ColorStyleField(field) = &style_fields[4] {
            self.model.style.button_text_color = field.color;
        }

        if let Ok(json ) = sonic_rs::to_string_pretty(&self.model.style) {
            if let Ok(mut file )= File::create(paths::PATH_EDITOR_WINDO_TOOL_BAR_STYLE) {
                match file.write_all(json.as_bytes()) {
                    Ok(_) => (),
                    Err(error) => {
                        kairos_dialog::error_message_window("Write File Falied", &format!("Write the ToolBar json file Failed When Write, Error: {}", error));
                    },
                }
            }
            else {
                kairos_dialog::error_message_window("Write File Failed", "Write the ToolBar json file Failed When Open");
            }
        } else {
            kairos_dialog::error_message_window("Serialize Json Failed", "Serialize ToolBar Style to json Failed");
        }
    }
    
    fn get_title(&self) -> egui::WidgetText {
        "ToolBar".into()
    }
}
