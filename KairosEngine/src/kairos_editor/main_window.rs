mod title_bar;
mod title_bar_model;
mod title_bar_view;

use super::editor_window_model::EditorWindowModel;
use super::editor_window_view::EditorWindowView;

pub struct MainEditorWindow {
    model: EditorWindowModel,
    view: EditorWindowView,
}

impl MainEditorWindow {
    pub fn new(title: &str, _cc: &eframe::CreationContext) -> Result<Self, Box<dyn std::error::Error>> {
        let model = EditorWindowModel::new(title)?;
        let view = EditorWindowView::new();
        Ok(Self {
            model,
            view,
        })
    }
}

impl eframe::App for MainEditorWindow {
    fn update(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        self.view.draw(ctx, frame, &self.model);
    }
}