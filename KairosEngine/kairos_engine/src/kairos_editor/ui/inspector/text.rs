use std::fs;

/// 纯文本文件（Script/Shader/Document）：只读前若干行预览
pub fn draw(ui: &mut egui::Ui, path: &std::path::Path) {
    ui.separator();
    ui.label("Preview:");
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            ui.label(format!("Failed to read file: {e}"));
            return;
        }
    };
    let line_count = content.lines().count();
    ui.label(format!("Lines: {line_count}"));
    egui::ScrollArea::vertical()
        .id_salt("inspector_text_preview")
        .max_height(300.0)
        .show(ui, |ui| {
            ui.monospace(&content);
        });
}
