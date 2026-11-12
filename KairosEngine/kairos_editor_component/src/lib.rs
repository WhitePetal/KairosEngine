//! # kairos_editor_component
//!
//! L2 components: the production counterpart of the prototype's
//! `editor-components-prototype` crate
//! (`prototypes/editor-ui/crates/editor-components`).
//!
//! A component knows domain types (`EditorState`) and composes the pure widget
//! kit (`kairos_editor_widgets`); it never does IO, never owns wgpu types and
//! never depends on the app shell (`kairos_editor_app`).
//!
//! ## Layering (enforced by cargo)
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
//! ## Panel framework
//!
//! [`panel`] is the docked workspace layer: [`Panel`] is the trait every panel
//! implements, [`PanelRegistry`] maps `PanelId` kinds to factories, and
//! [`PanelHost`] owns the live instances and implements the docking crate's
//! `TabViewer`. The app never calls a concrete panel: it registers factories,
//! syncs the host with `LayoutState`, and lets `Docker::show` draw.
//!
//! [`panels`] holds the built-in panels and their ids; adding a panel is one
//! impl in `panels/<name>.rs` plus one registration line in
//! [`panels::register_default`]. The app shell does not change.
//!
//! ## Deliberate deviation from the prototype
//!
//! The prototype's `Frame`/`UiOut` (read-only state + action buffer) is *not*
//! ported: panels get `&mut EditorState` through [`PanelContext`] and mutate it
//! directly. This editor has no headless core, so `Frame`/`UiOut` would be pure
//! indirection (see `docs/plan/editor-ui-architecture/panel-viewport-design.md`
//! §6.1).

#![forbid(unsafe_code)]

pub mod panel;
pub mod panels;

// Panel kinds are store vocabulary, not a component concept: re-export the core
// newtype so panels and the app share one id type without a second import.
pub use kairos_editor_core::PanelId;
pub use panel::{
    Panel, PanelContext, PanelFactory, PanelHost, PanelHostViewer, PanelInstanceId, PanelRegistry,
};
