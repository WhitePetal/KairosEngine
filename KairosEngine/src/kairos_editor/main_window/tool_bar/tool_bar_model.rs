use std::{fs, rc::Rc, sync::Arc};

use eframe::egui::{self, IconData, Image};
use sonic_rs::{Deserialize, Serialize, from_str};

use crate::kairos_editor::{self, paths};
use kairos_engine::math;

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolBarStyle {
    pub height: f32,
    pub button_width: f32,
    pub corner_radius: f32,
    pub fill_color: math::Color32,
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
    pub fn new(ctx: &egui::Context) -> Result<Self, Box<dyn std::error::Error>> {
        let style = ToolBarStyle::new()?;        

        Ok(Self { 
            style,
        })
    }
}