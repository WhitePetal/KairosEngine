//! # kairos_editor_core
//!
//! The editor's state tree and its one writer: no egui, no window, no IO.
//!
//! This is the production counterpart of the prototype's
//! `editor-core-prototype` crate (`prototypes/editor-ui/crates/editor-core`).
//! It currently carries the layout slice (`LayoutState` + `LayoutAction`) and
//! the app lifecycle slice (quit: `AppAction`/`AppEffect`, no state of its
//! own); document/selection/viewport slices land here as the editor grows, with
//! the prototype's `reduce/` split.
//!
//! Dependency direction (enforced by cargo):
//!
//! ```text
//! kairos_editor_core        (state + actions + reducers, no egui)
//! kairos_editor_widgets     (L1 widget kit, no domain types)
//! kairos_editor_docking     (dock layout, only ids + egui)
//!             \                  |                  /
//!              +--► kairos_editor_component ◄--+
//!                              |
//!                     kairos_editor_app   (composition root + window backend)
//! ```
//!
//! Nothing in this crate may know that egui exists.

#![forbid(unsafe_code)]

pub mod action;
pub mod effect;
pub mod panel;
pub mod reduce;
pub mod state;

pub use action::{Action, AppAction, LayoutAction};
pub use effect::{AppEffect, Effect, LayoutEffect};
pub use panel::{PanelId, PanelInstanceId};
pub use reduce::reduce;
pub use state::{EditorState, LayoutState};
