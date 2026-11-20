//! Panel skins as editable fields: what the Preferences panel draws.
//!
//! # Why this exists
//!
//! A 0.2.0 component owns its appearance twice: as a `*Style` struct (code
//! defaults) and as a RON file under `Preferences/Styles/` (`dark` + `light`),
//! loaded in `Panel::init`. The legacy `PreferencesWindow` edited those values by
//! asking every window for a `Vec<StyleField>` and pushing the page back each
//! frame; the defects (index-based readback, a write per frame, pages keyed by
//! `drawers` position) are catalogued in
//! `docs/plan/editor-ui-architecture/preferences-panel.md`.
//!
//! This module keeps the *capability* and replaces the mechanism:
//!
//! * `#[derive(StyleFields)]` (`macros/`) generates the field table from the
//!   struct itself, so a token and its editor row cannot drift apart;
//! * fields are addressed by a dotted **path** (`"icons.warning"`), not by an
//!   index into a `Vec`, so reordering a struct is not a runtime panic;
//! * a write is typed (`StyleValue`) and returns whether it matched, so a
//!   wrong kind is a no-op, not a silent mismatch.
//!
//! Live apply and persistence are the panel's job: `Panel::set_skin_field`
//! writes its live style (the next pass redraws from it) and
//! `Panel::save_skin` writes the file. This module only describes and
//! persists; it does not know about the Preferences panel.

use kairos_color::Color;
use kairos_editor_core::{ListEdit, PanelId, StyleValue};
use kairos_editor_widgets::color_picker::ColorProperty;
use kairos_editor_widgets::style::{Style, StyleFile};

/// Derive macro: generates [`StyleNode`] and [`StyleFields`] for a style struct.
///
/// With `#[SharedStyle(...)]` it also declares the type as a
/// [`SharedStyle`](crate::shared_style::SharedStyle): id constant, sealed
/// erasure and the `SharedStyleDef` submission are generated, and the type
/// implements only `preload`.
pub use kairos_editor_component_macros::StyleFields;

/// The editor a field needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StyleKind {
    /// A fractional token: drag value with a magnitude-based speed.
    Float,
    /// A whole-number token. `max` is the field type's own limit (`255` for a
    /// `u8` corner radius), not a UI preference.
    Integer { max: u32 },
    /// An on/off token.
    Toggle,
    /// A text token (icon path, font family).
    Text,
    /// A straight sRGBA color.
    Color,
    /// A linear-light HDR color (channels may exceed 1).
    HdrColor,
    /// A list of tokens (`Vec<T>`) rather than one token.
    ///
    /// The field's value is the list's length as an [`StyleValue::Integer`],
    /// so writing the field resizes the list. The editor draws it as a list
    /// header (name, count, add/remove) instead of a row with a value box.
    List,
}

/// Largest list the editor will grow a `Vec<T>` token to.
///
/// A list token is a hand-authored style value (a font fallback chain, the
/// profiler's category palette), not a user document, so a few dozen entries
/// is the whole range that exists. The cap keeps a dragged or typed count from
/// allocating an unbounded number of default entries; a file may still author
/// more, it just cannot be grown past this from the editor.
pub const LIST_MAX: u32 = 64;

/// One editable token of a panel's skin.
#[derive(Clone, Debug, PartialEq)]
pub struct StyleField {
    /// Dotted path from the style root (`"icons.warning"`, `"toolbar_height"`).
    pub path: String,
    /// Humanized leaf name (`"Toolbar height"`).
    pub name: String,
    /// The value as it is now.
    pub value: StyleValue,
    /// How to edit it.
    pub kind: StyleKind,
}

impl StyleField {
    /// First path segment (`"icons"`), or `""` for a root-level token.
    pub fn group(&self) -> &str {
        self.path.split_once('.').map_or("", |(group, _)| group)
    }

    /// The token's name inside its group (`"warning"` for `"icons.warning"`).
    pub fn short_path(&self) -> &str {
        self.path.rsplit('.').next().unwrap_or(&self.path)
    }
}

/// A style struct that can describe and write its own fields.
///
/// Implement it with `#[derive(StyleFields)]`; hand-written impls are only
/// needed for a type whose tokens are not fields of the struct.
pub trait StyleFields {
    /// Every editable token, in declaration order (groups in place).
    fn style_fields(&self) -> Vec<StyleField>;

    /// Write one token by dotted path. Returns whether the path named a field
    /// of the matching kind; a mismatched value leaves the style untouched.
    fn set_style_field(&mut self, path: &str, value: &StyleValue) -> bool;

