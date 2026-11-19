//! Engine math: the `kairos_math` types plus the shared color types.
//!
//! Color lives in its own crate (`kairos_color`) so UI crates can depend on
//! the color types without pulling in the engine; this re-export keeps
//! `kairos_engine::math::Color` working for existing engine/editor code.

pub use kairos_color::{Color, ColorParseError, ColorSpace, ColorUsage};
pub use kairos_math::*;
