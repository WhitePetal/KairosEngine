mod title_bar;
mod title_bar_model;
mod title_bar_view;

use crate::math::color;

use super::editor_window_model::EditorWindowModel;
use super::editor_window_view::EditorWindowView;
use self::title_bar::TitleBar;


pub struct MainEditorWindow {
    model: EditorWindowModel,
    view: EditorWindowView,

    title_bar: TitleBar,
}

impl MainEditorWindow {
    pub fn new(title: &str, cc: &eframe::CreationContext) -> Result<Self, Box<dyn std::error::Error>> {
        let model = EditorWindowModel::new(title)?;
        let view = EditorWindowView::new();
        let title_bar = TitleBar::new(&cc.egui_ctx)?;
        Ok(Self{
            model: model,
            view: view,

            title_bar: title_bar
        })
    }
}

impl eframe::App for MainEditorWindow {
    fn update(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        self.model.title_bar_spaces = self.title_bar.get_height();
        self.view.draw(ctx, frame, &self.model);
        self.title_bar.update(ctx, frame);
    }
}