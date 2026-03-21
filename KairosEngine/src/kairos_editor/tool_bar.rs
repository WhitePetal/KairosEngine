use std::fs;

use eframe::egui::{self, Pos2, Rect, TopBottomPanel, Vec2, containers::menu};
use kairos_engine::math;
use serde::{Deserialize, Serialize};
use sonic_rs::from_str;

use crate::kairos_editor::{UIDrawer, UIMessage, paths};

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolBarStyle {
    pub height: f32,
    pub button_width: f32,
    pub corner_radius: f32,
    pub fill_color: math::Color32,
    pub button_text_color: math::Color32,
}

pub struct ToolBarModel {
    pub style: ToolBarStyle,
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

impl UIDrawer for ToolBar {
    fn update(&self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame, messager: &mut super::UIMessager) {
        let model = &self.model;
        TopBottomPanel::top("toolbar").show(ctx, |ui|{
            ui.visuals_mut().override_text_color = Some(model.style.button_text_color.into());
            menu::MenuBar::new().ui(ui, |ui| {
                // 工具栏区域
                let toolbar_rect = Rect::from_min_size(
                    Pos2::new(0.0, 0.0), 
                    Vec2::new(ctx.content_rect().width(), model.style.height));

                // 工具栏背景
                ui.painter().rect_filled(
                    toolbar_rect, 
                    model.style.corner_radius, 
                    model.style.fill_color
                );
                
                // Icon
                let icon = egui::Image::new(paths::URI_ENGINE_ICON);
                ui.menu_image_button(icon, |ui| {
                    if ui.button("About Kairos").clicked() {
                        messager.send(UIMessage::OpenAboutWindow);
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        messager.send(UIMessage::QuitEngine);
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
                        messager.send(UIMessage::OpenPreferenceWindow);
                    }
                });
            });
        });
    }
}
