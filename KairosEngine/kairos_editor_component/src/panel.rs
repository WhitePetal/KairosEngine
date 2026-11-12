//! Panel framework: stable ids, the [`Panel`] trait, the registry and the host
//! that owns live panel instances.
//!
//! # Where a panel is drawn (start here)
//!
//! There is exactly one call chain from the frame loop to a panel body:
//!
//! ```text
//! kairos_editor_app/src/app.rs    EditorApp::ui
//!   -> app sync                   host.sync(&state.layout); dock tab reconcile
//!   -> docker::Docker::show       dock tree + tab chrome + splitter/tab drag
//!     -> viewer::TabViewer::ui    (the dock asks for one tab body)
//!       -> panel/viewer.rs        PanelHostViewer::ui   (id -> live instance)
//!         -> panels/<name>.rs     <dyn Panel>::ui      (panel body only)
//! ```
//!
//! The app never draws a tab bar and never calls a concrete panel: opening a
//! panel is a `LayoutState` change, and the dock asks the host for the content.
//!
//! # Pieces
//!
//! * [`Panel`], [`PanelContext`], [`PanelInstanceId`]: what a panel is (this file).
//! * [`PanelRegistry`]: `PanelId` -> factory registration (`registry` module).
//! * [`PanelHost`]: owns the instances and reconciles them with `LayoutState`
//!   (`host` module); [`PanelHostViewer`] adapts the host to the dock's
//!   `TabViewer` seam (`viewer` module).

mod host;
mod registry;
mod viewer;

pub use host::PanelHost;
pub use registry::{PanelFactory, PanelRegistry};
pub use viewer::PanelHostViewer;

use egui::{Ui, WidgetText};
use kairos_editor_core::{EditorState, PanelId};

// Instance identity is layout vocabulary: `LayoutState` focuses by instance, so
// the type lives in core next to `PanelId`. Re-exported here because this file
// is where the panel framework surfaces it.
pub use kairos_editor_core::PanelInstanceId;

/// What a panel gets while drawing: the concrete editor state, mutable.
///
/// No `Frame`/`UiOut` indirection: panels can read and update editor state
/// directly (that is the intended model for this editor). When a panel needs a
/// command with side effects it will grow into an explicit result instead of a
/// generic headless boundary.
pub struct PanelContext<'a> {
    pub state: &'a mut EditorState,
}

/// One drawable panel body. The host owns the tab/title bar.
pub trait Panel {
    fn kind(&self) -> PanelId;

    /// Title shown in the dock tab bar; may read editor state (document name,
    /// selection, ...), unlike a title cached at construction time.
    fn title(&self, state: &EditorState) -> WidgetText;

    fn ui(&mut self, ui: &mut Ui, ctx: &mut PanelContext);

    fn context_menu(&mut self, _ui: &mut Ui, _ctx: &mut PanelContext) {}

    fn on_rect_changed(&mut self) {}

    /// Whether the dock offers a close entry for this tab.
    fn closeable(&self) -> bool {
        true
    }

    /// Close callback; returning `false` vetoes the close.
    fn on_close(&mut self, _ctx: &mut PanelContext) -> bool {
        true
    }

    fn allowed_in_windows(&self) -> bool {
        true
    }

    fn scroll_bars(&self) -> [bool; 2] {
        [false, true]
    }
}
