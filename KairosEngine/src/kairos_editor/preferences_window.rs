

use std::fs;

use eframe::egui;
use serde::{Deserialize, Serialize};
use sonic_rs::from_str;

use crate::kairos_editor::{UIDrawer, consts, paths};


#[derive(Debug, Serialize, Deserialize)]
pub struct PreferencesStyle {
    // pub height: f32,
    // pub width: f32
}

impl PreferencesStyle {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // let style_json = fs::read_to_string(paths::PATH_ABOUT_WINDOW_STYLE)
        // .map_err(|error| format!("Load AboutWindow Model Json Failed, path: {}, error: {}", paths::PATH_ABOUT_WINDOW_STYLE, error))?;
        // let style = from_str(&style_json)
        //     .map_err(|error| format!("Deserialize AboutWindow Model Json Failed, error: {}", error))?;

        // Ok(style)
        Ok(
            Self {  }
        )
    }
}

pub struct PreferencesModel {
    pub style: PreferencesStyle,
    pub open: bool,
}

impl PreferencesModel {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style = PreferencesStyle::new()?;

        Ok(
            Self { 
                style,
                open: false,
            }
        )
    }
}

pub struct PreferencesWindow {
    pub open: bool
}

impl PreferencesWindow {
    pub fn new() -> Self {
        Self {
            open: false
        }
    }
}

impl UIDrawer for PreferencesWindow {
    fn update(&self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame, messager: &mut super::UIMessager, model: &super::UIModel) {
        if let Some(model) = &model.preferences_window {
            let mut is_open = model.open;
            if is_open {
                egui::Window::new("KairosEngine Preferences")
                    .default_width(320.0)
                    .default_height(160.0)
                    .open(&mut is_open)
                    .resizable([true, false])
                    .scroll(false)
                    .constrain_to(ctx.available_rect())
                    .show(ctx, |ui| {
                        // TODO
                        ui.heading("Preferences");
                        ui.separator();
                        ui.label("Prefercens demo. TODO..")
                    }
                );
    
                if !is_open {
                    messager.send(super::UIMessage::CloseAboutWindow);
                }
            }
        }
    }
}