mod tool_bar_model;
mod tool_bar_view;

use eframe::egui;

use tool_bar_model::ToolBarModel;
use tool_bar_view::ToolBarView;

use crate::kairos_editor::ui_message::{self, Messager};

pub struct ToolBar{
    model: ToolBarModel,
    view: ToolBarView,
}

impl ToolBar {
    pub fn new(ctx: &egui::Context, messager: &mut Messager) -> Result<Self, Box<dyn std::error::Error>> {
        let model = ToolBarModel::new(ctx)?;
        let view = ToolBarView::new();

        Ok(Self{
            model: model,
            view: view,
        })
    }

    pub fn update(&self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame, messager: &mut Messager) {
        messager.send(&ui_message::tool_bar::SetToolBarHeightMessage::new(self.model.style.height));
        self.view.draw(ctx, frame, &self.model);
    }

    pub fn get_height(&self) -> f32 {
        self.model.style.height
    }
}