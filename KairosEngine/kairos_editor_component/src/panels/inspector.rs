//! Inspector panel: demo body until the document inspector is ported.
//!
//! Read-only by construction: it renders `Frame` and pushes actions for anything
//! the user changes. Real document editing (Material/Toml/Text) lands here when
//! the project-tree + document slices are ported.

use egui::{Ui, WidgetText};
use kairos_editor_core::PanelId;

use crate::frame::{Frame, UiOut};
use crate::panel::Panel;

use super::INSPECTOR;

pub(crate) struct InspectorPanel;

impl Panel for InspectorPanel {
    fn kind(&self) -> PanelId {
        INSPECTOR
    }

    fn title(&self, _frame: &Frame) -> WidgetText {
        "Inspector".into()
    }

    fn ui(&mut self, ui: &mut Ui, frame: &Frame, _out: &mut UiOut) {
        ui.heading("Inspector");
        ui.label(format!("project: {}", frame.state.project_name));
        ui.separator();
        ui.weak("The document inspector lands with the project-tree port.");
    }
}
