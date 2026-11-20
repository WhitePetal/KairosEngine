//! The editor's font list as a shared style.
//!
//! The widget kit owns "hand these files to egui"
//! ([`kairos_editor_widgets::fonts`]); this is the skin the editor shell owns:
//! `Preferences/Styles/EditorFonts.ron` (dark + light), a page in the
//! Preferences panel, and the [`preload`](SharedStyle::preload) hook that
//! installs it into the egui context.
//!
//! `path` is a `String`, not a `PathBuf`: the field editor writes text
//! tokens, and the widget's install structure ([`FontsConfig`]) is projected at
//! the call site. That is the same split the color picker uses between its
//! serde skin and its runtime tokens.
//!
//! The list's **shape** (how many fonts, in which order) still comes from the
//! file: the page edits the rows a file has, while adding or removing a row
//! needs a bespoke list editor — the same limit as the Profiler's palette.

use std::path::PathBuf;

use egui::Context;
use kairos_editor_widgets::fonts::{FontFile, FontPriority, FontsConfig};
use kairos_editor_widgets::style::Style;
use serde::{Deserialize, Serialize};

use crate::shared_style::SharedStyle;
use crate::skin::StyleFields;

/// The editor's font list.
///
/// Field order is the Preferences page order. Each entry is a token group
/// (`proportional.0.name`), so the page edits the rows the file has.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, StyleFields)]
#[SharedStyle(FONTS = "fonts", title = "Fonts")]
pub struct EditorFonts {
    /// Proportional family, front of the fallback list first.
    pub proportional: Vec<FontEntry>,
    /// Monospace family; empty means egui's own default is used.
    pub monospace: Vec<FontEntry>,
}

/// One font file in the list.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, StyleFields)]
pub struct FontEntry {
    /// Family name registered with egui.
    pub name: String,
    /// Path relative to the working directory.
    pub path: String,
    /// Front of the family vs. a fallback appended after it.
    ///
    /// Not a field-editor token yet: the page has no choice control, so the
    /// order stays a file-authored decision until it grows one.
    #[style(skip)]
    pub priority: FontPriority,
}

impl Style for EditorFonts {
    const FILE: &'static str = "EditorFonts.ron";

    fn dark() -> Self {
        Self {
            proportional: vec![
                FontEntry {
                    name: "Inter".to_owned(),
                    path: "Preferences/Fonts/Inter-Regular.otf".to_owned(),
                    priority: FontPriority::First,
                },
                FontEntry {
                    name: "Inter Medium".to_owned(),
                    path: "Preferences/Fonts/Inter-Medium.otf".to_owned(),
                    priority: FontPriority::Push,
                },
            ],
            monospace: Vec::new(),
        }
    }

    /// The same list: fonts have no theme axis yet, but the skin is a pair, so
    /// a theme-specific list can be authored in the file without code.
    fn light() -> Self {
        Self::dark()
    }
}

impl EditorFonts {
    /// Project the serde skin onto the widget's install structure.
    pub fn config(&self) -> FontsConfig {
        FontsConfig {
            proportional: self.proportional.iter().map(FontEntry::file).collect(),
            monospace: self.monospace.iter().map(FontEntry::file).collect(),
        }
    }
}

impl FontEntry {
    fn file(&self) -> FontFile {
        FontFile {
            name: self.name.clone(),
            path: PathBuf::from(&self.path),
            priority: self.priority,
        }
    }
}

impl SharedStyle for EditorFonts {
    /// Install the list into the context. Runs after the boot load, after a
    /// reload, and after a committed edit; a missing file is logged and leaves
    /// egui on its previous fonts rather than failing the frame.
    fn preload(&self, ctx: &Context) {
        if let Err(error) = self.config().install(ctx) {
            kairos_log::warn!(target: "kairos_editor", "shared style fonts: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use kairos_editor_widgets::style::StyleFile;

    use super::*;
    use crate::skin::{StyleFields, StyleKind};

    /// The code defaults point at files that ship with the workspace.
    #[test]
    fn the_default_list_points_at_runtime_font_files() {
        let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let fonts = EditorFonts::dark();
        assert!(!fonts.proportional.is_empty());
        for entry in &fonts.proportional {
            assert!(
                workspace.join(&entry.path).is_file(),
                "missing font {}",
                entry.path
            );
        }
    }

    /// The page edits names and paths; the install order stays file-authored
    /// until there is a choice control.
    #[test]
    fn the_page_edits_names_and_paths_but_not_the_install_order() {
        let fields = EditorFonts::dark().style_fields();
        let paths: Vec<&str> = fields.iter().map(|field| field.path.as_str()).collect();

        assert_eq!(
            paths,
            [
                "proportional.0.name",
                "proportional.0.path",
                "proportional.1.name",
                "proportional.1.path",
            ]
        );
        assert!(fields.iter().all(|field| field.kind == StyleKind::Text));
        assert!(
            !fields.iter().any(|field| field.path.ends_with("priority")),
            "the install order is not a token yet"
        );
    }

    /// The projection keeps the file's order and the two priorities.
    #[test]
    fn the_projection_keeps_the_install_order() {
        let config = EditorFonts::dark().config();

        assert_eq!(config.proportional[0].name, "Inter");
        assert_eq!(config.proportional[0].priority, FontPriority::First);
        assert_eq!(config.proportional[1].priority, FontPriority::Push);
        assert_eq!(
            config.proportional[0].path,
            PathBuf::from("Preferences/Fonts/Inter-Regular.otf")
        );
    }

    /// The checked-in reference skin and the code defaults are one skin.
    #[test]
    fn shipped_reference_skin_matches_the_code_defaults() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../Preferences/Styles/EditorFonts.ron");
        let file =
            StyleFile::<EditorFonts>::load_from(&path).expect("the shipped font list parses");
        let defaults = StyleFile::<EditorFonts>::defaults();

        assert_eq!(file.dark, defaults.dark, "dark skin drifted from code");
        assert_eq!(file.light, defaults.light, "light skin drifted from code");
    }
}
