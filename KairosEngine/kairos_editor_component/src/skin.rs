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
//!   wrong exact type is a no-op, not a silent mismatch.
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

/// Largest list the editor will grow a `Vec<T>` token to.
///
/// A list token is a hand-authored style value (a font fallback chain, the
/// profiler's category palette), not a user document, so a few dozen entries
/// is the whole range that exists. The cap keeps a dragged or typed count from
/// allocating an unbounded number of default entries; a file may still author
/// more, it just cannot be grown past this from the editor.
pub const LIST_MAX: u32 = 64;

/// One editable token of a panel's skin.
///
/// `value` names the field's exact type. `variants` and `range` are editor
/// metadata, not part of the value: `variants` is set only for a
/// [`StyleValue::Variant`], `range` is an optional UI scrub range for a number
/// box. The field's type bounds stay with the concrete `N` of
/// `kairos_editor_widgets::field::NumberField<N>`.
#[derive(Clone, Debug, PartialEq)]
pub struct StyleField {
    /// Dotted path from the style root (`"icons.warning"`, `"toolbar_height"`).
    pub path: String,
    /// Humanized leaf name (`"Toolbar height"`).
    pub name: String,
    /// The value as it is now, tagged with its exact type.
    pub value: StyleValue,
    /// The enum's names, in declaration order, for a [`StyleValue::Variant`].
    pub variants: Option<&'static [&'static str]>,
    /// Optional UI range for a number box; never the field type's bounds.
    pub range: Option<(f64, f64)>,
}

impl StyleField {
    /// A token with no editor-specific metadata.
    pub fn new(path: impl Into<String>, name: impl Into<String>, value: StyleValue) -> Self {
        Self {
            path: path.into(),
            name: name.into(),
            value,
            variants: None,
            range: None,
        }
    }

    /// Attach the enum's names to a [`StyleValue::Variant`] row.
    pub fn with_variants(mut self, variants: &'static [&'static str]) -> Self {
        self.variants = Some(variants);
        self
    }

    /// Attach a UI scrub range for this row's number box.
    pub fn with_range(mut self, min: f64, max: f64) -> Self {
        self.range = Some((min, max));
        self
    }

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
    /// of the matching exact type; a mismatched value leaves the style untouched.
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

/// An enum token: its variants, the active one, and how to rebuild a variant.
///
/// `#[derive(StyleFields)]` generates this for an enum, so the field table,
/// the menu labels and the write path cannot drift from the declaration. The
/// enum also derives `strum::VariantNames`: that is the one table of names
/// both a typed `EnumField` and the type-erased Preferences page use. The id
/// written to RON is the Rust variant ident; do not override the strum name
/// (`#[strum(serialize = ...)]`) on a style enum, or the menu and the stored id
/// would diverge.
///
/// A type from another crate cannot take the derive; if a style needs one,
/// declare a parallel enum in this crate and project it onto the runtime type
/// (the font list's `EditorFontPriority` -> `FontPriority` is the example).
///
/// A variant with named fields is a *payload*: [`StyleNode::collect`] appends
/// its fields after the enum's own row, and [`StyleNode::write`] routes a
/// dotted path into the active variant's payload. A single-field tuple variant
/// is transparent: its payload's fields are the enum's fields directly, with no
/// header for the unnamed field. Switching variants rebuilds the new one from
/// `Default`, which is the value semantics of a Rust enum.
///
/// A typed `EnumField` reads the same names and variant order through strum's
/// `Display` and `EnumIter`; a style enum that a typed editor should own also
/// derives those two. The Preferences page does not need them: it uses
/// `VariantMenu`, which edits the style token id against the names carried by
/// [`StyleField::variants`].
pub trait StyleEnum: Sized + strum::VariantNames {
    /// Every variant name, in declaration order.
    fn variants() -> &'static [&'static str] {
        Self::VARIANTS
    }

    /// The active variant's stable id.
    fn variant_id(&self) -> &'static str;

    /// Rebuild the enum as the named variant, with a default payload.
    ///
    /// `None` for an id this enum does not have: an unknown write is a no-op,
    /// exactly like a mismatched [`StyleValue`](kairos_editor_core::StyleValue).
    fn with_variant(id: &str) -> Option<Self>;
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
    ($ty:ty, $variant:ident) => {
        impl StyleNode for $ty {
            fn collect(&self, path: &str, out: &mut Vec<StyleField>) {
                out.push(StyleField::new(
                    path,
                    humanize(path),
                    StyleValue::$variant(*self),
                ));
            }

            fn write(&mut self, path: &str, value: &StyleValue) -> bool {
                match value {
                    StyleValue::$variant(value) if path.is_empty() => {
                        *self = *value;
                        true
                    }
                    _ => false,
                }
            }
        }
    };
}

