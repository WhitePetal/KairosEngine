//! Scene panel: placeholder until the wgpu/egui viewport lands.
//!
//! Production counterpart of the prototype's `viewport_panel.rs`: when the host
//! side (offscreen target -> `egui::TextureId`) exists, this draws that texture
//! through [`crate::HostView`] and forwards camera/pick input as actions.

use egui::{Ui, WidgetText};
use kairos_editor_core::PanelId;

use crate::frame::{Frame, UiOut};
use crate::panel::Panel;

use super::SCENE;

pub(crate) struct ScenePanel;

impl Panel for ScenePanel {
    fn kind(&self) -> PanelId {
        SCENE
    }

    fn title(&self, _frame: &Frame) -> WidgetText {
        "Scene".into()
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &Frame, _out: &mut UiOut) {
        ui.centered_and_justified(|ui| {
            ui.label("3D viewport placeholder");
        });
    }
}
