//! Intents the UI produces.
//!
//! Actions are plain data: the UI layer pushes them into its output buffer
//! (`kairos_editor_component::UiOut`), and [`crate::reduce`] is the only thing
//! that applies them to [`crate::EditorState`]. Panels never mutate state.
//!
//! Slices stay separate enums (`LayoutAction`, `AppAction`, `StyleAction`,
//! `SharedStyleAction`, later `DocumentAction`, `SelectionAction`,
//! `ViewportAction`, ...) routed by the aggregate [`Action`], so no slice
//! becomes a second global message bus.

use crate::panel::PanelId;
use crate::style::{SharedStyleId, StyleValue};

/// Aggregate UI intent, routed by domain slice.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Layout(LayoutAction),
    /// The application as a whole, not one panel or document.
    App(AppAction),
    /// Panel skins (the Preferences panel's field editor).
    Style(StyleAction),
    /// Shared styles: editor-shell appearance that no panel instance owns.
    SharedStyle(SharedStyleAction),
}

/// Intents produced by panel chrome: tab close buttons, toolbar toggles, ...
#[derive(Clone, Debug, PartialEq)]
pub enum LayoutAction {
    /// Show/hide the panel. Opening docks it in the focused leaf and focuses it.
    TogglePanel { panel: PanelId },
    /// Show and focus the panel. If the dock already has its tab, that tab is
    /// activated and focused; otherwise the panel opens as a floating window.
    /// Idempotent: a second Show only focuses again.
    ShowPanel { panel: PanelId },
    /// Explicit close, used by the tab close button. Unlike `TogglePanel` it is
    /// idempotent and never opens a panel by accident.
    ClosePanel { panel: PanelId },
    /// Bring the panel to the front / focus it. Opens it docked when the dock
    /// has no tab for it.
    FocusPanel { panel: PanelId },
}

/// Intents that outlive every panel: the editor's own lifecycle.
#[derive(Clone, Debug, PartialEq)]
pub enum AppAction {
    /// The user asked to leave the editor (the toolbar's Quit entry).
    ///
    /// A *request*, not a decision: the reducer owns the answer, so the views
    /// never have to know whether it is safe to quit. Today it always is; when
    /// documents land, unsaved work turns this into a confirmation here (the
    /// prototype's `QuitRequested` / `QuitConfirmed` pair) and the toolbar's
    /// call site does not change.
    QuitRequested,
}

/// Intents produced by the Preferences panel's skin editor.
///
/// The styles themselves are not `EditorState`: each panel owns its skin and
/// loads it in `Panel::init`, so these intents describe *host work on a panel
/// instance*, not a state transition. They still travel as actions so the panel
/// stays side-effect free and the app has one command path.
#[derive(Clone, Debug, PartialEq)]
pub enum StyleAction {
    /// One field of one panel's skin changed (live preview while dragging).
    ///
    /// `path` is the dotted field path the panel reported
    /// (`"icons.warning"`, `"toolbar_height"`); the panel decides whether
    /// the path and the value's kind match, and reports `false` if they do not.
    SetField {
        panel: PanelId,
        path: String,
        value: StyleValue,
    },
    /// The interaction that started the edit ended: write the panel's skin file.
    Commit { panel: PanelId },
}

/// Intents produced by the Preferences panel's shared-style pages.
///
/// The toolbar and the color picker are loaded once by the editor shell, not by
/// a panel instance, so their files cannot travel through the panel-skin path
/// above. They still share the same gesture boundary: set while dragging or
/// typing, persist when the interaction ends. Reload re-reads the file so a
/// hand edit can be picked up without restarting the editor.
#[derive(Clone, Debug, PartialEq)]
pub enum SharedStyleAction {
    /// One field of one shared style changed (live preview).
    SetField {
        style: SharedStyleId,
        path: String,
        value: StyleValue,
    },
    /// The interaction ended: write the shared style's RON file.
    Commit { style: SharedStyleId },
    /// Re-read the shared style's file into the live value.
    Reload { style: SharedStyleId },
}
