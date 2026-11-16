use kairos_color::Color32;
use kairos_editor_widgets::style::Style;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct ProfilerStyle {
    pub icons: ProfilerIconPaths,

    pub recording_color: Color32,

    pub max_tree_rows: usize,
    pub max_raw_rows: usize,
}

impl Style for ProfilerStyle {
    const FILE: &'static str = "Profiler.ron";

    fn dark() -> Self {
        Self {
            icons: ProfilerIconPaths::drak(),

            recording_color: Color32::from_rgb(224, 82, 82),

            max_tree_rows: 4_000,
            max_raw_rows: 4_000,
        }
    }

    fn light() -> Self {
        Self {
            icons: ProfilerIconPaths::light(),

            recording_color: Color32::from_rgb(224, 82, 82),

            max_tree_rows: 4_000,
            max_raw_rows: 4_000,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ProfilerIconPaths {
    pub tab: String,
}

impl ProfilerIconPaths {
    pub fn drak() -> Self {
        Self {
            tab: "Preferences/Textures/tab_icons/profiler.png".to_owned(),
        }
    }

    pub fn light() -> Self {
        Self {
            tab: "Preferences/Textures/tab_icons/profiler.png".to_owned(),
        }
    }

    pub fn all(&self) -> [&str; 1] {
        [&self.tab]
    }
}
