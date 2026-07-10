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
use petgraph::graph::NodeIndex;
use serde::{Deserialize, Serialize};
use toml::from_str;

use crate::kairos_editor::ui::{Drawer, Message, paths};

// ============================================================
// Style — 从 ProjectWindowStyle.toml 反序列化
// ============================================================
#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectWindowStyle {
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
    /// 当前选中的目录节点
    selected_node: Option<NodeIndex>,
    /// Content panel 当前展示的目录（双击目录进入时更新）
    active_directory: Option<NodeIndex>,
    /// 创建或重命名时进入编辑状态的节点
    renaming_node: Option<NodeIndex>,
    /// 重命名字符串buffer
    renaming_buffer: Option<String>,
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
            renaming_node: None,
            renaming_buffer: None,
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
    pub fn select_node(&mut self, node: Option<NodeIndex>) {
        self.model.selected_node = node;
    }

    /// hierarchy 点击进入（选中 + 更新 content_panel 展示的目录）。
    pub fn navigate_to(&mut self, node: NodeIndex) {
        self.model.selected_node = Some(node);
        self.model.active_directory = Some(node);
    }

    /// 创建节点
    /// `clicked_node` 是右键点击的节点（文件或目录）
    pub fn create_node(&mut self, clicked_node: NodeIndex, name: String, kind: ProjectNodeKind) {
        let request = CreateRequest {
            base_node: clicked_node,
            name,
            kind,
        };

        if let Some(new_node) = self
            .model
            .project_path_graph
            .create_node(&mut self.model.asset_registry, request)
        {
            let parent = self.model.project_path_graph.get_parent(new_node);
            self.model.active_directory = parent;
            self.model.selected_node = Some(new_node);

            // 预填 buffer（stem）并进入重命名
            if let Some(data) = self.model.project_path_graph.get_node(new_node) {
                let full = data.name();
                let stem = std::path::Path::new(&full)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&full);

                self.model.renaming_buffer = Some(stem.to_owned());
            } else {
                self.model.renaming_buffer = Some(String::new());
            }

            self.model.renaming_node = Some(new_node);

            if let Err(err) = self.model.asset_registry.save() {
                log::error!("save asset registry failed: {}", err);
            }
        }
    }

    /// 重命名节点。
    pub fn rename_node(&mut self, node: NodeIndex, new_name: String) {
        // 防止 hierarchy 和 content 同时发送消息导致重复处理
        if self.model.renaming_node != Some(node) {
            return;
        }

        if let Ok(()) = self.model.project_path_graph.rename_node(
            &mut self.model.asset_registry,
            node,
            &new_name,
        ) {
            let _ = self.model.asset_registry.save();
        }
        self.exit_rename();
    }

    pub fn update_renaming_buffer(&mut self, buffer: String) {
        self.model.renaming_buffer = Some(buffer);
    }

    /// 进入重命名模式：预填当前名称（不含扩展名）到缓冲区。
    /// `origin` — 从哪个面板发起；`None` 表示两边都显示（创建后自动重命名）。
    pub fn start_rename(&mut self, node: NodeIndex) {
        if let Some(data) = self.model.project_path_graph.get_node(node) {
            let full_name = data.name();
            let stem = std::path::Path::new(&full_name)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&full_name);
            self.model.renaming_buffer = Some(stem.to_owned());
            self.model.renaming_node = Some(node);
        }
    }

    /// 退出重命名模式。
    pub fn exit_rename(&mut self) {
        self.model.renaming_node = None;
        self.model.renaming_buffer = None;
    }

    /// 删除节点。
    pub fn delete_node(&mut self, node: NodeIndex) {
        match self
            .model
            .project_path_graph
            .delete_node(&mut self.model.asset_registry, node)
        {
            Ok(()) => {
                // 如果删除的是选中节点或当前目录，清除选中状态
                if self.model.selected_node == Some(node) {
                    self.model.selected_node = None;
                }
                if self.model.active_directory == Some(node) {
                    self.model.active_directory = None;
                }
                let _ = self.model.asset_registry.save();
            }
            Err(e) => log::warn!("Failed to delete node: {e}"),
        }
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
        let renaming_node = self.model.renaming_node;
        let renaming_buffer = &self.model.renaming_buffer;

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
                        renaming_node,
                        renaming_buffer,
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
