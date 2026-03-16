mod editor_window_model;
mod editor_window_view;

mod tool_bar;

use std::{any::Any, cell::RefCell, rc::Rc};

use editor_window_model::EditorWindowModel;
use editor_window_view::EditorWindowView;
use tool_bar::ToolBar;

use crate::kairos_editor::ui_message::{self, Message, Messager, tool_bar::SetToolBarHeightMessage};

pub struct MainEditorWindow {
    model: Rc<RefCell<EditorWindowModel>>,
    view: EditorWindowView,

    messager: Messager,

    tool_bar: ToolBar
}

impl MainEditorWindow {
    pub fn new(title: &str, _cc: &eframe::CreationContext, mut messager: Messager) -> Result<Self, Box<dyn std::error::Error>> {
        let model = Rc::new(RefCell::new(EditorWindowModel::new(title)?));        
        let view = EditorWindowView::new();
        
        let cmodel = Rc::clone(&model);
        messager.registe(ui_message::tool_bar::SetToolBarHeightMessage::get_id(), Box::new(
            move |msg: &dyn Any| {
                if let Some(set_tool_bar_msg)  = msg.downcast_ref::<ui_message::tool_bar::SetToolBarHeightMessage>() {
                    let mut model = cmodel.borrow_mut();
                    model.tool_bar_height = set_tool_bar_msg.height();
                }
            }
        ));

        let tool_bar = ToolBar::new(&_cc.egui_ctx, &mut messager)?;

        let controller = Self {
            model,
            view,

            messager,

            tool_bar,
        };

        Ok(controller)
    }
}

impl eframe::App for MainEditorWindow {
    fn update(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        self.tool_bar.update(ctx, frame, &mut self.messager);

        self.view.draw(ctx, frame, &self.model);
    }
}