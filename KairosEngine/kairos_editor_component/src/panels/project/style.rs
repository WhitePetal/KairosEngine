//! Project panel style: the measured Unity/Tuanjie Project window appearance.
//!
//! The values are code defaults for [`ProjectStyle`], persisted per component in
//! `Preferences/Styles/ProjectStyle.ron` (dark + light). The panel loads the file
//! in `Panel::init`; editing it restyles the Project panel without a rebuild, and
//! `#[derive(StyleFields)]` puts every token in the Preferences skin editor for
//! free.
//!
//! Two halves:
//!
//! * the tree's visual tokens (row height, indent, icon size, the selection
//!   color) are also what `kairos_editor_widgets::tree::TreeStyle` consumes, so
//!   [`ProjectStyle::tree`] projects them into the widget's own type;
//! * the panel chrome (toolbar, splitter, breadcrumb, status bar) is drawn by the
//!   panel itself, so it reads these tokens directly.
//!
//! The seven tree tokens that the legacy `ProjectWindowStyle.toml` actually
//! defined are inherited 1:1 (see [`ProjectStyle::dark`]); everything the legacy
//! window had no token for (toolbar, status bar, breadcrumb, hover, separator,
//! splitter) is measured against the Console panel's palette, which is the same
//! Unity/Tuanjie skin.

use kairos_color::Color;
use kairos_editor_widgets::color::ColorExt;
use kairos_editor_widgets::style::Style;
use kairos_editor_widgets::tree::TreeStyle;
use serde::{Deserialize, Serialize};

use crate::skin::StyleFields;

/// Appearance tokens for the Project panel.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, StyleFields)]
pub struct ProjectStyle {
    // --- tree pane ---
    /// Background of the whole panel body (both columns and the status bar).
    pub background: Color,
    /// Row names. Folders and files share one colour, as Unity 2022 draws
    /// them; the legacy `directory_header_color` pale yellow is not carried over.
    pub row_text: Color,
    /// The grey type label at the right end of a file row (legacy `file_suffix_color`).
    pub suffix_text: Color,
    /// Selected-row fill (legacy `selected_file_background_color`).
    pub selected_bg: Color,
    /// Selected-row corner radius (legacy `selected_file_background_corner_radius`).
    pub selected_radius: u8,
    /// Row under the pointer.
    pub hover_bg: Color,
    /// Expand/collapse triangle.
    pub expander: Color,
    /// The 1 px rule above a system directory (Library) and under the toolbar.
    pub separator: Color,
    pub separator_width: f32,

    // --- tree geometry (Unity's Project browser measures) ---
    pub row_height: f32,
    pub indent: f32,
    pub expander_size: f32,
    /// Legacy `file_icon_size`.
    pub icon_size: f32,
    pub row_text_size: f32,
    /// Legacy `file_header_size`: the type label's size.
    pub suffix_text_size: f32,

    // --- toolbar ---
    pub toolbar_bg: Color,
    pub toolbar_text: Color,
    pub toolbar_height: f32,
    /// Hover/active fill of a toolbar button and of a hovered breadcrumb.
    pub control_hover: Color,

    // --- splitter ---
    pub splitter_idle: Color,
    pub splitter_hover: Color,
    pub splitter_active: Color,
    pub splitter_width: f32,
    /// Half-width of the draggable band around the splitter line.
    pub splitter_grab: f32,

    // --- content header (the breadcrumb bar over the right pane) ---
    /// Header background: a shade darker than the body so the bar reads as
    /// chrome, like Unity/Tuanjie's Project browser header.
    pub header_bg: Color,
    /// Header height; the breadcrumb lives inside it and a 1 px rule closes it.
    pub header_height: f32,
    pub breadcrumb_text: Color,
    pub breadcrumb_separator: Color,

    /// Empty-state and hint text (the right pane until the grid lands).
    pub hint_text: Color,

    // --- status bar ---
    pub status_bg: Color,
    pub status_text: Color,
    pub status_height: f32,
    pub status_text_size: f32,

    /// Row icon files (runtime assets under `Preferences/Textures/project`).
    pub icons: ProjectIcons,

    /// Tab-header artwork. `serde(default)` keeps style files written before the
    /// icon existed loadable.
    #[serde(default = "default_tab_icon")]
    pub tab_icon: String,
}

/// Row icon files of the Project panel, relative to the editor's working
/// directory (see `kairos_editor_widgets::ui_image`).
///
/// The artwork is extracted from the local Unity/Tuanjie editor resources and
/// downscaled to 64x64 by `scripts/extract_tuanjie_icons.py`; the reason is in
/// `Preferences/Textures/project/README.md`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StyleFields)]
pub struct ProjectIcons {
    pub folder: String,
    pub folder_open: String,
    /// Generic document icon. M2 draws it for every file; the per-type icons
    /// arrive with the `FileTypeRegistry` in M5.
    pub file: String,
}

