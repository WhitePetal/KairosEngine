use std::{any::{Any, TypeId, type_name}, collections::HashMap};

use eframe::egui::{Visuals};

use crate::{kairos_dialog, kairos_editor::{about_window::AboutWindow, main_content::MainContent, preferences_window::PreferencesWindow, tool_bar::ToolBar, ui_style_fields::{StylePage, StyleField}}};

pub mod paths;
pub mod consts;
pub mod dialog;
pub mod ui_style_fields;
pub mod main_content;
pub mod tool_bar;
pub mod about_window;
pub mod preferences_window;
pub mod console_window;

pub enum UIMessage {
    CreateMainContent,
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

pub trait UIDrawer: Any {
    fn update(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame, messager: &mut UIMessager);

    fn get_name(&self) -> &'static str;

    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;

    fn get_style_fileds(&self) -> Vec<StyleField>;
    fn update_style(&mut self, style_fields: &Vec<StyleField>);
}

pub struct UIMessager {
    messages: Vec<UIMessage>
}

impl UIMessager {
    pub fn new() -> Self {
        Self{
            messages: Vec::new()
        }
    }

    pub fn send(&mut self, msg: UIMessage) {
        self.messages.push(msg);
    }
}

pub struct UIContext {
    messager: UIMessager,
    ids: HashMap<TypeId, usize>,
    on_offs: Vec<bool>,
    drawers: Vec<Box<dyn UIDrawer>>,
}

impl UIContext {
    pub fn new() -> Self {
        Self { 
            messager: UIMessager::new(),
            ids: HashMap::new(),
            on_offs: Vec::new(),
            drawers: Vec::new()
        }   
    }

    pub fn darw(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        self.drawers.iter_mut().zip(self.on_offs.iter()).filter(|(_, on_off)| **on_off).for_each(|(drawer, _)| {
            drawer.update(ctx, frame, &mut self.messager);
        });
    }

    pub fn handle(&mut self, ctx: &eframe::egui::Context) {
        let mut _new_messages: Vec<UIMessage> = Vec::new();
        let mut messages = std::mem::take(&mut self.messager.messages);
        for msg in &messages {
            match msg {
                UIMessage::CreateMainContent => {
                   let drawer = MainContent::new().unwrap_or_else(|error| {
                        UIContext::create_ui_failed(ctx, type_name::<MainContent>(), error);
                    });
                    UIContext::push_drawer::<MainContent>(self, Box::new(drawer));
                },
                UIMessage::CreateToolbar => {
                    let drawer = ToolBar::new().unwrap_or_else(|error| {
                        UIContext::create_ui_failed(ctx, type_name::<ToolBar>(), error);
                    });
                    self.push_drawer::<ToolBar>(Box::new(drawer));
                },
                UIMessage::QuitEngine => {
                    ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Close);
                },
                UIMessage::OpenAboutWindow => {
                    let type_id = TypeId::of::<AboutWindow>();
                    match self.ids.get(&type_id) {
                        Some(id) => self.on_offs[*id] = true,
                        None => {
                            let drawer = AboutWindow::new().unwrap_or_else(|error| {
                                UIContext::create_ui_failed(ctx, type_name::<AboutWindow>(), error);
                            });
                            let id = self.push_drawer::<AboutWindow>(Box::new(drawer));
                        },
                    };
                },
                UIMessage::CloseAboutWindow => {
                    let type_id = TypeId::of::<AboutWindow>();
                    if let Some(id) = self.ids.get(&type_id) {
                        self.on_offs[*id] = false;
                    }
                },
                UIMessage::OpenPreferenceWindow => {
                    let type_id = TypeId::of::<PreferencesWindow>();
                    match self.ids.get(&type_id) {
                        Some(id) => {
                            self.on_offs[*id] = true;
                        },
                        None => {
                            let drawer = PreferencesWindow::new().unwrap_or_else(|error| {
                                UIContext::create_ui_failed(ctx, type_name::<PreferencesWindow>(), error);
                            });
                            self.push_drawer::<PreferencesWindow>(Box::new(drawer));
                        }
                    };
                    _new_messages.push(UIMessage::RefershPreferenceWindow);
                },
                UIMessage::ClosePreferenceWindow => {
                    let type_id = TypeId::of::<PreferencesWindow>();
                    if let Some(id) = self.ids.get(&type_id) {
                        self.on_offs[*id] = false;
                    }
                },
                UIMessage::RefershPreferenceWindow => {
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
                UIMessage::SetPreferenceWindowSelectedId(selected_id) => {
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
                UIMessage::UpdateUIStyle(style_page) => {
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
                UIMessage::OpenConsoleWindow => {
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

    fn push_drawer<T>(&mut self, drawer: Box<dyn UIDrawer>)
        where T: 'static + UIDrawer
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

pub struct KairosEngine {
    ui_context: UIContext,
}

impl KairosEngine {
    pub fn new(_cc: &eframe::CreationContext) -> Result<Self, Box<dyn std::error::Error>> {
        let mut ui_context = UIContext::new();
        ui_context.messager.send(UIMessage::CreateToolbar);
        ui_context.messager.send(UIMessage::CreateMainContent);

        Ok(Self{
            ui_context
        })
    }
}

impl eframe::App for KairosEngine {
    fn update(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        let mut visuals = Visuals::dark();
        visuals.button_frame = true;
        ctx.set_visuals(visuals);

        self.ui_context.handle(ctx);

        self.ui_context.darw(ctx, frame);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        
    }
}

// struct Context {
//     param: String,
//     id: u32,
// }

// trait FromContect {
//     fn from_context(context: &Context) -> Self;
// }

// struct Params(String);

// struct Id(u32);

// impl FromContect for Params {
//     fn from_context(context: &Context) -> Self {
//         Self(context.param.clone())
//     }
// }

// trait Handler<T> {
//     fn call(self, context: Context);
// }

// impl<F, T> Handler<T> for F
//     where 
//         F: Fn(T),
//         T: FromContect
// {
//     fn call(self, context: Context) {
//         self(T::from_context(&context))
//     }
// }

// fn trigger<T, H>(context: Context, handler: H)
//     where H: Handler<T>
// {
//     handler.call(context);
// }

// fn print_param(param: Params)
// {
//     println!("Param is {}", param.0);
// }

// fn print_all(Params(param): Params, Id(id) : Id) {
//     println!("param is {param}, id is {id}");
// }

// fn test() {
//     let context = Context {
//         param: "WTF".to_string(),
//         id: 32
//     };

//     trigger(context, print_param);
// }