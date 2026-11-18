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
//! kairos_editor_core        (state + actions + reducers, no egui)
//! kairos_editor_widgets     (L1 widget kit, no domain types)
//! kairos_editor_docking     (dock layout, only ids + egui)
//! kairos_log                (runtime log: levels, terminal/file sinks)
//! kairos_profile            (capture + structured frames, no egui)
//!             \                  |                  /
//!              +--► kairos_editor_component ◄--+
//!                              |
//!                     kairos_editor_app   (composition root + window backend)
//! ```
//!
//! ## Frame / UiOut (ported from the prototype)
//!
//! Panels read a read-only [`Frame`] (state + host resources) and push
//! [`Action`]s into [`UiOut`]; they never mutate the state tree. The app drains
//! the buffer after the last UI pass and applies every action through
//! `kairos_editor_core::reduce` (the only writer). Host-owned display resources
//! (structured console lines, future viewport textures) travel through
//! [`HostView`]; they are not state.
//!
//! ## Console log
//!
//! The console history is a host resource: the app owns a [`ConsoleLog`], attaches
//! it to `kairos_log` as a sink, and the Console panel reads it through
//! [`HostView::console_lines`]. It is deliberately not a separate
//! `kairos_editor_log` crate — one consumer lives beside the panel, and a second
//! one would justify extracting it.
//!
//! ## Profiler panel
//!
//! The capture is runtime infrastructure, not editor state. The engine owns the
//! [`kairos_profile::Profiler`] value, the host loop opens and closes the
//! frame in `Engine::update`, and [`HostView::profiler`] exposes
//! `engine.profiler()` to the panel. The Profiler
//! panel reads a [`kairos_profile::ProfileSession`], caches the aggregated
//! hierarchy per revision/frame/thread, and can toggle recording, clear the
//! capture, or save it. Saving, the file formats and the paths all live in
//! `kairos_profile`; the panel only displays what it is handed. It draws no
//! charts yet: the drawing phase is planned separately.
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
//! ## About panel
//!
//! `panels::about` is the editor's identity card. Its content is **injected**
//! through [`EditorInfo`]: the composition root knows what the product is
//! called and which version it is, and this crate cannot depend on the app
//! crate, so the identity travels through
//! [`panels::register_default`] and is captured by the About factory. A
//! version literal compiled into a component is how the legacy window ended up
//! disagreeing with its own title bar; see
//! `docs/plan/editor-ui-architecture/about-panel.md`.

#![forbid(unsafe_code)]

pub mod console;
pub mod frame;
pub mod panel;
pub mod panels;
pub mod toolbar;

pub use console::ConsoleLog;
pub use frame::{ConsoleLine, Frame, HostView, UiOut};
// Panel kinds are store vocabulary, not a component concept: re-export the core
// newtype so panels and the app share one id type without a second import.
pub use kairos_editor_core::PanelId;
// The composition root builds the About panel's view model, so the type has to
// be reachable from the crate root.
pub use panels::EditorInfo;
pub use panel::{Panel, PanelFactory, PanelHost, PanelHostViewer, PanelInstanceId, PanelRegistry};
