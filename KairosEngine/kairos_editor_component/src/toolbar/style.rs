use kairos_color::Color32;
use kairos_editor_widgets::style::Style;
use serde::{Deserialize, Serialize};


#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolBarStyle {
    pub icons: ToolBarIconPaths,

    pub background_color: Color32,
    pub text_color: Color32,

    pub height: f32,
    pub corner_radius: f32,
}

impl Style for ToolBarStyle {
    const FILE: &'static str = "ToolBar.ron";

    fn dark() -> Self {
        Self {
            icons: ToolBarIconPaths::dart(),

            background_color: Color32::from_rgb(25, 25, 25),
            text_color: Color32::from_rgb(254, 254, 254),

            height: 23.0,
            corner_radius: 0.0,
        }
    }

    fn light() -> Self {
        Self {
            icons: ToolBarIconPaths::light(),

            background_color: Color32::from_rgb(25, 25, 25),
            text_color: Color32::from_rgb(254, 254, 254),

            height: 23.0,
            corner_radius: 0.0,
        }
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolBarIconPaths {
    pub about: String
}

impl ToolBarIconPaths {
    pub fn dart() -> Self {
        Self {
            about: "Preferences/Textures/engine_icon.png".to_owned()
        }
    }

    pub fn light() -> Self {
        Self {
            about: "Preferences/Textures/engine_icon.png".to_owned()
        }
    }

    pub fn all(&self) -> [&str; 1] {
        [
            &self.about,
        ]
    }
}
