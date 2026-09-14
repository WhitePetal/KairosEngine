use kairos_editor::{
    dialog,
    runtime::{EditorRuntime, EditorRuntimeEvent},
};
use winit::event_loop::EventLoop;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("kairos_engine=debug,warn"),
    )
    .init();

    // --- Windowed (normal editor) mode ---
    let event_loop = EventLoop::<EditorRuntimeEvent>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();
    let mut runtime = EditorRuntime::new(proxy.clone()).unwrap_or_else(|error| {
        dialog::error_message_window(
            "Init Failed",
            &format!("new MainEditorWindow struct Failed:\n {}", error),
        );
        panic!("new MainEditorWindow Failed: {}", error);
    });

    event_loop.run_app(&mut runtime)?;

    Ok(())
}
