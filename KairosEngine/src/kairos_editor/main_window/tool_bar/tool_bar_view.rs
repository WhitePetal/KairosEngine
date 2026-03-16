use eframe::egui::{self, AtomExt, Color32, Margin, Pos2, Rect, RichText, TopBottomPanel, Vec2, containers::menu::{self, MenuButton}, vec2};

use crate::kairos_editor::paths;

use super::tool_bar_model::ToolBarModel;



pub struct ToolBarView {

}

impl ToolBarView {
    pub fn new() -> Self {
        Self {  }
    }

    pub fn draw(&self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame, model: &ToolBarModel) {
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
                
                ui.menu_button("File", |ui| {
                    if ui.button("New Scene").clicked() {
                        todo!()
                    }
                });
            });
        });
    }
}