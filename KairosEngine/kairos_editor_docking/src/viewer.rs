//! The extension point between the dock (layout + interaction) and the app
//! (what a tab actually is).
//!
//! `DockState` stores stable, copyable ids only; it never stores the tab
//! contents. `Docker::show` asks a [`TabViewer`] to produce a title, draw a
//! body, answer close requests and so on. The app owns the actual tab/panel
//! objects and resolves each id when the viewer is called.
//!
//! This keeps the crate free of editor/engine types: it depends only on
//! `egui`, and the same `DockState<PanelInstanceId>` can be serialized by the
//! app as layout data.

use std::hash::Hash;

use egui::{ImageSource, Response, Ui, WidgetText};

/// Stable identity of a tab in a [`crate::docker::DockState`].
///
/// The dock only needs to copy, compare and hash ids. Semantic meaning
/// (a panel kind + instance serial, a document id, ...) belongs to the app.
pub trait TabId: Copy + Eq + Hash + std::fmt::Debug {}

impl<T> TabId for T where T: Copy + Eq + Hash + std::fmt::Debug {}

/// Answer to [`TabViewer::on_close`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CloseResponse {
    /// Remove the tab from the dock.
    #[default]
    Close,
    /// Keep the tab and focus it instead.
    Focus,
    /// Keep the tab as it is.
    Ignore,
}

/// What the dock asks the app about a tab.
///
/// Implementations live in the app/UI crate, not here: this crate only knows
/// ids and egui. All methods take `&mut self` so the viewer can keep drawing
/// state or push an action into the surrounding app.
pub trait TabViewer {
    /// The id type stored in [`crate::docker::DockState`].
    type Id: TabId;

    /// Optional image drawn to the left of the tab title.
    ///
    /// The viewer owns the path-to-source conversion (runtime icon files go
    /// through `kairos_editor_widgets::ui_image::file_uri`); the dock only
    /// paints the given egui source and never interprets file paths. Returning
    /// an owned source keeps the trait object-free and lets the dock paint the
    /// tab header without borrowing the viewer across later callbacks.
    fn icon(&mut self, _id: Self::Id) -> Option<ImageSource<'static>> {
        None
    }

    /// Title shown in the tab bar and in floating-window title bars.
    fn title(&mut self, id: Self::Id) -> WidgetText;

    /// Draw the tab body into `ui`.
    fn ui(&mut self, ui: &mut Ui, id: Self::Id);

    /// Extra entries for the tab context menu, above the dock's close/eject.
    fn context_menu(&mut self, _ui: &mut Ui, _id: Self::Id) {}

    /// Called after the tab title widget is drawn (tooltips, click handling).
    fn on_tab_button(&mut self, _id: Self::Id, _response: &Response) {}

    /// Whether the user may close this tab (controls the context-menu entry).
    fn closeable(&mut self, _id: Self::Id) -> bool {
        true
    }

    /// Close request from the dock. Returning [`CloseResponse::Close`] makes the
    /// dock remove the tab; the app should update its own open-panel state here.
    fn on_close(&mut self, _id: Self::Id) -> CloseResponse {
        CloseResponse::Close
    }

    /// Whether this tab may be detached into a floating window.
    fn allowed_in_windows(&mut self, _id: Self::Id) -> bool {
        true
    }

    /// Called when the rect available to the tab body changes.
    fn on_rect_changed(&mut self, _id: Self::Id) {}

    /// `[horizontal, vertical]` scroll bars for the tab body.
    fn scroll_bars(&mut self, _id: Self::Id) -> [bool; 2] {
        [false, true]
    }
}
