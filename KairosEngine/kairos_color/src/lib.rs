//! The shared color type for the Kairos engine and editor.
//!
//! [`Color32`] is a straight (unmultiplied) sRGBA color stored as four `u8`
//! channels, matching how colors are authored in TOML/RON and passed through
//! the engine API. It parses and formats `#RRGGBB` / `#RRGGBBAA` strings (the
//! leading `#` is optional and hex digits are case-insensitive), and its serde
//! impls use that form so style files stay human-editable:
//!
//! ```ron
//! ( toolbar_bg: "#353535", row_selected: "#27537C", ... )
//! ```
//!
//! The crate deliberately stays UI-agnostic: conversions to `egui::Color32` or
//! `syntect::highlighting::Color` live in the modules that consume them (the
//! editor), not here. That is what keeps the engine/editor color type free of
//! UI-library dependencies.
//!
//! Extracted from `kairos_engine::math::color` as a standalone crate;
//! `kairos_engine` re-exports [`Color32`] under `kairos_engine::math::Color32`
//! for existing engine and editor code.

mod color;

pub use color::{Color32, Color32ParseError};

#[cfg(test)]
mod tests;
