//! 自定义TitleBar
//! 目前处于弃用状态，转为使用原生系统标题栏

// use std::fs;

// use eframe::egui::{self, TextureHandle};
// use sonic_rs::{Deserialize, Serialize, from_str};

// use crate::{consts, kairos_editor::paths, math};

// #[derive(Debug, Serialize, Deserialize)]
// pub struct TitleBarStyle {
//     pub height: f32,
//     pub button_width: f32,
//     pub corner_radius: f32,
//     pub fill_color: math::Color32,
//     pub icon_left_space: f32,
//     pub icon_boader: f32,
//     pub title_text_left_space: f32,
//     pub title_text_size: f32,
//     pub title_text_color: math::Color32,
//     pub title_text_font_size: f32
// }

// pub struct TitleBarModel {
//     pub style: TitleBarStyle,
//     pub title: String
// }

// impl TitleBarStyle {
//     fn new() -> Result<Self, Box<dyn std::error::Error>> {
//         let style_json = fs::read_to_string(paths::PATH_EDITOR_WINDO_TITLE_BAR_STYLE)
//             .map_err(|error| format!("Load MainWindow TitleBar Json Failed, path: {}, error: {}", paths::PATH_EDITOR_WINDO_TITLE_BAR_STYLE, error))?;
//         let style = from_str(&style_json)
//             .map_err(|error| format!("Deserialize MainWindow TitleBar Json Failed, error: {}", error))?;

//         Ok(style)
//     }
// }

// impl TitleBarModel {
//     pub fn new(ctx: &egui::Context) -> Result<Self, Box<dyn std::error::Error>> {
//         let style = TitleBarStyle::new()?;
//         let title = format!("Kairos Engine {}", consts::KAIROS_ENGINE_VERSION);
//         Ok(Self { 
//             style: style,
//             title: title
//         })
//     }
// }