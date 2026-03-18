use crate::kairos_editor::{about_window::{AboutWindow, AboutWindowModel}, main_content::{MainContent, MainContentModel}, preferences_window::{PreferencesModel, PreferencesWindow}, tool_bar::{ToolBar, ToolBarModel}};

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
    OpenAboutWindow,
    CloseAboutWindow,
    OpenPreferenceWindow,
    ClosePreferenceWindow,
    QuitEngine,
}

pub struct UIModel {
    main_content: Option<MainContentModel>,
    tool_bar: Option<ToolBarModel>,
    about_window: Option<AboutWindowModel>,
    preferences_window: Option<PreferencesModel>,
}

impl UIModel {
    pub fn new() -> Self {
        Self {
            main_content: None,
            tool_bar: None,
            about_window: None,
            preferences_window: None,
        }
    }
}

pub trait UIDrawer {
    fn update(&self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame, messager: &mut UIMessager, model: &UIModel);
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
    model : UIModel,
    messager: UIMessager,
    ui_drawers: Vec<Box<dyn UIDrawer>>,
}

impl UIContext {
    pub fn new() -> Self {
        Self { 
            model: UIModel::new(),
            messager: UIMessager::new(),
            ui_drawers: Vec::new()
        }   
    }

    pub fn darw(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        for ui in self.ui_drawers.iter() {
            ui.update(ctx, frame, &mut self.messager, &self.model);
        }
    }

    pub fn handle(&mut self, ctx: &eframe::egui::Context) {
        let mut _messages: Option<Vec<UIMessage>> = None;
        for msg in self.messager.messages.drain(..) {
            match msg {
                UIMessage::CreateMainContent => {
                    let model = MainContentModel::new().unwrap_or_else(|error| {
                        dialog::ui_model_load_error_window("MainContent", &error);
                        ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Close);
                        panic!("Load MainContent UI Model Failed: {}", error)
                    });
                    self.model.main_content = Some(model);
                    let drawer = Box::new(MainContent::new());
                    self.ui_drawers.push(drawer);
                },
                UIMessage::CreateToolbar => {
                    let model = ToolBarModel::new().unwrap_or_else(|error| {
                        dialog::ui_model_load_error_window("ToolBar", &error);
                        ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Close);
                        panic!("Load ToolBar UI Model Failed: {}", error)
                    });
                    self.model.tool_bar = Some(model);
                    let drawer = Box::new(ToolBar::new());
                    self.ui_drawers.push(drawer);
                },
                UIMessage::OpenAboutWindow => {
                    match &mut self.model.about_window {
                        Some(model) => model.open = true,
                        None => {
                            let mut model = AboutWindowModel::new().unwrap_or_else(|error| {
                                dialog::ui_model_load_error_window("AboutWindow", &error);
                                ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Close);
                                panic!("Load AboutWindow UI Model Failed: {}", error)
                            });
                            model.open = true;
                            self.model.about_window = Some(model);
                            let drawer = Box::new(AboutWindow::new());
                            self.ui_drawers.push(drawer);
                        },
                    }; 
                },
                UIMessage::QuitEngine => {
                    ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Close);
                },
                UIMessage::CloseAboutWindow => {
                    if let Some(model) = &mut self.model.about_window {
                        model.open = false;
                    }
                },
                UIMessage::OpenPreferenceWindow => {
                    match &mut self.model.preferences_window {
                        Some(model) => model.open = true,
                        None => {
                            let mut model = PreferencesModel::new().unwrap_or_else(|error| {
                                dialog::ui_model_load_error_window("PreferencesWindow", &error);
                                panic!("Load PreferencesWindow UI Model Failed: {}", error)
                            });
                            model.open = true;
                            self.model.preferences_window = Some(model);
                            let drawer = Box::new(PreferencesWindow::new());
                            self.ui_drawers.push(drawer);
                        }
                    }
                },
                UIMessage::ClosePreferenceWindow => {
                    if let Some(model) = &mut self.model.preferences_window {
                        model.open = false;
                    }
                }
            }
        }

        if let Some(messages) = _messages {
            self.messager.messages = messages;
        }
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

        self.ui_context.handle(ctx);

        self.ui_context.darw(ctx, frame);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        
    }
}