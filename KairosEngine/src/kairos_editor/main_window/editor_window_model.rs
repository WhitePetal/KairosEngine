use std::fs;

use crate::kairos_editor::paths;
use kairos_engine::{math};
use sonic_rs::{Deserialize, Serialize, from_str};


#[derive(Debug, Serialize, Deserialize)]
pub struct EditorWindowStyle {
    pub background_color: math::Color32,
    pub central_panel_color: math::Color32
}

pub struct EditorWindowModel {
    pub style: EditorWindowStyle,
    pub title: String,
    pub tool_bar_height: f32,
    pub is_maximized: bool,
}

impl EditorWindowStyle {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style_json = fs::read_to_string(paths::PATH_EDITOR_WINDOW_STYLE)
            .map_err(|e| format!("Loader EditorWindowStyle.json Failed: {}, Path: {}", e, paths::PATH_EDITOR_WINDOW_STYLE))?;

        let style = from_str(&style_json)
            .map_err(|e| format!("Desierialize Json Failed: {}", e))?;

        Ok(style)
    }
}

impl EditorWindowModel {
    pub fn new(title: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let style = EditorWindowStyle::new()?;

        Ok(Self { 
            style: style, 
            title: title.to_string(),
            tool_bar_height: 0.0, 
            is_maximized: false,
        })
    }
}