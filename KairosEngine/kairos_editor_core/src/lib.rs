//! # kairos_editor_core
//!
//! The editor's state tree and its one writer: no egui, no window, no IO.
//!
//! This is the production counterpart of the prototype's
//! `editor-core-prototype` crate (`prototypes/editor-ui/crates/editor-core`).
//! It currently carries the layout slice (`LayoutState` + `LayoutAction`), the
//! project slice ([`ProjectState`] + `ProjectAction`/`ProjectEffect`: the asset
//! workspace's tree, view intent and scan protocol), the app lifecycle slice
//! (quit: `AppAction`/`AppEffect`, no state of its own) and the style slices
//! (panel skins and editor-shell shared styles: `StyleAction`/`StyleEffect` and
//! `SharedStyleAction`/`SharedStyleEffect`, also no state — a skin belongs to the
//! panel or shell value that draws from it);
//! document/selection/viewport slices land here as the editor grows, with the
//! prototype's `reduce/` split.
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
pub mod project;
pub mod reduce;
pub mod state;
pub mod style;

pub use action::{Action, AppAction, LayoutAction, SharedStyleAction, StyleAction};
pub use effect::{AppEffect, Effect, LayoutEffect, SharedStyleEffect, StyleEffect};
pub use panel::{PanelId, PanelInstanceId};
pub use project::{ProjectAction, ProjectEffect, ProjectError, ProjectState};
pub use reduce::reduce;
pub use state::{EditorState, LayoutState};
pub use style::SharedStyleId;
