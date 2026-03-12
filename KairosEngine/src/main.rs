mod kairos_dialog;
mod kairos_editor;
mod math;
mod egui_utils;
mod consts;

use eframe::{self, App, egui};

use crate::kairos_editor::main_window::MainEditorWindow;

fn main() -> eframe::Result {
    const APP_NAME: &str = "KairosEngine";
    let options = eframe::NativeOptions{
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_decorations(false)
            .with_transparent(false),
        ..Default::default()
    };

    eframe::run_native(
        APP_NAME, 
        options, 
        Box::new(|_cc| {
            egui_extras::install_image_loaders(&_cc.egui_ctx);
            Ok(Box::new(MainEditorWindow::new(APP_NAME, _cc).unwrap_or_else(|error| {
                    kairos_dialog::error_message_window("Init Failed", &format!("new MainEditorWindow struct Failed:\n {}", error));
                    panic!("new MainEditorWindow Failed: {}", error);
                }
            )))
        }
    ))
}

