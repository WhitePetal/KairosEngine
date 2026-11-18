//! Inspector panel: demo body until the document inspector is ported.
//!
//! Read-only by construction: it renders `Frame` and pushes actions for anything
//! the user changes. Real document editing (Material/Toml/Text) lands here when
//! the project-tree + document slices are ported.

use egui::{Context, Ui, WidgetText};
use kairos_editor_widgets::ui_image;

use crate::frame::{Frame, UiOut};
use crate::panel::Panel;

/// Tab-header artwork; a runtime file under `Preferences/Textures`, not an
/// embedded asset (see `kairos_editor_widgets::ui_image`).
const TAB_ICON: &str = "Preferences/Textures/tab_icons/inspector.png";

#[derive(Default)]
pub(crate) struct InspectorPanel;

#[crate::panel(INSPECTOR = "inspector")]
impl Panel for InspectorPanel {
    fn icon(&self) -> Option<&str> {
        Some(TAB_ICON)
    }

    fn init(&mut self, ctx: &Context) {
        ui_image::preload(ctx, TAB_ICON);
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
