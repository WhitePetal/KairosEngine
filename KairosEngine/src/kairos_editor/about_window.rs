use std::fs;

use eframe::egui;
use serde::{Deserialize, Serialize};
use sonic_rs::from_str;

use crate::kairos_editor::{UIDrawer, consts, paths};


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
    pub style: AboutWindowStyle,
    pub open: bool,
}

impl AboutWindowModel {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style = AboutWindowStyle::new()?;

        Ok(
            Self { 
                style,
                open: false,
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
}