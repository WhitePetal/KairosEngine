//! Intents the UI produces.
//!
//! Actions are plain data: the UI layer pushes them into its output buffer
//! (`kairos_editor_component::UiOut`), and [`crate::reduce`] is the only thing
//! that applies them to [`crate::EditorState`]. Panels never mutate state.
//!
//! Slices stay separate enums (`LayoutAction`, `AppAction`, later
//! `DocumentAction`, `SelectionAction`, `ViewportAction`, ...) routed by the
//! aggregate [`Action`], so no slice becomes a second global message bus.

use crate::panel::PanelId;

/// Aggregate UI intent, routed by domain slice.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Layout(LayoutAction),
    /// The application as a whole, not one panel or document.
    App(AppAction),
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
