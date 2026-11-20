//! Skin values the Preferences panel edits.
//!
//! A style struct lives with its component (`PreferencesPanelStyle`,
//! `ConsoleStyle`, ...), and the panel that owns it can describe its fields
//! and write one of them (`kairos_editor_component::skin`). Crossing the
//! `Action`/`Effect` boundary needs a *value* type that is plain data: no
//! egui, no component types, and no `Any` downcast. That is this enum.
//!
//! Each numeric variant is the field's own type, not a common denominator:
//! a `u64` token stays `u64`, a `f64` stays `f64`. The variant is also the
//! page's type tag, so the component layer does not need a separate editor-kind
//! enum to choose a widget.
//!
//! Colors are the shared linear `kairos_color::Color` value; `Color` and
//! `HdrColor` say whether the field is presented as LDR or HDR. This crate
//! stays free of egui and IO.

/// Stable id of a shared style: appearance owned by the editor shell rather than
/// by one panel instance (the toolbar, the color picker).
///
/// The value is a plain string for the same reason PanelId is: adding a shared
/// style must not change core.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SharedStyleId(pub &'static str);

impl std::fmt::Display for SharedStyleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

/// One structural edit of a list-valued token (a `Vec<T>` field).
///
/// The list's *length* is an ordinary token — writing it through
/// [`StyleValue::ListLen`] resizes the list. These three edits are what a
/// resize cannot express: inserting in the middle, removing an entry that is
/// not the last, and reordering. They travel as values for the same reason
/// `StyleValue` does: the field table lives in the component layer, the host
/// that owns the style lives outside core, and the intent must cross the
/// `Action`/`Effect` boundary as plain data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListEdit {
    /// Insert a default entry at `index` (clamped to the list's length).
    Insert { index: usize },
    /// Remove the entry at `index`; out-of-range is a no-op.
    Remove { index: usize },
    /// Move the entry at `from` so it ends up at `to`, both in the final
    /// list's coordinates. Out-of-range is a no-op.
    Move { from: usize, to: usize },
}

/// One editable value from a panel's or shared style's skin.
///
/// The variant names the exact Rust type of the field. A `u64` style token is
/// [`StyleValue::U64`], not a widened `i64`; a `f64` is
/// [`StyleValue::F64`], not a narrowed `f32`. The page matches the variant
/// and hands the inner value to the matching `NumberField<N>`, so the widget's
/// own `N` bounds are the type range; a separate `StyleField::range` is only a
/// UI scrub limit.
#[derive(Clone, Debug, PartialEq)]
pub enum StyleValue {
    /// A `f32` token (sizes, paddings, text sizes).
    F32(f32),
    /// A `f64` token.
    F64(f64),
    /// An `i8` token.
    I8(i8),
    /// An `i16` token.
    I16(i16),
    /// An `i32` token (offsets and other signed layout values).
    I32(i32),
    /// An `i64` token.
    I64(i64),
    /// An `isize` token.
    Isize(isize),
    /// A `u8` token (corner radii are the common case).
    U8(u8),
    /// A `u16` token.
    U16(u16),
    /// A `u32` token.
    U32(u32),
    /// A `u64` token.
    U64(u64),
    /// A `usize` token.
    Usize(usize),
    /// A boolean token.
    Bool(bool),
    /// A text token: an icon path, a font family.
    Text(String),
    /// A color token: linear light, eight-byte `Color`.
    Color(kairos_color::Color),
    /// A linear-light color whose channels may exceed 1.
    HdrColor(kairos_color::Color),
    /// The length of a `Vec<T>` token; writing it resizes the list.
    ListLen(usize),
    /// The active variant of an enum token, by its stable name (`"First"`).
    ///
    /// A name rather than an index: reordering the variants in Rust must not
    /// silently change what a saved edit meant, and a name the type does not
    /// know is a no-op rather than a wrong variant. The variant's own fields
    /// (when it has any) travel as the ordinary values above, each written by
    /// its dotted path — the enum is a row and a block of rows, not one value.
    /// The enum's names travel as field metadata
    /// (`kairos_editor_component::skin::StyleField::variants`), not in this
    /// value: a write only needs the id.
    Variant(String),
}

impl StyleValue {
    /// Whether this is a numeric token, and therefore drawn by a number box.
    pub fn is_number(&self) -> bool {
        matches!(
            self,
            Self::F32(_)
                | Self::F64(_)
                | Self::I8(_)
                | Self::I16(_)
                | Self::I32(_)
                | Self::I64(_)
                | Self::Isize(_)
                | Self::U8(_)
                | Self::U16(_)
                | Self::U32(_)
                | Self::U64(_)
                | Self::Usize(_)
        )
    }

    /// Whether this is a list node (its value is the entry count).
    pub fn is_list(&self) -> bool {
        matches!(self, Self::ListLen(_))
    }

    /// Whether this is an enum node (its value is the active id).
    pub fn is_variant(&self) -> bool {
        matches!(self, Self::Variant(_))
    }
}
