//! Generic docking/tab layout for the Kairos editor.
//!
//! The crate is deliberately small and dependency-light: it knows about
//! rectangles, tab trees, drag/drop and egui, but nothing about the editor.
//!
//! # Model
//!
//! * [`docker::DockState`] stores the layout: surfaces, a binary split tree,
//!   tab order, active tab, split fractions. Its generic `Id` is a stable tab
//!   identity supplied by the app. The layout can therefore be serialized by
//!   the app without touching tab objects.
//! * [`docker::Docker`] is the single drawing/interaction entry:
//!   `Docker::new(id, &mut state).show(ui, &mut viewer)`.
//! * [`viewer::TabViewer`] is the seam back into the app: title, body, context
//!   menu, close/allow-window policies. One implementation (usually the
//!   panel host) resolves ids to real panels.
//!
//! This is the layer that replaces the prototype's fixed four-slot
//! `PanelHost` drawing; panel identity, ownership and close semantics stay in
//! the app, so the dock tree is only layout data.

pub mod docker;
pub mod drag;
pub mod split;
pub mod styles;
pub mod surface;
pub mod translations;
pub mod tree;
pub mod viewer;
