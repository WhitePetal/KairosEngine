use std::{fs, path::PathBuf};

use crate::kairos_editor::ui::{UIReader, dialog::Dialog, inspector::Inspector};

struct UnknownInspectorModel {
    path: PathBuf,
}

pub struct UnknownInspector {
    model: UnknownInspectorModel,
}

impl Inspector for UnknownInspector {
    fn create(
        path: &std::path::Path,
        _assets_server: &mut crate::asset_loader::assets::AssetsServer,
        _project_graph: &crate::kairos_editor::project_path_tree::ProjectPathGraph,
    ) -> Result<Self, Box<dyn std::error::Error>>
    where
        Self: Sized,
    {
        let model = UnknownInspectorModel {
            path: path.to_path_buf(),
        };

        Ok(Self { model })
    }

    fn draw(
        &self,
        ui: &mut egui::Ui,
        _reader: &UIReader,
        _messager: &mut crate::kairos_editor::ui::Messager,
        _assets_server: &crate::asset_loader::assets::AssetsServer,
        _dt: f32,
    ) {
        ui.label("not implement inspector");
        match fs::metadata(&self.model.path) {
            Ok(meta) => {
                ui.label(format!("Size: {} bytes", meta.len()));
            }
            Err(e) => {
                ui.label(format!("Failed to read metadata: {e}"));
            }
        }
    }

    fn on_exit(&mut self, _ctx: &egui::Context) -> Option<Box<dyn Dialog>> {
        None
    }
}
