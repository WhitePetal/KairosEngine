

mod app;
mod consts;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([960.0, 600.0])
            .with_title(consts::APP_NAME),
        ..Default::default()
    };

    eframe::run_native(
        consts::APP_NAME,
        options,
        Box::new(|cc| Ok(Box::new(app::EditorApp::new(cc)))),
    )
}
