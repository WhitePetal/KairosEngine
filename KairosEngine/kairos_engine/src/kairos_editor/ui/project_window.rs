pub mod hierarchy_panel;

use std::{any::type_name, fs};

use crate::{
    asset_loader::assets::AssetsServer,
    kairos_editor::{
        Engine,
        asset_registry::AssetRegistry,
        project_path_tree::ProjectPathGraph,
        ui::Messager,
    },
    kairos_game::KairosGame,
    log::Log,
};
use serde::{Deserialize, Serialize};
use toml::from_str;

use crate::kairos_editor::ui::{Drawer, Message, paths};

#[derive(Debug, Serialize, Deserialize)]
struct ProjectWindowStyle {
    pub title: String,
}

struct ProjectWindowModel {
    style: ProjectWindowStyle,
    asset_registry: AssetRegistry,
    project_path_graph: ProjectPathGraph,
}

pub struct ProjectWindow {
    model: ProjectWindowModel,
}

impl ProjectWindowStyle {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style_json = fs::read_to_string(paths::PATH_PROJECT_WINDOW_STYLE).map_err(|error| {
            format!(
                "Load ProjectWindow Style Json Failed, path: {}, error: {}",
                paths::PATH_PROJECT_WINDOW_STYLE,
                error
            )
        })?;
        let style = from_str(&style_json).map_err(|error| {
            format!(
                "Deserialize ProjectWindow Style Json Failed, error: {}",
                error
            )
        })?;
        Ok(style)
    }
}

impl ProjectWindowModel {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style = ProjectWindowStyle::new()?;

        // 从磁盘加载已有 Registry（不存在则创建空表）
        let mut asset_registry = AssetRegistry::load().unwrap_or_else(|e| {
            log::warn!("Failed to load AssetRegistry, creating new one: {}", e);
            AssetRegistry::new()
        });

        // 扫描项目目录，生成/复用 GUID
        let project_path_graph = ProjectPathGraph::new(&mut asset_registry);

        // 持久化 registry（确保新扫描到的文件 GUID 被保存）
        if let Err(e) = asset_registry.save() {
            log::warn!("Failed to save AssetRegistry: {}", e);
        }

        Ok(Self {
            style,
            asset_registry,
            project_path_graph,
        })
    }
}

impl ProjectWindow {
    #[inline(always)]
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let model = ProjectWindowModel::new()?;
        Ok(Self { model })
    }
}

impl Drawer for ProjectWindow {
    fn create(
        _assets_server: &mut AssetsServer,
    ) -> Result<Self, Box<dyn std::error::Error>>
    where
        Self: Sized,
    {
        Self::new()
    }

    fn show_window(&self, _state: Option<&mut super::docking_tab::window_state::WindowState>) {}

    fn ui(
        &self,
        ui: &mut egui::Ui,
        _messager: &mut super::Messager,
        _engine: &Engine,
        _log: &mut Log,
    ) {
        hierarchy_panel::draw(
            ui,
            &self.model.project_path_graph,
            &self.model.asset_registry,
        );
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
