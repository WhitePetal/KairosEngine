mod editor_window_model;
mod editor_window_view;

mod tool_bar;

use std::{cell::{RefCell}, rc::Rc};

use editor_window_model::EditorWindowModel;
use editor_window_view::EditorWindowView;
use tool_bar::ToolBar;

use crate::kairos_editor::{UI, UIID, floating_window::{FloatingWindow, about_window::AboutWindow}, ui_loader::UILoader, ui_message::{ Message, MessageHandler, MessageID, Messager}};

struct FloatingWindowCollection {
    windows: RefCell<Vec<FloatingWindow>>
}

impl FloatingWindowCollection {
    pub fn new() -> Self {
        Self { windows: RefCell::new(Vec::new()) }    
    }

    pub fn push(&self, window: FloatingWindow) {
        let mut windows = self.windows.borrow_mut();
        windows.push(window);
    }

    pub fn for_each<F: FnMut(&FloatingWindow)>(&self, f: F) {
        let windows = self.windows.borrow();
        windows.iter().for_each(f);
    }
}

pub struct MainEditorWindow {
    model: RefCell<EditorWindowModel>,
    view: EditorWindowView,

    tool_bar: ToolBar,

    messager: Messager,
    ui_loader: RefCell<UILoader>,

    floating_windows: FloatingWindowCollection
}

impl MessageHandler for MainEditorWindow {
    fn handle(&self, msg: &Message) {
        match msg {
            Message::SetToolBarHeight(height) => self.model.borrow_mut().tool_bar_height = *height,
            Message::OpenAboutWindow => {
                println!("FK");
                let mut ui_loader = self.ui_loader.borrow_mut();
                let about_window= ui_loader.load_ui::<AboutWindow>(&UIID::AboutWindow);
                match UI::to_floating_window(about_window) {
                    Some(window) => self.floating_windows.push(window),
                    None => ()
                };
            },
        }
    }
}

impl MainEditorWindow {
    pub fn new(title: &str, _cc: &eframe::CreationContext, messager: Messager) -> Result<Rc<Self>, Box<dyn std::error::Error>> {
        let model = RefCell::new(EditorWindowModel::new(title)?);        
        let view = EditorWindowView::new();

        let floating_windows = FloatingWindowCollection::new();

        let ui_loader = RefCell::new(UILoader::new());

        let tool_bar = ToolBar::new(&_cc.egui_ctx, &messager)?;

        let controller = Self {
            model,
            view,

            tool_bar,

            messager,

            ui_loader,

            floating_windows
        };

        let controller = Rc::new(controller);
        let handler = Rc::clone(&controller);
        controller.messager.registe(MessageID::SetToolBarHeight, handler);
        let handler = Rc::clone(&controller);
        controller.messager.registe(MessageID::OpenAboutWindow, handler);

        Ok(controller)
    }

    pub fn update(&self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        self.floating_windows.for_each(|window| {
            match window {
                FloatingWindow::AboutWindow(ui) => {
                    match ui.as_ref() {
                        UI::AboutWindow(ui) => {
                            ui.update(ctx, frame, &self.messager);
                        },
                        _ => ()
                    }
                }
            };
        });

        self.tool_bar.update(ctx, frame, &self.messager);
        self.view.draw(ctx, frame, self.model.borrow());
    }
}

pub struct KairosEngine {
    main_editor_window: Rc<MainEditorWindow>,
}

impl KairosEngine {
    pub fn new(title: &str, _cc: &eframe::CreationContext, messager: Messager) -> Result<Self, Box<dyn std::error::Error>> {
        let main_editor_window = MainEditorWindow::new(title, _cc, messager)?;
        let main_editor_window = Rc::clone(&main_editor_window);
        Ok(Self{
            main_editor_window
        })
    }
}

impl eframe::App for KairosEngine {
    fn update(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        let main_editor_window = Rc::clone(&self.main_editor_window);
        main_editor_window.update(ctx, frame);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        
    }
}