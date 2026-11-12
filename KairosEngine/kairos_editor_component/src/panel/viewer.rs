//! `TabViewer` adapter: the docking crate asks for a title/body/close policy by
//! id, the viewer resolves the id against the host's live instances.
//!
//! This is the only place where the panel framework meets the dock; the dock
//! crate itself stays free of editor types.

use egui::{Ui, WidgetText};
use kairos_editor_core::{Action, LayoutAction};
use kairos_editor_docking::viewer::{CloseResponse, TabViewer};

use crate::frame::{Frame, UiOut};

use super::{PanelHost, PanelInstanceId};

/// `TabViewer` implementation handed to `Docker::show`.
pub struct PanelHostViewer<'host, 'frame> {
    host: &'host mut PanelHost,
    out: &'host mut UiOut,
    frame: Frame<'frame>,
}

impl<'host, 'frame> PanelHostViewer<'host, 'frame> {
    pub(crate) fn new(
        host: &'host mut PanelHost,
        out: &'host mut UiOut,
        frame: Frame<'frame>,
    ) -> Self {
        Self { host, out, frame }
    }
}

impl TabViewer for PanelHostViewer<'_, '_> {
    type Id = PanelInstanceId;

    fn title(&mut self, id: Self::Id) -> WidgetText {
        let PanelHostViewer { host, frame, .. } = self;
        host.panel(id)
            .map(|panel| panel.title(frame))
            .unwrap_or_else(|| format!("{}#{}", id.kind, id.serial).into())
    }

    fn ui(&mut self, ui: &mut Ui, id: Self::Id) {
        let PanelHostViewer { host, out, frame } = self;
        if let Some(panel) = host.panel_mut(id) {
            panel.ui(ui, frame, *out);
        }
    }

    fn context_menu(&mut self, ui: &mut Ui, id: Self::Id) {
        let PanelHostViewer { host, out, frame } = self;
        if let Some(panel) = host.panel_mut(id) {
            panel.context_menu(ui, frame, *out);
        }
    }

    fn closeable(&mut self, id: Self::Id) -> bool {
        self.host.panel(id).is_some_and(|panel| panel.closeable())
    }

    /// Close is an *intent*: this callback never mutates the state tree. The
    /// dock removes the tab (via `CloseResponse::Close`), the viewer pushes
    /// `LayoutAction::ClosePanel`, and the app's reducer closes the layout. The
    /// next `PanelHost::sync` drops the instance.
    fn on_close(&mut self, id: Self::Id) -> CloseResponse {
        let PanelHostViewer { host, out, frame } = self;
        let Some(panel) = host.panel_mut(id) else {
            return CloseResponse::Ignore;
        };
        if !panel.on_close(frame, *out) {
            return CloseResponse::Ignore;
        }

        out.push(Action::Layout(LayoutAction::ClosePanel { panel: id.kind }));
        CloseResponse::Close
    }

    fn allowed_in_windows(&mut self, id: Self::Id) -> bool {
        self.host
            .panel(id)
            .is_some_and(|panel| panel.allowed_in_windows())
    }

    fn on_rect_changed(&mut self, id: Self::Id) {
        if let Some(panel) = self.host.panel_mut(id) {
            panel.on_rect_changed();
        }
    }

    fn scroll_bars(&mut self, id: Self::Id) -> [bool; 2] {
        self.host
            .panel(id)
            .map_or([false, true], |panel| panel.scroll_bars())
    }
}