macro_rules! integer_leaf {
    ($ty:ty, $variant:ident) => {
        impl StyleNode for $ty {
            fn collect(&self, path: &str, out: &mut Vec<StyleField>) {
                out.push(StyleField::new(
                    path,
                    humanize(path),
                    StyleValue::$variant(*self),
                ));
            }

            fn write(&mut self, path: &str, value: &StyleValue) -> bool {
                match value {
                    StyleValue::$variant(value) if path.is_empty() => {
                        *self = *value;
                        true
                    }
                    _ => false,
                }
            }
        }
    };
}

float_leaf!(f32, F32);
float_leaf!(f64, F64);
// Each integer keeps its own variant, so the page can use the concrete
// NumberField<N>: no widened integer and no type-bound clamp here.
integer_leaf!(i8, I8);
integer_leaf!(i16, I16);
integer_leaf!(i32, I32);
integer_leaf!(i64, I64);
integer_leaf!(isize, Isize);
integer_leaf!(u8, U8);
integer_leaf!(u16, U16);
integer_leaf!(u32, U32);
integer_leaf!(u64, U64);
integer_leaf!(usize, Usize);

impl StyleNode for bool {
    fn collect(&self, path: &str, out: &mut Vec<StyleField>) {
        out.push(StyleField::new(
            path,
            humanize(path),
            StyleValue::Bool(*self),
        ));
    }

    fn write(&mut self, path: &str, value: &StyleValue) -> bool {
        match value {
            StyleValue::Bool(value) if path.is_empty() => {
                *self = *value;
                true
            }
            _ => false,
        }
    }
}

