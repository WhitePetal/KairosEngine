use std::{any::type_name, fs};

use serde::{Deserialize, Serialize};
use toml::from_str;

use crate::kairos_editor::ui::{Drawer, Message, paths};

#[derive(Debug, Serialize, Deserialize)]
struct SceneWindowStyle {
    pub title: String,
}

struct SceneWindowModel {
    style: SceneWindowStyle,
}

pub struct SceneWindow {
    model: SceneWindowModel,
}

impl SceneWindowStyle {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style_json = fs::read_to_string(paths::PATH_SCENE_WINDOW_STYLE).map_err(|error| {
            format!(
                "Load SceneWindow Style Json Failed, path: {}, error: {}",
                paths::PATH_SCENE_WINDOW_STYLE,
                error
            )
        })?;
        let style = from_str(&style_json).map_err(|error| {
            format!(
                "Deserialize SceneWindow Style Json Failed, error: {}",
                error
            )
        })?;
        Ok(style)
    }
}

impl SceneWindowModel {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style = SceneWindowStyle::new()?;
        Ok(Self { style })
    }
}

impl SceneWindow {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let model = SceneWindowModel::new()?;
        Ok(Self { model })
    }
}

impl Drawer for SceneWindow {
    fn show_window(&self, _state: Option<&mut super::docking_tab::window_state::WindowState>) {}

    fn update(
        &self,
        ui: &mut egui::Ui,
        _messager: &mut super::Messager,
        _log: &mut crate::log::Log,
    ) {
        ui.label("TODO: Scene");
    }

    fn close(&self, messager: &mut super::Messager) {
        messager.send(Message::CloseSceneTab);
    }

    fn get_name(&self) -> &'static str {
        type_name::<SceneWindow>()
    }

    fn get_title(&self) -> egui::WidgetText {
        self.model.style.title.to_owned().into()
    }

    fn get_style_fileds(&self) -> Vec<super::ui_style_fields::StyleField> {
        Vec::new()
    }

    fn update_style(&mut self, _style_fields: &Vec<super::ui_style_fields::StyleField>) {}
}
