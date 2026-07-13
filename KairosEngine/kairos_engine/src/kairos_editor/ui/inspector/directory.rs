use std::fs;

use crate::kairos_editor::ui::inspector::Inspector;

pub struct DirectoryInspector {
    path: std::path::PathBuf,
}

impl Inspector for DirectoryInspector {
    fn create(
        path: &std::path::Path,
        _assets_server: &mut crate::asset_loader::assets::AssetsServer,
    ) -> Result<Self, Box<dyn std::error::Error>>
    where
        Self: Sized,
    {
        Ok(Self {
            path: path.to_path_buf(),
        })
    }

    fn draw(
        &self,
        ui: &mut egui::Ui,
        _messager: &mut crate::kairos_editor::ui::Messager,
        _assets_server: &crate::asset_loader::assets::AssetsServer,
    ) {
        ui.separator();
        match fs::read_dir(&self.path) {
            Ok(entries) => {
                let count = entries.filter_map(|e| e.ok()).count();
                ui.label(format!("Children: {count}"));
            }
            Err(e) => {
                ui.label(format!("Failed to read directory: {e}"));
            }
        }
    }

    fn on_exit(&self) {}
}
