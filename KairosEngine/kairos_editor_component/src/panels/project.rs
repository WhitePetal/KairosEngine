//! Project panel: demo body until the asset-browser component is ported.
//!
//! Production counterpart of the prototype's `project_panel.rs`: once the
//! project tree exists in `EditorState`, this grows the pure adapter functions
//! (`project_nodes` / `tree_rows` / `actions_from_tree`) ported from the
//! prototype and composes them with the widget kit.

use egui::{Context, Ui, WidgetText};
use kairos_editor_core::PanelId;

use kairos_editor_widgets::ui_image;

use crate::frame::{Frame, UiOut};
use crate::panel::Panel;

use super::PROJECT;

/// Tab-header artwork; a runtime file under `Preferences/Textures`, not an
/// embedded asset (see `kairos_editor_widgets::ui_image`).
const TAB_ICON: &str = "Preferences/Textures/tab_icons/project.png";

pub(crate) struct ProjectPanel;

impl Panel for ProjectPanel {
    fn kind(&self) -> PanelId {
        PROJECT
    }

    fn icon(&self) -> Option<&str> {
        Some(TAB_ICON)
    }

    fn init(&mut self, ctx: &Context) {
        ui_image::preload(ctx, TAB_ICON);
    }

    fn title(&self, frame: &Frame) -> WidgetText {
        format!("Project — {}", frame.state.project_name).into()
    }

    fn ui(&mut self, ui: &mut Ui, frame: &Frame, _out: &mut UiOut) {
        ui.heading("Project");
        ui.label(format!("name: {}", frame.state.project_name));
    }
}
