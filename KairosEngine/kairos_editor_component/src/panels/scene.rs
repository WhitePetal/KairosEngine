//! Scene panel: placeholder until the wgpu/egui viewport lands.
//!
//! Production counterpart of the prototype's `viewport_panel.rs`: when the
//! host side (offscreen target -> `egui::TextureId`) exists, this draws that
//! texture and forwards camera/pick input through the host.

use egui::{Ui, WidgetText};
use kairos_editor_core::{EditorState, PanelId};

use crate::panel::{Panel, PanelContext};

use super::SCENE;

pub(crate) struct ScenePanel;

impl Panel for ScenePanel {
    fn kind(&self) -> PanelId {
        SCENE
    }

    fn title(&self, _state: &EditorState) -> WidgetText {
        "Scene".into()
    }

    fn ui(&mut self, ui: &mut Ui, _ctx: &mut PanelContext) {
        ui.centered_and_justified(|ui| {
            ui.label("3D viewport placeholder");
        });
    }
}