impl ProjectIcons {
    pub fn dark() -> Self {
        Self {
            folder: String::from("Preferences/Textures/project/folder.png"),
            folder_open: String::from("Preferences/Textures/project/folder_open.png"),
            file: String::from("Preferences/Textures/project/file.png"),
        }
    }

    pub fn light() -> Self {
        Self {
            folder: String::from("Preferences/Textures/project/light/folder.png"),
            folder_open: String::from("Preferences/Textures/project/light/folder_open.png"),
            file: String::from("Preferences/Textures/project/light/file.png"),
        }
    }

    /// Every path, in deterministic order, for preloading in `Panel::init`.
    pub fn all(&self) -> [&str; 3] {
        [&self.folder, &self.folder_open, &self.file]
    }
}

fn default_tab_icon() -> String {
    String::from("Preferences/Textures/tab_icons/project.png")
}

impl Default for ProjectStyle {
    fn default() -> Self {
        Self::dark()
    }
}

impl ProjectStyle {
    /// The widget-side projection of the tree tokens.
    ///
    /// The widget deliberately does not know the panel's style type (the widgets
    /// crate cannot see domain components), so the panel hands it this narrow
    /// value every pass.
    pub fn tree(&self) -> TreeStyle {
        TreeStyle {
            row_height: self.row_height,
            indent: self.indent,
            expander: self.expander_size,
            icon: self.icon_size,
            text_size: self.row_text_size,
            suffix_size: self.suffix_text_size,
            text: self.row_text.to_egui(),
            suffix: self.suffix_text.to_egui(),
            selected_bg: self.selected_bg.to_egui(),
            selected_radius: self.selected_radius,
            hover_bg: self.hover_bg.to_egui(),
            expander_color: self.expander.to_egui(),
            separator: self.separator.to_egui(),
            separator_width: self.separator_width,
            bg: self.background.to_egui(),
        }
    }
}

impl Style for ProjectStyle {
    const FILE: &'static str = "ProjectStyle.ron";

    /// The dark skin. The tree tokens marked "legacy" are the ones
    /// `ProjectWindowStyle.toml` defined; the rest is measured against the
    /// Console panel (same Unity/Tuanjie 2022.3 skin).
    fn dark() -> Self {
        Self {
            // Legacy [hierachy] tokens.
            row_text: Color::from_srgb8([0xC8, 0xC8, 0xC8, 255]),
            suffix_text: Color::from_srgb8([0x78, 0x78, 0x78, 255]),
            selected_bg: Color::from_srgb8([0x3C, 0x4B, 0x64, 255]),
            selected_radius: 4,
            icon_size: 18.0,
            suffix_text_size: 11.0,

            // Measured against the Console skin.
            background: Color::from_srgb8([0x38, 0x38, 0x38, 255]),
            hover_bg: Color::from_srgb8([0x42, 0x42, 0x42, 255]),
            expander: Color::from_srgb8([0xA8, 0xA8, 0xA8, 255]),
            separator: Color::from_srgb8([0x2B, 0x2B, 0x2B, 255]),
            separator_width: 1.0,
            row_height: 20.0,
            indent: 14.0,
            expander_size: 13.0,
            row_text_size: 12.0,

            toolbar_bg: Color::from_srgb8([0x35, 0x35, 0x35, 255]),
            toolbar_text: Color::from_srgb8([0xB1, 0xB1, 0xB1, 255]),
            toolbar_height: 20.0,
            control_hover: Color::from_srgb8([0x46, 0x46, 0x46, 255]),

            splitter_idle: Color::from_srgb8([0x20, 0x20, 0x20, 255]),
            splitter_hover: Color::from_srgb8([0x46, 0x46, 0x46, 255]),
            splitter_active: Color::from_srgb8([0xB1, 0xB1, 0xB1, 255]),
            splitter_width: 1.0,
            splitter_grab: 3.0,

            header_bg: Color::from_srgb8([0x2F, 0x2F, 0x2F, 255]),
            header_height: 20.0,
            breadcrumb_text: Color::from_srgb8([0xC8, 0xC8, 0xC8, 255]),
            breadcrumb_separator: Color::from_srgb8([0x78, 0x78, 0x78, 255]),
            hint_text: Color::from_srgb8([0x78, 0x78, 0x78, 255]),

            status_bg: Color::from_srgb8([0x2B, 0x2B, 0x2B, 255]),
            status_text: Color::from_srgb8([0xA0, 0xA0, 0xA0, 255]),
            status_height: 18.0,
            status_text_size: 11.0,

            icons: ProjectIcons::dark(),
            tab_icon: default_tab_icon(),
        }
    }

