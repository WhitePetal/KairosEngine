use std::{any::type_name, fs};

use eframe::egui;
use serde::{Deserialize, Serialize};
use sonic_rs::from_str;

use crate::kairos_editor::{UIDrawer, consts, paths, ui_style_fields::{FloatFieldEditViewType, FloatStyleField, StyleField}};


#[derive(Debug, Serialize, Deserialize)]
pub struct AboutWindowStyle {
    pub height: f32,
    pub width: f32
}

impl AboutWindowStyle {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style_json = fs::read_to_string(paths::PATH_ABOUT_WINDOW_STYLE)
        .map_err(|error| format!("Load AboutWindow Model Json Failed, path: {}, error: {}", paths::PATH_ABOUT_WINDOW_STYLE, error))?;
        let style = from_str(&style_json)
            .map_err(|error| format!("Deserialize AboutWindow Model Json Failed, error: {}", error))?;

        Ok(style)
    }
}

pub struct AboutWindowModel {
    style: AboutWindowStyle,
}

impl AboutWindowModel {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style = AboutWindowStyle::new()?;

        Ok(
            Self { 
                style,
            }
        )
    }
}

pub struct AboutWindow {
    model: AboutWindowModel
}

impl AboutWindow {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let model = AboutWindowModel::new()?;
        Ok(
            Self { 
                model
            }   
        )
    }
}

impl UIDrawer for AboutWindow {
    fn update(&self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame, messager: &mut super::UIMessager) {
        let model = &self.model;
        let mut is_open = true;
        egui::Window::new("About KairosEngine")
            .default_width(model.style.width)
            .default_height(model.style.height)
            .open(&mut is_open)
            .resizable([true, false])
            .scroll(false)
            .constrain_to(ctx.available_rect())
            .show(ctx, |ui| {
                // TODO: Icon
                ui.heading("KairosEngine");
                ui.label(consts::VERSION);
                ui.separator();
                ui.label("KairosEngine is a game development engine that aims to be flexible and efficient.");
                ui.label("TODO: add icon...");
            }
        );

        if !is_open {
            messager.send(super::UIMessage::CloseAboutWindow);
        }
    }
    
    fn get_style_fileds(&self) -> Vec<super::ui_style_fields::StyleField> {
        let mut fields = Vec::new();
        let style = &self.model.style;

        fields.push(StyleField::FloatStyleField(FloatStyleField::new("height", style.height, 0.0, f32::MAX, FloatFieldEditViewType::Field)));
        fields.push(StyleField::FloatStyleField(FloatStyleField::new("width", style.width, 0.0, f32::MAX, FloatFieldEditViewType::Field)));

        fields
    }

    fn update_style(&mut self, style_fields: &Vec<StyleField>) {
        if let StyleField::FloatStyleField(field) = &style_fields[0] {
            self.model.style.height = field.value;
        }
        if let StyleField::FloatStyleField(field) = &style_fields[1] {
            self.model.style.width = field.value;
        }
    }
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    
    fn get_name(&self) -> &'static str {
        type_name::<AboutWindow>()
    }
}