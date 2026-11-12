//! `TabViewer` adapter: the docking crate asks for a title/body/close policy by
//! id, the viewer resolves the id against the host's live instances.
//!
//! This is the only place where the panel framework meets the dock; the dock
//! crate itself stays free of editor types.

use egui::{Ui, WidgetText};
use kairos_editor_core::EditorState;
use kairos_editor_docking::viewer::{CloseResponse, TabViewer};

use super::{PanelContext, PanelHost, PanelInstanceId};

/// `TabViewer` implementation handed to `Docker::show`.
pub struct PanelHostViewer<'a> {
    host: &'a mut PanelHost,
    state: &'a mut EditorState,
}

impl<'a> PanelHostViewer<'a> {
    pub(crate) fn new(host: &'a mut PanelHost, state: &'a mut EditorState) -> Self {
        Self { host, state }
    }
}

impl TabViewer for PanelHostViewer<'_> {
    type Id = PanelInstanceId;

    fn title(&mut self, id: Self::Id) -> WidgetText {
        let PanelHostViewer { host, state } = self;
        host.panel(id)
            .map(|panel| panel.title(state))
            .unwrap_or_else(|| format!("{}#{}", id.kind, id.serial).into())
    }

    fn ui(&mut self, ui: &mut Ui, id: Self::Id) {
        let PanelHostViewer { host, state } = self;
        if let Some(panel) = host.panel_mut(id) {
            let mut ctx = PanelContext {
                state: &mut **state,
            };
            panel.ui(ui, &mut ctx);
        }
    }

    fn context_menu(&mut self, ui: &mut Ui, id: Self::Id) {
        let PanelHostViewer { host, state } = self;
        if let Some(panel) = host.panel_mut(id) {
            let mut ctx = PanelContext {
                state: &mut **state,
            };
            panel.context_menu(ui, &mut ctx);
        }
    }

    fn closeable(&mut self, id: Self::Id) -> bool {
        self.host.panel(id).is_some_and(|panel| panel.closeable())
    }

    fn on_close(&mut self, id: Self::Id) -> CloseResponse {
        let PanelHostViewer { host, state } = self;
        let Some(panel) = host.panel_mut(id) else {
            return CloseResponse::Ignore;
        };
        let mut ctx = PanelContext {
            state: &mut **state,
        };
        if !panel.on_close(&mut ctx) {
            return CloseResponse::Ignore;
        }
        state.layout.close(id.kind);
        host.remove(id);
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
