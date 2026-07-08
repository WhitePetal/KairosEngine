pub mod content_panel;
pub mod context_menu;
pub mod hierarchy_panel;

use std::{any::type_name, fs};

use crate::{
    asset_loader::assets::AssetsServer,
    kairos_editor::{
        Engine,
        asset_registry::AssetRegistry,
        project_path_tree::{
            ProjectPathGraph, create_request::CreateRequest, tree_node::ProjectNodeKind,
        },
        ui::{
            Messager,
            global_styles::GlobalStyles,
            project_window::{
                content_panel::{ContentPanel, ContentStyle},
                context_menu::ContextMenuState,
                hierarchy_panel::{HierarchyPanel, HierarchyStyle},
            },
        },
    },
    kairos_game::KairosGame,
    log::Log,
};
use petgraph::{graph::NodeIndex, visit::EdgeRef};
use serde::{Deserialize, Serialize};
use toml::from_str;

use crate::kairos_editor::ui::{Drawer, Message, paths};

// ============================================================
// Style — 从 ProjectWindowStyle.toml 反序列化
// ============================================================
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct ProjectWindowStyle {
    pub title: String,
    pub hierachy: HierarchyStyle,
    pub content: ContentStyle,
}

// ============================================================
// Model
// ============================================================

struct ProjectWindowModel {
    style: ProjectWindowStyle,
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
                "Load ProjectWindow Style Toml Failed, path: {}, error: {}",
                paths::PATH_PROJECT_WINDOW_STYLE,
                error
            )
        })?;
        let style: Self = from_str(&style_json).map_err(|error| {
            format!(
                "Deserialize ProjectWindow Style Toml Failed, error: {}",
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
            asset_registry: asset_registry,
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

    /// 在指定父目录下创建节点，成功后自动选中。
    pub(super) fn create_node(&mut self, parent: NodeIndex, name: String, kind: ProjectNodeKind) {
        let unique_name = self.unique_name(parent, &name);
        let request = CreateRequest {
            parent_node: parent,
            name: unique_name,
            kind,
        };

        match self
            .model
            .project_path_graph
            .create_node(&mut self.model.asset_registry, request)
        {
            Ok(new_node) => {
                self.model.selected_node = Some(new_node);
                let _ = self.model.asset_registry.save();
            }
            Err(e) => {
                log::warn!("Failed to create node: {e}");
            }
        }
    }

    /// 生成不重复的名称（"New Folder" → "New Folder (1)" → ...）
    fn unique_name(&self, parent: NodeIndex, base: &str) -> String {
        let exists = |name: &str| {
            self.model.project_path_graph.get_edges(parent).any(|e| {
                self.model
                    .project_path_graph
                    .get_node(e.target())
                    .is_some_and(|n| n.name.to_string_lossy().eq_ignore_ascii_case(name))
            })
        };

        if !exists(base) {
            return base.to_string();
        }
        for i in 1..1000u32 {
            let candidate = format!("{base} ({i})");
            if !exists(&candidate) {
                return candidate;
            }
        }
        format!(
            "{base} ({})",
            uuid::Uuid::new_v4()
                .to_string()
                .chars()
                .take(8)
                .collect::<String>()
        )
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
        global_styles: &GlobalStyles,
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
                        HierarchyPanel::draw(
                            ui,
                            global_styles,
                            &self.model.project_path_graph,
                            &self.model.style,
                            messager,
                            selected,
                        );
                    });
            });

        // 右侧：Content Panel（只垂直滚动，水平自然换行）
        egui::CentralPanel::default().show_inside(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("content_scroll")
                .show(ui, |ui| {
                    ContentPanel::draw(
                        ui,
                        global_styles,
                        &self.model.project_path_graph,
                        &self.model.style,
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
