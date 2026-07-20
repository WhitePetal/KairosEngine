use kairos_engine::{
    kairos_dialog,
    kairos_editor::runtime::{KairosEditorRuntime, KairosEditorRuntimeEvent},
};
#[cfg(feature = "test-harness")]
use kairos_engine::kairos_test_harness;
use winit::event_loop::EventLoop;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("kairos_engine=debug,warn"),
    )
    .init();

    let event_loop = EventLoop::<KairosEditorRuntimeEvent>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();
    let mut runtime = KairosEditorRuntime::new(proxy).unwrap_or_else(|error| {
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

    event_loop.run_app(&mut runtime)?;

    Ok(())
}
