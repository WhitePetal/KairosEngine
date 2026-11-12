//! Console panel: renders the host-owned log snapshot.
//!
//! Ownership rule (prototype `console_panel.rs`): the log ring buffer is a host
//! resource, not domain state and not panel state. The panel reads a per-frame
//! snapshot through [`crate::HostView::console_lines`] and never consumes
//! history: closing the console loses nothing, reopening shows the same lines.
//!
//! The console is read-only: it pushes no actions, which is why `ui` ignores its
//! `UiOut` parameter.

use egui::{Ui, WidgetText};
use kairos_editor_core::PanelId;

use crate::frame::{Frame, UiOut};
use crate::panel::Panel;

use super::CONSOLE;

pub(crate) struct ConsolePanel;

impl Panel for ConsolePanel {
    fn kind(&self) -> PanelId {
        CONSOLE
    }

    fn title(&self, _frame: &Frame) -> WidgetText {
        "Console".into()
    }

    fn ui(&mut self, ui: &mut Ui, frame: &Frame, _out: &mut UiOut) {
        let lines = frame.host.console_lines();
        if lines.is_empty() {
            ui.weak("Console is empty");
            return;
        }

        egui::ScrollArea::vertical()
            .id_salt("console-log")
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for line in lines {
                    ui.monospace(line);
                }
            });
    }
}
