//! 提供一系列系统原生弹窗快捷创建方法

use native_dialog::{DialogBuilder, MessageLevel};

/// 创建系统原生报错弹窗
pub fn error_message_window(title: &str, content: &str)
{
    DialogBuilder::message()
        .set_level(MessageLevel::Error)
        .set_title(title)
        .set_text(content)
        .alert()
        .show()
        .unwrap_or_else(|error| {
            panic!("Create Dialog Window Failed,\n title: {title:?}\n content: {content:?}\n error: {error:?}");
        });
}