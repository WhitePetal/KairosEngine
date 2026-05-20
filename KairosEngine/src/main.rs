mod kairos_dialog;
mod kairos_editor;
mod egui_utils;


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

    let icon = std::fs::read(kairos_editor::ui::paths::PATH_ENGINE_ICON)
        .ok()
        .and_then(|bytes| {
                let image = image::load_from_memory(&bytes);
                match image {
                    Ok(image) => {
                        Some(image.into_rgba8())
                    },
                    Err(_) => None,
                }
            }
        );
    
    Ok(())

    // const APP_NAME: &str = "KairosEngine";
    // let window_title = format!("Kairos Engine {}", VERSION);
    // let icon = std::fs::read(kairos_editor::ui::paths::PATH_ENGINE_ICON)
    //     .ok()
    //     .and_then(|bytes| eframe::icon_data::from_png_bytes(&bytes).ok())
    //     .map(Arc::new);

    // let mut viewport = egui::ViewportBuilder::default()
    //     .with_inner_size([800.0, 600.0])
    //     .with_decorations(true)
    //     .with_transparent(false)
    //     .with_title(window_title);

    // match icon {
    //     Some(icon) => viewport = viewport.with_icon(icon),
    //     None => {}
    // };

    // let options = eframe::NativeOptions {
    //     viewport,
    //     ..Default::default()
    // };

    // eframe::run_native(
    //     APP_NAME, 
    //     options, 
    //     Box::new(|_cc| {
    //         egui_extras::install_image_loaders(&_cc.egui_ctx);
    //         Ok(Box::new(KairosEngine::new(_cc).unwrap_or_else(|error| {
    //                 kairos_dialog::error_message_window("Init Failed", &format!("new MainEditorWindow struct Failed:\n {}", error));
    //                 panic!("new MainEditorWindow Failed: {}", error);
    //             }
    //         )))
    //     }
    // ))
}

