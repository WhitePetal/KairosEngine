use eframe::egui::{self, AtomExt, Color32, Margin, Pos2, Rect, RichText, Vec2, vec2};

use crate::kairos_editor::paths;

use super::tool_bar_model::ToolBarModel;



pub struct ToolBarView {

}

impl ToolBarView {
    pub fn new() -> Self {
        Self {  }
    }

    pub fn draw(&self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame, model: &ToolBarModel) {
        // 标题栏区域
        let titlebar_rect = Rect::from_min_size(
            Pos2::new(0.0, 0.0), 
            Vec2::new(ctx.content_rect().width(), model.style.height));

        egui::Area::new(egui::Id::new("titlebar"))
            .fixed_pos(Pos2::new(0.0, 0.0))
            .show(ctx, |ui| {
                // 标题栏背景
                ui.painter().rect_filled(
                    titlebar_rect, 
                    model.style.corner_radius, 
                    model.style.fill_color
                );

                ui.horizontal(|ui| {
                    ui.set_height(model.style.height);

                    // 左侧: Icon + 标题
                    ui.add_space(model.style.icon_left_space);
                    // 引擎Icon
                    let icon_size = model.style.height - model.style.icon_boader;
                    
                    egui::Frame::NONE
                        .inner_margin(4)
                        .show(ui, |ui| { 
                            ui.add(egui::Image::new(paths::PATH_ENGINE_ICON).fit_to_exact_size(vec2(icon_size, icon_size)));
                        }
                    );
                    ui.add_space(model.style.title_text_left_space);

                    // 标题文本
                    ui.label(
                        RichText::new(&model.title)
                            .size(model.style.title_text_size)
                            .color(model.style.title_text_color),
                    );
                });
            }
        );
    }
}