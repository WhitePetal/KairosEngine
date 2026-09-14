use std::error::Error;

use native_dialog::{DialogBuilder, MessageLevel};

/// 创建 UI Model 加载失败窗口
pub fn ui_create_error_window(ui_name: &str, error: &Box<dyn Error>) {
    DialogBuilder::message()
        .set_level(MessageLevel::Error)
        .set_title("UI Create Failed")
        .set_text(format!("UI: {}, error: {}", ui_name, error))
        .alert()
        .show()
        .unwrap_or_else(|error| {
            panic!("Create UI Create Error Window Failed,\n model: {ui_name:?}\n error: {error:?}");
        });
}
