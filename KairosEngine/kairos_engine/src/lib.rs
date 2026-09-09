extern crate self as kairos_engine;

pub mod math;

pub mod kairos_dialog;

pub mod log;

pub mod kairos_editor;
pub mod kairos_game;
pub mod kairos_paths;
pub mod kairos_settings;
pub mod kairos_ui;

pub mod asset_loader;
pub mod audio;
pub mod graphics;
pub mod inputs;
pub mod physics;
pub mod spatial;

// Bevy parity: re-export the time crate so `crate::time` resolves like
// `bevy::time` (bevy re-exports `bevy_time` under the same name).
pub use kairos_time as time;
