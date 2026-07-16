use crate::{
    asset_loader::assets::{AssetHandle, AssetsServer, TomlTableAssetsSystem},
    graphics::graphics_graph::GraphicsCommand,
    kairos_editor::{
        Engine,
        asset_registry::AssetKind,
        ui::{
            game_window::GameWindow,
            global_styles::{FontDataConfig, FontsConfig, GlobalStyles},
            layout::{
                EditorLayout, LayoutBottomContainer, LayoutContainerIds, LayoutLeftContainer,
                LayoutRightContainer, Zone,
            },
        },
    },
    kairos_game::KairosGame,
    log::Log,
    types::TypeIdMap,
};
use egui::{self};
use std::{
    any::{Any, TypeId, type_name},
    collections::VecDeque,
    sync::Arc,
};

use crate::{
    kairos_dialog,
    kairos_editor::ui::{
        about_window::AboutWindow,
        console_window::ConsoleWindow,
        docking_tab::{
            DockArea,
            dock_state::DockState,
            tab_drawer::{OnCloseResponse, TabDrawer},
            window_state::WindowState,
        },
        hierarchy_window::HierarchyWindow,
        inspector_window::InspectorWindow,
        preferences_window::PreferencesWindow,
        project_window::ProjectWindow,
        scene_window::SceneWindow,
        tool_bar::ToolBar,
        ui_style_fields::{StyleField, StylePage},
    },
};

pub mod about_window;
pub mod console_window;
pub mod dialog;
pub mod docking_tab;
pub mod egui_ext;
pub mod game_window;
pub mod global_styles;
pub mod hierarchy_window;
pub mod inspector;
pub mod inspector_window;
pub mod layout;
pub mod native_dialog;
pub mod paths;
pub mod preferences_window;
pub mod project_window;
pub mod scene_window;
pub mod tool_bar;
pub mod ui_style_fields;

pub enum Message {
    CreateToolbar,
    InitLayout,
    QuitEngine,
    OpenAboutWindow,
    CloseAboutWindow,
    OpenPreferenceWindow,
    ClosePreferenceWindow,
    RefershPreferenceWindow,
    SetPreferenceWindowSelectedId(usize),
    UpdateUIStyle(StylePage),
    OpenConsoleTab,
    CloseConsoleTab,
    OpenInspectorTab,
    CloseInspectorTab,
    OpenHierarchyTab,
    CloseHierarchyTab,
    OpenProjectTab,
    CloseProjectTab,
    OpenSceneTab,
    CloseSceneTab,
    /// (widht, height)
    UpdateSceneWindowSize(u32, u32),
    RegisteSceneWindowViewBind(tokio::sync::oneshot::Receiver<egui::TextureId>),
    SceneWindowTryReceTextureId,
    OpenGameTab,
    CloseGameTab,
    /// (width, height)
    UpdateGameWindowSize(u32, u32),
    RegisteGameWindowViewBind(tokio::sync::oneshot::Receiver<egui::TextureId>),
    GameWindowTryReceTextureId,

    /// SceneCamera orbit (dx, dy, dt) in pixels
    SceneCameraOrbit(f32, f32, f32),
    /// Camera zoom (delta, dt)
    CameraZoom(f32, f32),
    /// Camera fly movement (right, forward, dt) each in [-1, 0, 1]
    CameraFly(f32, f32, f32),

    /// ProjectWindow: 选中节点（NodeIndex::index()）
    SelectProjectNode(Option<petgraph::graph::NodeIndex>),
    /// ProjectWindow: Hierachy点击进入目录（NodeIndex::index()）
    NavigateToProjectDirectory(petgraph::graph::NodeIndex),
    /// ProjectWindow: 创建节点(右键节点Idx, 名称, 类型, 来源面板)
    CreateProjectNode(petgraph::graph::NodeIndex, String, AssetKind),
    /// ProjectWindow: 重命名节点 (node_idx, new_name)
    RenameProjectNode(petgraph::graph::NodeIndex, Arc<parking_lot::Mutex<String>>),
    /// 更新重命名Buffer
    UpdateProjectWindowRenamingBuffer(Arc<parking_lot::Mutex<String>>),
    /// ProjectWindow: 进入重命名模式 (node_idx, origin)
    StartRenameProjectNode(petgraph::graph::NodeIndex),
    /// ProjectWindow: 退出重命名模式
    ExitRenameProjectNode,
    /// ProjectWindow: 打开节点（根据类型执行不同操作）
    OpenProjectNode(petgraph::graph::NodeIndex),
    /// ProjectWindow: 删除节点 (node_idx)
    DeleteProjectNode(petgraph::graph::NodeIndex),
    /// InspectorWindow: 更新 TOML 字段值 (key_path, new_value)
    UpdateInspectorToml(
        Arc<AssetHandle<TomlTableAssetsSystem>>,
        Arc<parking_lot::Mutex<Option<toml::Table>>>,
    ),
    /// Audio Inspector: toggle play/pause preview.
    ToggleAudioPreview,
    /// Audio Inspector: seek to position (seconds) and start playback.
    SeekAudioPreview(f32),
}

