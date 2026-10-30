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

#![forbid(unsafe_code)]

pub mod frame;
pub mod inspector_panel;
pub mod project_panel;

pub use frame::{Frame, UiOut};
pub use inspector_panel::{
    actions_from_toolbar as inspector_actions_from_toolbar, set_material_action, InspectorPanel,
    InspectorPanelOutput,
};
pub use project_panel::{
    actions_from_toolbar as project_actions_from_toolbar, actions_from_tree as project_actions_from_tree,
    folder_paths, open_action, project_nodes, row_id, tree_rows, ProjectNode, ProjectPanel,
    ProjectPanelOutput, ProjectViewState,
};
