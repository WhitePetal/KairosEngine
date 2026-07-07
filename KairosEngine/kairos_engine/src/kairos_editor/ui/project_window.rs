pub mod content_panel;
pub mod hierarchy_panel;

use std::{any::type_name, fs};

use crate::{
    asset_loader::assets::AssetsServer,
    kairos_editor::{
        Engine,
        asset_registry::AssetRegistry,
        project_path_tree::{ProjectPathGraph, tree_node::ProjectTreeNode},
        ui::Messager,
    },
    kairos_game::KairosGame,
    log::Log,
    math,
};
use egui::Color32;
use petgraph::graph::NodeIndex;
use serde::{Deserialize, Serialize};
use toml::from_str;

use crate::kairos_editor::{
    project_path_tree::tree_node::ProjectNodeKind,
    ui::{Drawer, Message, paths},
};

// ============================================================
// Style — 从 ProjectWindowStyle.toml 反序列化
// ============================================================

#[derive(Debug, Default, Serialize, Deserialize)]
pub(super) struct ProjectWindowIcons {
    #[serde(default = "default_icon_size")]
    pub size: [f32; 2],
    #[serde(default = "default_icon_path")]
    pub default: String,
    #[serde(default)]
    pub directory: Option<String>,
    #[serde(default)]
    pub mesh: Option<String>,
    #[serde(default)]
    pub material: Option<String>,
    #[serde(default)]
    pub audio: Option<String>,
    #[serde(default)]
    pub shader: Option<String>,
    #[serde(default, alias = "generic_asset")]
    pub generic_asset: Option<String>,
    #[serde(default)]
    pub script: Option<String>,
    #[serde(default)]
    pub document: Option<String>,
    #[serde(default)]
    pub toml: Option<String>,
}

fn default_icon_size() -> [f32; 2] {
    [18.0, 18.0]
}
fn default_icon_path() -> String {
    paths::PATH_ENGINE_ICON.into()
}

impl ProjectWindowIcons {
    /// 根据节点类型获取对应图标路径，未配置则回退到 `default`。
    pub fn for_kind<'a>(&'a self, node: &'a ProjectTreeNode) -> &'a str {
        let opt = match node.kind {
            ProjectNodeKind::Directory => self.directory.as_deref(),
            ProjectNodeKind::Texture => node.path.to_str(),
            ProjectNodeKind::Mesh => self.mesh.as_deref(),
            ProjectNodeKind::Material => self.material.as_deref(),
            ProjectNodeKind::Audio => self.audio.as_deref(),
            ProjectNodeKind::Shader => self.shader.as_deref(),
            ProjectNodeKind::GenericAsset => self.generic_asset.as_deref(),
            ProjectNodeKind::Script => self.script.as_deref(),
            ProjectNodeKind::Document => self.document.as_deref(),
            ProjectNodeKind::Toml => self.toml.as_deref(),
            ProjectNodeKind::Unknown => None,
        };
        opt.unwrap_or(&self.default)
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(super) struct ProjectWindowColors {
    #[serde(default = "default_arrow_color")]
    pub arrow: math::Color32,
    #[serde(default = "default_directory_color")]
    pub directory: math::Color32,
    #[serde(default = "default_file_color")]
    pub file: math::Color32,
    #[serde(default = "default_selection_color")]
    pub selection: math::Color32,
}

fn default_arrow_color() -> math::Color32 {
    math::Color32::new(160, 160, 160, 255)
}
fn default_directory_color() -> math::Color32 {
    math::Color32::new(220, 220, 180, 255)
}
fn default_file_color() -> math::Color32 {
    math::Color32::new(200, 200, 200, 255)
}
fn default_selection_color() -> math::Color32 {
    math::Color32::new(60, 75, 100, 255)
}

impl ProjectWindowColors {
    pub fn directory(&self) -> Color32 {
        self.directory.into()
    }
    pub fn file(&self) -> Color32 {
        self.file.into()
    }
    pub fn _arrow(&self) -> Color32 {
        self.arrow.into()
    }
    pub fn selection(&self) -> Color32 {
        self.selection.into()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct ProjectWindowStyle {
    pub title: String,
    #[serde(default)]
    pub icons: ProjectWindowIcons,
    #[serde(default)]
    pub colors: ProjectWindowColors,
}

// ============================================================
// Model
// ============================================================

struct ProjectWindowModel {
    style: ProjectWindowStyle,
    #[allow(dead_code)]
    asset_registry: AssetRegistry,
    project_path_graph: ProjectPathGraph,
    /// 当前在 Hierarchy 中选中的目录节点
    selected_node: Option<NodeIndex>,
    /// Content panel 当前展示的目录（双击目录进入时更新）
    active_directory: Option<NodeIndex>,
}

// ============================================================
// Window
// ============================================================

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
        let style: Self = from_str(&style_json).map_err(|error| {
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

        let mut asset_registry = AssetRegistry::load().unwrap_or_else(|e| {
            println!("Failed to load AssetRegistry, creating new one: {}", e);
            AssetRegistry::new()
        });

        let project_path_graph = ProjectPathGraph::new(&mut asset_registry);

        if let Err(e) = asset_registry.save() {
            println!("Failed to save AssetRegistry: {}", e);
        }

        Ok(Self {
            style,
            asset_registry,
            project_path_graph,
            selected_node: None,
            active_directory: None,
        })
    }
}

impl ProjectWindow {
    #[inline(always)]
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let model = ProjectWindowModel::new()?;
        Ok(Self { model })
    }

    /// 点击选中节点（仅高亮，不改变 content_panel 展示内容）。
    pub(super) fn select_node(&mut self, node: NodeIndex) {
        self.model.selected_node = Some(node);
    }

    /// 双击目录进入（选中 + 更新 content_panel 展示的目录）。
    pub(super) fn navigate_to(&mut self, node: NodeIndex) {
        self.model.selected_node = Some(node);
        self.model.active_directory = Some(node);
    }
}

impl Drawer for ProjectWindow {
    fn create(_assets_server: &mut AssetsServer) -> Result<Self, Box<dyn std::error::Error>>
    where
        Self: Sized,
    {
        Self::new()
    }

    fn show_window(&self, _state: Option<&mut super::docking_tab::window_state::WindowState>) {}

    fn scroll_bars(&self) -> [bool; 2] {
        [false, false]
    }

    fn ui(
        &self,
        ui: &mut egui::Ui,
        messager: &mut super::Messager,
        _engine: &Engine,
        _log: &mut Log,
    ) {
        let selected = self.model.selected_node;
        let active_dir = self.model.active_directory;

        // 左侧：Hierarchy Panel
        egui::Panel::left("project_window_hierachy_panel")
            .resizable(true)
            .show_inside(ui, |ui| {
                egui::ScrollArea::both()
                    .id_salt("hierarchy_scroll")
                    .show(ui, |ui| {
                        hierarchy_panel::draw(
                            ui,
                            &self.model.project_path_graph,
                            &self.model.style.icons,
                            &self.model.style.colors,
                            messager,
                            selected,
                        );
                    });
            });

        // 右侧：Content Panel（只垂直滚动，水平自然换行）
        egui::CentralPanel::default()
            .show_inside(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("content_scroll")
                .show(ui, |ui| {
                content_panel::draw(
                    ui,
                    &self.model.project_path_graph,
                    &self.model.style.icons,
                    &self.model.style.colors,
                    messager,
                    active_dir,
                    selected,
                );
            });
        });
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
