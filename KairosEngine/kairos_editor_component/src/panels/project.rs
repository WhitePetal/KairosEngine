//! Project panel: demo body until the asset-browser component is ported.
//!
//! Production counterpart of the prototype's `project_panel.rs`: once the
//! project tree exists in `EditorState`, this grows the pure adapter functions
//! (`project_nodes` / `tree_rows` / `actions_from_tree`) ported from the
//! prototype and composes them with the widget kit.

use egui::{Ui, WidgetText};
use kairos_editor_core::{EditorState, PanelId};

use crate::panel::{Panel, PanelContext};

use super::PROJECT;

pub(crate) struct ProjectPanel;

impl Panel for ProjectPanel {
    fn kind(&self) -> PanelId {
        PROJECT
    }

    fn title(&self, state: &EditorState) -> WidgetText {
        format!("Project — {}", state.project_name).into()
    }

    fn ui(&mut self, ui: &mut Ui, ctx: &mut PanelContext) {
        ui.heading("Project");
        ui.label(format!("name: {}", ctx.state.project_name));
        if ui.button("Rename project").clicked() {
            ctx.state.project_name = format!("{}-renamed", ctx.state.project_name);
        }
    }
}