struct KairosTabDrawer {
    // drawers: &'a Vec<Box<dyn Drawer>>,
}

impl TabDrawer for KairosTabDrawer {
    type Tab = usize;

    fn title(&self, tab: &mut Self::Tab, drawers: &Vec<Box<dyn Drawer>>) -> egui::WidgetText {
        let tab = &drawers[*tab];
        tab.get_title()
    }

    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        global_styles: &GlobalStyles,
        tab: &mut Self::Tab,
        messager: &mut Messager,
        engine: &Engine,
        log: &mut Log,
        drawers: &Vec<Box<dyn Drawer>>,
    ) {
        let tab = &drawers[*tab];
        tab.ui(ui, global_styles, messager, engine, log);
    }

    fn on_close(
        &mut self,
        tab: &mut Self::Tab,
        messager: &mut Messager,
        drawers: &Vec<Box<dyn Drawer>>,
    ) -> OnCloseResponse {
        let tab = &drawers[*tab];
        tab.close(messager);
        OnCloseResponse::Close
    }

    fn scroll_bars(&self, tab: &Self::Tab, drawers: &Vec<Box<dyn Drawer>>) -> [bool; 2] {
        let tab = &drawers[*tab];
        tab.scroll_bars()
    }

    // fn on_add(&mut self, surface: SurfaceIndex, node: NodeIndex) {
    //     println!("add tab: {0}, {1}", surface.0, node.0);
    //     self.drawer_paths.push((surface, node));
    // }
}

pub trait Drawer: Any {
    fn create(assets_server: &mut AssetsServer) -> Result<Self, Box<dyn std::error::Error>>
    where
        Self: Sized;

    fn show_window(&self, state: Option<&mut WindowState>);

    fn ui(
        &self,
        ui: &mut egui::Ui,
        global_styles: &GlobalStyles,
        messager: &mut Messager,
        engine: &Engine,
        log: &mut Log,
    );

    fn render(
        &self,
        engine: &mut Engine,
        game: &mut KairosGame,
        messager: &mut Messager,
    ) -> Option<GraphicsCommand>;

    fn close(&self, messager: &mut Messager);

    fn scroll_bars(&self) -> [bool; 2] {
        [true, true]
    }

    fn get_name(&self) -> &'static str;

    fn get_title(&self) -> egui::WidgetText;

    fn get_style_fileds(&self) -> Vec<StyleField>;

    fn update_style(&mut self, style_fields: &Vec<StyleField>);
}

pub struct Messager {
    messages: VecDeque<Message>,
}

impl Messager {
    pub fn new() -> Self {
        Self {
            messages: VecDeque::new(),
        }
    }

    pub fn send(&mut self, msg: Message) {
        self.messages.push_back(msg);
    }
}

pub struct Context {
    pub messager: Messager,
    layout: EditorLayout,
    ids: TypeIdMap<usize>,
    drawers: Vec<Box<dyn Drawer>>,
    actives: Vec<bool>,
    tab_tree: DockState<usize>,
    tab_viewer: KairosTabDrawer,
    dialogs: Vec<Box<dyn dialog::Dialog>>,
    global_styles: GlobalStyles,
}

