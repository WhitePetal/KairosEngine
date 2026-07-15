use std::fs;

use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

use crate::{
    asset_loader::assets::AssetsServer,
    kairos_editor::ui::{dialog::Dialog, inspector::Inspector},
};

struct DocumentModel {
    content: String,
}

pub struct DocumentInspector {
    model: DocumentModel,
}

impl Inspector for DocumentInspector {
    fn create(
        path: &std::path::Path,
        _assets_server: &mut crate::asset_loader::assets::AssetsServer,
    ) -> Result<Self, Box<dyn std::error::Error>>
    where
        Self: Sized,
    {
        let content = fs::read_to_string(path)?;
        let model = DocumentModel { content };

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
        egui::ScrollArea::vertical()
            .id_salt("inspector_document_preview")
            .show(ui, |ui| {
                ui.label(format!("Lines: {line_count}"));
                CommonMarkViewer::new().show(
                    ui,
                    &mut CommonMarkCache::default(),
                    &self.model.content,
                );
            });
    }

    fn on_exit(
        &self,
        _ctx: &egui::Context,
        _assets_server: &AssetsServer,
    ) -> Option<Box<dyn Dialog>> {
        None
    }
}
