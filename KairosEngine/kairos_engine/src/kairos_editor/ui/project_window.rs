pub mod content_panel;
pub mod context_menu;
pub mod hierarchy_panel;

use std::{any::type_name, cell::Cell, cell::RefCell, fs};

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
use rapier3d::na;
use serde::{Deserialize, Serialize};
use toml::from_str;

use crate::kairos_editor::ui::{Drawer, Message, paths};

// ============================================================
// RenameOrigin — 标记重命名从哪个面板发起
// ============================================================

#[derive(Clone, Copy, PartialEq)]
pub enum RenameOrigin {
    Hierarchy,
    Content,
}

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
    /// 当前在 Hierarchy 中选中的目录节点
    selected_node: Option<NodeIndex>,
    /// Content panel 当前展示的目录（双击目录进入时更新）
    active_directory: Option<NodeIndex>,
    /// 需要强制展开的父目录（下一帧消费并清除）
    pending_expand: Cell<Option<NodeIndex>>,
    /// 需要滚动到的节点（下一帧消费并清除）
    pending_scroll: Cell<Option<NodeIndex>>,
    /// 创建或重命名时进入编辑状态的节点
    pub(crate) renaming_node: Cell<Option<NodeIndex>>,
    /// 重命名从哪个面板发起（None = 两边都显示，用于创建后自动重命名）
    renaming_origin: Cell<Option<RenameOrigin>>,
    /// 重命名时的文本缓冲区
    rename_buffer: RefCell<String>,
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
            pending_expand: Cell::new(None),
            pending_scroll: Cell::new(None),
            renaming_node: Cell::new(None),
            renaming_origin: Cell::new(None),
            rename_buffer: RefCell::new(String::new()),
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
    pub fn select_node(&mut self, node: NodeIndex) {
        self.model.selected_node = Some(node);
    }

    /// hierarchy 点击进入（选中 + 更新 content_panel 展示的目录）。
    pub fn navigate_to(&mut self, node: NodeIndex) {
        self.model.selected_node = Some(node);
        self.model.active_directory = Some(node);
    }

    /// content_panel 双击目录：导航 + 强制 hierarchy 展开并滚动到该目录。
    pub fn navigate_to_with_expand(&mut self, node: NodeIndex) {
        self.navigate_to(node);
        self.model.pending_expand.set(Some(node));
        self.model.pending_scroll.set(Some(node));
    }

    /// 创建节点
    /// `clicked_node` 是右键点击的节点（文件或目录）
    pub fn create_node(
        &mut self,
        clicked_node: NodeIndex,
        name: String,
        kind: ProjectNodeKind,
        origin: Option<RenameOrigin>,
    ) {
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
            self.model.pending_expand.set(parent);
            self.model.pending_scroll.set(Some(new_node));

            // 预填 buffer（stem）并进入重命名
            if let Some(data) = self.model.project_path_graph.get_node(new_node) {
                let full = data.name();
                let stem = std::path::Path::new(&full)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&full);
                *self.model.rename_buffer.borrow_mut() = stem.to_owned();
            }
            self.model.renaming_node.set(Some(new_node));
            self.model.renaming_origin.set(origin);

            let _ = self.model.asset_registry.save();
        }
    }

    /// 重命名节点。
    pub fn rename_node(&mut self, node: NodeIndex, new_name: String) {
        // 防止 hierarchy 和 content 同时发送消息导致重复处理
        if self.model.renaming_node.get() != Some(node) {
            return;
        }

        if let Ok(()) = self.model.project_path_graph.rename_node(
            &mut self.model.asset_registry,
            node,
            &new_name,
        ) {
            let _ = self.model.asset_registry.save();
        }
        self.cancel_rename();
    }

    /// 进入重命名模式：预填当前名称（不含扩展名）到缓冲区。
    /// `origin` — 从哪个面板发起；`None` 表示两边都显示（创建后自动重命名）。
    pub fn start_rename(&self, node: NodeIndex, origin: Option<RenameOrigin>) {
        if let Some(data) = self.model.project_path_graph.get_node(node) {
            let full_name = data.name();
            let stem = std::path::Path::new(&full_name)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&full_name);
            *self.model.rename_buffer.borrow_mut() = stem.to_owned();
            self.model.renaming_node.set(Some(node));
            self.model.renaming_origin.set(origin);
        }
    }

    /// 取消重命名模式。
    pub fn cancel_rename(&self) {
        self.model.renaming_node.set(None);
        self.model.renaming_origin.set(None);
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
        let pending_expand = self.model.pending_expand.take();
        let pending_scroll = self.model.pending_scroll.take();
        let renaming_node = self.model.renaming_node.get();
        let renaming_origin = self.model.renaming_origin.get();
        let mut rename_buffer = self.model.rename_buffer.borrow_mut();

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
                            pending_expand,
                            pending_scroll,
                            renaming_node,
                            renaming_origin,
                            &mut *rename_buffer,
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
                        renaming_origin,
                        &mut *rename_buffer,
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
