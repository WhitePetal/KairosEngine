//! # kairos_editor_core
//!
//! The editor's state tree, and nothing else: no egui, no window, no IO.
//!
//! This is the production counterpart of the prototype's
//! `editor-core-prototype` crate (`prototypes/editor-ui/crates/editor-core`).
//! Only the state half is ported for now: the prototype's reducers / effects /
//! headless boundary stay in the prototype until this editor grows real domain
//! logic (project tree, documents, selection). When it does, it lands here.
//!
//! Dependency direction (enforced by cargo):
//!
//! ```text
//! kairos_editor_core        (state + panel ids, no egui)
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

pub mod id;
pub mod state;

pub use id::{PanelId, PanelInstanceId};
pub use state::{EditorState, LayoutState};
