//! # editor-components-prototype (throwaway)
//!
//! Feature components between the **real core prototype**
//! (`editor-core-prototype`) and the **pure widget kit**
//! (`ui-widgets-prototype`). This crate is the only one that knows domain types.
//!
//! Layering (enforced by Cargo):
//!
//! ```text
//! editor-core-prototype   (pure state + reducers + effects, no egui)
//!        ^
//! editor-components-prototype   (Frame/UiOut + panels: state -> props -> Actions)
//!        ^
//! ui-widgets-prototype    (theme/atoms/compounds, no domain types)
//! ```
//!
//! Components never mutate state, never do IO and never hold domain state:
//! they read `&Frame`, draw widgets, and push `Action`s into `UiOut`.
//!
//! ## Panel framework
//!
//! [`panel`] is the docked workspace layer: [`Panel`] is the trait every panel
//! implements, [`PanelRegistry`] maps `PanelId` kinds to factories/slots, and
//! [`PanelHost`] draws per-slot tab bars plus the active panel body and reports
//! focus/close clicks as `LayoutAction`s. The single drawing entry is
//! `PanelHost::draw` -> `draw_slot` -> `draw_panel_body` -> [`Panel::draw`]
//! (see `panel/host.rs`); `docs/architecture-tour.md` walks the whole path.
//!
//! Built-in panels:
//!
//! * [`ProjectPanel`] (left), [`InspectorPanel`] (right), [`ConsolePanel`]
//!   (bottom), registered by [`PanelRegistry::editor_defaults`];
//! * [`ViewportPanel`] (center), registered by the app shell because it needs
//!   the host's egui texture through [`HostView`].
//!
//! Domain panels and host resources are joined by [`Frame`]: `state` is the
//! read-only store snapshot, `host` exposes GPU/log resources
//! ([`HostView`]). Headless tests use `Frame::new` (`EmptyHost`).

#![forbid(unsafe_code)]

pub mod console_panel;
pub mod frame;
pub mod inspector_panel;
pub mod panel;
pub mod project_panel;
pub mod viewport_panel;

// Panel kinds are store vocabulary, not a components concept: re-export the
// core newtype so panels and the app share one id type without a second import.
pub use editor_core_prototype::PanelId;

pub use console_panel::{ConsolePanel, ConsolePanelOutput};
pub use frame::{EmptyHost, Frame, HostView, UiOut};
pub use inspector_panel::{
    InspectorPanel, InspectorPanelOutput, actions_from_toolbar as inspector_actions_from_toolbar,
    set_material_action,
};
pub use panel::{
    DockSlot, Panel, PanelFactory, PanelHost, PanelHostOutput, PanelInstanceId, PanelRegistry,
};
pub use project_panel::{
    ProjectNode, ProjectPanel, ProjectPanelOutput, ProjectViewState,
    actions_from_toolbar as project_actions_from_toolbar,
    actions_from_tree as project_actions_from_tree, folder_paths, open_action, project_nodes,
    row_id, tree_rows,
};
pub use viewport_panel::ViewportPanel;
