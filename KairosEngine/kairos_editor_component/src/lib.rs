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
//! implements, [`PanelDef`]/[`PanelCatalog`] describe the panel set, and
//! [`PanelHost`] owns the live instances and implements the docking crate's
//! `TabViewer`. The app never calls a concrete panel: it hands the host a
//! catalog and a [`PanelContext`], syncs it with `LayoutState`, and lets
//! `Docker::show` draw. No instance exists for a panel that is not open.
//!
//! [`panels`] holds the built-in panels and their ids. Every panel struct
//! carries the `#[panel]` attribute, which declares its id, implements
//! [`PanelRegister`] and submits the factory; the submissions share a
//! link-section table and [`panels::catalog`] collects them once, sorted by id.
//! No central table names a panel's factory, and the app shell does not change.
//!
//! ## About panel
//!
//! `panels::about` is the editor's identity card. Its content is **injected**
//! through [`EditorInfo`]: the composition root knows what the product is
//! called and which version it is, and this crate cannot depend on the app
//! crate, so the identity travels in the [`PanelContext`] the host hands to
//! every factory, and the About factory copies it when the panel opens. A
//! version literal compiled into a component is how the legacy window ended up
//! disagreeing with its own title bar; see
//! `docs/plan/editor-ui-architecture/about-panel.md`.
//!
//! ## Preferences panel and skins
//!
//! `panels::preferences` is the 0.2.0 rewrite of the legacy `PreferencesWindow`:
//! a two-column editor that lists every open panel's `Preferences/Styles/*.ron`
//! tokens as widgets and writes them back. Each style derives `StyleFields`
//! ([`skin`], macro crate `macros/`), so the field table comes from the
//! struct itself: a token and its editor row cannot drift apart, and fields are
//! addressed by dotted path instead of a `Vec` index. A panel exposes its
//! table and accepts writes through [`Panel::skin`],
//! [`Panel::set_skin_field`] and [`Panel::save_skin`]; the app snapshots the
//! live tables into `Frame::skins` while Preferences is open, applies edits to
//! the owning panel (live preview) and persists its file when the interaction
//! ends. See `docs/plan/editor-ui-architecture/preferences-panel.md`.
//!
//! ## Shared styles
//!
//! The toolbar, the color picker and the font list are not panels, so they have
//! no instance to load a skin into. They declare `#[SharedStyle(...)]` next to
//! their `#[derive(StyleFields)]`: the derive emits the id constant, the sealed erasure
//! impl and a [`SharedStyleDef`](shared_style::SharedStyleDef) submission (the
//! same inventory mechanism as panels). The app loads the
//! [`StyleCatalog`](shared_style::StyleCatalog) once at boot into a
//! [`SharedStyles`](shared_style::SharedStyles) registry, and [`Frame::shared`]
//! hands the live values to every panel. Preferences appends their pages after
//! the panel skins and edits them through the separate
//! `SharedStyleAction`/`SharedStyleEffect` path.

#![forbid(unsafe_code)]

// Both macros expand to absolute paths back into this crate; the alias makes
// those paths work inside the crate as well as from dependents.
extern crate self as kairos_editor_component;

/// The seals behind the declaration macros: unreachable outside this crate, so
/// a panel or shared style cannot be registered by hand.
pub(crate) mod __sealed {
    pub trait Sealed {}

    /// The erasure half of `SharedStyle`, implemented only by the
    /// `StyleFields` derive for `#[SharedStyle(...)]`.
    pub trait SharedStyleSealed {
        fn as_any(&self) -> &dyn std::any::Any;
    }
}
pub mod console;
pub mod frame;
pub mod panel;
pub mod panels;
pub mod shared_style;
pub mod skin;
pub mod toolbar;

pub use console::ConsoleLog;
pub use frame::{ConsoleLine, Frame, HostView, UiOut};
// Panel kinds are store vocabulary, not a component concept: re-export the core
// newtype so panels and the app share one id type without a second import.
pub use kairos_editor_core::PanelId;
// The composition root builds the panels' identity, so the type has to be
// reachable from the crate root.
pub use panel::{
    EditorInfo, Panel, PanelCatalog, PanelContext, PanelDef, PanelFactory, PanelHost,
    PanelHostViewer, PanelInstanceId, PanelRegister,
};
// The Preferences editor consumes these; a panel implementation needs
// `StyleFields` (derive) and builds a `PanelSkin` in `Panel::skin`.
pub use skin::{PanelSkin, StyleField, StyleFields as StyleFieldsTrait, StyleKind};

// The panel declaration attribute: id, registration and factory from one item.
// Panels are defined in this crate only (`PanelRegister` is sealed), so the
// attribute is not re-exported publicly either.
pub(crate) use kairos_editor_component_macros::panel;

/// Paths the declaration macros expand to. Not part of the public API.
#[doc(hidden)]
pub mod __macro {
    pub use crate::PanelId;
    pub use crate::panel::{Panel, PanelContext, PanelDef, PanelRegister};
    pub use crate::shared_style::SharedStyleDef;
    pub use inventory;
    pub use kairos_editor_core::SharedStyleId;
}