impl Context {
    pub fn new(egui_ctx: &egui::Context) -> Result<Self, Box<dyn std::error::Error>> {
        let tab_tree = DockState::new(vec![]);

        let mut messager = Messager::new();
        messager.send(Message::CreateToolbar);
        messager.send(Message::InitLayout);
        messager.send(Message::OpenSceneTab);
        messager.send(Message::OpenGameTab);
        messager.send(Message::OpenProjectTab);
        messager.send(Message::OpenInspectorTab);
        messager.send(Message::OpenHierarchyTab);

        let global_styles = GlobalStyles::new()?;

        Self::setup_font(egui_ctx, &global_styles.fonts);

        Ok(Self {
            messager,
            ids: TypeIdMap::default(),
            drawers: Vec::new(),
            actives: Vec::new(),
            tab_tree,
            tab_viewer: KairosTabDrawer {},
            layout: EditorLayout::new(),
            dialogs: Vec::new(),
            global_styles: global_styles,
        })
    }

    pub fn darw(&mut self, ui: &mut egui::Ui, engine: &Engine, log: &mut Log) {
        ui.ctx().all_styles_mut(|style| {
            style.debug.warn_if_rect_changes_id = false;
        });

        // tool_bar
        let tool_bar_type_id = TypeId::of::<ToolBar>();
        if let Some(id) = self.ids.get(&tool_bar_type_id) {
            self.drawers[*id].ui(ui, &self.global_styles, &mut self.messager, engine, log);
        }

        // 中央区域显示内容
        egui::CentralPanel::default().show(ui, |ui| {
            DockArea::new("KairosEditor Main DockArea", &mut self.tab_tree).show_inside(
                ui,
                &self.global_styles,
                &mut self.messager,
                engine,
                log,
                &self.drawers,
                &mut self.tab_viewer,
            );
        });

        // dialogs
        self.dialogs.retain(|dialog| {
            let state = dialog.draw(ui);
            match state {
                dialog::DialogState::Opening => true,
                dialog::DialogState::Closed => false,
            }
        });
    }

