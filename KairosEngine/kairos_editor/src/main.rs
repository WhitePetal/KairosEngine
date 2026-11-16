use kairos_editor::{consts, dialog, host};
use kairos_engine::{Engine, kairos_settings::EngineSettings, runtime::RuntimeConfig};
use winit::dpi::LogicalSize;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("kairos_engine=debug,warn"),
    )
    .init();

    let settings = EngineSettings::load()?;

    let engine = Engine::new().unwrap_or_else(|error| {
        dialog::error_message_window(
            "Init Failed",
            &format!("creating the engine failed:\n{error}"),
        );
        panic!("creating the engine failed: {error}");
    });

    // The engine's loop owns the window, the swapchain and the frame boundary;
    // the editor is a layer on top of it. `run` never returns while the loop is
    // alive.
    let config = RuntimeConfig {
        title: format!("{} {}", consts::APP_NAME, consts::VERSION),
        size: LogicalSize::new(800.0, 600.0),
        min_size: None,
        icon: host::load_icon(),
        texture_compression: settings.texture_compression,
    };

    if let Err(error) = kairos_engine::runtime::run(engine, config, host::EditorLayer::new) {
        dialog::error_message_window(
            "Init Failed",
            &format!("the editor runtime failed:\n{error}"),
        );
        return Err(error);
    }

    Ok(())
}
