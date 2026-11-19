//! Console panel style: the measured Unity/Tuanjie 2022.3 appearance.
//!
//! The values are code defaults for [`ConsoleStyle`], persisted per component in
//! `Preferences/Styles/ConsoleStyle.ron` (dark + light). The panel loads the file
//! in `Panel::init`; editing it restyles the Console without a rebuild.

use kairos_color::Color;
use kairos_editor_widgets::style::Style;
use kairos_log::Level;
use serde::{Deserialize, Serialize};

use crate::skin::StyleFields;

/// Appearance tokens for the Console panel: palette, icon file paths and the
/// measured geometry. Pure data so it can be serialized to RON and later edited
/// from Preferences; colors are [`kairos_color::Color`], written as hex strings
/// (`"#353535"`) by that crate's serde impls.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, StyleFields)]
pub struct ConsoleStyle {
    // Palette (the dark values are sampled from the reference capture).
    pub toolbar_bg: Color,
    pub toolbar_text: Color,
    pub toggle_bg: Color,
    pub search_bg: Color,
    pub search_edge: Color,
    pub separator: Color,
    pub row_odd: Color,
    pub row_even: Color,
    pub row_selected: Color,
    pub text: Color,
    pub text_selected: Color,
    pub link: Color,
    pub detail_bg: Color,

    /// Icon file locations (runtime assets, see `kairos_editor_widgets::ui_image`).
    pub icons: ConsoleIconPaths,

    /// Console panel (tab header) icon, drawn left of the tab title. Runtime
    /// appearance data: the panel preloads it with its other icons in `init`.
    ///
    /// The `serde(default)` keeps style files written before the tab icon
    /// existed loadable.
    #[serde(default = "default_tab_icon")]
    pub tab_icon: String,

    // Typography.
    pub text_size: f32,
    pub badge_text_size: f32,

    // Toolbar geometry (button widths are the reference's measured widths).
    pub toolbar_height: f32,
    pub clear_width: f32,
    pub collapse_width: f32,
    pub error_pause_width: f32,
    pub editor_width: f32,
    /// Floor width of each level-count toggle; the toggles grow to fit their
    /// number (Unity: 40 px for `39`, 49 px for `303`, 32 px for `5`).
    pub count_widths: f32,
    pub count_icon: f32,
    pub search_radius: u8,

    // Log-list geometry.
    pub row_height: f32,
    pub row_text_top: f32,
    pub icon_size: f32,
    pub icon_left: f32,
    pub text_left: f32,
    pub badge_size: f32,

    // Stack-trace detail pane.
    pub detail_height: f32,
    pub detail_padding: f32,

    pub separator_width: f32,

    pub splitter_grab: f32,
}

/// Fallback for style files written before the tab icon existed.
fn default_tab_icon() -> String {
    "Preferences/Textures/tab_icons/console.png".to_owned()
}

impl Default for ConsoleStyle {
    fn default() -> Self {
        Self::dark()
    }
}

impl Style for ConsoleStyle {
    const FILE: &'static str = "ConsoleStyle.ron";

    /// The measured Unity/Tuanjie 2022.3 dark-skin console.
    fn dark() -> Self {
        Self {
            toolbar_bg: Color::from_srgb8([53, 53, 53, 255]),
            toolbar_text: Color::from_srgb8([177, 177, 177, 255]),
            toggle_bg: Color::from_srgb8([70, 70, 70, 255]),
            search_bg: Color::from_srgb8([37, 37, 37, 255]),
            search_edge: Color::from_srgb8([29, 29, 29, 255]),
            separator: Color::from_srgb8([32, 32, 32, 255]),
            row_odd: Color::from_srgb8([55, 55, 55, 255]),
            row_even: Color::from_srgb8([49, 49, 49, 255]),
            row_selected: Color::from_srgb8([39, 83, 124, 255]),
            text: Color::from_srgb8([192, 192, 192, 255]),
            text_selected: Color::from_srgb8([241, 245, 247, 255]),
            link: Color::from_srgb8([65, 110, 238, 255]),
            detail_bg: Color::from_srgb8([49, 49, 49, 255]),
            icons: ConsoleIconPaths::dark(),
            tab_icon: "Preferences/Textures/tab_icons/console.png".to_owned(),
            text_size: 12.0,
            badge_text_size: 10.0,
            toolbar_height: 20.0,
            clear_width: 55.0,
            collapse_width: 58.0,
            error_pause_width: 73.0,
            editor_width: 50.0,
            count_widths: 40.0,
            count_icon: 13.0,
            search_radius: 3,
            row_height: 38.0,
            row_text_top: 6.0,
            icon_size: 26.0,
            icon_left: 10.0,
            text_left: 41.0,
            badge_size: 20.0,
            detail_height: 130.0,
            detail_padding: 3.0,
            separator_width: 1.0,
            splitter_grab: 3.0,
        }
    }

