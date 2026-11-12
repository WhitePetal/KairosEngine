//! Kairos editor app shell (composition root).
//!
//! This crate is the composition root: it installs the process logger,
//! registers panels, owns the concrete `EditorState`, reconciles open panels
//! with the dock layout, and draws the dock through `kairos_editor_docking`.
//! Panel implementations live in `kairos_editor_component` and the state tree
//! in `kairos_editor_core`; this crate has no panel impls and no headless core
//! of its own.
//!
//! eframe is still the window backend while the production winit/wgpu event
//! loop is prepared; switching it does not change panel/dock APIs.

mod app;
mod consts;

fn main() -> eframe::Result<()> {
    // One process logger for the whole run: stderr plus
    // `Library/Logs/KairosEngine.log` by default. `EditorApp` attaches the
    // Console panel sink when it is constructed; the guard flushes the file on
    // exit, so it must outlive the event loop.
    let _log = match kairos_log::init() {
        Ok(guard) => Some(guard),
        Err(error) => {
            eprintln!("kairos_log: {error}; continuing without the process logger");
            None
        }
    };

    kairos_log::info!(target: "kairos_editor", "editor starting");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([960.0, 600.0])
            .with_title(format!("{} {}", consts::APP_NAME, consts::VERSION)),
        ..Default::default()
    };

    eframe::run_native(
        consts::APP_NAME,
        options,
        Box::new(|cc| Ok(Box::new(app::EditorApp::new(cc)))),
    )
}
