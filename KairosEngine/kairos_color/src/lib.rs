//! The shared color types for the Kairos engine and editor.
//!
//! Color is the canonical color: four half-float channels in linear light,
//! straight (unmultiplied) alpha, and an eight-byte layout. RGB is not clamped,
//! so emissive values may exceed white. HDR versus LDR is field policy
//! (ColorUsage), not a storage variant.
//!
//! The crate deliberately stays UI-agnostic: conversions to egui::Color32 or
//! syntect::highlighting::Color live in the modules that consume them (the
//! editor), not here. That is what keeps the engine/editor color type free of
//! UI-library dependencies.
//!
//! Extracted from kairos_engine::math::color as a standalone crate;
//! kairos_engine re-exports the types under kairos_engine::math for existing
//! engine and editor code.

mod linear;

pub use linear::{Color, ColorParseError, ColorSpace, ColorUsage, linear_to_srgb, srgb_to_linear};

#[cfg(test)]
mod tests;
