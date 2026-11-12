//! Kairos editor app shell.
//!
//! This crate is the composition root: it registers panels, owns the concrete
//! `EditorState`, reconciles open panels with the dock layout, and draws the
//! dock through `kairos_editor_docking`. It deliberately has no separate
//! headless core: panels get `&mut EditorState` through `PanelContext`.
//!
//! eframe is still the window backend while the production winit/wgpu event
//! loop is prepared; switching it does not change panel/dock APIs.

mod app;
mod consts;
mod panel;
mod panels;
mod state;

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
