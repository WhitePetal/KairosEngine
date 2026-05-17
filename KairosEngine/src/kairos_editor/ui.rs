use std::{any::{Any, TypeId, type_name}, collections::{HashMap, HashSet, VecDeque}};

use eframe::egui::{self, Id};

use crate::{kairos_dialog, kairos_editor::ui::{about_window::AboutWindow, console_window::ConsoleWindow, docking_tab::{DockArea, dock_state::{DockState, tree::NodeIndex}, styles::Style, surfaces::SurfaceIndex, tab_drawer::{OnCloseResponse, TabDrawer}}, preferences_window::PreferencesWindow, tool_bar::ToolBar, ui_style_fields::{StyleField, StylePage}}};

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

pub trait Drawer: Any {
    fn update(&self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame, messager: &mut Messager);

    fn get_name(&self) -> &'static str;

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
    on_offs: Vec<bool>,
    drawers: Vec<Box<dyn Drawer>>,
    doc_tab_viewer: DocTabDrawer,
    doc_tree: DockState<TabDrawers>,
}

impl Context {
    pub fn new() -> Self {
        let mut doc_tree = DockState::new(vec![TabDrawers::Default]);
        let [r_root, _] = doc_tree.main_surface_mut().split_right(
            NodeIndex::root(), 
            0.7,
            vec![TabDrawers::Inspector]
        );
        let [r_root, _] = doc_tree.main_surface_mut().split_below(
            r_root, 
            0.7,
            vec![TabDrawers::Project, TabDrawers::Console] 
        );
        let [_, _] = doc_tree.main_surface_mut().split_left(
            r_root, 
            0.3,
            vec![TabDrawers::Hierarchy]
        );
        let mut open_tabs = HashSet::new();
        for node in doc_tree[SurfaceIndex::main()].iter() {
            if let Some(tabs) = node.drawers() {
                for tab in tabs {
                    open_tabs.insert(tab);
                }
            }
        }

        let doc_tab_viewer = DocTabDrawer {
            // open_tabs
        };

        let mut messager = Messager::new();
        messager.send(Message::CreateToolbar);

        Self { 
            messager,
            ids: HashMap::new(),
            on_offs: Vec::new(),
            drawers: Vec::new(),
            doc_tree,
            doc_tab_viewer,
        }   
    }

    pub fn darw(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        self.drawers.iter().zip(self.on_offs.iter()).filter(|(_, on_off)| **on_off).for_each(|(drawer, _)| {
            drawer.update(ctx, frame, &mut self.messager);
        });

        // 中央区域显示内容
        egui::CentralPanel::default()
            .show(ctx, |ui| {
                DockArea::new("KairosEditor Main DockArea", &mut self.doc_tree)
                    .show_inside(ui, &mut self.messager, &mut self.doc_tab_viewer);
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
                            self.push_drawer::<AboutWindow>(Box::new(drawer));
                        },
                    };
                },
                Message::CloseAboutWindow => {
                    let type_id = TypeId::of::<AboutWindow>();
                    if let Some(id) = self.ids.get(&type_id) {
                        self.on_offs[*id] = false;
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
                        Some(id) => self.on_offs[*id] = true,
                        None => {
                            let drawer = ConsoleWindow::new().unwrap_or_else(|error| {
                                Context::create_ui_failed(ctx, type_name::<ConsoleWindow>(), error);
                            });
                            self.push_drawer::<ConsoleWindow>(Box::new(drawer));
                        }
                    }
                    // self.doc_tree.push_to_focused_leaf(TabDrawers::Console);
                },
            }
        }
    }

    fn push_drawer<T>(&mut self, drawer: Box<dyn Drawer>)
        where T: 'static + Drawer
    {
        let id = self.drawers.len();
        let type_id = TypeId::of::<T>();
        self.ids.insert(type_id, id);
        self.on_offs.push(true);
        self.drawers.push(drawer);
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

// impl Drawer for Context {
//     fn update(&self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame, messager: &mut Messager) {
//         todo!()
//     }

//     fn get_name(&self) -> &'static str {
//         todo!()
//     }

//     fn as_any(&self) -> &dyn Any {
//         todo!()
//     }

//     fn as_any_mut(&mut self) -> &mut dyn Any {
//         todo!()
//     }

//     fn get_style_fileds(&self) -> Vec<StyleField> {
//         todo!()
//     }

//     fn update_style(&mut self, style_fields: &Vec<StyleField>) {
//         todo!()
//     }
// }

struct DocTabDrawer {
    // open_tabs: HashSet<&'a TabDrawers>
}

impl TabDrawer for DocTabDrawer {
    type Tab = TabDrawers;

    fn title(&self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.as_str().into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab, messager: &mut Messager) {
        match tab {
            _ => {
                ui.label(tab.as_str());
            }
        }
    }

    fn on_close(&mut self, _tab: &mut Self::Tab) -> OnCloseResponse {
        OnCloseResponse::Close
    }
}