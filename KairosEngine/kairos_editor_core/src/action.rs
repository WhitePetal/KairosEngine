//! Intents the UI produces.
//!
//! Actions are plain data: the UI layer pushes them into its output buffer
//! (`kairos_editor_component::UiOut`), and [`crate::reduce`] is the only thing
//! that applies them to [`crate::EditorState`]. Panels never mutate state.
//!
//! Slices stay separate enums (`LayoutAction`, `AppAction`, `ProjectAction`,
//! `StyleAction`, `SharedStyleAction`, later `DocumentAction`,
//! `SelectionAction`, `ViewportAction`, ...) routed by the aggregate [`Action`],
//! so no slice becomes a second global message bus.

use crate::panel::PanelId;
use crate::project::ProjectAction;
use crate::style::{SharedStyleId, Skin};

/// Aggregate UI intent, routed by domain slice.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Layout(LayoutAction),
    /// The application as a whole, not one panel or document.
    App(AppAction),
    /// The project panel's own intents: scans (M0), view changes (M2), file
    /// operations (M3). Completions produced by host work travel the same way,
    /// so the reducer has exactly one entry point.
    Project(ProjectAction),
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
///
/// A skin edit is one whole value, never a token: the page draws a typed copy of
/// the panel's style and hands the copy back when it changed.
#[derive(Clone, Debug, PartialEq)]
pub enum StyleAction {
    /// Replace the panel's whole live skin with the typed copy the editor page
    /// drew. One value, no paths: the page owns the copy and hands it back.
    Replace { panel: PanelId, skin: Skin },
    /// Replace the panel's live skin with its code defaults for the active
    /// theme, then write the file. One click, one write: this is the factory
    /// reset, not a field edit followed by a commit.
    Reset { panel: PanelId },
    /// The interaction that started the edit ended: write the panel's skin file.
    Commit { panel: PanelId },
}

/// Intents produced by the Preferences panel's shared-style pages.
///
/// The toolbar and the color picker are loaded once by the editor shell, not by
/// a panel instance, so their files cannot travel through the panel-skin path
/// above. They still share the same gesture boundary: replace the live value
/// while drawing, persist when the interaction ends. Reset replaces the live
/// value with the code defaults and writes it in one go.
#[derive(Clone, Debug, PartialEq)]
pub enum SharedStyleAction {
    /// Replace the shared style's whole live value with the page's typed copy.
    Replace { style: SharedStyleId, skin: Skin },
    /// Replace the shared style's live value with its code defaults for the
    /// active theme, then write the file.
    Reset { style: SharedStyleId },
    /// The interaction ended: write the shared style's RON file.
    Commit { style: SharedStyleId },
}
