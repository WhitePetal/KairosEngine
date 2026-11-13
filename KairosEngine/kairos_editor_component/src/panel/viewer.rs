//! `TabViewer` adapter: the docking crate asks for a title/body/close policy by
//! id, the viewer resolves the id against the host's live instances.
//!
//! This is the only place where the panel framework meets the dock; the dock
//! crate itself stays free of editor types. It is also the path-to-source
//! bridge for tab icons: `Panel::icon` exposes an editor-relative appearance
//! path, this adapter converts it to an `egui::ImageSource` with
//! `ui_image::file_uri`, and the dock only paints that source.

use egui::{ImageSource, Ui, WidgetText};
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

    fn icon(&mut self, id: Self::Id) -> Option<ImageSource<'static>> {
        // The panel owns its editor-relative appearance path; the dock must not
        // know about file URIs, so this adapter is the single path-to-source
        // bridge.
        let path = self.host.panel(id)?.icon()?;
        Some(ImageSource::Uri(
            kairos_editor_widgets::ui_image::file_uri(path).into(),
        ))
    }

    fn title(&mut self, id: Self::Id) -> WidgetText {
        let PanelHostViewer { host, frame, .. } = self;
        host.panel(id)
            .map(|panel| panel.title(frame))
            .unwrap_or_else(|| format!("{}#{}", id.kind, id.serial).into())
    }

    fn ui(&mut self, ui: &mut Ui, id: Self::Id) {
        let PanelHostViewer { host, out, frame } = self;
        if let Some(panel) = host.panel_mut(id) {
            // One scope per panel body; the panel kind is the per-invocation
            // data, so the hierarchy can separate console/scene/profiler work
            // without a marker per panel implementation.
            kairos_profile::scope!("Kairos.Editor.Panel", id.kind.0);
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
