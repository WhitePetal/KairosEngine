use std::{any::type_name, fs};

use crate::{
    kairos_editor::{Engine, ui::Messager},
    kairos_game::KairosGame,
    log::Log,
};
use serde::{Deserialize, Serialize};
use toml::from_str;

use crate::kairos_editor::ui::{Drawer, Message, paths};

#[derive(Debug, Serialize, Deserialize)]
struct InspectorWindowStyle {
    pub title: String,
}

struct InspectorWindowModel {
    style: InspectorWindowStyle,
}

pub struct InspectorWindow {
    model: InspectorWindowModel,
}

impl InspectorWindowStyle {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style_json =
            fs::read_to_string(paths::PATH_INSPECTOR_WINDOW_STYLE).map_err(|error| {
                format!(
                    "Load InspectorWindow Style Json Failed, path: {}, error: {}",
                    paths::PATH_INSPECTOR_WINDOW_STYLE,
                    error
                )
            })?;
        let style = from_str(&style_json).map_err(|error| {
            format!(
                "Deserialize InspectorWindow Style Json Failed, error: {}",
                error
            )
        })?;
        Ok(style)
    }
}

impl InspectorWindowModel {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style = InspectorWindowStyle::new()?;
        Ok(Self { style })
    }
}

impl InspectorWindow {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let model = InspectorWindowModel::new()?;
        Ok(Self { model })
    }
}

impl Drawer for InspectorWindow {
    fn show_window(&self, _state: Option<&mut super::docking_tab::window_state::WindowState>) {}

    fn ui(&self, ui: &mut egui::Ui, _messager: &mut super::Messager, _log: &mut Log) {
        ui.label("TODO: Inspector");
    }

    fn close(&self, messager: &mut super::Messager) {
        messager.send(Message::CloseInspectorTab);
    }

    fn get_name(&self) -> &'static str {
        type_name::<InspectorWindow>()
    }

    fn get_title(&self) -> egui::WidgetText {
        self.model.style.title.to_owned().into()
    }

    fn get_style_fileds(&self) -> Vec<super::ui_style_fields::StyleField> {
        Vec::new()
    }

    fn update_style(&mut self, _style_fields: &Vec<super::ui_style_fields::StyleField>) {}

    fn render(
        &self,
        _engine: &mut Engine,
        _game: &mut KairosGame,
        _messager: &mut Messager,
    ) -> Option<crate::graphics::graphics_graph::GraphicsCommand> {
        None
    }
}
