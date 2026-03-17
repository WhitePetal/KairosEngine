use std::error::Error;

use native_dialog::{DialogBuilder, MessageLevel};



/// 创建 UI Model 加载失败窗口
pub fn ui_model_load_error_window(model_name: &str, error: &Box<dyn Error>) {
    DialogBuilder::message()
        .set_level(MessageLevel::Error)
        .set_title("UI Model Load Failed")
        .set_text(format!("Model: {}, error: {}", model_name, error))
        .alert()
        .show()
        .unwrap_or_else(|error| {
            panic!("Create UI Model Load Error Window Failed,\n model: {model_name:?}\n error: {error:?}");
        });
}