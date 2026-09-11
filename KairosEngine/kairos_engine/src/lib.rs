extern crate self as kairos_engine;

pub mod math;

pub mod kairos_dialog;

pub mod log;

pub mod kairos_editor;
pub mod kairos_game;
pub mod kairos_paths;
pub mod kairos_settings;
pub mod kairos_ui;

// The asset core is its own crate (`kairos_asset`); re-export it under the name
// engine code uses, so `crate::asset::…` resolves like `crate::graphics` /
// `kairos_graphics`.
pub use kairos_asset as asset;

pub mod audio;
pub mod inputs;
pub mod spatial;

// The physics subsystem is its own crate (`kairos_physics`); re-export it under
// the name engine code already uses, so `crate::physics::…` keeps resolving
// exactly like `crate::graphics` / `kairos_graphics`.
pub use kairos_physics as physics;

// The graphics subsystem is its own crate (`kairos_graphics`); re-export it
// under the name engine code already uses, so `crate::graphics::…` keeps
// resolving exactly like `crate::time` / `kairos_time`.
pub use kairos_graphics as graphics;

// Bevy parity: re-export the time crate so `crate::time` resolves like
// `bevy::time` (bevy re-exports `bevy_time` under the same name).
pub use kairos_time as time;
