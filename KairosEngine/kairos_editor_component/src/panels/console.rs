//! Console panel: renders the host-owned `kairos_log` history.
//!
//! Ownership rule (prototype `console_panel.rs`): the log ring buffer is a host
//! resource, not domain state and not panel state. [`ConsoleLog`] captures
//! `kairos_log` records; the panel reads a per-frame snapshot through
//! [`HostView::console_lines`](crate::HostView::console_lines) and never consumes
//! history: closing the console loses nothing, reopening shows the same lines.
//!
//! Level colors follow the theme: warnings use `warn_fg_color`, errors use
//! `error_fg_color`, info uses the normal text color. Filtering/collapsing would
//! be panel view state added here; capture itself stays in `kairos_log`.
//!
//! The console is read-only: it pushes no actions, which is why `ui` ignores its
//! `UiOut` parameter.

use egui::{Color32, RichText, Ui, WidgetText};
use kairos_editor_core::PanelId;
use kairos_log::Level;

use crate::frame::{ConsoleLine, Frame, UiOut};
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
        frame.host.console_lines(&mut |lines| {
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
                        let text = RichText::new(line.text.as_str()).monospace();
                        ui.label(text.color(color_for(ui, line)));
                    }
                });
        });
    }
}

fn color_for(ui: &Ui, line: &ConsoleLine) -> Color32 {
    match line.level {
        Level::Info => ui.visuals().widgets.noninteractive.fg_stroke.color,
        Level::Warning => ui.visuals().warn_fg_color,
        Level::Error => ui.visuals().error_fg_color,
    }
}
