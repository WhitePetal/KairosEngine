//! Inspector panel: demo body until the document inspector is ported.

use egui::{Ui, WidgetText};
use kairos_editor_core::{EditorState, PanelId};

use crate::panel::{Panel, PanelContext};

use super::INSPECTOR;

pub(crate) struct InspectorPanel;

impl Panel for InspectorPanel {
    fn kind(&self) -> PanelId {
        INSPECTOR
    }

    fn title(&self, _state: &EditorState) -> WidgetText {
        "Inspector".into()
    }

    fn ui(&mut self, ui: &mut Ui, ctx: &mut PanelContext) {
        ui.heading("Inspector");
        ui.label(format!("selected project: {}", ctx.state.project_name));
        ui.separator();
        if ui.button("Increment shared counter").clicked() {
            ctx.state.counter += 1;
        }
        ui.label(format!("counter = {}", ctx.state.counter));
    }
}
