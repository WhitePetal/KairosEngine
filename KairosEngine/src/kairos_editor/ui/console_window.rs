use std::{any::type_name, fs};

use crate::{graphics::render_pipeline::RenderPipeline, log::Log};
use egui;
use serde::{Deserialize, Serialize};
use toml::from_str;

use crate::kairos_editor::ui::{Drawer, Message, Messager, paths};

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
        let style_toml = fs::read_to_string(paths::PATH_CONSOLE_WINDOW_STYLE).map_err(|error| {
            format!(
                "Load ConsoleWindow Model Json Failed, path: {}, error: {}",
                paths::PATH_CONSOLE_WINDOW_STYLE,
                error
            )
        })?;
        let style = from_str(&style_toml).map_err(|error| {
            format!(
                "Deserialize ConsoleWindow Model Json Failed, error: {}",
                error
            )
        })?;

        Ok(style)
    }
}

impl ConsoleWindowModel {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style = ConsoleWindowStyle::new()?;

        Ok(Self { style })
    }
}

impl ConsoleWindow {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let model = ConsoleWindowModel::new()?;
        Ok(Self { model })
    }
}

impl Drawer for ConsoleWindow {
    fn show_window(&self, _state: Option<&mut super::docking_tab::window_state::WindowState>) {}

    fn ui(&self, ui: &mut egui::Ui, _messager: &mut Messager, log: &mut Log) {
        ui.label("TODO: Print Console");

        while let Some(log) = log.pop_front() {
            ui.label(log.message);
        }
    }

    fn close(&self, messager: &mut Messager) {
        messager.send(Message::CloseConsoleTab);
    }

    fn get_name(&self) -> &'static str {
        type_name::<ConsoleWindow>()
    }

    fn get_style_fileds(&self) -> Vec<super::ui_style_fields::StyleField> {
        Vec::new()
    }

    fn update_style(&mut self, _style_fields: &Vec<super::ui_style_fields::StyleField>) {}

    fn get_title(&self) -> egui::WidgetText {
        self.model.style.title.to_owned().into()
    }

    fn render(
        &self,
        _messager: &mut Messager,
        _render_pipeline: &RenderPipeline,
    ) -> Option<crate::graphics::graphics_graph::GraphicsCommand> {
        None
    }
}
