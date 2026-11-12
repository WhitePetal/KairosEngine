//! Hierarchy panel: demo body until the scene tree is ported.

use egui::{Ui, WidgetText};
use kairos_editor_core::{EditorState, PanelId};

use crate::panel::{Panel, PanelContext};

use super::HIERARCHY;

pub(crate) struct HierarchyPanel;

impl Panel for HierarchyPanel {
    fn kind(&self) -> PanelId {
        HIERARCHY
    }

    fn title(&self, _state: &EditorState) -> WidgetText {
        "Hierarchy".into()
    }

    fn ui(&mut self, ui: &mut Ui, _ctx: &mut PanelContext) {
        ui.heading("Hierarchy");
        for item in ["World", "Main Camera", "Editor Camera", "Sun"] {
            ui.label(item);
        }
    }
}