    /// Insert, remove or move one entry of the list token at `path`.
    ///
    /// Returns whether `path` named a list (and the edit was in range). The
    /// default is "this style has no lists"; the derive generates the routing
    /// and `Vec<T>` implements the edit itself.
    fn edit_style_list(&mut self, _path: &str, _edit: ListEdit) -> bool {
        false
    }
}

/// One node of a style's field tree: a leaf token or a group of them.
///
/// The derive and the leaf impls below are the only implementations. `path`
/// is this node's own dotted path (`""` at the root; `"icons"` for a nested
/// group), so a parent does not need to know whether a field is a group.
pub trait StyleNode {
    /// Append this node's tokens (a leaf appends one, a group appends its
    /// children) to `out`.
    fn collect(&self, path: &str, out: &mut Vec<StyleField>);

    /// Write one leaf below this node. `path` is the remainder after the
    /// node's own key: empty means this node itself is the leaf.
    fn write(&mut self, path: &str, value: &StyleValue) -> bool;

    /// Apply one structural list edit below this node, with the same
    /// remainder-path convention as [`StyleNode::write`].
    ///
    /// The default is "no list here"; `Vec<T>` implements it and a derived
    /// struct routes it like a write.
    fn edit_list(&mut self, _path: &str, _edit: ListEdit) -> bool {
        false
    }
}

/// One panel's editable skin, as the Preferences panel sees it.
#[derive(Clone, Debug, PartialEq)]
pub struct PanelSkin {
    pub panel: PanelId,
    /// Page heading: the panel's own `Panel::title` for a frame, so a skin page
    /// is always named exactly like the tab it belongs to.
    pub title: String,
    /// File name under `Preferences/Styles/`, shown in the page header so the
    /// user knows which file the edits are saved to.
    pub file: &'static str,
    pub fields: Vec<StyleField>,
}

/// Persist one panel's live style into its RON file.
///
/// The panel keeps only the skin it draws with. The other half is read back
/// from the file first, so editing the dark skin does not throw away a
/// hand-edited light one; a missing file is seeded from the code defaults.
pub fn save_skin<S: Style>(style: &S, theme: egui::Theme) -> Result<(), String> {
    let path = StyleFile::<S>::path();
    let mut file = if path.is_file() {
        StyleFile::<S>::load_from(&path)?
    } else {
        StyleFile::<S>::defaults()
    };
    match theme {
        egui::Theme::Light => file.light = style.clone(),
        egui::Theme::Dark => file.dark = style.clone(),
    }
    file.save_to(&path)
}

/// `toolbar_bg` → `"Toolbar bg"`: the editor label for a token.
///
/// The legacy window showed `&'static str` names it had to repeat by hand
/// ("default width"); here the name comes from the field, so a token cannot be
/// shown under the wrong label.
pub fn humanize(path: &str) -> String {
    let leaf = path.rsplit('.').next().unwrap_or(path);
    let mut name = String::with_capacity(leaf.len());
    for (index, word) in leaf.split('_').filter(|word| !word.is_empty()).enumerate() {
        if index > 0 {
            name.push(' ');
        }
        if index == 0 {
            // Sentence case: only the first word is capitalized, because these
            // labels sit in a dense form where Title Case on every word reads
            // like an acronym.
            let mut chars = word.chars();
            if let Some(first) = chars.next() {
                name.extend(first.to_uppercase());
                name.push_str(chars.as_str());
            }
        } else {
            name.push_str(word);
        }
    }
    name
}

macro_rules! float_leaf {
    ($ty:ty) => {
        impl StyleNode for $ty {
            fn collect(&self, path: &str, out: &mut Vec<StyleField>) {
                out.push(StyleField {
                    path: path.to_owned(),
                    name: humanize(path),
                    value: StyleValue::Float(*self as f32),
                    kind: StyleKind::Float,
                });
            }

            fn write(&mut self, path: &str, value: &StyleValue) -> bool {
                match value {
                    StyleValue::Float(value) if path.is_empty() => {
                        *self = *value as $ty;
                        true
                    }
                    _ => false,
                }
            }
        }
    };
}

