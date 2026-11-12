//! Console feature component: renders the host-owned log snapshot.
//!
//! Ownership rule (see docs/plan/editor-ui-architecture/README.md 3.4): the log
//! ring buffer is a host resource, not domain state and not panel state. The
//! panel reads a per-frame snapshot through [`crate::frame::HostView::console_lines`]
//! and never consumes history: closing the console loses nothing, reopening it
//! shows the same lines. Query/filter/cursor state would live here as panel view
//! state (the prototype has none yet).
//!
//! The console is read-only: it pushes no Actions, which is why [`ConsolePanel::show`]
//! ignores its UiOut parameter.

use editor_core_prototype::PanelId;
use egui::{Rect, Ui};
use ui_widgets_prototype::{EmptyState, IconId, Label, StatusBar, Theme};

use crate::frame::{Frame, UiOut};
use crate::panel::Panel;

/// Non-action results of one console frame (hit rects for tests/overlay anchoring).
#[derive(Clone, Debug, PartialEq)]
pub struct ConsolePanelOutput {
    /// Number of log lines the host offered this frame.
    pub line_count: usize,
    /// Rect of the scrolling log area; `Rect::NOTHING` when the log is empty.
    pub log_rect: Rect,
    /// Rect of the bottom status bar.
    pub status_rect: Rect,
}

impl Default for ConsolePanelOutput {
    fn default() -> Self {
        Self {
            line_count: 0,
            log_rect: Rect::NOTHING,
            status_rect: Rect::NOTHING,
        }
    }
}

/// Log console panel: monospace, sticky-to-bottom scroll area plus a status bar.
#[derive(Default)]
pub struct ConsolePanel;

impl ConsolePanel {
    pub fn new() -> Self {
        Self
    }

    /// Draw the console body (the host tab bar already shows the title).
    pub fn show(&mut self, ui: &mut Ui, frame: &Frame, _out: &mut UiOut) -> ConsolePanelOutput {
        let theme = Theme::get(ui);
        let lines = frame.host.console_lines();
        let mut output = ConsolePanelOutput {
            line_count: lines.len(),
            ..Default::default()
        };

        if lines.is_empty() {
            let mut empty = EmptyState::new(
                "Console is empty",
                "The host forwards structured log lines here; the panel never owns log history.",
            )
            .icon(IconId::Info);
            let _ = empty.show(ui);
        } else {
            // Reserve the status-bar row, then let the log take the rest. The
            // scroll offset lives in egui memory (sticky at the bottom), so the
            // panel struct itself stays stateless.
            let status_height = theme.row_height + theme.space_xs;
            let log_height =
                (ui.available_height() - status_height).max(theme.row_height + theme.space_xs);
            let scroll = egui::ScrollArea::vertical()
                .id_salt("console-log")
                .auto_shrink([false, false])
                .max_height(log_height)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for line in lines {
                        ui.add(Label::new(line).mono(true));
                    }
                });
            output.log_rect = scroll.inner_rect;
        }

        let right = format!("{} lines", lines.len());
        let mut status = StatusBar::new("console", &right);
        output.status_rect = status.show(ui).rect;
        output
    }
}

impl Panel for ConsolePanel {
    fn kind(&self) -> PanelId {
        PanelId::CONSOLE
    }

    fn title(&self, _frame: &Frame) -> String {
        "Console".to_owned()
    }

    fn draw(&mut self, ui: &mut Ui, frame: &Frame, out: &mut UiOut) {
        let _ = self.show(ui, frame, out);
    }
}
