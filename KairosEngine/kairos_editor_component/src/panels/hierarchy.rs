//! Hierarchy panel: demo body until the scene tree is ported.

use egui::{Ui, WidgetText};
use kairos_editor_core::PanelId;

use crate::frame::{Frame, UiOut};
use crate::panel::Panel;

use super::HIERARCHY;

pub(crate) struct HierarchyPanel;

impl Panel for HierarchyPanel {
    fn kind(&self) -> PanelId {
        HIERARCHY
    }

    fn title(&self, _frame: &Frame) -> WidgetText {
        "Hierarchy".into()
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &Frame, _out: &mut UiOut) {
        ui.heading("Hierarchy");
        for item in ["World", "Main Camera", "Editor Camera", "Sun"] {
            ui.label(item);
        }
    }
}
