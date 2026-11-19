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
//! of growing one flat "god effect" enum. Panel skins and editor-shell shared
//! styles are separate slices because they have different owners.

use crate::PanelId;
use crate::style::{SharedStyleId, StyleValue};

/// One unit of host work returned by `reduce`.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    Layout(LayoutEffect),
    /// Lifecycle work: the host owns the window and the loop, so only it can
    /// act on "the user wants to leave".
    App(AppEffect),
    /// Skin work: a panel's style lives in the panel instance and its file is
    /// written by the app, so both are outside core.
    Style(StyleEffect),
    /// Shared-style work: the editor shell owns these styles, not a panel.
    SharedStyle(SharedStyleEffect),
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

/// Lifecycle work the app executes against the window backend.
///
/// Core decides *that* the editor should go away; how a window closes, and
/// which cleanup runs on the way out, belongs to `kairos_editor_app` and the
/// engine's loop (`Layer::on_exit`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppEffect {
    /// Close the editor's window so the loop shuts down.
    Quit,
}

/// Skin work owned by the style slice.
///
/// A panel's skin is a host resource (it lives in the panel instance and is
/// persisted to `Preferences/Styles/*.ron`), so the reducer routes these
/// intents to the app instead of writing state.
#[derive(Clone, Debug, PartialEq)]
pub enum StyleEffect {
    /// Write one field on the panel's live style; the panel redraws from it on
    /// the next pass, which is what makes the edit feel immediate.
    SetField {
        panel: PanelId,
        path: String,
        value: StyleValue,
    },
    /// Persist the panel's live style to its RON file.
    Persist { panel: PanelId },
}

/// Host work for the editor's shared styles.
///
/// These are the same three steps as a panel skin, but the value lives in the
/// app's shared-style registry instead of a panel instance.
#[derive(Clone, Debug, PartialEq)]
pub enum SharedStyleEffect {
    /// Write one field on the live shared style.
    SetField {
        style: SharedStyleId,
        path: String,
        value: StyleValue,
    },
    /// Persist the live shared style to its RON file.
    Persist { style: SharedStyleId },
    /// Re-read the shared style's file into the live value.
    Reload { style: SharedStyleId },
}
