//! Hierarchy panel: demo body until the scene tree is ported.

use egui::{Context, Ui, WidgetText};
use kairos_editor_core::PanelId;

use kairos_editor_widgets::ui_image;

use crate::frame::{Frame, UiOut};
use crate::panel::Panel;

use super::HIERARCHY;

/// Tab-header artwork; a runtime file under `Preferences/Textures`, not an
/// embedded asset (see `kairos_editor_widgets::ui_image`).
const TAB_ICON: &str = "Preferences/Textures/tab_icons/hierarchy.png";

pub(crate) struct HierarchyPanel;

impl Panel for HierarchyPanel {
    fn kind(&self) -> PanelId {
        HIERARCHY
    }

    fn icon(&self) -> Option<&str> {
        Some(TAB_ICON)
    }

    fn init(&mut self, ctx: &Context) {
        ui_image::preload(ctx, TAB_ICON);
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
