mod editor_window_model;
mod editor_window_view;

mod tool_bar;

use std::{cell::RefCell, rc::Rc};

use editor_window_model::EditorWindowModel;
use editor_window_view::EditorWindowView;
use tool_bar::ToolBar;

use crate::kairos_editor::{UI, UIFactor, UIID, floating_window::{FloatingWindow, about_window::AboutWindow}, ui_loader::UILoader, ui_message::{self, Message, Messager}};

pub struct MainEditorWindow {
    model: Rc<RefCell<EditorWindowModel>>,
    view: EditorWindowView,

    tool_bar: ToolBar,

    messager: Messager,
    ui_loader: Rc<RefCell<UILoader>>,

    floating_windows: Rc<RefCell<Vec<FloatingWindow>>>
}

impl MainEditorWindow {
    pub fn new(title: &str, _cc: &eframe::CreationContext, mut messager: Messager) -> Result<Self, Box<dyn std::error::Error>> {
        let model = Rc::new(RefCell::new(EditorWindowModel::new(title)?));        
        let view = EditorWindowView::new();

        let floating_windows = Rc::new(RefCell::new(vec![]));

        let ui_loader = Rc::new(RefCell::new(UILoader::new()));

        let tool_bar = ToolBar::new(&_cc.egui_ctx, &mut messager)?;

        let mut controller = Self {
            model: Rc::clone(&model),
            view,

            tool_bar,

            messager,

            ui_loader,

            floating_windows: floating_windows
        };
        
        let model = Rc::clone(&model);
        controller.messager.registe(ui_message::tool_bar::SetToolBarHeightMessage::get_id(), Box::new(
            move |msg| {
                if let Some(msg)  = msg.downcast_ref::<ui_message::tool_bar::SetToolBarHeightMessage>() {
                    let model = Rc::clone(&model);
                    let mut model = model.borrow_mut();
                    model.tool_bar_height = msg.height();
                }
            }
        ));
        let ui_loader = Rc::clone(&controller.ui_loader);
        let floating_windows = Rc::clone(&controller.floating_windows);
        controller.messager.registe(ui_message::tool_bar::ShowAboutWindow::get_id(), Box::new(
            move |msg| {
                if let Some(_msg) = msg.downcast_ref::<ui_message::tool_bar::ShowAboutWindow>() {
                    let mut ui_loader = ui_loader.borrow_mut();
                    let about_window= ui_loader.load_ui::<AboutWindow>(&UIID::AboutWindow);
                    match about_window.as_ref() {
                        UI::AboutWindow(_) => {
                            let mut floating_windows = floating_windows.borrow_mut();
                            match UI::to_floating_window(about_window) {
                                Some(window) => floating_windows.push(window),
                                None => ()
                            };
                        }
                        _ => ()
                    }
                }
            }
        ));

        Ok(controller)
    }
}

impl eframe::App for MainEditorWindow {
    fn update(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        // floating_windows 的处理需要在单独一个作用域内
        // 因为其使用了 floating_windows 的 不可变引用
        // 而在之后，其他 ui 可能会发起某个floating_window的创建事件
        // 此时会创建 floating_windows 的 可变引用 来向其添加 window 元素
        // 那么就会出现 不可变引用 + 可变引用 同时存在的情况，程序 painc
        // 改成 try_borrow 也可以，但 单独作用域 更简洁
        {
            let floating_windows = Rc::clone(&self.floating_windows);
            let floating_windows = floating_windows.borrow();
            floating_windows.iter().for_each(|window| {
                match window {
                    FloatingWindow::AboutWindow(ui) => {
                        match ui.as_ref() {
                            UI::AboutWindow(ui) => {
                                ui.update(ctx, frame, &mut self.messager);
                            },
                            _ => ()
                        }
                    }
                };
            });
        }

        self.tool_bar.update(ctx, frame, &mut self.messager);
        self.view.draw(ctx, frame, &self.model);
    }
}