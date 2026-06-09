use std::{any::type_name, fs};

use crate::{
    asset_loader::assets::AssetsServer, ecs::world::World, kairos_editor::ui::Messager,
    kairos_game::KairosGame, log::Log,
};
use serde::{Deserialize, Serialize};
use toml::from_str;

use crate::kairos_editor::ui::{Drawer, Message, paths};

#[derive(Debug, Serialize, Deserialize)]
struct HierarchyWindowStyle {
    pub title: String,
}

struct HierarchyWindowModel {
    style: HierarchyWindowStyle,
}

pub struct HierarchyWindow {
    model: HierarchyWindowModel,
}

impl HierarchyWindowStyle {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style_json =
            fs::read_to_string(paths::PATH_HIERARCHY_WINDOW_STYLE).map_err(|error| {
                format!(
                    "Load HierarchyWindow Style Json Failed, path: {}, error: {}",
                    paths::PATH_HIERARCHY_WINDOW_STYLE,
                    error
                )
            })?;
        let style = from_str(&style_json).map_err(|error| {
            format!(
                "Deserialize HierarchyWindow Style Json Failed, error: {}",
                error
            )
        })?;

        Ok(style)
    }
}

impl HierarchyWindowModel {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style = HierarchyWindowStyle::new()?;
        Ok(Self { style })
    }
}

impl HierarchyWindow {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let model = HierarchyWindowModel::new()?;
        Ok(Self { model })
    }
}

impl Drawer for HierarchyWindow {
    fn show_window(&self, _state: Option<&mut super::docking_tab::window_state::WindowState>) {}

    fn ui(&self, ui: &mut egui::Ui, _messager: &mut super::Messager, _log: &mut Log) {
        ui.label("TODO: Hierarchy");
    }

    fn close(&self, messager: &mut super::Messager) {
        messager.send(Message::CloseHierarchyTab);
    }

    fn get_name(&self) -> &'static str {
        type_name::<HierarchyWindow>()
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
        _world: &mut World,
        _game: &mut KairosGame,
        _messager: &mut Messager,
    ) -> Option<crate::graphics::graphics_graph::GraphicsCommand> {
        None
    }
}
