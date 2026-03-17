use crate::kairos_editor::{about_window::AboutWindow, main_content::{MainContent, MainContentModel}, tool_bar::{ToolBar, ToolBarModel}, ui_message::Message};

pub mod paths;
pub mod dialog;
pub mod ui_message;
pub mod ui_loader;
pub mod main_content;
pub mod tool_bar;
pub mod about_window;
pub mod floating_window;

struct OtherWindow {

}

pub trait UIFactor {
    fn new() -> UI;
    fn id() -> UIID;
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum UIID {
    AboutWindow,
    OtherWindow
}

pub enum UI {
    AboutWindow(AboutWindow),
    OtherWindow(OtherWindow),
}

pub enum FloatingWindow {
    AboutWindow(AboutWindow)
}

pub struct UIModel {
    main_content: Option<MainContentModel>,
    tool_bar: Option<ToolBarModel>,
}

impl UIModel {
    pub fn new() -> Self {
        Self {
            main_content: None,
            tool_bar: None,
        }
    }
}

pub trait UIDrawer {
    fn update(&self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame, messager: &mut UIMessager, model: &UIModel);
}

pub struct UIMessager {
    messages: Vec<Message>
}

impl UIMessager {
    pub fn new() -> Self {
        Self{
            messages: Vec::new()
        }
    }

    pub fn send(&mut self, msg: Message) {
        self.messages.push(msg);
    }

    pub fn append(&mut self, msgs: &mut Vec<Message>) {
        self.messages.append(msgs);
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
        let mut new_messages: Option<Vec<Message>> = None;
        for msg in self.messager.messages.drain(..) {
            match msg {
                Message::CreateMainContent(title) => {
                    let model = MainContentModel::new(&title).unwrap_or_else(|error| {
                        dialog::ui_model_load_error_window("MainContent", &error);
                        ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Close);
                        panic!("Load MainContent UI Model Failed: {}", error)
                    });
                    self.model.main_content = Some(model);
                    let drawer = Box::new(MainContent::new());
                    self.ui_drawers.push(drawer);
                },
                Message::CreateToolbar => {
                    let model = ToolBarModel::new().unwrap_or_else(|error| {
                        dialog::ui_model_load_error_window("ToolBar", &error);
                        ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Close);
                        panic!("Load ToolBar UI Model Failed: {}", error)
                    });
                    let new_msg = Message::SetToolBarHeight(model.style.height);
                    match &mut new_messages {
                        Some(messages) => messages.push(new_msg),
                        None => new_messages = Some(vec![new_msg]),
                    }
                    self.model.tool_bar = Some(model);
                    let drawer = Box::new(ToolBar::new());
                    self.ui_drawers.push(drawer);
                },
                Message::SetToolBarHeight(height) => {
                    if let Some(main_content_model) = &mut self.model.main_content {
                        main_content_model.tool_bar_height = height;
                    }
                },
                Message::OpenAboutWindow => {
                    let drawer = Box::new(AboutWindow::new());
                    self.ui_drawers.push(drawer);
                },
            }
        }

        if let Some(messages) = new_messages {
            self.messager.messages = messages;
        }
    }
}

pub struct KairosEngine {
    ui_context: UIContext,
}

impl KairosEngine {
    pub fn new(title: &str, _cc: &eframe::CreationContext) -> Result<Self, Box<dyn std::error::Error>> {

        let mut ui_context = UIContext::new();
        ui_context.messager.send(Message::CreateMainContent(title.to_string()));
        ui_context.messager.send(Message::CreateToolbar);

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