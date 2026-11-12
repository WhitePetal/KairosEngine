//! Console panel: demo body reading the app-owned log lines.

use egui::{Ui, WidgetText};
use kairos_editor_core::{EditorState, PanelId};

use crate::panel::{Panel, PanelContext};

use super::CONSOLE;

pub(crate) struct ConsolePanel;

impl Panel for ConsolePanel {
    fn kind(&self) -> PanelId {
        CONSOLE
    }

    fn title(&self, _state: &EditorState) -> WidgetText {
        "Console".into()
    }

    fn ui(&mut self, ui: &mut Ui, ctx: &mut PanelContext) {
        ui.heading("Console");
        for line in &ctx.state.log {
            ui.monospace(line);
        }
        if ui.button("Push log line").clicked() {
            let n = ctx.state.log.len() + 1;
            ctx.state.log.push(format!("log line {n}"));
        }
    }
}
