//! Scene panel: placeholder until the wgpu/egui viewport lands.
//!
//! Production counterpart of the prototype's `viewport_panel.rs`: when the host
//! side (offscreen target -> `egui::TextureId`) exists, this draws that texture
//! through [`crate::HostView`] and forwards camera/pick input as actions.

use egui::{Context, Ui, WidgetText};
use kairos_editor_core::PanelId;

use kairos_editor_widgets::ui_image;

use crate::frame::{Frame, UiOut};
use crate::panel::{Panel, PanelContext, PanelDef};

use super::SCENE;

/// Tab-header artwork; a runtime file under `Preferences/Textures`, not an
/// embedded asset (see `kairos_editor_widgets::ui_image`).
const TAB_ICON: &str = "Preferences/Textures/tab_icons/scene.png";

pub(crate) struct ScenePanel;

/// Catalog entry: the host builds the panel through this when it opens.
pub(super) const DEF: PanelDef = PanelDef::new(SCENE, create);

fn create(_context: &PanelContext) -> Box<dyn Panel> {
    Box::new(ScenePanel)
}

impl Panel for ScenePanel {
    fn kind(&self) -> PanelId {
        SCENE
    }

    fn icon(&self) -> Option<&str> {
        Some(TAB_ICON)
    }

    fn init(&mut self, ctx: &Context) {
        ui_image::preload(ctx, TAB_ICON);
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
