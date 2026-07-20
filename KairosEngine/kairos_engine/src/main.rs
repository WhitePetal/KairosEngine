#[cfg(feature = "test-harness")]
use kairos_engine::kairos_test_harness;
use kairos_engine::{
    kairos_dialog,
    kairos_editor::runtime::{KairosEditorRuntime, KairosEditorRuntimeEvent},
};
use winit::event_loop::EventLoop;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("kairos_engine=debug,warn"),
    )
    .init();

    // Parse CLI args once — used by both headless and windowed mode.
    #[cfg(feature = "test-harness")]
    let cli = kairos_test_harness::headless::parse_args();

    #[cfg(feature = "test-harness")]
    {
        if cli.gen_docs {
            kairos_test_harness::docs_gen::generate()?;
            println!("Generated docs/ai/test-harness-commands.md");
            return Ok(());
        }

        if cli.headless {
            return kairos_test_harness::headless::run(cli).await;
        }
    }

    // --- Windowed (normal editor) mode ---
    let event_loop = EventLoop::<KairosEditorRuntimeEvent>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();
    let mut runtime = KairosEditorRuntime::new(proxy.clone()).unwrap_or_else(|error| {
        kairos_dialog::error_message_window(
            "Init Failed",
            &format!("new MainEditorWindow struct Failed:\n {}", error),
        );
        panic!("new MainEditorWindow Failed: {}", error);
    });

    #[cfg(feature = "test-harness")]
    if let Some(sender) = runtime.test_bridge_sender() {
        tokio::spawn(async move {
            kairos_test_harness::ws_server::start(sender, 9999).await;
        });
    }

    // Run a TOML test file in windowed mode (UI interaction tests).
    #[cfg(feature = "test-harness")]
    if let Some(test_file) = cli.test_file.clone() {
        log::info!("Windowed test mode — running: {test_file}");
        if let Some(sender) = runtime.test_bridge_sender() {
            let proxy = proxy.clone();
            tokio::spawn(async move {
                let result = kairos_test_harness::test_runner::run_test_file(&test_file, sender).await;
                let json = serde_json::to_string_pretty(&result)
                    .unwrap_or_else(|_| format!("{:?}", result));
                let passed = result.status == "passed";
                let _ = proxy.send_event(KairosEditorRuntimeEvent::TestCompleted {
                    passed,
                    message: json,
                });
            });
        }
    }

    event_loop.run_app(&mut runtime)?;

    Ok(())
}
