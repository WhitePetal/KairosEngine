//! Panel framework: stable ids, the [`Panel`] trait, the registry and the host
//! that owns live panel instances.
//!
//! # Where a panel is drawn (start here)
//!
//! There is exactly one call chain from the frame loop to a panel body:
//!
//! ```text
//! kairos_editor_app/src/app.rs   EditorApp::ui
//!   -> app sync                  host.sync(&state.layout, ctx); dock tab reconcile
//!   -> docker::Docker::show      tabs/splitters/drag; asks the viewer per tab
//!     -> panel/viewer.rs         PanelHostViewer        (id -> live instance)
//!       -> panels/<name>.rs      <dyn Panel>::ui(ui, &Frame, &mut UiOut)
//!   -> app pump                  reduce(state, action) for every action
//! ```
//!
//! Panels never mutate state: they read [`Frame`] and push [`Action`]s into
//! [`UiOut`]. The app drains that buffer after the last UI pass and applies the
//! actions through `kairos_editor_core::reduce` -- the only writer of the state
//! tree. Closing a tab goes through the same path: the dock removes the tab,
//! the viewer emits `LayoutAction::ClosePanel`, and the reducer closes the
//! layout (the next `PanelHost::sync` then drops the instance).
//!
//! # Pieces
//!
//! * [`Panel`], [`Frame`], [`UiOut`], [`PanelInstanceId`]: what a panel is.
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

use egui::{Context, Ui, WidgetText};
use kairos_editor_core::PanelId;

use crate::frame::{Frame, UiOut};

// Instance identity is layout vocabulary: `LayoutState` focuses by instance, so
// the type lives in core next to `PanelId`. Re-exported here because this file
// is where the panel framework surfaces it.
pub use kairos_editor_core::PanelInstanceId;

/// One drawable panel body. The host owns the tab/title bar; the panel draws the
/// body only.
///
/// Implementations are owned by [`PanelHost`] and live across frames. They may
/// keep UI-only view state (search text, expanded paths, ...) but never domain
/// state: reads go through [`Frame`], writes go through [`UiOut`] as [`Action`]s.
pub trait Panel {
    /// Panel kind, matching the `PanelId` the factory was registered under.
    fn kind(&self) -> PanelId;

    /// Tab-header icon: path to a runtime image file, relative to the editor's
    /// working directory. Each panel returns its own artwork (Console from
    /// `ConsoleStyle::tab_icon`; the not-yet-styled demo panels from a
    /// module-local const), so a panel stays the single owner of its tab
    /// identity — the same seam also covers per-instance icons such as a
    /// document tab showing its file-type icon.
    ///
    /// `init` should preload the path with
    /// `kairos_editor_widgets::ui_image::preload`; `None` draws a text-only
    /// tab. The dock never sees file paths: `PanelHostViewer` converts the
    /// path to an `egui::ImageSource`.
    fn icon(&self) -> Option<&str> {
        None
    }

    /// Title shown in the dock tab bar and in floating-window title bars.
    fn title(&self, frame: &Frame) -> WidgetText;

    /// Called once by the host right after the instance is created, before the
    /// first draw. Panels load their own UI resources here (icon files, cached
    /// textures) instead of doing it in `ui` or at application startup.
    fn init(&mut self, _ctx: &Context) {}

    /// Draw the panel body. Only the active tab of a leaf is drawn.
    fn ui(&mut self, ui: &mut Ui, frame: &Frame, out: &mut UiOut);

    /// Extra context-menu entries for this panel (drawn by the dock). Keep this
    /// side-effect free as well: push actions into `out`.
    fn context_menu(&mut self, _ui: &mut Ui, _frame: &Frame, _out: &mut UiOut) {}

    /// Called when the rect available to the panel body changes (dock resize,
    /// tab re-dock, window resize).
    fn on_rect_changed(&mut self) {}

    /// Whether the dock offers a close entry for this tab.
    fn closeable(&self) -> bool {
        true
    }

    /// Close veto, called before the viewer emits `LayoutAction::ClosePanel`.
    ///
    /// Returning `false` keeps the panel open (the view should then explain why
    /// through normal UI/state, not by blocking the reducer). The panel may push
    /// actions of its own (e.g. open a confirm modal) into `out`.
    fn on_close(&mut self, _frame: &Frame, _out: &mut UiOut) -> bool {
        true
    }

    /// Whether the dock may detach this panel into a floating window.
    fn allowed_in_windows(&self) -> bool {
        true
    }

    /// `[horizontal, vertical]` scroll bars for the panel body.
    fn scroll_bars(&self) -> [bool; 2] {
        [false, true]
    }
}
