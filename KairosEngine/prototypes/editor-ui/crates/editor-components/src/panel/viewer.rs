//! Production-dock seam (mirror of `kairos_editor_docking::viewer::TabViewer`).
//!
//! The prototype still draws its four slots through `PanelHost::draw`, but a
//! real dock owns layout and calls a viewer by id instead. This module is the
//! code-level bridge: `PanelHostViewer` implements `DockViewer` for
//! `PanelInstanceId`, so the same `Panel` implementations can be used by a
//! dock widget without introducing a second `Tab` trait.

use std::hash::Hash;

use egui::{Ui, WidgetText};

use editor_core_prototype::{Action, LayoutAction};

use crate::frame::{Frame, UiOut};

use super::{PanelHost, PanelInstanceId};

/// Answer to [`DockViewer::on_close`].
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

/// What a dock widget asks the panel host about one tab.
///
/// This deliberately mirrors production's `kairos_editor_docking::viewer::
/// TabViewer`; keep the method set in sync.
pub trait DockViewer {
    type Id: Copy + Eq + Hash + std::fmt::Debug;

    fn title(&mut self, id: Self::Id) -> WidgetText;
    fn ui(&mut self, ui: &mut Ui, id: Self::Id);

    /// Extra context-menu entries before the dock's close/eject items.
    fn context_menu(&mut self, _ui: &mut Ui, _id: Self::Id) {}

    /// Called after the tab title widget is drawn.
    fn on_tab_button(&mut self, _id: Self::Id, _response: &egui::Response) {}

    fn closeable(&mut self, id: Self::Id) -> bool;
    fn on_close(&mut self, id: Self::Id) -> CloseResponse;
    fn allowed_in_windows(&mut self, id: Self::Id) -> bool;
    fn scroll_bars(&mut self, id: Self::Id) -> [bool; 2];
    fn on_rect_changed(&mut self, id: Self::Id);
}

/// Adapter from the prototype `PanelHost` to a dock's viewer interface.
///
/// The frame/out pair is now also production's contract
/// (`kairos_editor_component/src/panel/viewer.rs`): the dock asks for a title /
/// body / close policy, the viewer resolves the id and emits `LayoutAction`s
/// into `UiOut` instead of mutating state. The id/panel mapping is identical.
pub struct PanelHostViewer<'host, 'frame> {
    pub(crate) host: &'host mut PanelHost,
    pub(crate) frame: Frame<'frame>,
    pub(crate) out: &'host mut UiOut,
}

impl DockViewer for PanelHostViewer<'_, '_> {
    type Id = PanelInstanceId;

    fn title(&mut self, id: Self::Id) -> WidgetText {
        self.host
            .panel(id)
            .map(|panel| panel.title(&self.frame).into())
            .unwrap_or_else(|| format!("{}#{}", id.kind.0, id.serial).into())
    }

    fn ui(&mut self, ui: &mut Ui, id: Self::Id) {
        let Self { host, frame, out } = self;
        if let Some(panel) = host.panel_mut(id) {
            panel.draw(ui, frame, &mut **out);
        }
    }

    fn closeable(&mut self, id: Self::Id) -> bool {
        self.host.panel(id).is_none_or(|panel| panel.closable())
    }

    fn on_close(&mut self, id: Self::Id) -> CloseResponse {
        let Self { host, frame, out } = self;
        let should_close = host
            .panel_mut(id)
            .is_none_or(|panel| panel.on_close(frame, &mut **out));
        if !should_close {
            return CloseResponse::Ignore;
        }
        out.push(Action::Layout(LayoutAction::ClosePanel { panel: id.kind }));
        CloseResponse::Close
    }

    fn allowed_in_windows(&mut self, id: Self::Id) -> bool {
        self.host
            .panel(id)
            .is_none_or(|panel| panel.allowed_in_windows())
    }

    fn scroll_bars(&mut self, id: Self::Id) -> [bool; 2] {
        self.host
            .panel(id)
            .map_or([false, true], |panel| panel.scroll_bars())
    }

    fn on_rect_changed(&mut self, id: Self::Id) {
        if let Some(panel) = self.host.panel_mut(id) {
            panel.on_rect_changed();
        }
    }
}
