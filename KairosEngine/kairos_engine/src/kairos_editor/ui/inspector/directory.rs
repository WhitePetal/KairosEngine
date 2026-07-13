use std::fs;

/// 目录：统计子项数量
pub fn draw(ui: &mut egui::Ui, path: &std::path::Path) {
    ui.separator();
    match fs::read_dir(path) {
        Ok(entries) => {
            let count = entries.filter_map(|e| e.ok()).count();
            ui.label(format!("Children: {count}"));
        }
        Err(e) => {
            ui.label(format!("Failed to read directory: {e}"));
        }
    }
}
