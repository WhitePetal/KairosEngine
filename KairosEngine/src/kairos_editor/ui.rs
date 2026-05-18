use std::{any::{Any, TypeId, type_name}, collections::{HashMap, VecDeque}};

use eframe::egui::{self};

use crate::{kairos_dialog, kairos_editor::{ui::{about_window::AboutWindow, console_window::ConsoleWindow, docking_tab::{DockArea, dock_state::{DockState, tree::{NodeIndex}}, tab_drawer::{OnCloseResponse, TabDrawer}, window_state::WindowState}, preferences_window::PreferencesWindow, tool_bar::ToolBar, ui_style_fields::{StyleField, StylePage}}}};

pub mod paths;
pub mod dialog;
pub mod ui_style_fields;
pub mod tool_bar;
pub mod about_window;
pub mod preferences_window;
pub mod console_window;
pub mod docking_tab;

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
    OpenConsoleWindow,
}

#[derive(PartialEq, Eq, Hash)]
pub enum TabDrawers
{
    Default,
    Inspector,
    Hierarchy,
    Console,
    Project
}
impl TabDrawers {
    pub fn as_str(&self) -> &'static str {
        match self {
            TabDrawers::Default => "Default",
            TabDrawers::Inspector => "Inspector",
            TabDrawers::Hierarchy => "Hierarchy",
            TabDrawers::Console => "Console",
            TabDrawers::Project => "Project",
        }
    }
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

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab, ctx: &eframe::egui::Context, frame: &mut eframe::Frame, messager: &mut Messager, drawers: &Vec<Box<dyn Drawer>>) {
        let tab = &drawers[*tab];
        tab.update(Some(ui), ctx, frame, messager);
    }

    fn on_close(&mut self, tab: &mut Self::Tab, messager: &mut Messager, drawers: &Vec<Box<dyn Drawer>>) -> OnCloseResponse {
        let tab = &drawers[*tab];
        tab.close(messager);
        OnCloseResponse::Close
    }

    // fn on_add(&mut self, surface: SurfaceIndex, node: NodeIndex) {
    //     println!("add tab: {0}, {1}", surface.0, node.0);
    //     self.drawer_paths.push((surface, node));
    // }
}

pub trait Drawer: Any {
    fn show(&self, state: Option<&mut WindowState>);

    fn update(&self, ui: Option<&mut egui::Ui>, ctx: &eframe::egui::Context, frame: &mut eframe::Frame, messager: &mut Messager);

    fn close(&self, messager: &mut Messager);

    fn get_name(&self) -> &'static str;

    fn get_title(&self) -> egui::WidgetText;

    fn get_style_fileds(&self) -> Vec<StyleField>;

    fn update_style(&mut self, style_fields: &Vec<StyleField>);
}

pub struct Messager {
    messages: VecDeque<Message>
}

