use std::{any::{TypeId, type_name}, collections::HashMap};

use eframe::egui::Visuals;

use crate::kairos_editor::{about_window::AboutWindow, main_content::MainContent, preferences_window::PreferencesWindow, tool_bar::ToolBar};

pub mod paths;
pub mod consts;
pub mod dialog;
pub mod ui_message;
pub mod main_content;
pub mod tool_bar;
pub mod about_window;
pub mod preferences_window;

pub enum UIMessage {
    CreateMainContent,
    CreateToolbar,
    QuitEngine,
    OpenAboutWindow,
    CloseAboutWindow,
    OpenPreferenceWindow,
    ClosePreferenceWindow,
}

pub trait UIDrawer {
    fn update(&self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame, messager: &mut UIMessager);
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
        self.drawers.iter().zip(self.on_offs.iter()).filter(|(_, on_off)| **on_off).for_each(|(drawer, _)| {
            drawer.update(ctx, frame, &mut self.messager);
        });
    }

    pub fn handle(&mut self, ctx: &eframe::egui::Context) {
        let mut _new_messages: Option<Vec<UIMessage>> = None;
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
                },
                UIMessage::ClosePreferenceWindow => {
                    let type_id = TypeId::of::<PreferencesWindow>();
                    if let Some(id) = self.ids.get(&type_id) {
                        self.on_offs[*id] = false;
                    }
                }
            }
        }
        messages.clear();
        self.messager.messages = messages;

        if let Some(messages) = _new_messages {
            self.messager.messages = messages;
        }
    }

    fn push_drawer<T>(&mut self, drawer: Box<dyn UIDrawer>) where T: 'static + UIDrawer
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