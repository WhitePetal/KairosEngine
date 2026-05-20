use std::{any::type_name, fs};

use crate::log::Log;
use serde::{Deserialize, Serialize};
use sonic_rs::from_str;

use crate::kairos_editor::ui::{Drawer, Message, paths};


#[derive(Debug, Serialize, Deserialize)]
struct ProjectWindowStyle {
    pub title: String,
}

struct ProjectWindowModel {
    style: ProjectWindowStyle,
}

pub struct ProjectWindow {
    model: ProjectWindowModel,
}

impl ProjectWindowStyle {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style_json = fs::read_to_string(paths::PATH_PROJECT_WINDOW_STYLE)
            .map_err(|error| format!("Load ProjectWindow Style Json Failed, path: {}, error: {}", paths::PATH_PROJECT_WINDOW_STYLE, error))?;
        let style = from_str(&style_json)
            .map_err(|error| format!("Deserialize ProjectWindow Style Json Failed, error: {}", error))?;
        Ok(style)
    }
}

impl ProjectWindowModel {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style = ProjectWindowStyle::new()?;
        Ok(
            Self { 
                style 
            }
        )
    }
}

impl ProjectWindow {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let model = ProjectWindowModel::new()?;
        Ok(
            Self { 
                model 
            }
        )
    }
}

impl Drawer for ProjectWindow {
    fn show_window(&self, _state: Option<&mut super::docking_tab::window_state::WindowState>) {

    }

    fn update(
        &self, 
        ui: Option<&mut egui::Ui>, 
        _ctx: &egui::Context, 
        _messager: &mut super::Messager,
        _log: &mut Log
    ) {
        let ui = ui.unwrap();
        ui.label("TODO: Project");
    }

    fn close(&self, messager: &mut super::Messager) {
        messager.send(Message::CloseProjectTab);
    }

    fn get_name(&self) -> &'static str {
        type_name::<ProjectWindow>()
    }

    fn get_title(&self) -> egui::WidgetText {
        self.model.style.title.to_owned().into()
    }

    fn get_style_fileds(&self) -> Vec<super::ui_style_fields::StyleField> {
        Vec::new()
    }

    fn update_style(&mut self, _style_fields: &Vec<super::ui_style_fields::StyleField>) {
        
    }
}