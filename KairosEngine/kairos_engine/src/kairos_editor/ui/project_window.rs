pub mod content_panel;
pub mod context_menu;
pub mod hierarchy_panel;

use std::{any::type_name, cell::Cell, fs, ops::Deref, sync::Arc};

use crate::{
    asset_loader::assets::AssetsServer,
    kairos_editor::{
        Engine,
        asset_registry::{AssetKind, AssetRegistry},
        project_path_tree::{ProjectPathGraph, create_request::CreateRequest},
        ui::{
            self, Messager,
            global_styles::GlobalStyles,
            inspector::creater::InspectorCreater,
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
use parking_lot::Mutex;
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
    renaming_buffer: Option<Arc<Mutex<String>>>,
    /// 一次性强制展开标记：下一帧 hierarchy 渲染时将展开到该节点的完整路径，
    /// 渲染后立即清除。用 `Cell` 使得 `ui(&self)` 中也能写入。
    force_expand_to: Cell<Option<NodeIndex>>,
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
            force_expand_to: Cell::new(None),
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
    /// 锁定时拒绝取消选中（`None`）。
    pub fn select_node(&mut self, node: Option<NodeIndex>) {
        self.model.selected_node = node;
    }

    /// 根据文件路径查找并选中节点（for test harness）。
    #[cfg(feature = "test-harness")]
    pub(crate) fn find_node_by_path(&self, path: &std::path::Path) -> Option<NodeIndex> {
        self.model.project_path_graph.find_by_path(path)
    }

    /// 获取当前选中节点的身份信息（供 InspectorWindow 使用）。
    pub fn get_selected_node_info(
        &self,
        assets_server: &mut AssetsServer,
    ) -> Option<super::inspector_window::InspectorNodeInfo> {
        let node = self.model.selected_node?;
        let data = self.model.project_path_graph.get_node(node)?;
        // Use the asset path for imported types (Texture → .texture, not .png).
        let inspector_path = data.asset_path.as_ref().unwrap_or(&data.path);
        let inspector =
            InspectorCreater::create_from_asseet_kind(data.kind, inspector_path, assets_server);
        match inspector {
            Ok(inspector) => Some(super::inspector_window::InspectorNodeInfo {
                name: data.name(),
                kind: data.kind,
                path: data.path.clone(),
                guid: data.guid,
                inspector,
            }),
            Err(err) => {
                log::warn!("create inspector failed: {:?}", err);
                None
            }
        }
    }

    /// hierarchy 点击进入（选中 + 更新 content_panel 展示的目录）。
    /// 不设置 force_expand_to —— CollapsingHeader 自己管理折叠/展开状态。
    pub fn navigate_to(&mut self, node: NodeIndex) {
        self.model.selected_node = Some(node);
        self.model.active_directory = Some(node);
    }

    /// 创建节点
    /// `clicked_node` 是右键点击的节点（文件或目录）
    pub fn create_node(&mut self, clicked_node: NodeIndex, name: String, kind: AssetKind) {
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
            self.model.force_expand_to.set(parent);
            self.model.selected_node = Some(new_node);

            // 预填 buffer（stem）并进入重命名
            if let Some(data) = self.model.project_path_graph.get_node(new_node) {
                let full = data.name();
                let stem = std::path::Path::new(&full)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&full);

                self.model.renaming_buffer = Some(Arc::new(Mutex::new(stem.to_owned())));
            } else {
                self.model.renaming_buffer = Some(Arc::new(Mutex::new(String::new())));
            }

            self.model.renaming_node = Some(new_node);

            if let Err(err) = self.model.asset_registry.save() {
                log::error!("save asset registry failed: {}", err);
            }
        }
    }

    /// 重命名节点。
    pub fn rename_node(&mut self) {
        // 防止 hierarchy 和 content 同时发送消息导致重复处理
        let Some(node) = self.model.renaming_node else {
            return;
        };

        let Some(reanme_buffer) = self.model.renaming_buffer.clone() else {
            return;
        };
        let new_name = reanme_buffer.lock();
        let new_name = new_name.deref();

        if let Ok(()) = self.model.project_path_graph.rename_node(
            &mut self.model.asset_registry,
            node,
            new_name,
        ) {
            let _ = self.model.asset_registry.save();
        }
        self.exit_rename();
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
            self.model.renaming_buffer = Some(Arc::new(Mutex::new(stem.to_owned())));
            self.model.renaming_node = Some(node);
        }
        if let Some(parent) = self.model.project_path_graph.get_parent(node) {
            self.navigate_to(parent);
        }
    }

    /// 退出重命名模式。
    pub fn exit_rename(&mut self) {
        self.model.renaming_node = None;
        self.model.renaming_buffer = None;
    }

    /// 打开节点。根据节点类型分发到不同的打开逻辑：
    /// - `Directory`：进入该目录（等价于 `navigate_to`）
    /// - 其他类型：预留扩展点，当前打印日志提示未实现
    pub fn open_node(&mut self, node: NodeIndex) {
        let Some(node_data) = self.model.project_path_graph.get_node(node) else {
            return;
        };
        match node_data.kind {
            AssetKind::Directory => {
                self.navigate_to(node);
                self.model.force_expand_to.set(Some(node));
            }
            AssetKind::Texture => {
                log::info!("Open Texture is not yet implemented: {}", node_data.name());
            }
            AssetKind::Mesh => {
                log::info!("Open Mesh is not yet implemented: {}", node_data.name());
            }
            AssetKind::Material => {
                log::info!("Open Material is not yet implemented: {}", node_data.name());
            }
            AssetKind::Audio => {
                log::info!("Open Audio is not yet implemented: {}", node_data.name());
            }
            AssetKind::Shader
            | AssetKind::Script
            | AssetKind::Document
            | AssetKind::Toml
            | AssetKind::Font => {
                Self::open_file_in_vscode(&node_data.path);
            }
            AssetKind::Unknown => {
                log::info!("Open Unknown is not yet implemented: {}", node_data.name());
            }
        }
    }

    /// 通过 VS Code 打开文件。
    /// 优先尝试 `code` 命令（终端环境），失败时回退到 macOS 完整路径。
    /// `--reuse-window` 复用已有窗口。
    fn open_file_in_vscode(path: &std::path::Path) {
        let result = std::process::Command::new("code")
            .arg(path)
            .arg("--reuse-window")
            .spawn()
            .or_else(|_| {
                std::process::Command::new(
                    "/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code",
                )
                .arg(path)
                .arg("--reuse-window")
                .spawn()
            });

        if let Err(e) = result {
            log::warn!("Failed to open '{}' with VS Code: {e}", path.display());
        }
    }

    /// 绘制底部栏：当有文件节点被选中时，展示选中文件的路径。
    fn draw_bottom_bar(
        ui: &mut egui::Ui,
        graph: &ProjectPathGraph,
        style: &ProjectWindowStyle,
        selected_node: Option<NodeIndex>,
    ) {
        if let Some(node) = selected_node {
            if let Some(data) = graph.get_node(node)
                && let Some(path) = data.path.to_str()
            {
                ui.label(
                    egui::RichText::new(path)
                        .size(style.content.bottom_bar_font_size)
                        .color(style.content.bottom_bar_text_color),
                );
            }
        }
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
        // 一次性强制展开标记：本帧使用后立即清除
        let force_expand_to = self.model.force_expand_to.get();

        // 左侧：Hierarchy Panel
        egui::Panel::left("project_window_hierachy_panel")
            .resizable(true)
            .show(ui, |ui| {
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
                            force_expand_to,
                        );
                    });
            });

        // 清除一次性标记，下一帧不再强制展开
        self.model.force_expand_to.set(None);

        // 右侧：Content Panel
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("content_scroll")
                .max_height(
                    ui.available_height()
                        - self.model.style.content.bottom_bar_height
                        - ui::DEFAULT_SPEATOR_HEIGHT,
                )
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
            ui.separator();
            ui.vertical(|ui| {
                Self::draw_bottom_bar(
                    ui,
                    &self.model.project_path_graph,
                    &self.model.style,
                    self.model.selected_node,
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
