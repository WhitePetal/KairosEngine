//! Runnable window demo for the merged editor-ui prototype.
//!
//! `cargo run` from `prototypes/editor-ui` opens a real eframe/egui window that
//! drives the same core reducers, registry-hosted panels and pure widget kit the
//! headless tests use:
//!
//! ```text
//! real App (reducer) -> PanelHost (registry of Box<dyn Panel>)
//!   -> Project/Inspector/Console/Viewport/Gallery panels -> ui-widgets
//!   -> UiOut(Action) -> App::dispatch -> Effect -> host (IO / wgpu viewport)
//! ```
//!
//! The 3D viewport is real: `editor-viewport` renders the scene into an offscreen
//! wgpu texture and registers it with egui, mirroring the real engine's
//! `GraphicsCommand::bind_attachment_to_egui` path.
//!
//! Read `app/mod.rs` first (impl eframe::App = frame entry), then
//! `docs/architecture-tour.md` for the full draw/message path.
//!
//! This binary is still throwaway prototype code, but its architecture is the
//! same one the production editor is migrating to (ADR 0008/0009).

mod app;
mod gallery;
mod host;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([960.0, 600.0])
            .with_title("Kairos Editor - UI architecture prototype"),
        ..Default::default()
    };

    eframe::run_native(
        "Kairos Editor - UI architecture prototype",
        options,
        Box::new(|cc| Ok(Box::new(app::EditorApp::new(cc)))),
    )
}
