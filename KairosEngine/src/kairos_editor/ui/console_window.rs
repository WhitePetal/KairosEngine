use std::{any::type_name, fs};

use serde::{Deserialize, Serialize};
use sonic_rs::from_str;

use crate::kairos_editor::ui::{Drawer, paths};


#[derive(Debug, Serialize, Deserialize)]
pub struct ConsoleWindowStyle {
    pub title: String,
}

pub struct ConsoleWindowModel {
    style: ConsoleWindowStyle,
}

pub struct ConsoleWindow {
    model: ConsoleWindowModel,
}

impl ConsoleWindowStyle {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style_json = fs::read_to_string(paths::PATH_CONSOLE_WINDOW_STYLE)
        .map_err(|error| format!("Load ConsoleWindow Model Json Failed, path: {}, error: {}", paths::PATH_CONSOLE_WINDOW_STYLE, error))?;
        let style = from_str(&style_json)
            .map_err(|error| format!("Deserialize ConsoleWindow Model Json Failed, error: {}", error))?;

        Ok(style)
    }
}

impl ConsoleWindowModel {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style = ConsoleWindowStyle::new()?;

        Ok(
            Self {
                style
            }
        )
    }
}

impl ConsoleWindow {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let model = ConsoleWindowModel::new()?;
        Ok(
            Self {
                model
            }
        )
    }
}

impl Drawer for ConsoleWindow {
    fn update(&self, _ctx: &eframe::egui::Context, _frame: &mut eframe::Frame, _messager: &mut super::Messager) {

    }

    fn get_name(&self) -> &'static str {
        type_name::<ConsoleWindow>()
    }

    fn get_style_fileds(&self) -> Vec<super::ui_style_fields::StyleField> {
        Vec::new()
    }

    fn update_style(&mut self, style_fields: &Vec<super::ui_style_fields::StyleField>) {

    }
    
    fn get_title(&self) -> eframe::egui::WidgetText {
        self.model.style.title.to_owned().into()
    }
}