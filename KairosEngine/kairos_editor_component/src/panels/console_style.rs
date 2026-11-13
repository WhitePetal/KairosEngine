//! Console panel style: the measured Unity/Tuanjie 2022.3 appearance.
//!
//! The values are code defaults for [`ConsoleStyle`], persisted per component in
//! `Preferences/Styles/ConsoleStyle.ron` (dark + light). The panel loads the file
//! in `Panel::init`; editing it restyles the Console without a rebuild.

use kairos_color::Color32;
use kairos_editor_widgets::icons::ConsoleIconPaths;
use kairos_editor_widgets::style::Style;
use serde::{Deserialize, Serialize};

/// Appearance tokens for the Console panel: palette, icon file paths and the
/// measured geometry. Pure data so it can be serialized to RON and later edited
/// from Preferences; colors are [`kairos_color::Color32`], written as hex strings
/// (`"#353535"`) by that crate's serde impls.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConsoleStyle {
    // Palette (the dark values are sampled from the reference capture).
    pub toolbar_bg: Color32,
    pub toolbar_text: Color32,
    pub toggle_bg: Color32,
    pub search_bg: Color32,
    pub search_edge: Color32,
    pub separator: Color32,
    pub row_odd: Color32,
    pub row_even: Color32,
    pub row_selected: Color32,
    pub text: Color32,
    pub text_selected: Color32,
    pub link: Color32,
    pub detail_bg: Color32,

    /// Icon file locations (runtime assets, see `kairos_editor_widgets::ui_image`).
    pub icons: ConsoleIconPaths,

    // Typography.
    pub text_size: f32,
    pub badge_text_size: f32,

    // Toolbar geometry (button widths are the reference's measured widths).
    pub toolbar_height: f32,
    pub clear_width: f32,
    pub collapse_width: f32,
    pub error_pause_width: f32,
    pub editor_width: f32,
    pub count_widths: [f32; 3],
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
            toolbar_bg: Color32::from_rgb(53, 53, 53),
            toolbar_text: Color32::from_rgb(177, 177, 177),
            toggle_bg: Color32::from_rgb(70, 70, 70),
            search_bg: Color32::from_rgb(37, 37, 37),
            search_edge: Color32::from_rgb(29, 29, 29),
            separator: Color32::from_rgb(32, 32, 32),
            row_odd: Color32::from_rgb(55, 55, 55),
            row_even: Color32::from_rgb(49, 49, 49),
            row_selected: Color32::from_rgb(39, 83, 124),
            text: Color32::from_rgb(192, 192, 192),
            text_selected: Color32::from_rgb(241, 245, 247),
            link: Color32::from_rgb(65, 110, 238),
            detail_bg: Color32::from_rgb(49, 49, 49),
            icons: ConsoleIconPaths::dark(),
            text_size: 12.0,
            badge_text_size: 10.0,
            toolbar_height: 20.0,
            clear_width: 55.0,
            collapse_width: 58.0,
            error_pause_width: 73.0,
            editor_width: 50.0,
            count_widths: [40.0, 49.0, 32.0],
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
        }
    }

    /// Light-skin starting point. Unity's dark console was measured; the light
    /// palette is a conventional Unity-light equivalent and should be reviewed
    /// against a light-skin capture before being called final.
    fn light() -> Self {
        Self {
            toolbar_bg: Color32::from_rgb(0xC8, 0xC8, 0xC8),
            toolbar_text: Color32::from_rgb(0x20, 0x20, 0x20),
            toggle_bg: Color32::from_rgb(0xB4, 0xB4, 0xB4),
            search_bg: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            search_edge: Color32::from_rgb(0x9A, 0x9A, 0x9A),
            separator: Color32::from_rgb(0xA6, 0xA6, 0xA6),
            row_odd: Color32::from_rgb(0xF4, 0xF4, 0xF4),
            row_even: Color32::from_rgb(0xEC, 0xEC, 0xEC),
            row_selected: Color32::from_rgb(0x3B, 0x6E, 0xA5),
            text: Color32::from_rgb(0x1E, 0x1E, 0x1E),
            text_selected: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            link: Color32::from_rgb(0x1A, 0x5F, 0xB4),
            detail_bg: Color32::from_rgb(0xE8, 0xE8, 0xE8),
            icons: ConsoleIconPaths::light(),
            ..Self::dark()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kairos_editor_widgets::style::StyleFile;

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
        assert!(!text.contains("Color32"), "{text}");

        let loaded = StyleFile::<ConsoleStyle>::load_from(&path).expect("load style");
        assert_eq!(loaded.dark, ConsoleStyle::dark());
        assert_eq!(loaded.light, ConsoleStyle::light());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
