use std::{
    cell::Cell,
    fs,
    ops::{Deref, DerefMut},
    path::PathBuf,
    sync::Arc,
};

use egui::Vec2;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use parking_lot::Mutex;

use crate::{
    asset_loader::assets::AssetsServer,
    kairos_editor::ui::{
        dialog::{ConfirmDialogWindow, Dialog},
        inspector::Inspector,
    },
};

struct DocumentModel {
    path: PathBuf,
    content: Arc<Mutex<Option<String>>>,
}

pub struct DocumentInspector {
    model: DocumentModel,
    editing: Cell<bool>,
}

impl Inspector for DocumentInspector {
    fn create(
        path: &std::path::Path,
        _assets_server: &mut crate::asset_loader::assets::AssetsServer,
    ) -> Result<Self, Box<dyn std::error::Error>>
    where
        Self: Sized,
    {
        let content = Arc::new(Mutex::new(Some(fs::read_to_string(path)?)));
        let model = DocumentModel {
            path: path.to_path_buf(),
            content,
        };

        Ok(Self {
            model,
            editing: Cell::new(false),
        })
    }

    fn draw(
        &self,
        ui: &mut egui::Ui,
        _messager: &mut crate::kairos_editor::ui::Messager,
        _assets_server: &crate::asset_loader::assets::AssetsServer,
    ) {
        ui.separator();
        ui.vertical_centered(|ui| {
            if self.editing.get() {
                let btn = egui::Button::new("Save").min_size(Vec2::new(ui.available_width(), 20.0));
                if ui.add(btn).clicked() {
                    self.editing.replace(false);
                    self.save();
                }
            } else {
                let btn = egui::Button::new("Edit").min_size(Vec2::new(ui.available_width(), 20.0));
                if ui.add(btn).clicked() {
                    self.editing.replace(true);
                }
            }
        });
        ui.separator();
        let content = self.model.content.clone();
        let mut content_mut = content.lock();
        if let Some(content_mut) = content_mut.deref_mut() {
            let line_count = content_mut.lines().count();
            egui::ScrollArea::vertical()
                .id_salt("inspector_document_preview")
                .show(ui, |ui| {
                    ui.label(format!("Lines: {line_count}"));
                    if self.editing.get() {
                        ui.text_edit_multiline(content_mut);
                    } else {
                        CommonMarkViewer::new().show(
                            ui,
                            &mut CommonMarkCache::default(),
                            content_mut,
                        );
                    }
                });
        }
    }

    fn on_exit(
        &mut self,
        _ctx: &egui::Context,
        _assets_server: &AssetsServer,
    ) -> Option<Box<dyn Dialog>> {
        if self.editing.get() {
            self.editing.replace(false);
            let content = self.model.content.clone();
            let path = self.model.path.clone();
            let dialog = ConfirmDialogWindow::new(
                "have modify not save".into(),
                "save the modify?".into(),
                "save".into(),
                "cancel".into(),
                Some(move || {
                    Self::save_content(&path, content);
                }),
                None::<fn()>,
            );
            Some(Box::new(dialog))
        } else {
            None
        }
    }
}

impl DocumentInspector {
    fn save(&self) {
        let content = self.model.content.clone();
        Self::save_content(&self.model.path, content);
    }

    fn save_content(path: &PathBuf, content: Arc<Mutex<Option<String>>>) {
        let content = content.lock();
        if let Some(content) = content.deref() {
            if let Err(e) = fs::write(path, content) {
                log::warn!("Failed to write Document '{}': {e}", path.display());
            }
        }
    }
}
