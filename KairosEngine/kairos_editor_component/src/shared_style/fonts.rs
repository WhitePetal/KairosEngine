//! The editor's font list as a shared style.
//!
//! The widget kit owns "hand these files to egui"
//! ([kairos_editor_widgets::fonts]); this is the skin the editor shell owns:
//! Preferences/Styles/EditorFonts.ron (dark + light), a page in the
//! Preferences panel, and the preload hook that installs it into the egui
//! context.
//!
//! path is a String, not a PathBuf: the field editor writes text tokens, and
//! the widget's install structure (FontsConfig) is projected at the call site.
//! That is the same split the color picker uses between its serde skin and its
//! runtime tokens.
//!
//! The list's shape is editable too: the list node carries the entry count, so
//! the page's list header can add, remove and reorder entries (the order is
//! meaningful: the first First entry registers the family, later ones are
//! fallbacks).
//!
//! priority is an enum token drawn as the enum field. The runtime
//! [FontPriority] lives in the widget crate and cannot take this crate's
//! derive, so the token is a parallel enum here, [EditorFontPriority], exactly
//! like [EditorFonts] is the serde skin of the runtime [FontsConfig]. The
//! derive gives the page the variant list, the payload rows and the write
//! path; [FontEntry::file] projects it onto the runtime value.

use std::path::PathBuf;

use egui::Context;
use kairos_editor_widgets::fonts::{FontFile, FontPriority, FontsConfig};
use kairos_editor_widgets::style::Style;
use serde::{Deserialize, Serialize};

use crate::shared_style::SharedStyle;
use crate::style_row::StyleRow;

/// The editor's font list.
///
/// Field order is the Preferences page order. Each entry is a token group
/// (proportional.0.name), so the page edits the rows the file has.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, StyleRow)]
#[SharedStyle(FONTS = "fonts", title = "Fonts")]
pub struct EditorFonts {
    /// Proportional family, front of the fallback list first.
    pub proportional: Vec<FontEntry>,
    /// Monospace family; empty means egui's own default is used.
    pub monospace: Vec<FontEntry>,
}

/// One font file in the list.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, StyleRow)]
pub struct FontEntry {
    /// Family name registered with egui.
    pub name: String,
    /// Path relative to the working directory.
    pub path: String,
    /// Front of the family vs. a fallback appended after it.
    pub priority: EditorFontPriority,
}

/// The priority token: the serde skin of the widget's runtime [FontPriority].
///
/// Two unit variants, so the page draws one menu row and no payload;
/// [EditorFontPriority::priority] maps one onto the other.
#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Serialize,
    Deserialize,
    StyleRow,
    strum::EnumIter,
    strum::Display,
    strum::VariantNames,
)]
pub enum EditorFontPriority {
    /// Goes in front of its family, so it draws the editor's text.
    #[default]
    First,
    /// Appends to its family, so it is a fallback for glyphs the front lacks.
    Push,
}

impl EditorFontPriority {
    /// Project the token onto the widget's runtime priority.
    pub fn priority(&self) -> FontPriority {
        match self {
            Self::First => FontPriority::First,
            Self::Push => FontPriority::Push,
        }
    }
}

impl FontEntry {
    fn file(&self) -> FontFile {
        FontFile {
            name: self.name.clone(),
            path: PathBuf::from(&self.path),
            priority: self.priority.priority(),
        }
    }
}

impl Style for EditorFonts {
    const FILE: &'static str = "EditorFonts.ron";

    fn dark() -> Self {
        Self {
            proportional: vec![
                FontEntry {
                    name: "Inter".to_owned(),
                    path: "Preferences/Fonts/Inter-Regular.otf".to_owned(),
                    priority: EditorFontPriority::First,
                },
                FontEntry {
                    name: "Inter Medium".to_owned(),
                    path: "Preferences/Fonts/Inter-Medium.otf".to_owned(),
                    priority: EditorFontPriority::Push,
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

impl SharedStyle for EditorFonts {
    /// Install the list into the context. Runs after the boot load, after a
    /// reset, and after a committed edit; a missing file is logged and leaves
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

    /// The enum's variant order and labels come from strum, which is what the
    /// typed `VariantMenu` reads.
    #[test]
    fn the_style_enum_lists_its_variants_with_strum() {
        use strum::IntoEnumIterator;

        let labels: Vec<String> = EditorFontPriority::iter()
            .map(|value| value.to_string())
            .collect();
        assert_eq!(labels, ["First", "Push"]);
    }

    /// The page draws the two lists and the priority menu. A list is one row
    /// (its header and count) followed by one block per entry, so the walk
    /// names every token without a field table.
    #[test]
    fn the_page_walks_the_two_lists_and_the_priority_menu() {
        use kairos_color::Color;

        use crate::style_row::{RowSink, StyleRow};

        /// Records one path per row; a group opens a path prefix.
        #[derive(Default)]
        struct Recorder {
            rows: Vec<String>,
            path: Vec<String>,
        }

        impl RowSink for Recorder {
            fn number<N: kairos_editor_widgets::field::Number>(
                &mut self,
                name: &str,
                _value: &mut N,
            ) {
                self.rows.push(self.join(name));
            }

            fn text(&mut self, name: &str, _value: &mut String) {
                self.rows.push(self.join(name));
            }

            fn toggle(&mut self, name: &str, _value: &mut bool) {
                self.rows.push(self.join(name));
            }

            fn color(&mut self, name: &str, _value: &mut Color) {
                self.rows.push(self.join(name));
            }

            fn color_property(
                &mut self,
                name: &str,
                _value: &mut kairos_editor_widgets::color_picker::ColorProperty,
            ) {
                self.rows.push(self.join(name));
            }

            fn variant<E: crate::skin::StyleEnum>(&mut self, name: &str, _value: &mut E) {
                self.rows.push(self.join(name));
            }

            fn group(&mut self, name: &str, body: impl FnOnce(&mut Self)) {
                if name.is_empty() {
                    return body(self);
                }
                self.path.push(name.to_owned());
                body(self);
                self.path.pop();
            }

            fn payload(&mut self, name: &str, body: impl FnOnce(&mut Self)) {
                self.path.push(name.to_owned());
                body(self);
                self.path.pop();
            }

            fn list<T: StyleRow + Default>(&mut self, name: &str, values: &mut Vec<T>) {
                self.rows.push(self.join(name));
                self.path.push(name.to_owned());
                for (index, item) in values.iter_mut().enumerate() {
                    let index = index.to_string();
                    self.path.push(index);
                    StyleRow::row(item, "", self);
                    self.path.pop();
                }
                self.path.pop();
            }
        }

        impl Recorder {
            fn join(&self, name: &str) -> String {
                let mut path = self.path.join(".");
                if !name.is_empty() {
                    if !path.is_empty() {
                        path.push('.');
                    }
                    path.push_str(name);
                }
                path
            }
        }

        let mut fonts = EditorFonts::dark();
        let mut recorder = Recorder::default();
        fonts.row("", &mut recorder);

        assert_eq!(
            recorder.rows,
            [
                "proportional",
                "proportional.0.name",
                "proportional.0.path",
                "proportional.0.priority",
                "proportional.1.name",
                "proportional.1.path",
                "proportional.1.priority",
                "monospace",
            ]
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