impl Messager {
    pub fn new() -> Self {
        Self{
            messages: VecDeque::new()
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
    on_offs: Vec<bool>,
    tab_tree: DockState<usize>,
    tab_viewer: KairosTabDrawer,
}

impl Context {
    pub fn new() -> Self {
        let tab_tree = DockState::new(vec![]);
        // let [r_root, _] = doc_tree.main_surface_mut().split_right(
        //     NodeIndex::root(), 
        //     0.7,
        //     vec![TabDrawers::Inspector]
        // );
        // let [r_root, _] = doc_tree.main_surface_mut().split_below(
        //     r_root, 
        //     0.7,
        //     vec![TabDrawers::Project, TabDrawers::Console] 
        // );
        // let [_, _] = doc_tree.main_surface_mut().split_left(
        //     r_root, 
        //     0.3,
        //     vec![TabDrawers::Hierarchy]
        // );
        // let mut open_tabs = HashSet::new();
        // for node in doc_tree[SurfaceIndex::main()].iter() {
        //     if let Some(tabs) = node.drawers() {
        //         for tab in tabs {
        //             open_tabs.insert(tab);
        //         }
        //     }
        // }

        let mut messager = Messager::new();
        messager.send(Message::CreateToolbar);
        messager.send(Message::OpenConsoleWindow);

        let drawers = Vec::new();

        Self {
            messager,
            ids: HashMap::new(),
            drawers,
            on_offs: Vec::new(),
            tab_tree,
            tab_viewer: KairosTabDrawer {  }
        }
    }

    pub fn darw(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        // tool_bar
        let tool_bar_type_id = TypeId::of::<ToolBar>();
        if let Some(id) = self.ids.get(&tool_bar_type_id) {
            self.drawers[*id].update(None, ctx, frame, &mut self.messager);
        }

        // 中央区域显示内容
        egui::CentralPanel::default()
            .show(ctx, |ui| {
                DockArea::new("KairosEditor Main DockArea", &mut self.tab_tree)
                    .show_inside(ui, ctx, frame, &mut self.messager, &self.drawers, &mut self.tab_viewer);
            }
        );
    }

    pub fn handle(&mut self, ctx: &eframe::egui::Context) {
        while let Some(msg) = self.messager.messages.pop_front() {
            match msg {
                Message::CreateToolbar => {
                    let drawer = ToolBar::new().unwrap_or_else(|error| {
                        Context::create_ui_failed(ctx, type_name::<ToolBar>(), error);
                    });
                    self.push_drawer::<ToolBar>(Box::new(drawer));
                },
                Message::QuitEngine => {
                    ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Close);
                },
                Message::OpenAboutWindow => {
                    let type_id = TypeId::of::<AboutWindow>();
                    match self.ids.get(&type_id) {
                        Some(id) => self.on_offs[*id] = true,
                        None => {
                            let drawer = AboutWindow::new().unwrap_or_else(|error| {
                                Context::create_ui_failed(ctx, type_name::<AboutWindow>(), error);
                            });
                            let id = self.push_drawer::<AboutWindow>(Box::new(drawer));
                            let surface = self.tab_tree.add_window(vec![id]);
                            self.drawers[id].show(self.tab_tree.get_window_state_mut(surface));
                        },
                    };
                },
                Message::CloseAboutWindow => {
                    let type_id = TypeId::of::<AboutWindow>();
                    if let Some(id) = self.ids.get(&type_id) {
                        self.on_offs[*id] = false;
                        // TODO: how destroy?
                    }
                },
                Message::OpenPreferenceWindow => {
                    let type_id = TypeId::of::<PreferencesWindow>();
                    match self.ids.get(&type_id) {
                        Some(id) => {
                            self.on_offs[*id] = true;
                        },
                        None => {
                            let drawer = PreferencesWindow::new().unwrap_or_else(|error| {
                                Context::create_ui_failed(ctx, type_name::<PreferencesWindow>(), error);
                            });
                            self.push_drawer::<PreferencesWindow>(Box::new(drawer));
                        }
                    };
                    self.messager.messages.push_back(Message::RefershPreferenceWindow);
                },
                Message::ClosePreferenceWindow => {
                    let type_id = TypeId::of::<PreferencesWindow>();
                    if let Some(id) = self.ids.get(&type_id) {
                        self.on_offs[*id] = false;
                    }
                },
                Message::RefershPreferenceWindow => {
                    let mut style_pages = Vec::new();
                    for (id, drawer) in self.drawers.iter().enumerate() {
                        let fields = drawer.get_style_fileds();
                        let page = StylePage::new(id, drawer.get_name(), fields);
                        style_pages.push(page);
                    }
                    match self.get_preference_window_mut() {
                        Some(preferences_window) => {
                            preferences_window.registe_ui_styles(style_pages);
                        },
                        None => {
                            kairos_dialog::error_message_window("PreferenceWindow Error", "Get PreferenceWindow Failed")
                        },
                    }
                },
                Message::SetPreferenceWindowSelectedId(selected_id) => {
                    match self.get_preference_window_mut() {
                        Some(preferences_window) => preferences_window.set_selected_id(selected_id),
                        None => {
                            kairos_dialog::error_message_window("PreferenceWindow Error", "Get PreferenceWindow Failed");
                        },
                    }
                },
                Message::UpdateUIStyle(style_page) => {
                    match self.get_preference_window_mut() {
                        Some(preferences_window) => preferences_window.update_style_page(&style_page),
                        None => {
                            kairos_dialog::error_message_window("PreferenceWindow Error", "Get PreferenceWindow Failed");
                        },
                    }

                    let drawer = &mut self.drawers[style_page.id];
                    drawer.update_style(&style_page.fields);
                },
                Message::OpenConsoleWindow => {
                    let type_id = TypeId::of::<ConsoleWindow>();
                    match self.ids.get(&type_id) {
                        Some(id) => {
                            println!("open console window (exist)");
                            if !self.on_offs[*id] {
                                self.tab_tree.main_surface_mut().split_below(
                                    NodeIndex::root(), 0.7, vec![*id]
                                );
                                self.on_offs[*id] = true;
                            }
                            else {
                                if let Some(tab_location) = self.tab_tree.find_drawer(id) {
                                    self.tab_tree.set_active_drawer(tab_location);
                                }
                            }
                        }
                        None => {
                            let drawer = ConsoleWindow::new().unwrap_or_else(|error| {
                                Context::create_ui_failed(ctx, type_name::<ConsoleWindow>(), error);
                            });
                            // self.tab_tree.add_window(vec![Box::new(drawer)]);
                            let id = self.push_drawer::<ConsoleWindow>(Box::new(drawer));
                            let [_, _] = self.tab_tree.main_surface_mut().split_below(
                                NodeIndex::root(), 
                                0.7,
                                vec![id] 
                            );
                        }
                    }
                },
            }
        }
    }

    fn push_drawer<T>(&mut self, drawer: Box<dyn Drawer>) -> usize
        where T: 'static + Drawer
    {
        let id = self.drawers.len();
        let type_id = TypeId::of::<T>();
        self.ids.insert(type_id, id);
        self.drawers.push(drawer);
        self.on_offs.push(true);
        id
    }

    fn create_ui_failed(ctx: &eframe::egui::Context, ui_name: &str, error: Box<dyn std::error::Error>) -> ! {
        dialog::ui_create_error_window(ui_name, &error);
        ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Close);
        panic!("Create {} UI Failed: {}", ui_name, error)
    }

    fn get_preference_window(&self) -> Option<&PreferencesWindow> {
        let type_id = TypeId::of::<PreferencesWindow>();
        match self.ids.get(&type_id) {
            Some(id) => {
                let drawer = self.drawers[*id].as_ref();
                (drawer as &dyn  Any).downcast_ref::<PreferencesWindow>()
            },
            None => {
                None
            }
        }
    }

    fn get_preference_window_mut(&mut self) -> Option<&mut PreferencesWindow> {
        let type_id = TypeId::of::<PreferencesWindow>();
        match self.ids.get(&type_id) {
            Some(id) => {
                let drawer = self.drawers[*id].as_mut();
                (drawer as &mut dyn  Any).downcast_mut::<PreferencesWindow>()
            },
            None => {
                None
            }
        }
    }
}