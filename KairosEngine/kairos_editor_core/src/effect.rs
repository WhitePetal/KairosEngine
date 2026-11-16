//! Host work produced by reducers.
//!
//! Effects are plain data: a reducer decides *what* should happen, the app
//! executes it against host-owned resources (the dock tree, the panel host,
//! later the filesystem). Keeping them out of `EditorState` means the state
//! tree stays the serializable layout intent and a discarded UI pass cannot
//! leave host work queued in it.
//!
//! Effects are partitioned by domain exactly like `Action` (see ADR 0008), so a
//! new feature area adds a variant here and an executor arm in the app instead
//! of growing one flat "god effect" enum.

use crate::PanelId;

/// One unit of host work returned by `reduce`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    Layout(LayoutEffect),
}

/// Placement/focus work owned by the layout slice.
///
/// Only the app knows how a `PanelId` maps to a live instance and where that
/// instance's tab lives; the layout reducer therefore asks for the work instead
/// of touching the dock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutEffect {
    /// Focus the panel's dock tab. When the dock has no tab for it, open the
    /// panel as a floating window instead of splitting an arbitrary leaf.
    ShowPanel { panel: PanelId },
    /// Focus the panel. When the dock has no tab for it, open it docked in the
    /// dock's focused leaf.
    FocusPanel { panel: PanelId },
}
