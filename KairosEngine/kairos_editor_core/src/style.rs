//! Skin values the Preferences panel edits.
//!
//! A style struct lives with its component (`PreferencesPanelStyle`,
//! `ConsoleStyle`, ...), and the panel that owns it can describe its fields
//! and write one of them (`kairos_editor_component::skin`). Crossing the
//! `Action`/`Effect` boundary needs a *value* type that is plain data: no
//! egui, no component types, and no `Any` downcast. That is this enum.
//!
//! Colors are straight (unmultiplied) sRGBA bytes rather than a color type, so
//! this crate keeps its "no UI dependency" property; the component layer
//! converts to `kairos_color::Color32` for its picker.

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
    /// A color token: straight sRGBA.
    Color([u8; 4]),
}
