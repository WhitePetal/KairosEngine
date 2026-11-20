//! Skin values the Preferences panel edits.
//!
//! A style struct lives with its component (`PreferencesPanelStyle`,
//! `ConsoleStyle`, ...), and the panel that owns it can describe its fields
//! and write one of them (`kairos_editor_component::skin`). Crossing the
//! `Action`/`Effect` boundary needs a *value* type that is plain data: no
//! egui, no component types, and no `Any` downcast. That is this enum.
//!
//! Colors are the shared linear `kairos_color::Color` value; the field's
//! `StyleKind` (in the component layer) says whether that color is presented
//! as LDR or HDR. This crate stays free of egui and IO.

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
/// The list's *length* is an ordinary integer token — writing it through
/// [`StyleValue::Integer`] resizes the list. These three edits are what a
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
#[derive(Clone, Debug, PartialEq)]
pub enum StyleValue {
    /// A fractional token (sizes, paddings, text sizes).
    Float(f32),
    /// A whole-number token (corner radii are `u8`, counts are `usize`; the
    /// component clamps to the field's own range on write).
    Integer(u32),
    /// A boolean token.
    Toggle(bool),
    /// A text token: an icon path, a font family.
    Text(String),
    /// A color token: linear light, eight-byte `Color`.
    Color(kairos_color::Color),
}