macro_rules! integer_leaf {
    ($ty:ty, $max:expr) => {
        impl StyleNode for $ty {
            fn collect(&self, path: &str, out: &mut Vec<StyleField>) {
                out.push(StyleField {
                    path: path.to_owned(),
                    name: humanize(path),
                    value: StyleValue::Integer(*self as u32),
                    kind: StyleKind::Integer { max: $max },
                });
            }

            fn write(&mut self, path: &str, value: &StyleValue) -> bool {
                match value {
                    StyleValue::Integer(value) if path.is_empty() => {
                        *self = (*value).min($max) as $ty;
                        true
                    }
                    _ => false,
                }
            }
        }
    };
}

float_leaf!(f32);
integer_leaf!(u8, u8::MAX as u32);
integer_leaf!(u16, u16::MAX as u32);
integer_leaf!(u32, u32::MAX);
integer_leaf!(usize, u32::MAX);

impl StyleNode for bool {
    fn collect(&self, path: &str, out: &mut Vec<StyleField>) {
        out.push(StyleField {
            path: path.to_owned(),
            name: humanize(path),
            value: StyleValue::Toggle(*self),
            kind: StyleKind::Toggle,
        });
    }

    fn write(&mut self, path: &str, value: &StyleValue) -> bool {
        match value {
            StyleValue::Toggle(value) if path.is_empty() => {
                *self = *value;
                true
            }
            _ => false,
        }
    }
}

impl StyleNode for String {
    fn collect(&self, path: &str, out: &mut Vec<StyleField>) {
        out.push(StyleField {
            path: path.to_owned(),
            name: humanize(path),
            value: StyleValue::Text(self.clone()),
            kind: StyleKind::Text,
        });
    }

    fn write(&mut self, path: &str, value: &StyleValue) -> bool {
        match value {
            StyleValue::Text(value) if path.is_empty() => {
                *self = value.clone();
                true
            }
            _ => false,
        }
    }
}

/// A list of tokens (the Profiler's category palette, a font fallback chain).
///
/// The list itself is a field: `collect` emits it before its entries, so an
/// *empty* list still exists in the field table (it has a header and a count,
/// which is what lets the editor add the first entry). Its value is the length;
/// writing an integer resizes the list, growing with `T::default()` entries.
/// `edit_list` covers the changes a resize cannot express.
impl<T: StyleNode + Default> StyleNode for Vec<T> {
    fn collect(&self, path: &str, out: &mut Vec<StyleField>) {
        out.push(StyleField {
            path: path.to_owned(),
            name: humanize(path),
            value: StyleValue::Integer(self.len() as u32),
            kind: StyleKind::List,
        });
        for (index, item) in self.iter().enumerate() {
            let child = if path.is_empty() {
                format!("{index}")
            } else {
                format!("{path}.{index}")
            };
            item.collect(&child, out);
        }
    }

    fn write(&mut self, path: &str, value: &StyleValue) -> bool {
        if path.is_empty() {
            // The list node itself: the value is its length.
            let StyleValue::Integer(len) = value else {
                return false;
            };
            if *len > LIST_MAX {
                return false;
            }
            self.resize_with(*len as usize, T::default);
            return true;
        }
        let (head, tail) = match path.split_once('.') {
            Some((head, tail)) => (head, tail),
            None => (path, ""),
        };
        let Ok(index) = head.parse::<usize>() else {
            return false;
        };
        self.get_mut(index)
            .is_some_and(|item| item.write(tail, value))
    }

    fn edit_list(&mut self, path: &str, edit: ListEdit) -> bool {
        let (head, tail) = match path.split_once('.') {
            Some((head, tail)) => (head, tail),
            None => (path, ""),
        };
        if head.is_empty() && tail.is_empty() {
            return match edit {
                ListEdit::Insert { index } => {
                    let at = index.min(self.len());
                    self.insert(at, T::default());
                    true
                }
                ListEdit::Remove { index } => {
                    if index >= self.len() {
                        return false;
                    }
                    self.remove(index);
                    true
                }
                ListEdit::Move { from, to } => {
                    if from >= self.len() || to >= self.len() {
                        return false;
                    }
                    if from != to {
                        let item = self.remove(from);
                        self.insert(to, item);
                    }
                    true
                }
            };
        }
        let Ok(index) = head.parse::<usize>() else {
            return false;
        };
        self.get_mut(index)
            .is_some_and(|item| item.edit_list(tail, edit))
    }
}

impl StyleNode for Color {
    fn collect(&self, path: &str, out: &mut Vec<StyleField>) {
        out.push(StyleField {
            path: path.to_owned(),
            name: humanize(path),
            value: StyleValue::Color(*self),
            kind: StyleKind::Color,
        });
    }

