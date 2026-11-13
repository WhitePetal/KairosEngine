//! Engine math: the `kairos_math` types plus the shared [`Color32`].
//!
//! Color lives in its own crate (`kairos_color`) so UI crates can depend on
//! the color type without pulling in the engine; this re-export keeps
//! `kairos_engine::math::Color32` working for existing engine/editor code.

pub use kairos_color::Color32;
pub use kairos_math::*;