impl StyleNode for String {
    fn collect(&self, path: &str, out: &mut Vec<StyleField>) {
        out.push(StyleField::new(
            path,
            humanize(path),
            StyleValue::Text(self.clone()),
        ));
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
/// writing a [`StyleValue::ListLen`] resizes the list, growing with
/// `T::default()` entries. `edit_list` covers the changes a resize cannot
/// express.
impl<T: StyleNode + Default> StyleNode for Vec<T> {
    fn collect(&self, path: &str, out: &mut Vec<StyleField>) {
        out.push(
            StyleField::new(path, humanize(path), StyleValue::ListLen(self.len()))
                .with_range(0.0, LIST_MAX as f64),
        );
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
            let StyleValue::ListLen(len) = value else {
                return false;
            };
            if *len > LIST_MAX as usize {
                return false;
            }
            self.resize_with(*len, T::default);
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
        out.push(StyleField::new(
            path,
            humanize(path),
            StyleValue::Color(*self),
        ));
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
        let value = if self.usage.hdr {
            StyleValue::HdrColor(self.color)
        } else {
            StyleValue::Color(self.color)
        };
        out.push(StyleField::new(path, humanize(path), value));
    }

    fn write(&mut self, path: &str, value: &StyleValue) -> bool {
        match value {
            StyleValue::Color(color) | StyleValue::HdrColor(color) if path.is_empty() => {
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
        assert_eq!(fields[0].value, StyleValue::F32(20.0));
        assert_eq!(fields[1].value, StyleValue::U8(3));
        assert_eq!(
            fields[3].value,
            StyleValue::Color(Color::from_srgb8([1, 2, 3, 255]))
        );
        assert_eq!(fields[4].group(), "icons");
        assert_eq!(fields[4].short_path(), "warning");
    }

    #[test]
    fn setting_by_path_requires_the_exact_type_to_match() {
        let mut style = style();

        assert!(style.set_style_field("height", &StyleValue::F32(32.0)));
        assert_eq!(style.height, 32.0);

        assert!(style.set_style_field("icons.warning", &StyleValue::Text("x.png".into())));
        assert_eq!(style.icons.warning, "x.png");

        assert!(!style.set_style_field("height", &StyleValue::Bool(true)));
        assert!(!style.set_style_field("missing", &StyleValue::F32(1.0)));
        assert!(!style.set_style_field("height.nested", &StyleValue::F32(1.0)));
        assert_eq!(style.height, 32.0, "a mismatched write changes nothing");
    }

    #[test]
    fn integer_tokens_keep_their_exact_width() {
        let mut style = style();

        assert!(style.set_style_field("radius", &StyleValue::U8(255)));
        assert_eq!(style.radius, u8::MAX);
        assert!(
            !style.set_style_field("radius", &StyleValue::U16(255)),
            "a different integer width is not this token's type"
        );
        assert_eq!(style.radius, u8::MAX);
    }

    #[derive(Clone, Debug, PartialEq, StyleFields)]
    struct SignedStyle {
        offset: i32,
    }

    #[derive(Clone, Debug, PartialEq, StyleFields)]
    struct WideStyle {
        big: u64,
        precise: f64,
    }

    #[test]
    fn wide_numbers_keep_their_exact_type() {
        let precise = 0.123_456_789_012_345_67_f64;
        let mut style = WideStyle {
            big: u64::MAX,
            precise,
        };
        let fields = style.style_fields();

        assert_eq!(fields[0].value, StyleValue::U64(u64::MAX));
        assert_eq!(fields[1].value, StyleValue::F64(precise));
        assert!(style.set_style_field("big", &StyleValue::U64(u64::MAX - 1)));
        assert_eq!(style.big, u64::MAX - 1);
    }

    #[test]
    fn signed_integer_tokens_keep_their_exact_type() {
        let mut style = SignedStyle { offset: -5 };
        assert_eq!(style.style_fields()[0].value, StyleValue::I32(-5));

        assert!(style.set_style_field("offset", &StyleValue::I32(-12)));
        assert_eq!(style.offset, -12);

        assert!(
            !style.set_style_field("offset", &StyleValue::I64(i32::MAX as i64 + 1)),
            "a wider integer is not this token's type"
        );
        assert_eq!(style.offset, -12);
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
    fn hdr_colors_are_their_own_variant() {
        let mut style = HdrStyle {
            glow: ColorProperty::hdr(Color::new(4.0, 1.5, 0.25, 1.0)),
        };
        let fields = style.style_fields();
        assert_eq!(fields.len(), 1);
        assert_eq!(
            fields[0].value,
            StyleValue::HdrColor(Color::new(4.0, 1.5, 0.25, 1.0))
        );

        assert!(style.set_style_field(
            "glow",
            &StyleValue::HdrColor(Color::new(2.0, 0.5, 0.0, 0.5))
        ));
        assert_eq!(
            style.glow,
            ColorProperty::hdr(Color::new(2.0, 0.5, 0.0, 0.5))
        );
    }

    /// An enum token is one row plus the active variant's payload. Unit and
    /// struct-like variants share the same expansion: the unit one simply has
    /// no payload rows.
    #[derive(Clone, Debug, PartialEq, StyleFields, strum::VariantNames)]
    enum TestMode {
        Off,
        Soft { radius: f32, samples: u32 },
    }

    #[derive(Clone, Debug, PartialEq, StyleFields)]
    struct EnumStyle {
        mode: TestMode,
    }

    #[test]
    fn an_enum_token_collects_the_active_variants_fields() {
        let style = EnumStyle {
            mode: TestMode::Soft {
                radius: 4.0,
                samples: 2,
            },
        };
        let fields = style.style_fields();
        let paths: Vec<&str> = fields.iter().map(|field| field.path.as_str()).collect();

        assert_eq!(paths, ["mode", "mode.radius", "mode.samples"]);
        assert_eq!(fields[0].name, "Mode");
        assert_eq!(fields[0].value, StyleValue::Variant("Soft".to_owned()));
        assert_eq!(
            fields[0].variants,
            Some(<TestMode as StyleEnum>::variants()),
            "the row carries the variant names as editor metadata"
        );
        assert_eq!(
            <TestMode as StyleEnum>::variants(),
            ["Off", "Soft"].as_slice(),
            "the menu reads the enum's own variant names"
        );
        assert_eq!(fields[1].name, "Radius");
        assert_eq!(fields[1].value, StyleValue::F32(4.0));

        let unit = EnumStyle {
            mode: TestMode::Off,
        };
        let paths: Vec<String> = unit
            .style_fields()
            .into_iter()
            .map(|field| field.path)
            .collect();
        assert_eq!(paths, ["mode"], "a unit variant has no payload rows");
    }

    #[test]
    fn switching_an_enum_rebuilds_the_variant_and_routes_the_payload() {
        let mut style = EnumStyle {
            mode: TestMode::Soft {
                radius: 4.0,
                samples: 2,
            },
        };

        // Re-choosing the active variant keeps its payload untouched.
        assert!(style.set_style_field("mode", &StyleValue::Variant("Soft".to_owned())));
        assert_eq!(
            style.mode,
            TestMode::Soft {
                radius: 4.0,
                samples: 2
            }
        );

        // A different variant is rebuilt from Default, Rust's value semantics.
        assert!(style.set_style_field("mode", &StyleValue::Variant("Off".to_owned())));
        assert_eq!(style.mode, TestMode::Off);
        assert!(style.set_style_field("mode", &StyleValue::Variant("Soft".to_owned())));
        assert_eq!(
            style.mode,
            TestMode::Soft {
                radius: 0.0,
                samples: 0
            }
        );

        // Payload fields are ordinary dotted tokens.
        assert!(style.set_style_field("mode.radius", &StyleValue::F32(2.5)));
        assert_eq!(
            style.mode,
            TestMode::Soft {
                radius: 2.5,
                samples: 0
            }
        );
        assert!(
            !style.set_style_field("mode.samples", &StyleValue::F32(1.0)),
            "the payload field's exact type has to match"
        );
        assert!(
            !style.set_style_field("mode", &StyleValue::Variant("Sideways".to_owned())),
            "an unknown variant is a no-op"
        );
        assert_eq!(
            style.mode,
            TestMode::Soft {
                radius: 2.5,
                samples: 0
            }
        );
    }

    /// A list inside a payload is still a list: its resize and structural
    /// edits route through the active variant.
    #[derive(Clone, Debug, PartialEq, StyleFields, strum::VariantNames)]
    enum TestLayers {
        None,
        Stack { items: Vec<f32> },
    }

    #[derive(Clone, Debug, PartialEq, StyleFields)]
    struct ListEnumStyle {
        layers: TestLayers,
    }

    #[test]
    fn a_list_inside_an_enum_payload_takes_structural_edits() {
        let mut style = ListEnumStyle {
            layers: TestLayers::None,
        };

        assert!(style.set_style_field("layers", &StyleValue::Variant("Stack".to_owned())));
        assert!(style.edit_style_list("layers.items", ListEdit::Insert { index: 0 }));
        assert_eq!(
            style.layers,
            TestLayers::Stack { items: vec![0.0] },
            "the edit reaches the active payload's list"
        );
        assert!(style.set_style_field("layers.items", &StyleValue::ListLen(2)));
        assert_eq!(
            style.layers,
            TestLayers::Stack {
                items: vec![0.0, 0.0]
            }
        );
        assert!(!style.edit_style_list("layers.items", ListEdit::Remove { index: 5 }));
    }

    #[test]
    fn the_label_is_derived_from_the_field_name() {
        assert_eq!(humanize("toolbar_bg"), "Toolbar bg");
        assert_eq!(humanize("icons.warning"), "Warning");
        assert_eq!(humanize("x"), "X");
    }
}
