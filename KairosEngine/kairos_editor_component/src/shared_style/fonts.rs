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
use kairos_color::Color;
use kairos_editor_widgets::fonts::{FontFile, FontPriority, FontsConfig, TestEnumStruct};
use kairos_editor_widgets::style::Style;
use serde::{Deserialize, Serialize};

use crate::shared_style::SharedStyle;
use crate::skin::StyleFields;

/// The editor's font list.
///
/// Field order is the Preferences page order. Each entry is a token group
/// (proportional.0.name), so the page edits the rows the file has.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, StyleFields)]
#[SharedStyle(FONTS = "fonts", title = "Fonts")]
pub struct EditorFonts {
    /// Proportional family, front of the fallback list first.
    pub proportional: Vec<FontEntry>,
    /// Monospace family; empty means egui's own default is used.
    pub monospace: Vec<FontEntry>,
}

/// One font file in the list.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, StyleFields)]
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
/// The payload is a single unnamed field, so it is transparent: the struct's
/// fields are the variant's payload rows, directly under the enum row, with no
/// header for the tuple field. The widget type keeps the same shape;
/// [EditorFontPriority::priority] maps one onto the other.
#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Serialize,
    Deserialize,
    StyleFields,
    strum::EnumIter,
    strum::Display,
    strum::VariantNames,
)]
pub enum EditorFontPriority {
    /// Goes in front of its family, so it draws the editor's text.
    #[default]
    First,
    /// Appends to its family, so it is a fallback for glyphs the front lacks.
    ///
    /// The tuple payload is transparent: its struct fields are this enum's own
    /// payload rows, with no named header between the menu row and them.
    Push(EditorTestEnumStruct),
}

/// The style-token mirror of the widget crate's TestEnumStruct.
///
/// The widget's runtime type is not a style token (it lives below this crate),
/// so the page edits this struct and [EditorFontPriority::priority] converts.
/// Every field is an ordinary token kind, which is what makes a struct payload
/// display under the variant row.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, StyleFields)]
pub struct EditorTestEnumStruct {
    pub fvalue: f32,
    pub ivalue: i32,
    pub svalue: String,
    pub cvalue: Color,
}