    fn write(&mut self, path: &str, value: &StyleValue) -> bool {
        match value {
            StyleValue::Color(color) if path.is_empty() => {
                *self = *color;
                true
            }
            _ => false,
        }
    }
}

impl StyleNode for ColorProperty {
    fn collect(&self, path: &str, out: &mut Vec<StyleField>) {
        out.push(StyleField {
            path: path.to_owned(),
            name: humanize(path),
            value: StyleValue::Color(self.color),
            kind: if self.usage.hdr {
                StyleKind::HdrColor
            } else {
                StyleKind::Color
            },
        });
    }

    fn write(&mut self, path: &str, value: &StyleValue) -> bool {
        match value {
            StyleValue::Color(color) if path.is_empty() => {
                self.color = *color;
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize, StyleFields)]
    struct TestIcons {
        warning: String,
    }

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize, StyleFields)]
    struct TestStyle {
        height: f32,
        radius: u8,
        enabled: bool,
        background: Color,
        #[style(skip)]
        cache: u32,
        icons: TestIcons,
    }

    fn style() -> TestStyle {
        TestStyle {
            height: 20.0,
            radius: 3,
            enabled: true,
            background: Color::from_srgb8([1, 2, 3, 255]),
            cache: 7,
            icons: TestIcons {
                warning: "w.png".to_owned(),
            },
        }
    }

    #[test]
    fn a_derived_style_lists_its_tokens_in_declaration_order() {
        let fields = style().style_fields();
        let paths: Vec<&str> = fields.iter().map(|field| field.path.as_str()).collect();

        assert_eq!(
            paths,
            vec!["height", "radius", "enabled", "background", "icons.warning"],
            "nested groups are dotted; #[style(skip)] stays out"
        );
        assert_eq!(fields[0].name, "Height");
        assert_eq!(fields[0].kind, StyleKind::Float);
        assert_eq!(fields[1].kind, StyleKind::Integer { max: 255 });
        assert_eq!(fields[3].kind, StyleKind::Color);
        assert_eq!(fields[4].group(), "icons");
        assert_eq!(fields[4].short_path(), "warning");
    }

    #[test]
    fn setting_by_path_requires_the_kind_to_match() {
        let mut style = style();

        assert!(style.set_style_field("height", &StyleValue::Float(32.0)));
        assert_eq!(style.height, 32.0);

        assert!(style.set_style_field("icons.warning", &StyleValue::Text("x.png".into())));
        assert_eq!(style.icons.warning, "x.png");

        assert!(!style.set_style_field("height", &StyleValue::Toggle(true)));
        assert!(!style.set_style_field("missing", &StyleValue::Float(1.0)));
        assert!(!style.set_style_field("height.nested", &StyleValue::Float(1.0)));
        assert_eq!(style.height, 32.0, "a mismatched write changes nothing");
    }

    #[test]
    fn integer_tokens_clamp_to_their_type() {
        let mut style = style();

        assert!(style.set_style_field("radius", &StyleValue::Integer(400)));
        assert_eq!(style.radius, u8::MAX);
    }

    #[test]
    fn colors_round_trip_as_straight_srgba() {
        let mut style = style();

        let color = Color::from_srgb8([9, 8, 7, 128]);
        assert!(style.set_style_field("background", &StyleValue::Color(color)));
        assert_eq!(style.background, color);
        assert_eq!(style.style_fields()[3].value, StyleValue::Color(color));
    }

    #[derive(Clone, Debug, PartialEq, StyleFields)]
    struct HdrStyle {
        glow: ColorProperty,
    }

    #[test]
    fn hdr_colors_are_their_own_kind() {
        let mut style = HdrStyle {
            glow: ColorProperty::hdr(Color::new(4.0, 1.5, 0.25, 1.0)),
        };
        let fields = style.style_fields();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].kind, StyleKind::HdrColor);
        assert_eq!(
            fields[0].value,
            StyleValue::Color(Color::new(4.0, 1.5, 0.25, 1.0))
        );

        assert!(style.set_style_field("glow", &StyleValue::Color(Color::new(2.0, 0.5, 0.0, 0.5))));
        assert_eq!(
            style.glow,
            ColorProperty::hdr(Color::new(2.0, 0.5, 0.0, 0.5))
        );
    }

    #[test]
    fn the_label_is_derived_from_the_field_name() {
        assert_eq!(humanize("toolbar_bg"), "Toolbar bg");
        assert_eq!(humanize("icons.warning"), "Warning");
        assert_eq!(humanize("x"), "X");
    }
}
