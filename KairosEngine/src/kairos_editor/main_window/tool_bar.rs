mod tool_bar_model;
mod tool_bar_view;

use eframe::egui;

use tool_bar_model::ToolBarModel;
use tool_bar_view::ToolBarView;

use crate::kairos_editor::ui_message::{self, MessageID, Messager};

pub struct ToolBar{
    model: ToolBarModel,
    view: ToolBarView,
}

impl ToolBar {
    pub fn new(ctx: &egui::Context, messager: &Messager) -> Result<Self, Box<dyn std::error::Error>> {
        let model = ToolBarModel::new(ctx)?;
        let view = ToolBarView::new();

        messager.send(&MessageID::SetToolBarHeight, &ui_message::Message::SetToolBarHeight(model.style.height));

        Ok(Self{
            model: model,
            view: view,
        })
    }

    pub fn update(&self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame, messager: &Messager) {
        self.view.draw(ctx, frame, &self.model, messager);
    }
}