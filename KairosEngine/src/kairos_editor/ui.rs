use std::{any::{Any, TypeId, type_name}, collections::{HashMap, HashSet}};

use eframe::egui::{self};

use crate::{kairos_dialog, kairos_editor::ui::{about_window::AboutWindow, docking_tab::dock_state::DockState, preferences_window::PreferencesWindow, tool_bar::ToolBar, ui_style_fields::{StyleField, StylePage}}};

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
pub enum TabDrawerName
{
    Default,
    Inspector,
    Hierarchy,
    Console,
    Project
}
impl TabDrawerName {
    pub fn as_str(&self) -> &'static str {
        match self {
            TabDrawerName::Default => "Default",
            TabDrawerName::Inspector => "Inspector",
            TabDrawerName::Hierarchy => "Hierarchy",
            TabDrawerName::Console => "Console",
            TabDrawerName::Project => "Project",
        }
    }
}

pub trait Drawer: Any {
    fn update(&self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame, messager: &mut Messager);

    fn get_name(&self) -> &'static str;

    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;

    fn get_style_fileds(&self) -> Vec<StyleField>;
    fn update_style(&mut self, style_fields: &Vec<StyleField>);
}

pub struct Messager {
    messages: Vec<Message>
}

impl Messager {
    pub fn new() -> Self {
        Self{
            messages: Vec::new()
        }
    }

    pub fn send(&mut self, msg: Message) {
        self.messages.push(msg);
    }
}

pub struct Context {
    pub messager: Messager,
    ids: HashMap<TypeId, usize>,
    on_offs: Vec<bool>,
    drawers: Vec<Box<dyn Drawer>>,
    doc_tab_viewer: DocTabViewer,
    doc_tree: DockState<Box<dyn Drawer>>,

}

impl Context {
    pub fn new() -> Self {
        let mut doc_tree = DockState::new(vec![TabDrawerName::Default]);
        let [r_root, _] = doc_tree.main_surface_mut().split_right(
            NodeIndex::root(), 
            0.7,
            vec![TabDrawerName::Inspector]
        );
        let [r_root, _] = doc_tree.main_surface_mut().split_below(
            r_root, 
            0.7,
            vec![TabDrawerName::Project, TabDrawerName::Console] 
        );
        let [_, _] = doc_tree.main_surface_mut().split_left(
            r_root, 
            0.3,
            vec![TabDrawerName::Hierarchy]
        );
        let mut open_tabs = HashSet::new();
        for node in doc_tree[SurfaceIndex::main()].iter() {
            if let Some(tabs) = node.tabs() {
                for tab in tabs {
                    open_tabs.insert(tab);
                }
            }
        }
        let doc_tab_viewer = DocTabViewer {

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
            // .frame(egui::Frame::NONE.fill(model.style.central_panel_color.into()))
            .show(ctx, |ui| {
                // ui.vertical_centered(|ui| {
                //     ui.label(RichText::new("Main Content Area").size(24.0).color(Color32::LIGHT_GRAY));
                //     ui.label(RichText::new("Custom titlebar demo").size(14.0).color(Color32::GRAY));
                // }

                DockArea::new(&mut self.doc_tree)
                    .show_inside(ui, &mut self.doc_tab_viewer);
            }
        );
    }

    pub fn handle(&mut self, ctx: &eframe::egui::Context) {
        let mut _new_messages: Vec<Message> = Vec::new();
        let mut messages = std::mem::take(&mut self.messager.messages);
        for msg in &messages {
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
                    _new_messages.push(Message::RefershPreferenceWindow);
                },
                Message::ClosePreferenceWindow => {
                    let type_id = TypeId::of::<PreferencesWindow>();
                    if let Some(id) = self.ids.get(&type_id) {
                        self.on_offs[*id] = false;
                    }
                },
                Message::RefershPreferenceWindow => {
                    let type_id = TypeId::of::<PreferencesWindow>();
                    match self.ids.get(&type_id) {
                        Some(id) => {
                            let drawers = &self.drawers;
                            match drawers[*id].as_any().downcast_ref::<PreferencesWindow>() {
                                Some(_) => {
                                    let mut style_pages = Vec::new();
                                    for (id, drawer) in self.drawers.iter().enumerate() {
                                        let fields = drawer.get_style_fileds();
                                        let page = StylePage::new(id, drawer.get_name(), fields);
                                        style_pages.push(page);
                                    }

                                    let drawers = &mut self.drawers;
                                    let drawer = drawers[*id].as_any_mut().downcast_mut::<PreferencesWindow>().unwrap();
                                    drawer.registe_ui_styles(style_pages);
                                },
                                None => {
                                    kairos_dialog::error_message_window("PreferenceWindow Error", "Refersh PreferenceWindow Failed");
                                },
                            }
                        }
                        None => {
                            kairos_dialog::error_message_window("PreferenceWindow Error", "Can't Find PreferenceWindow obj");
                        },
                    } 
                },
                Message::SetPreferenceWindowSelectedId(selected_id) => {
                    let type_id = TypeId::of::<PreferencesWindow>();
                    match self.ids.get(&type_id) {
                        Some(id) => {
                            if let Some(drawer) = self.drawers[*id].as_any_mut().downcast_mut::<PreferencesWindow>() {
                                drawer.set_selected_id(*selected_id);
                            } else {
                                kairos_dialog::error_message_window("PreferenceWindow Error", "Downcast PreferenceWindow Failed");
                            }
                        },
                        None => {
                            kairos_dialog::error_message_window("PreferenceWindow Error", "Can't Find PreferenceWindow obj");
                        },
                    }
                },
                Message::UpdateUIStyle(style_page) => {
                    let type_id = TypeId::of::<PreferencesWindow>();
                    match self.ids.get(&type_id) {
                        Some(id) => {
                            if let Some(drawer) = self.drawers[*id].as_any_mut().downcast_mut::<PreferencesWindow>() {
                                drawer.update_style_page(style_page);
                            } else {
                                kairos_dialog::error_message_window("PreferenceWindow Error", "Downcast PreferenceWindow Failed");
                            }
                        },
                        None => {
                            kairos_dialog::error_message_window("PreferenceWindow Error", "Can't Find PreferenceWindow obj");
                        },
                    }

                    let drawer = &mut self.drawers[style_page.id];
                    drawer.update_style(&style_page.fields);
                },
                Message::OpenConsoleWindow => {
                    todo!("OpenConsoleWindow")
                },
            }
        }
        messages.clear();
        self.messager.messages = messages;

        if _new_messages.len() > 0 {
            self.messager.messages = _new_messages;
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

struct DocTabViewer {

}

impl TabViewer for DocTabViewer {
    type Tab = TabDrawerName;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.as_str().into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab {
            _ => {
                ui.label(tab.as_str());
            }
        }
    }

    fn is_closeable(&self, _tab: &Self::Tab) -> bool {
        true
    }
}