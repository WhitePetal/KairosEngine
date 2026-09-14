use std::fs;

use crate::ui::{UIReader, inspector::Inspector};

pub struct DirectoryInspector {
    path: std::path::PathBuf,
}

impl Inspector for DirectoryInspector {
    fn create(
        path: &std::path::Path,
        _world: &kairos_ecs::world::World,
        _project_graph: &crate::project_path_tree::ProjectPathGraph,
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
        _reader: &UIReader,
        _messager: &mut crate::ui::Messager,
        _world: &kairos_ecs::world::World,
        _dt: f32,
    ) {
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
}
