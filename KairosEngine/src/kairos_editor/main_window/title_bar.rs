use eframe::egui;

use super::title_bar_model::TitleBarModel;
use super::title_bar_view::TitleBarView;

pub struct TitleBar {
    model: TitleBarModel,
    view: TitleBarView
}

impl TitleBar {
    pub fn new(ctx: &egui::Context) -> Result<Self, Box<dyn std::error::Error>> {
        let model = TitleBarModel::new(ctx)?;
        let view = TitleBarView::new();

        Ok(Self{
            model: model,
            view: view
        })
    }

    pub fn update(&self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        self.view.draw(ctx, frame, &self.model);
    }

    pub fn get_height(&self) -> f32 {
        self.model.style.height
    }
}