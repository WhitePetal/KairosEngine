use std::{
    any::{Any, TypeId, type_name},
    collections::{HashMap, VecDeque},
};

use crate::{
    graphics::{
        graphics_graph::{GraphicsCommand, GraphicsGraph},
        render_pipeline::RenderPipeline,
    },
    log::Log,
};
use egui::{self};
use tokio::sync::mpsc::Receiver;

use crate::{
    kairos_dialog,
    kairos_editor::ui::{
        about_window::AboutWindow,
        console_window::ConsoleWindow,
        docking_tab::{
            DockArea,
            dock_state::{
                DockState, SurfaceBottomPanelLocation, SurfaceCenterPanelLocation,
                SurfaceLeftPanelLocation, SurfaceRightPanelLocation, tree::NodeIndex,
            },
            surfaces::SurfaceIndex,
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
pub mod hierarchy_window;
pub mod inspector_window;
pub mod paths;
pub mod preferences_window;
pub mod project_window;
pub mod scene_window;
pub mod tool_bar;
pub mod ui_style_fields;

pub enum Message {
    CreateToolbar,
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
    RegesiterSceneWindowViewBind(Receiver<egui::TextureId>),
    SceneWindowTryReceive,
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
    fn show_window(&self, state: Option<&mut WindowState>);

    fn ui(&self, ui: &mut egui::Ui, messager: &mut Messager, log: &mut Log);

    fn render(
        &self,
        messager: &mut Messager,
        render_pipeline: &RenderPipeline,
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
    ids: HashMap<TypeId, usize>,
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
        messager.send(Message::OpenSceneTab);
        messager.send(Message::OpenProjectTab);
        messager.send(Message::OpenInspectorTab);
        messager.send(Message::OpenHierarchyTab);

        let drawers = Vec::new();

        Self {
            messager,
            ids: HashMap::new(),
            drawers,
            actives: Vec::new(),
            tab_tree,
            tab_viewer: KairosTabDrawer {},
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

    pub fn handle(&mut self, ui: &egui::Ui, _log: &mut Log) {
        while let Some(msg) = self.messager.messages.pop_front() {
            match msg {
                Message::CreateToolbar => {
                    let drawer = ToolBar::new().unwrap_or_else(|error| {
                        Context::create_ui_failed(ui, type_name::<ToolBar>(), error);
                    });
                    self.push_drawer::<ToolBar>(Box::new(drawer));
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
                    self.show_tab::<ConsoleWindow, _>(ui, ConsoleWindow::new, |state, id| {
                        let location = state.find_surface_bottom_panel_location(
                            SurfaceIndex::main(),
                            NodeIndex::root(),
                        );
                        match location {
                            SurfaceBottomPanelLocation::None => {
                                state.main_surface_mut().split_below(
                                    NodeIndex::root(),
                                    0.7,
                                    vec![id],
                                );
                            }
                            SurfaceBottomPanelLocation::Center(surface_index, node_index) => {
                                state[surface_index].split_below(node_index, 1.0, vec![id]);
                            }
                            SurfaceBottomPanelLocation::Bottom(surface_index, node_index) => {
                                state[surface_index][node_index].append_drawer(id);
                            }
                        }
                    });
                }
                Message::CloseConsoleTab => {
                    self.close_drawer::<ConsoleWindow>();
                }
                Message::OpenInspectorTab => {
                    self.show_tab::<InspectorWindow, _>(ui, InspectorWindow::new, |state, id| {
                        let location = state.find_surface_right_panel_location(
                            SurfaceIndex::main(),
                            NodeIndex::root(),
                        );
                        match location {
                            SurfaceRightPanelLocation::None => {
                                state.main_surface_mut().split_right(
                                    NodeIndex::root(),
                                    0.7,
                                    vec![id],
                                );
                            }
                            SurfaceRightPanelLocation::Center(surface_index, node_index) => {
                                state[surface_index].split_right(node_index, 0.7, vec![id]);
                            }
                            SurfaceRightPanelLocation::Right(surface_index, node_index) => {
                                state[surface_index][node_index].append_drawer(id);
                            }
                        }
                    });
                }
                Message::CloseInspectorTab => {
                    self.close_drawer::<InspectorWindow>();
                }
                Message::OpenHierarchyTab => {
                    self.show_tab::<HierarchyWindow, _>(ui, HierarchyWindow::new, |state, id| {
                        let location = state.find_surface_left_panel_location(
                            SurfaceIndex::main(),
                            NodeIndex::root(),
                        );
                        match location {
                            SurfaceLeftPanelLocation::None => {
                                state.main_surface_mut().split_left(
                                    NodeIndex::root(),
                                    0.3,
                                    vec![id],
                                );
                            }
                            SurfaceLeftPanelLocation::Center(surfcade_index, node_index) => {
                                state[surfcade_index].split_left(node_index, 0.3, vec![id]);
                            }
                            SurfaceLeftPanelLocation::Left(surface_index, node_index) => {
                                state[surface_index][node_index].append_drawer(id);
                            }
                        }
                    });
                }
                Message::CloseHierarchyTab => {
                    self.close_drawer::<HierarchyWindow>();
                }
                Message::OpenProjectTab => {
                    self.show_tab::<ProjectWindow, _>(ui, ProjectWindow::new, |state, id| {
                        let location = state.find_surface_bottom_panel_location(
                            SurfaceIndex::main(),
                            NodeIndex::root(),
                        );
                        match location {
                            SurfaceBottomPanelLocation::None => {
                                state.main_surface_mut().split_below(
                                    NodeIndex::root(),
                                    0.7,
                                    vec![id],
                                );
                            }
                            SurfaceBottomPanelLocation::Center(surface_index, node_index) => {
                                state[surface_index].split_below(node_index, 0.7, vec![id]);
                            }
                            SurfaceBottomPanelLocation::Bottom(surface_index, node_index) => {
                                state[surface_index][node_index].append_drawer(id);
                            }
                        }
                    });
                }
                Message::CloseProjectTab => {
                    self.close_drawer::<ProjectWindow>();
                }
                Message::OpenSceneTab => {
                    self.show_tab::<SceneWindow, _>(ui, SceneWindow::new, |state, id| {
                        let location = state.find_surface_center_panel_location(
                            SurfaceIndex::main(),
                            NodeIndex::root(),
                        );
                        match location {
                            SurfaceCenterPanelLocation::None => {
                                state.main_surface_mut().split_above(
                                    NodeIndex::root(),
                                    0.7,
                                    vec![id],
                                );
                            }
                            SurfaceCenterPanelLocation::Above(surface_index, node_index) => {
                                state[surface_index].split_above(node_index, 0.7, vec![id]);
                            }
                            SurfaceCenterPanelLocation::Center(surface_index, node_index) => {
                                state[surface_index][node_index].append_drawer(id);
                            }
                        }
                    });
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
                Message::RegesiterSceneWindowViewBind(receiver) => {
                    if let Some(scene_window) = self.get_window_mut::<SceneWindow>() {}
                }
                Message::SceneWindowTryReceive => todo!(),
            }
        }
    }

    pub fn render(&mut self, render_pipeline: &RenderPipeline) -> Vec<GraphicsCommand> {
        let mut commands = Vec::new();
        self.drawers.iter().for_each(|drawer| {
            let cmd = drawer.render(&mut self.messager, render_pipeline);
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

    fn show_tab<T, F>(
        &mut self,
        ui: &egui::Ui,
        create: impl FnOnce() -> Result<T, Box<dyn std::error::Error>>,
        split: F,
    ) where
        T: Drawer,
        F: FnOnce(&mut DockState<usize>, usize),
    {
        let type_id = TypeId::of::<T>();
        match self.ids.get(&type_id) {
            Some(id) => {
                if !self.actives[*id] {
                    split(&mut self.tab_tree, *id);
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
                split(&mut self.tab_tree, id);
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