    /// Light-skin starting point. The legacy window only had a dark skin, so
    /// these are the Console light skin's conventions applied to the same tokens
    /// and should be reviewed against a light-skin capture before being called
    /// final.
    fn light() -> Self {
        Self {
            row_text: Color::from_srgb8([0x20, 0x20, 0x20, 255]),
            suffix_text: Color::from_srgb8([0x6A, 0x6A, 0x6A, 255]),
            selected_bg: Color::from_srgb8([0x3B, 0x6E, 0xA5, 255]),
            background: Color::from_srgb8([0xC8, 0xC8, 0xC8, 255]),
            hover_bg: Color::from_srgb8([0xD4, 0xD4, 0xD4, 255]),
            expander: Color::from_srgb8([0x50, 0x50, 0x50, 255]),
            separator: Color::from_srgb8([0xA6, 0xA6, 0xA6, 255]),
            toolbar_bg: Color::from_srgb8([0xC8, 0xC8, 0xC8, 255]),
            toolbar_text: Color::from_srgb8([0x20, 0x20, 0x20, 255]),
            control_hover: Color::from_srgb8([0xB4, 0xB4, 0xB4, 255]),
            splitter_idle: Color::from_srgb8([0xA6, 0xA6, 0xA6, 255]),
            splitter_hover: Color::from_srgb8([0x8A, 0x8A, 0x8A, 255]),
            splitter_active: Color::from_srgb8([0x5A, 0x5A, 0x5A, 255]),
            header_bg: Color::from_srgb8([0xBD, 0xBD, 0xBD, 255]),
            breadcrumb_text: Color::from_srgb8([0x20, 0x20, 0x20, 255]),
            breadcrumb_separator: Color::from_srgb8([0x6A, 0x6A, 0x6A, 255]),
            hint_text: Color::from_srgb8([0x6A, 0x6A, 0x6A, 255]),
            status_bg: Color::from_srgb8([0xB4, 0xB4, 0xB4, 255]),
            status_text: Color::from_srgb8([0x60, 0x60, 0x60, 255]),
            icons: ProjectIcons::light(),
            tab_icon: String::from("Preferences/Textures/tab_icons/light/project.png"),
            ..Self::dark()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kairos_editor_widgets::style::StyleFile;

    fn workspace() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
    }

    #[test]
    fn icon_files_exist_for_both_skins() {
        for style in [ProjectStyle::dark(), ProjectStyle::light()] {
            for path in style.icons.all() {
                assert!(
                    workspace().join(path).is_file(),
                    "missing project row icon {path}"
                );
            }
        }
    }

    #[test]
    fn tab_icon_files_exist_for_both_skins() {
        for style in [ProjectStyle::dark(), ProjectStyle::light()] {
            assert!(
                workspace().join(&style.tab_icon).is_file(),
                "missing tab icon {}",
                style.tab_icon
            );
        }
    }

    /// The committed skin is the documented starting point, so it must carry
    /// every token: adding one means updating `Preferences/Styles/ProjectStyle.ron`
    /// as well (a missing field fails to parse; it does not fall back silently).
    #[test]
    fn the_committed_style_file_parses() {
        let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let file = StyleFile::<ProjectStyle>::load_from(
            &workspace.join(StyleFile::<ProjectStyle>::path()),
        )
        .expect("Preferences/Styles/ProjectStyle.ron parses");

        assert!(file.dark.header_height > 0.0);
        assert!(file.light.header_height > 0.0);
        assert_ne!(
            file.dark.header_bg, file.dark.background,
            "the content header must read as chrome, not as body"
        );
    }

    #[test]
    fn defaults_round_trip_through_ron_as_hex_colors() {
        let dir = std::env::temp_dir().join(format!(
            "kairos-project-style-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let path = dir.join("ProjectStyle.ron");
        let _ = std::fs::remove_dir_all(&dir);

        StyleFile::<ProjectStyle>::defaults()
            .save_to(&path)
            .expect("save style");
        let text = std::fs::read_to_string(&path).expect("read style");
        assert!(text.contains("\"#383838\""), "{text}");
        assert!(!text.contains("Color"), "{text}");

        let loaded = StyleFile::<ProjectStyle>::load_from(&path).expect("load style");
        assert_eq!(loaded.dark, ProjectStyle::dark());
        assert_eq!(loaded.light, ProjectStyle::light());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_tree_projection_carries_the_tree_tokens() {
        let style = ProjectStyle::dark();
        let tree = style.tree();
        assert_eq!(tree.row_height, style.row_height);
        assert_eq!(tree.indent, style.indent);
        assert_eq!(tree.icon, style.icon_size);
        assert_eq!(tree.selected_bg, style.selected_bg.to_egui());
        assert_eq!(tree.selected_radius, style.selected_radius);
        assert_eq!(tree.bg, style.background.to_egui());
    }
}
