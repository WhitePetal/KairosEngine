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
//! * [`PanelDef`], [`PanelCatalog`], [`PanelContext`]: the panel set, collected
//!   from the link-section table, and what a factory is handed when an instance
//!   is built (`catalog` and `context` modules).
//! * [`PanelHost`]: owns the instances and reconciles them with `LayoutState`
//!   (`host` module); [`PanelHostViewer`] adapts the host to the dock's
//!   `TabViewer` seam (`viewer` module).

mod catalog;
mod context;
mod host;
mod viewer;

pub use catalog::{PanelCatalog, PanelDef, PanelFactory};
pub use context::{EditorInfo, PanelContext};
pub use host::PanelHost;
pub use viewer::PanelHostViewer;

use egui::{Context, Ui, WidgetText};
use kairos_editor_core::PanelId;

use crate::frame::{Frame, UiOut};

// Instance identity is layout vocabulary: `LayoutState` focuses by instance, so
// the type lives in core next to `PanelId`. Re-exported here because this file
// is where the panel framework surfaces it.
pub use kairos_editor_core::PanelInstanceId;

/// Registration half of a panel: its stable id, implemented by `#[panel]`.
///
/// [`Panel`] requires this trait, so a type that implements `Panel` without
/// declaring an id (normally through the `#[panel]` attribute) does not
/// compile. The attribute also submits the [`PanelDef`] to the link-section
/// table.
///
/// **Sealed.** The supertrait is only reachable inside this crate, so a panel
/// cannot implement this by hand, and panels are defined in this crate only.
/// The seal is what makes "implements `Panel` but was never registered"
/// impossible rather than merely discouraged.
pub trait PanelRegister: crate::__sealed::Sealed {
    /// Stable kind id: the key in `LayoutState` and persisted layouts.
    fn panel_id(&self) -> PanelId;
}

/// One drawable panel body. The host owns the tab/title bar; the panel draws the
/// body only.
///
/// Implementations are owned by [`PanelHost`] and live across frames. They may
/// keep UI-only view state (search text, expanded paths, ...) but never domain
/// state: reads go through [`Frame`], writes go through [`UiOut`] as [`Action`]s.
pub trait Panel: PanelRegister {
    /// Panel kind, matching the `PanelId` the factory was registered under.
    ///
    /// Provided: the id is whatever [`PanelRegister::panel_id`] returns, so a
    /// panel never restates it.
    fn kind(&self) -> PanelId {
        self.panel_id()
    }

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

    /// This panel's live style as a self-drawing page value. `None` when the
    /// panel has no editable skin.
    ///
    /// The value is borrowed, not copied: the app asks for a copy only when it
    /// (re)builds the Preferences page snapshot or an owner was reset, never per
    /// frame. The Preferences page is the only caller. `#[panel]` generates this
    /// method for panels declared with `skin = StyleType`; it is the one seam the
    /// page needs from a panel that owns a style.
    fn skin_value(&self) -> Option<&dyn crate::style_row::StylePage> {
        None
    }

    /// The `Preferences/Styles/*.ron` file this panel's style is persisted to,
    /// or `None` when the panel has no editable skin.
    ///
    /// Separate from [`Self::skin_value`] because the file is a constant of the
    /// style type while the value belongs to the live instance. `#[panel]`
    /// generates both for `skin = StyleType`.
    fn skin_file(&self) -> Option<&'static str> {
        None
    }

    /// A typed page for this panel's live skin: the style draws itself, and the
    /// page hands a new copy back through `StyleAction::Replace`. `None` when the
    /// panel has no editable skin.
    ///
    /// The page heading is `self.title(frame)`: a skin page is named like the
    /// tab, so there is no second title to keep in sync. The app only asks while
    /// Preferences is open; it takes the snapshot once and then copies a page
    /// again only when the owner's value changed outside the page (a reset).
    fn skin_page(&self, frame: &Frame) -> Option<crate::skin::PageSkin> {
        let meta = self.skin_meta(frame)?;
        Some(crate::skin::PageSkin {
            id: meta.id,
            title: meta.title,
            file: meta.file,
            page: self.skin_value()?.clone_box(),
        })
    }

    /// The page without its value: what the page list and the heading show.
    ///
    /// Cheap by construction (no style copy), so the app can rebuild the page
    /// list every frame and compare it with the snapshot it holds.
    fn skin_meta(&self, frame: &Frame) -> Option<crate::skin::PageMeta> {
        let file = self.skin_file()?;
        self.skin_value()?;
        Some(crate::skin::PageMeta {
            id: crate::skin::SkinId::Panel(self.kind()),
            title: self.title(frame).text().to_owned(),
            file,
        })
    }

    /// Replace this panel's live skin with a whole value the page handed back.
    ///
    /// Returns whether the box held this panel's style type. The page draws its
    /// own copy, so this is the one write: no path, no per-token dispatch.
    fn replace_skin(&mut self, _skin: Box<dyn std::any::Any>) -> bool {
        false
    }

    /// Replace this panel's live skin with its code defaults for the active
    /// theme. Returns whether the panel has an editable skin.
    ///
    /// The app follows a successful reset with [`Panel::save_skin`], so the
    /// factory values reach the file in the same click. The default is "this
    /// panel has no skin file".
    fn reset_skin(&mut self, _ctx: &Context) -> bool {
        false
    }

    /// Persist this panel's live style to its skin file.
    ///
    /// Called when the user finishes an edit (pointer release, page change,
    /// panel close). The theme picks which half of the file is replaced.
    fn save_skin(&self, _ctx: &Context) -> Result<(), String> {
        Err(String::from("this panel has no skin file"))
    }

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
