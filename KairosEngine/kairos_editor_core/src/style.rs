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

/// One editable value from a panel's skin.
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
