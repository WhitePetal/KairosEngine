use std::{
    any::{Any, TypeId, type_name},
    collections::VecDeque,
};

use crate::{
    asset_loader::assets::AssetsServer,
    graphics::graphics_graph::GraphicsCommand,
    kairos_editor::{
        Engine,
        ui::{
            game_window::GameWindow,
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
pub mod game_window;
pub mod hierarchy_window;
pub mod inspector_window;
pub mod layout;
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
    CreateSceneTabRt(egui::TextureId),
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

    /// SceneCamera orbit (dx, dy) in pixels
    SceneCameraOrbit(f32, f32),
    /// Camera zoom delta
    CameraZoom(f32),
    /// Camera fly movement (right, forward) each in [-1, 0, 1]
    CameraFly(f32, f32),
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
        tab: &mut Self::Tab,
        messager: &mut Messager,
        log: &mut Log,
        drawers: &Vec<Box<dyn Drawer>>,
    ) {
        let tab = &drawers[*tab];
        tab.ui(ui, messager, log);
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

    fn ui(&self, ui: &mut egui::Ui, messager: &mut Messager, log: &mut Log);

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
}

impl Context {
    pub fn new() -> Self {
        let tab_tree = DockState::new(vec![]);

        let mut messager = Messager::new();
        messager.send(Message::CreateToolbar);
        messager.send(Message::InitLayout);
        messager.send(Message::OpenSceneTab);
        messager.send(Message::OpenGameTab);
        messager.send(Message::OpenProjectTab);
        messager.send(Message::OpenInspectorTab);
        messager.send(Message::OpenHierarchyTab);

        Self {
            messager,
            ids: TypeIdMap::default(),
            drawers: Vec::new(),
            actives: Vec::new(),
            tab_tree,
            tab_viewer: KairosTabDrawer {},
            layout: EditorLayout::new(),
        }
    }

    pub fn darw(&mut self, ui: &mut egui::Ui, log: &mut Log) {
        // tool_bar
        let tool_bar_type_id = TypeId::of::<ToolBar>();
        if let Some(id) = self.ids.get(&tool_bar_type_id) {
            self.drawers[*id].ui(ui, &mut self.messager, log);
        }

        // 中央区域显示内容
        egui::CentralPanel::default().show_inside(ui, |ui| {
            DockArea::new("KairosEditor Main DockArea", &mut self.tab_tree).show_inside(
                ui,
                &mut self.messager,
                log,
                &self.drawers,
                &mut self.tab_viewer,
            );
        });
    }

    pub fn handle(&mut self, assets_server: &mut AssetsServer, ui: &egui::Ui, _log: &mut Log) {
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
                    self.show_tab::<ConsoleWindow>(assets_server, ui, self.layout.bottom);
                }
                Message::CloseConsoleTab => {
                    self.close_drawer::<ConsoleWindow>();
                }
                Message::OpenInspectorTab => {
                    self.show_tab::<InspectorWindow>(assets_server, ui, self.layout.right);
                }
                Message::CloseInspectorTab => {
                    self.close_drawer::<InspectorWindow>();
                }
                Message::OpenHierarchyTab => {
                    self.show_tab::<HierarchyWindow>(assets_server, ui, self.layout.left);
                }
                Message::CloseHierarchyTab => {
                    self.close_drawer::<HierarchyWindow>();
                }
                Message::OpenProjectTab => {
                    self.show_tab::<ProjectWindow>(assets_server, ui, self.layout.bottom);
                }
                Message::CloseProjectTab => {
                    self.close_drawer::<ProjectWindow>();
                }
                Message::OpenSceneTab => {
                    self.show_tab::<SceneWindow>(assets_server, ui, self.layout.center);
                }
                Message::CloseSceneTab => {
                    self.close_drawer::<SceneWindow>();
                }
                Message::CreateSceneTabRt(rt_id) => {
                    if let Some(scene_window) = self.get_window_mut::<SceneWindow>() {
                        scene_window.set_rt_id(rt_id);
                    }
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
                    self.show_tab::<GameWindow>(assets_server, ui, self.layout.center);
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
                Message::SceneCameraOrbit(dx, dy) => {
                    if let Some(scene_window) = self.get_window_mut::<SceneWindow>() {
                        scene_window.on_camera_orbit(dx, dy);
                    }
                }
                Message::CameraZoom(delta) => {
                    if let Some(scene_window) = self.get_window_mut::<SceneWindow>() {
                        scene_window.on_camera_zoom(delta);
                    }
                }
                Message::CameraFly(right, forward) => {
                    if let Some(scene_window) = self.get_window_mut::<SceneWindow>() {
                        scene_window.on_camera_fly(right, forward);
                    }
                }
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
        dialog::ui_create_error_window(ui_name, &error);
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
}