impl EditorFontPriority {
    /// Project the token onto the widget's runtime priority.
    pub fn priority(&self) -> FontPriority {
        match self {
            Self::First => FontPriority::First,
            Self::Push(payload) => FontPriority::Push(TestEnumStruct {
                fvalue: payload.fvalue,
                ivalue: payload.ivalue,
                svalue: payload.svalue.clone(),
                cvalue: payload.cvalue,
            }),
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
                    priority: EditorFontPriority::Push(EditorTestEnumStruct::default()),
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

    use kairos_editor_core::{ListEdit, StyleValue};

    use super::*;
    use crate::skin::{LIST_MAX, StyleFields};

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

    /// The page edits names, paths, priorities and the enum payload; each list
    /// node is a token of its own, so the header can add, remove and reorder
    /// entries.
    #[test]
    fn the_page_edits_names_paths_the_list_and_the_priority_payload() {
        let fields = EditorFonts::dark().style_fields();
        let paths: Vec<&str> = fields.iter().map(|field| field.path.as_str()).collect();

        assert_eq!(
            paths,
            [
                "proportional",
                "proportional.0.name",
                "proportional.0.path",
                "proportional.0.priority",
                "proportional.1.name",
                "proportional.1.path",
                "proportional.1.priority",
                "proportional.1.priority.fvalue",
                "proportional.1.priority.ivalue",
                "proportional.1.priority.svalue",
                "proportional.1.priority.cvalue",
                "monospace",
            ]
        );
        assert_eq!(fields[0].value, StyleValue::ListLen(2));
        assert_eq!(
            fields[11].value,
            StyleValue::ListLen(0),
            "an empty list still has a node, which is what gives it a header"
        );
        let priority = &fields[6];
        assert_eq!(priority.path, "proportional.1.priority");
        assert_eq!(priority.name, "Priority");
        assert_eq!(priority.value, StyleValue::Variant("Push".to_owned()));
        let variants: &[&str] = <EditorFontPriority as crate::skin::StyleEnum>::variants();
        assert_eq!(
            priority.variants,
            Some(variants),
            "the enum row carries the two install orders"
        );
        assert_eq!(variants, ["First", "Push"].as_slice());
    }

    /// The install order is an editable token now: writing the variant changes
    /// the font list (and therefore the projection the installer reads).
    #[test]
    fn the_install_order_is_written_as_a_variant() {
        let mut fonts = EditorFonts::dark();
        assert_eq!(
            fonts.proportional[1].priority,
            EditorFontPriority::Push(EditorTestEnumStruct::default())
        );

        assert!(fonts.set_style_field(
            "proportional.1.priority",
            &StyleValue::Variant("First".to_owned())
        ));
        assert_eq!(fonts.proportional[1].priority, EditorFontPriority::First);
        assert_eq!(fonts.config().proportional[1].priority, FontPriority::First);

        assert!(
            !fonts.set_style_field(
                "proportional.1.priority",
                &StyleValue::Variant("Sideways".to_owned())
            ),
            "an unknown variant is a no-op"
        );
        assert_eq!(fonts.proportional[1].priority, EditorFontPriority::First);
        assert!(
            !fonts.set_style_field("proportional.1.priority", &StyleValue::Text("First".into())),
            "the exact type has to match"
        );
    }

    /// A payload field is an ordinary token: writing its dotted path edits the
    /// active variant's struct, which is what the page draws as rows.
    #[test]
    fn the_priority_payload_fields_are_writable_tokens() {
        let mut fonts = EditorFonts::dark();
        assert!(fonts.set_style_field("proportional.1.priority.fvalue", &StyleValue::F32(2.5),));
        assert!(fonts.set_style_field(
            "proportional.1.priority.svalue",
            &StyleValue::Text("payload".to_owned()),
        ));
        assert!(fonts.set_style_field("proportional.1.priority.ivalue", &StyleValue::I32(-3),));
        let EditorFontPriority::Push(payload) = &fonts.proportional[1].priority else {
            panic!("entry 1 is Push");
        };
        assert_eq!(payload.fvalue, 2.5);
        assert_eq!(payload.ivalue, -3, "the payload integer is signed");
        assert_eq!(payload.svalue, "payload");
        assert!(
            !fonts.set_style_field("proportional.0.priority.fvalue", &StyleValue::F32(2.5),),
            "First has no payload: the path does not exist"
        );
    }

    /// The list node's value is the entry count: writing it resizes, and the
    /// structural edits cover what a resize cannot express.
    #[test]
    fn the_list_token_resizes_and_reorders() {
        let mut fonts = EditorFonts::dark();
        let entries = fonts.proportional.len();

        assert!(fonts.set_style_field("proportional", &StyleValue::ListLen(entries + 1)));
        assert_eq!(fonts.proportional.len(), entries + 1);
        assert_eq!(
            fonts.proportional[entries],
            FontEntry::default(),
            "growth appends a blank entry"
        );
        assert!(fonts.set_style_field("proportional", &StyleValue::ListLen(entries)));
        assert_eq!(fonts.proportional.len(), entries);
        assert!(
            !fonts.set_style_field("proportional", &StyleValue::ListLen(LIST_MAX as usize + 1)),
            "the editor's cap does not grow an unbounded list"
        );

        assert!(fonts.edit_style_list("proportional", ListEdit::Insert { index: 1 }));
        assert_eq!(fonts.proportional[1], FontEntry::default());
        let first = fonts.proportional[0].clone();
        assert!(fonts.edit_style_list("proportional", ListEdit::Move { from: 0, to: 1 }));
        assert_eq!(fonts.proportional[1], first);
        assert!(fonts.edit_style_list("proportional", ListEdit::Remove { index: 1 }));
        assert_eq!(fonts.proportional.len(), entries);
        assert!(!fonts.edit_style_list("proportional", ListEdit::Remove { index: entries }));
        assert!(!fonts.edit_style_list("monospace", ListEdit::Remove { index: 0 }));
    }

    /// The projection keeps the file's order and the two priorities.
    #[test]
    fn the_projection_keeps_the_install_order() {
        let config = EditorFonts::dark().config();

        assert_eq!(config.proportional[0].name, "Inter");
        assert_eq!(config.proportional[0].priority, FontPriority::First);
        assert_eq!(
            config.proportional[1].priority,
            FontPriority::Push(TestEnumStruct::default())
        );
        assert_eq!(
            config.proportional[0].path,
            PathBuf::from("Preferences/Fonts/Inter-Regular.otf")
        );
    }

    /// The enum's variant order and labels come from strum, which is what the
    /// typed EnumField reads; the style table still uses the same declaration.
    #[test]
    fn the_style_enum_lists_its_variants_with_strum() {
        use strum::IntoEnumIterator;

        let labels: Vec<String> = EditorFontPriority::iter()
            .map(|value| value.to_string())
            .collect();
        assert_eq!(labels, ["First", "Push"]);
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
