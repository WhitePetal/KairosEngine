use eframe::egui::{self, Pos2, Rect, TopBottomPanel, Vec2, containers::menu};

use crate::kairos_editor::{paths, ui_message::{Messager, tool_bar::ShowAboutWindow}};

use super::tool_bar_model::ToolBarModel;



pub struct ToolBarView {

}

impl ToolBarView {
    pub fn new() -> Self {
        Self {  }
    }

    pub fn draw(&self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame, model: &ToolBarModel, messager: &mut Messager) {
        // 工具栏区域
        let toolbar_rect = Rect::from_min_size(
            Pos2::new(0.0, 0.0), 
            Vec2::new(ctx.content_rect().width(), model.style.height));

        TopBottomPanel::top("toolbar").show(ctx, |ui|{
            menu::MenuBar::new().ui(ui, |ui| {
                // 标题栏背景
                ui.painter().rect_filled(
                    toolbar_rect, 
                    model.style.corner_radius, 
                    model.style.fill_color
                );
                
                // Icon
                let icon = egui::Image::new(paths::PATH_ENGINE_ICON_URI);
                ui.menu_button(icon, |ui| {
                    if ui.button("About Kairos").clicked() {
                        messager.send(&ShowAboutWindow::new());
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {

                    }
                });
                // File
                ui.menu_button("File", |ui| {
                    if ui.button("New Scene").clicked() {
                        todo!()
                    }
                });
                
                // Editor
                ui.menu_button("Edit", |ui| {
                    if ui.button("Preferences").clicked() {
                        todo!()
                    }
                })
            });
        });
    }
}