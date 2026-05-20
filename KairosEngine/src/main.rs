mod kairos_dialog;
mod kairos_editor;


use winit::event_loop::EventLoop;

use crate::kairos_editor::{runtime::{KairosEditorRuntime, KairosEditorRuntimeEvent}};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>>{
    env_logger::init();

    let event_loop = EventLoop::<KairosEditorRuntimeEvent>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();
    let mut runtime = KairosEditorRuntime::new(proxy).unwrap_or_else(|error| {
        kairos_dialog::error_message_window("Init Failed", &format!("new MainEditorWindow struct Failed:\n {}", error));
        panic!("new MainEditorWindow Failed: {}", error);
    });
    event_loop.run_app(&mut runtime)?;
    
    Ok(())
}