    pub fn handle(&mut self, engine: &mut Engine, ui: &egui::Ui, _log: &mut Log) {
        while let Some(msg) = self.messager.messages.pop_front() {
            match msg {
                Message::CreateToolbar => {
                    let drawer = ToolBar::new().unwrap_or_else(|error| {
                        Context::create_ui_failed(ui, type_name::<ToolBar>(), error);
                    });
                    self.push_drawer::<ToolBar>(Box::new(drawer));
                }
                Message::InitLayout => {
                    let left_id =
                        self.push_drawer::<LayoutLeftContainer>(Box::new(LayoutLeftContainer {}));
                    let right_id =
                        self.push_drawer::<LayoutRightContainer>(Box::new(LayoutRightContainer {}));
                    let bottom_id = self
                        .push_drawer::<LayoutBottomContainer>(Box::new(LayoutBottomContainer {}));

                    let container_ids = LayoutContainerIds {
                        left: left_id,
                        right: right_id,
                        bottom: bottom_id,
                    };
                    self.layout.init_layout(&mut self.tab_tree, container_ids);

                    // 从各 zone 的 leaf 中移除容器（保留空 leaf 结构）
                    for (zone, container_id) in [
                        (self.layout.left, left_id),
                        (self.layout.right, right_id),
                        (self.layout.bottom, bottom_id),
                    ] {
                        use crate::kairos_editor::ui::docking_tab::dock_state::tree::node::Node;
                        let node = &mut self.tab_tree[zone.surface][zone.node];
                        if let Node::Leaf(leaf) = node {
                            leaf.retain_drawers(|id| *id != container_id);
                        }
                    }

                    // 标记容器为关闭，后续不会再被复用
                    self.close_drawer::<LayoutLeftContainer>();
                    self.close_drawer::<LayoutRightContainer>();
                    self.close_drawer::<LayoutBottomContainer>();
                }
                Message::QuitEngine => {
                    ui.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Message::OpenAboutWindow => {
                    self.show_window::<AboutWindow>(ui, AboutWindow::new);
                }
                Message::CloseAboutWindow => {
                    self.close_drawer::<AboutWindow>();
                }
                Message::OpenPreferenceWindow => {
                    self.show_window::<PreferencesWindow>(ui, PreferencesWindow::new);
                    self.messager
                        .messages
                        .push_back(Message::RefershPreferenceWindow);
                }
                Message::ClosePreferenceWindow => {
                    self.close_drawer::<PreferencesWindow>();
                }
                Message::RefershPreferenceWindow => {
                    let mut style_pages = Vec::new();
                    for (id, drawer) in self.drawers.iter().enumerate() {
                        let fields = drawer.get_style_fileds();
                        let page = StylePage::new(id, drawer.get_name(), fields);
                        style_pages.push(page);
                    }
                    if let Some(preferences_window) = self.get_window_mut::<PreferencesWindow>() {
                        preferences_window.registe_ui_styles(style_pages);
                    }
                }
                Message::SetPreferenceWindowSelectedId(selected_id) => {
                    if let Some(preferences_window) = self.get_window_mut::<PreferencesWindow>() {
                        preferences_window.set_selected_id(selected_id)
                    }
                }
                Message::UpdateUIStyle(style_page) => {
                    match self.get_window_mut::<PreferencesWindow>() {
                        Some(preferences_window) => {
                            preferences_window.update_style_page(&style_page);
                            let drawer = &mut self.drawers[style_page.id];
                            drawer.update_style(&style_page.fields);
                        }
                        None => {
                            kairos_dialog::error_message_window(
                                "PreferenceWindow Error",
                                "Get PreferenceWindow Failed",
                            );
                        }
                    }
                }
                Message::OpenConsoleTab => {
                    self.show_tab::<ConsoleWindow>(
                        &mut engine.assets_server,
                        ui,
                        self.layout.bottom,
                    );
                }
                Message::CloseConsoleTab => {
                    self.close_drawer::<ConsoleWindow>();
                }
                Message::OpenInspectorTab => {
                    self.show_tab::<InspectorWindow>(
                        &mut engine.assets_server,
                        ui,
                        self.layout.right,
                    );
                }
                Message::CloseInspectorTab => {
                    if let Some(inspector_window) = self.get_window_mut::<InspectorWindow>() {
                        inspector_window.stop_audio_preview();
                    }
                    self.close_drawer::<InspectorWindow>();
                }
                Message::OpenHierarchyTab => {
                    self.show_tab::<HierarchyWindow>(
                        &mut engine.assets_server,
                        ui,
                        self.layout.left,
                    );
                }
                Message::CloseHierarchyTab => {
                    self.close_drawer::<HierarchyWindow>();
                }
                Message::OpenProjectTab => {
                    self.show_tab::<ProjectWindow>(
                        &mut engine.assets_server,
                        ui,
                        self.layout.bottom,
                    );
                }
                Message::CloseProjectTab => {
                    self.close_drawer::<ProjectWindow>();
                }
                Message::SelectProjectNode(node) => {
                    if let Some(project_window) = self.get_window_mut::<ProjectWindow>() {
                        project_window.select_node(node);
                        let info = project_window.get_selected_node_info(&mut engine.assets_server);
                        if let Some(inspector) = self.get_window_mut::<InspectorWindow>() {
                            if let Some(dialog) =
                                inspector.set_selected(ui.ctx(), &mut engine.assets_server, info)
                            {
                                self.dialogs.push(dialog);
                            }
                        }
                    }
                }
                Message::NavigateToProjectDirectory(node_idx) => {
                    if let Some(project_window) = self.get_window_mut::<ProjectWindow>() {
                        project_window.navigate_to(node_idx);
                    }
                }
                Message::CreateProjectNode(parent_idx, name, kind) => {
                    if let Some(project_window) = self.get_window_mut::<ProjectWindow>() {
                        project_window.create_node(parent_idx, name, kind);
                    }
                }
                Message::RenameProjectNode(node_idx, new_name) => {
                    if let Some(project_window) = self.get_window_mut::<ProjectWindow>() {
                        project_window.rename_node(node_idx, new_name);
                    }
                }
                Message::UpdateProjectWindowRenamingBuffer(buffer) => {
                    if let Some(project_window) = self.get_window_mut::<ProjectWindow>() {
                        project_window.update_renaming_buffer(buffer);
                    }
                }
                Message::StartRenameProjectNode(node_idx) => {
                    if let Some(project_window) = self.get_window_mut::<ProjectWindow>() {
                        project_window.start_rename(node_idx);
                    }
                }
                Message::ExitRenameProjectNode => {
                    if let Some(project_window) = self.get_window_mut::<ProjectWindow>() {
                        project_window.exit_rename();
                    }
                }
                Message::OpenProjectNode(node_idx) => {
                    if let Some(project_window) = self.get_window_mut::<ProjectWindow>() {
                        project_window.open_node(node_idx);
                    }
                }
                Message::DeleteProjectNode(node_idx) => {
                    if let Some(project_window) = self.get_window_mut::<ProjectWindow>() {
                        project_window.delete_node(node_idx);
                    }
                }
                Message::UpdateInspectorToml(handle, table) => {
                    inspector::toml::TomlTableInspector::update_table(
                        handle,
                        table,
                        &mut engine.assets_server,
                    );
                }
                Message::ToggleAudioPreview => {
                    if let Some(inspector_window) = self.get_window_mut::<InspectorWindow>() {
                        inspector_window.toggle_audio_preview(engine);
                    }
                }
                Message::SeekAudioPreview(position) => {
                    if let Some(inspector_window) = self.get_window_mut::<InspectorWindow>() {
                        inspector_window.seek_audio_preview(engine, position);
                    }
                }
                Message::OpenSceneTab => {
                    self.show_tab::<SceneWindow>(&mut engine.assets_server, ui, self.layout.center);
                }
                Message::CloseSceneTab => {
                    self.close_drawer::<SceneWindow>();
                }
                Message::UpdateSceneWindowSize(width, height) => {
                    if let Some(scene_window) = self.get_window_mut::<SceneWindow>() {
                        scene_window.update_size(width, height);
                    }
                }
                Message::RegisteSceneWindowViewBind(recever) => {
                    if let Some(scene_window) = self.get_window_mut::<SceneWindow>() {
                        scene_window.register_view_bind(recever);
                    }
                }
                Message::SceneWindowTryReceTextureId => {
                    if let Some(scene_window) = self.get_window_mut::<SceneWindow>() {
                        scene_window.try_rece_texture_id();
                    }
                }
                Message::OpenGameTab => {
                    self.show_tab::<GameWindow>(&mut engine.assets_server, ui, self.layout.center);
                }
                Message::CloseGameTab => {
                    self.close_drawer::<GameWindow>();
                }
                Message::UpdateGameWindowSize(width, height) => {
                    if let Some(game_window) = self.get_window_mut::<GameWindow>() {
                        game_window.update_size(width, height);
                    }
                }
                Message::RegisteGameWindowViewBind(receiver) => {
                    if let Some(game_window) = self.get_window_mut::<GameWindow>() {
                        game_window.register_view_bind(receiver);
                    }
                }
                Message::GameWindowTryReceTextureId => {
                    if let Some(game_window) = self.get_window_mut::<GameWindow>() {
                        game_window.try_rece_texture_id();
                    }
                }
                Message::SceneCameraOrbit(dx, dy, dt) => {
                    if let Some(scene_window) = self.get_window_mut::<SceneWindow>() {
                        scene_window.on_camera_orbit(dx, dy, dt);
                    }
                }
                Message::CameraZoom(delta, dt) => {
                    if let Some(scene_window) = self.get_window_mut::<SceneWindow>() {
                        scene_window.on_camera_zoom(delta, dt);
                    }
                }
                Message::CameraFly(right, forward, dt) => {
                    if let Some(scene_window) = self.get_window_mut::<SceneWindow>() {
                        scene_window.on_camera_fly(right, forward, dt);
                    }
                }
            }

            // ---- per-frame: tick audio playback position ----
            if let Some(inspector_window) = self.get_window_mut::<InspectorWindow>() {
                inspector_window.tick_audio_preview();
            }
        }
    }

    pub fn render(&mut self, engine: &mut Engine, game: &mut KairosGame) -> Vec<GraphicsCommand> {
        let mut commands = Vec::new();
        self.drawers.iter().for_each(|drawer| {
            let cmd = drawer.render(engine, game, &mut self.messager);
            if let Some(cmd) = cmd {
                commands.push(cmd);
            }
        });

        commands
    }

    fn push_drawer<T>(&mut self, drawer: Box<dyn Drawer>) -> usize
    where
        T: 'static + Drawer,
    {
        let id = self.drawers.len();
        let type_id = TypeId::of::<T>();
        self.ids.insert(type_id, id);
        self.drawers.push(drawer);
        self.actives.push(true);
        id
    }

    fn show_window<T>(
        &mut self,
        ui: &egui::Ui,
        create: impl FnOnce() -> Result<T, Box<dyn std::error::Error>>,
    ) where
        T: Drawer,
    {
        let type_id = TypeId::of::<T>();
        match self.ids.get(&type_id) {
            Some(id) => {
                if !self.actives[*id] {
                    let surface = self.tab_tree.add_window(vec![*id]);
                    self.drawers[*id].show_window(self.tab_tree.get_window_state_mut(surface));
                    self.actives[*id] = true;
                } else {
                    if let Some(tab_location) = self.tab_tree.find_drawer(id) {
                        self.tab_tree.set_active_drawer(tab_location);
                    }
                }
            }
            None => {
                let drawer = create().unwrap_or_else(|error| {
                    Context::create_ui_failed(ui, type_name::<T>(), error);
                });
                let id = self.push_drawer::<T>(Box::new(drawer));
                let surface = self.tab_tree.add_window(vec![id]);
                self.drawers[id].show_window(self.tab_tree.get_window_state_mut(surface));
            }
        };
    }

    fn show_tab<T>(&mut self, assets_server: &mut AssetsServer, ui: &egui::Ui, zone: Zone)
    where
        T: Drawer,
    {
        let type_id = TypeId::of::<T>();
        match self.ids.get(&type_id) {
            Some(id) => {
                if !self.actives[*id] {
                    self.tab_tree[zone.surface][zone.node].append_drawer(*id);
                    self.actives[*id] = true;
                } else {
                    if let Some(tab_location) = self.tab_tree.find_drawer(id) {
                        self.tab_tree.set_active_drawer(tab_location);
                    }
                }
            }
            None => {
                let drawer = T::create(assets_server).unwrap_or_else(|error| {
                    Context::create_ui_failed(ui, type_name::<T>(), error);
                });
                let id = self.push_drawer::<T>(Box::new(drawer));
                self.tab_tree[zone.surface][zone.node].append_drawer(id);
            }
        }
    }

    fn close_drawer<T>(&mut self)
    where
        T: 'static + Drawer,
    {
        let type_id = TypeId::of::<T>();
        if let Some(id) = self.ids.get(&type_id) {
            self.actives[*id] = false;
        }
    }

    fn create_ui_failed(ui: &egui::Ui, ui_name: &str, error: Box<dyn std::error::Error>) -> ! {
        native_dialog::ui_create_error_window(ui_name, &error);
        ui.send_viewport_cmd(egui::ViewportCommand::Close);
        panic!("Create {} UI Failed: {}", ui_name, error)
    }

    fn _get_window<T>(&self) -> Option<&T>
    where
        T: Drawer,
    {
        let type_id = TypeId::of::<T>();
        match self.ids.get(&type_id) {
            Some(id) => {
                let drawer = self.drawers[*id].as_ref();
                (drawer as &dyn Any).downcast_ref::<T>()
            }
            None => None,
        }
    }

    fn get_window_mut<T>(&mut self) -> Option<&mut T>
    where
        T: Drawer,
    {
        let type_id = TypeId::of::<T>();
        match self.ids.get(&type_id) {
            Some(id) => {
                let drawer = self.drawers[*id].as_mut();
                (drawer as &mut dyn Any).downcast_mut::<T>()
            }
            None => None,
        }
    }

    fn set_up_font_to_family(
        fonts: &mut egui::FontDefinitions,
        family: &egui::FontFamily,
        font_datas: &Vec<FontDataConfig>,
    ) {
        for font in font_datas {
            let font_data = match std::fs::read(&font.path) {
                Ok(data) => data,
                Err(e) => {
                    log::warn!("Failed to load font from {:?}: {}", font.path, e);
                    return;
                }
            };
            fonts.font_data.insert(
                font.name.to_owned(),
                std::sync::Arc::new(egui::FontData::from_owned(font_data)),
            );
            match fonts.families.get_mut(family) {
                Some(family) => match font.priority {
                    super::ui::global_styles::FontPriority::First => {
                        family.insert(0, font.name.to_owned());
                    }
                    super::ui::global_styles::FontPriority::Push => {
                        family.push(font.name.to_owned());
                    }
                },
                None => {
                    log::warn!("Failed to get font family: {}", family)
                }
            }
        }
    }

    fn setup_font(ctx: &egui::Context, fonts_cfg: &FontsConfig) {
        let mut fonts = egui::FontDefinitions::default();
        Self::set_up_font_to_family(
            &mut fonts,
            &egui::FontFamily::Proportional,
            &fonts_cfg.proportional,
        );
        Self::set_up_font_to_family(
            &mut fonts,
            &egui::FontFamily::Monospace,
            &fonts_cfg.monospace,
        );

        ctx.set_fonts(fonts);
    }
}
