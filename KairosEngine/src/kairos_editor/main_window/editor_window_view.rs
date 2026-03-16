use std::{cell::RefCell, rc::Rc};

use eframe::egui::{self, Color32, RichText};
use super::editor_window_model::EditorWindowModel;

pub struct EditorWindowView
{

}

impl EditorWindowView {
    pub fn new() -> Self
    {
        Self {  }
    }
}

impl EditorWindowView {
    pub fn draw(&self, ctx: &egui::Context, frame: &mut eframe::Frame, model: &Rc<RefCell<EditorWindowModel>>)
    {
        let model = model.borrow();
        // 设置整体背景色
        ctx.style_mut(|style| {
            style.visuals.window_fill = model.style.background_color.into();
            style.visuals.panel_fill = model.style.background_color.into();
        });

        // // 中央区域显示内容
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(model.style.central_panel_color.into()))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(model.tool_bar_height);
                    // ui.add_space(100.0);
                    ui.label(RichText::new("Main Content Area").size(24.0).color(Color32::LIGHT_GRAY));
                    ui.label(RichText::new("Custom titlebar demo").size(14.0).color(Color32::GRAY));
                });
            });
    }
}