    /// Light-skin starting point. Unity's dark console was measured; the light
    /// palette is a conventional Unity-light equivalent and should be reviewed
    /// against a light-skin capture before being called final.
    fn light() -> Self {
        Self {
            toolbar_bg: Color::from_srgb8([0xC8, 0xC8, 0xC8, 255]),
            toolbar_text: Color::from_srgb8([0x20, 0x20, 0x20, 255]),
            toggle_bg: Color::from_srgb8([0xB4, 0xB4, 0xB4, 255]),
            search_bg: Color::from_srgb8([0xFF, 0xFF, 0xFF, 255]),
            search_edge: Color::from_srgb8([0x9A, 0x9A, 0x9A, 255]),
            separator: Color::from_srgb8([0xA6, 0xA6, 0xA6, 255]),
            row_odd: Color::from_srgb8([0xF4, 0xF4, 0xF4, 255]),
            row_even: Color::from_srgb8([0xEC, 0xEC, 0xEC, 255]),
            row_selected: Color::from_srgb8([0x3B, 0x6E, 0xA5, 255]),
            text: Color::from_srgb8([0x1E, 0x1E, 0x1E, 255]),
            text_selected: Color::from_srgb8([0xFF, 0xFF, 0xFF, 255]),
            link: Color::from_srgb8([0x1A, 0x5F, 0xB4, 255]),
            detail_bg: Color::from_srgb8([0xE8, 0xE8, 0xE8, 255]),
            icons: ConsoleIconPaths::light(),
            tab_icon: "Preferences/Textures/tab_icons/light/console.png".to_owned(),
            ..Self::dark()
        }
    }
}

/// Runtime icon file paths for the Console panel, relative to the editor's
/// working directory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StyleFields)]
pub struct ConsoleIconPaths {
    pub info: String,
    pub warning: String,
    pub error: String,
    pub info_off: String,
    pub warning_off: String,
    pub error_off: String,
    pub badge: String,
}

impl ConsoleIconPaths {
    /// The measured Unity/Tuanjie 2022.3 dark-skin icon set.
    pub fn dark() -> Self {
        Self {
            info: "Preferences/Textures/console/info.png".to_owned(),
            warning: "Preferences/Textures/console/warning.png".to_owned(),
            error: "Preferences/Textures/console/error.png".to_owned(),
            info_off: "Preferences/Textures/console/info_off.png".to_owned(),
            warning_off: "Preferences/Textures/console/warning_off.png".to_owned(),
            error_off: "Preferences/Textures/console/error_off.png".to_owned(),
            badge: "Preferences/Textures/console/count_badge.png".to_owned(),
        }
    }

    /// Light-skin active icons. Unity ships separate light artwork; the dimmed
    /// variants are grey enough to work on both skins, so they are shared.
    pub fn light() -> Self {
        Self {
            info: "Preferences/Textures/console/light/info.png".to_owned(),
            warning: "Preferences/Textures/console/light/warning.png".to_owned(),
            error: "Preferences/Textures/console/light/error.png".to_owned(),
            ..Self::dark()
        }
    }

    /// The icon for one severity, active or dimmed (filtered out).
    pub fn severity(&self, severity: Level, visible: bool) -> &str {
        match (severity, visible) {
            (Level::Info, true) => &self.info,
            (Level::Info, false) => &self.info_off,
            (Level::Warning, true) => &self.warning,
            (Level::Warning, false) => &self.warning_off,
            (Level::Error, true) => &self.error,
            (Level::Error, false) => &self.error_off,
        }
    }

    /// The collapse repeat-count badge.
    pub fn badge(&self) -> &str {
        &self.badge
    }

    /// All paths in deterministic order, for preloading in `Panel::init`.
    pub fn all(&self) -> [&str; 7] {
        [
            &self.info,
            &self.warning,
            &self.error,
            &self.info_off,
            &self.warning_off,
            &self.error_off,
            &self.badge,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kairos_editor_widgets::style::StyleFile;

    #[test]
    fn tab_icon_files_exist_for_both_skins() {
        let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        for style in [ConsoleStyle::dark(), ConsoleStyle::light()] {
            assert!(
                workspace.join(&style.tab_icon).is_file(),
                "missing tab icon {}",
                style.tab_icon
            );
        }
    }

    #[test]
    fn defaults_round_trip_through_ron_as_hex_colors() {
        let dir = std::env::temp_dir().join(format!(
            "kairos-console-style-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let path = dir.join("ConsoleStyle.ron");
        let _ = std::fs::remove_dir_all(&dir);

        StyleFile::<ConsoleStyle>::defaults()
            .save_to(&path)
            .expect("save style");
        let text = std::fs::read_to_string(&path).expect("read style");
        assert!(text.contains("\"#353535\""), "{text}");
        assert!(!text.contains("Color"), "{text}");

        let loaded = StyleFile::<ConsoleStyle>::load_from(&path).expect("load style");
        assert_eq!(loaded.dark, ConsoleStyle::dark());
        assert_eq!(loaded.light, ConsoleStyle::light());

        let _ = std::fs::remove_dir_all(&dir);
    }

    fn assert_exist(paths: ConsoleIconPaths) {
        let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        for path in paths.all() {
            assert!(
                workspace.join(path).is_file(),
                "missing console icon {path}"
            );
        }
    }

    #[test]
    fn console_icons_are_external_files() {
        // Icons are runtime files under `Preferences/Textures`, not `include_bytes!`.
        assert_exist(ConsoleIconPaths::dark());
    }

    #[test]
    fn light_console_icons_are_external_files() {
        assert_exist(ConsoleIconPaths::light());
    }
}
