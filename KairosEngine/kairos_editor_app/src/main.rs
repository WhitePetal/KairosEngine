//! Kairos editor app shell (composition root).
//!
//! This crate is the composition root: it installs the process logger,
//! registers panels, owns the concrete `EditorState`, reconciles open panels
//! with the dock layout, and draws the dock through `kairos_editor_docking`.
//! Panel implementations live in `kairos_editor_component` and the state tree
//! in `kairos_editor_core`; this crate has no panel impls and no headless core
//! of its own.
//!
//! The window, the swapchain and the frame loop are the engine's
//! (`kairos_engine::runtime`). This crate is one `Layer` on that loop: it draws
//! panels and nothing else. The shell borrows the loop's engine for the
//! Profiler panel — there is no second recorder in this process.

mod app;
mod consts;
mod effects;
mod host;
mod paths;

use kairos_engine::{
    Engine,
    kairos_settings::EngineSettings,
    runtime::{RuntimeConfig, run},
};
use winit::dpi::LogicalSize;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // One process logger for the whole run: stderr plus
    // `Library/Logs/KairosEngine.log` by default. The app attaches the Console
    // panel sink when it is constructed; the guard flushes the file on exit, so
    // it must outlive the event loop.
    let _log = match kairos_log::init() {
        Ok(guard) => Some(guard),
        Err(error) => {
            eprintln!("kairos_log: {error}; continuing without the process logger");
            None
        }
    };

    kairos_log::info!(target: "kairos_editor", "editor starting");

    let settings = EngineSettings::load()?;

    // The shell hosts an engine; a machine without an audio device still gets a
    // working editor, so a failed boot falls back to the headless shape.
    let engine = match Engine::new() {
        Ok(engine) => engine,
        Err(error) => {
            kairos_log::warn!(
                target: "kairos_editor",
                "engine boot failed ({error}); continuing headless"
            );
            Engine::headless()
        }
    };

    let config = RuntimeConfig {
        title: format!("{} {}", consts::APP_NAME, consts::VERSION),
        size: LogicalSize::new(1280.0, 820.0),
        min_size: Some(LogicalSize::new(960.0, 600.0)),
        icon: load_icon(),
        texture_compression: settings.texture_compression,
    };

    run(engine, config, |_engine, proxy| {
        host::EditorLayer::new(proxy)
    })
}

fn load_icon() -> Option<winit::window::Icon> {
    let bytes = std::fs::read(paths::PATH_ENGINE_ICON).ok()?;
    let image = image::load_from_memory(&bytes).ok()?.into_rgba8();
    let (width, height) = image.dimensions();

    winit::window::Icon::from_rgba(image.into_raw(), width, height).ok()
}
