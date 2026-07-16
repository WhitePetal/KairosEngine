use std::fs;

use crate::{
    asset_loader::assets::AssetsServer,
    kairos_editor::ui::{dialog::Dialog, inspector::Inspector},
};

struct TextInspectorModel {
    content: String,
}

pub struct TextInspector {
    model: TextInspectorModel,
}

impl Inspector for TextInspector {
    fn create(
        path: &std::path::Path,
        _assets_server: &mut crate::asset_loader::assets::AssetsServer,
    ) -> Result<Self, Box<dyn std::error::Error>>
    where
        Self: Sized,
    {
        let content = fs::read_to_string(path)?;
        let model = TextInspectorModel { content };

        Ok(Self { model })
    }

    fn draw(
        &self,
        ui: &mut egui::Ui,
        _messager: &mut crate::kairos_editor::ui::Messager,
        _assets_server: &crate::asset_loader::assets::AssetsServer,
    ) {
        ui.separator();
        ui.label("Preview:");
        let content = &self.model.content;
        let line_count = content.lines().count();
        ui.label(format!("Lines: {line_count}"));
        egui::ScrollArea::vertical()
            .id_salt("inspector_text_preview")
            .show(ui, |ui| {
                // TODO: code editor
                // https://github.com/emilk/egui/blob/main/crates/egui_demo_lib/src/demo/code_editor.rs
                ui.monospace(content);
            });
    }

    fn on_exit(
        &mut self,
        _ctx: &egui::Context,
        _assets_server: &AssetsServer,
    ) -> Option<Box<dyn Dialog>> {
        None
    }
}
