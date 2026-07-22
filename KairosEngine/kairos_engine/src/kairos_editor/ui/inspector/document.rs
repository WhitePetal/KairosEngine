use std::{cell::Cell, fs, ops::DerefMut, path::PathBuf, sync::Arc};

use egui::Vec2;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::{
    asset_loader::assets::{AssetHandle, AssetsServer, asset::TextAssetsSystem},
    kairos_editor::ui::{
        Message, Messager,
        dialog::{ConfirmDialogWindow, Dialog},
        inspector::Inspector,
        paths,
    },
};

#[derive(Debug, Serialize, Deserialize)]
struct DocumentStyle {
    button_height: f32,
}
impl DocumentStyle {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let bytes = fs::read(paths::PATH_DOCUMENT_INSPECTOR_STYLE)?;
        let style = toml::from_slice(&bytes)?;
        Ok(style)
    }
}

struct DocumentModel {
    style: DocumentStyle,
    path: PathBuf,
    handle: Arc<AssetHandle<TextAssetsSystem>>,
    content: Arc<Mutex<Option<String>>>,
}

pub struct DocumentInspector {
    model: DocumentModel,
    editing: Cell<bool>,
}

impl Inspector for DocumentInspector {
    fn create(
        path: &std::path::Path,
        assets_server: &mut AssetsServer,
        _project_graph: &crate::kairos_editor::project_path_tree::ProjectPathGraph,
    ) -> Result<Self, Box<dyn std::error::Error>>
    where
        Self: Sized,
    {
        let style = DocumentStyle::new()?;
        let path = path.to_path_buf();
        let handle = assets_server.load(&path);
        let content = Arc::new(Mutex::new(None));
        let model = DocumentModel {
            style,
            path,
            handle,
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
        messager: &mut Messager,
        assets_server: &AssetsServer,
        _dt: f32,
    ) {
        {
            let mut content_mut = self.model.content.lock();
            let content_mut = content_mut.deref_mut();
            if content_mut.is_none() {
                if let Some(content) = assets_server.get(&self.model.handle) {
                    *content_mut = Some(content.clone());
                }
                ui.label("Document is Loading...");
                return;
            }
        }

        ui.vertical_centered(|ui| {
            if self.editing.get() {
                let btn = egui::Button::new("Save").min_size(Vec2::new(
                    ui.available_width(),
                    self.model.style.button_height,
                ));
                if ui.add(btn).clicked() {
                    self.editing.replace(false);
                    messager.send(Message::DocumentInspectorSave(
                        self.model.path.clone(),
                        self.model.handle.clone(),
                        self.model.content.clone(),
                    ));
                }
            } else {
                let btn = egui::Button::new("Edit").min_size(Vec2::new(
                    ui.available_width(),
                    self.model.style.button_height,
                ));
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
                        let editor =
                            egui::TextEdit::multiline(content_mut).min_size(ui.available_size());
                        ui.add(editor);
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

    fn on_exit(&mut self, _ctx: &egui::Context) -> Option<Box<dyn Dialog>> {
        if self.editing.get() {
            self.editing.replace(false);
            let content = self.model.content.clone();
            let path = self.model.path.clone();
            let handle = self.model.handle.clone();
            let dialog = ConfirmDialogWindow::new(
                "have modify not save".into(),
                "save the modify?".into(),
                "save".into(),
                "cancel".into(),
                Some(Message::DocumentInspectorSave(path, handle, content)),
                None,
                None::<fn()>,
                None::<fn()>,
            );
            Some(Box::new(dialog))
        } else {
            None
        }
    }
}

impl DocumentInspector {
    pub fn save_content(
        assets_server: &mut AssetsServer,
        path: &PathBuf,
        handle: Arc<AssetHandle<TextAssetsSystem>>,
        content: Arc<Mutex<Option<String>>>,
    ) {
        let mut content = content.lock();
        if let Some(content) = content.deref_mut().take() {
            if let Err(e) = fs::write(path, &content) {
                log::warn!("Failed to write Document '{}': {e}", path.display());
            }
            if let Some(doc_res) = assets_server.get_mut(&handle) {
                *doc_res = content;
            }
        }
    }
}
