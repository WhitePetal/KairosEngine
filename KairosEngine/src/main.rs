mod kairos_dialog;
mod kairos_editor;
mod egui_utils;

use std::sync::Arc;

use eframe::{self, egui};

use crate::kairos_editor::{KairosEngine, consts::VERSION};

fn main() -> eframe::Result {
    const APP_NAME: &str = "KairosEngine";
    let window_title = format!("Kairos Engine {}", VERSION);
    let icon = std::fs::read(kairos_editor::ui::paths::PATH_ENGINE_ICON)
        .ok()
        .and_then(|bytes| eframe::icon_data::from_png_bytes(&bytes).ok())
        .map(Arc::new);

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([800.0, 600.0])
        .with_decorations(true)
        .with_transparent(false)
        .with_title(window_title);

    match icon {
        Some(icon) => viewport = viewport.with_icon(icon),
        None => {}
    };

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        APP_NAME, 
        options, 
        Box::new(|_cc| {
            egui_extras::install_image_loaders(&_cc.egui_ctx);
            Ok(Box::new(KairosEngine::new(_cc).unwrap_or_else(|error| {
                    kairos_dialog::error_message_window("Init Failed", &format!("new MainEditorWindow struct Failed:\n {}", error));
                    panic!("new MainEditorWindow Failed: {}", error);
                }
            )))
        }
    ))
}

