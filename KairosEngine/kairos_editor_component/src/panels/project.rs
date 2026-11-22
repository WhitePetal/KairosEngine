//! Project panel: the project tree's debug readout, until M2 draws the tree.
//!
//! The tree itself is M1's (`kairos_editor_core::project`), and the panel that
//! draws it — toolbar, splitter, `TreeView` rows, breadcrumb — is M2. This body
//! exists so M1 is *visible*: it prints the snapshot's size, the scan's timing and
//! truncation, and a screenful of the exact rows the tree would draw. That turns
//! every M1 acceptance check into something the editor shows instead of something
//! only `cargo test` knows, and it makes the M2 port a replacement of this file
//! rather than a leap of faith.
//!
//! Nothing here holds state (M2's row cache, search box and rename buffer are the
//! panel's own then): the readout is a pure projection of `frame.state.project`.

use egui::{Context, ScrollArea, Ui, WidgetText};
use kairos_editor_core::project::ProjectState;
use kairos_editor_widgets::ui_image;

use crate::frame::{Frame, UiOut};
use crate::panel::Panel;

/// Tab-header artwork; a runtime file under `Preferences/Textures`, not an
/// embedded asset (see `kairos_editor_widgets::ui_image`).
const TAB_ICON: &str = "Preferences/Textures/tab_icons/project.png";

/// How many rows the readout lists. A couple of screenfuls is enough to check
/// the ordering rule, the hidden-name rule and the folder suffix by eye; M2's
/// list is virtualised and has no limit.
const ROW_LIMIT: usize = 200;

#[derive(Default)]
pub(crate) struct ProjectPanel;

impl ProjectPanel {
    /// One line of scan state: the size, the system subtree, the timing.
    fn summary(project: &ProjectState) -> String {
        let nodes = project.tree.len().saturating_sub(1);
        let scan = if project.scan.running {
            String::from("scanning...")
        } else {
            format!(
                "scan #{} in {} ms",
                project.scan.generation, project.scan.last_ms
            )
        };
        let truncated = if project.tree.truncated() {
            " · truncated"
        } else {
            ""
        };
        format!(
            "{nodes} nodes · library {} · {scan}{truncated}",
            project.tree.system_nodes()
        )
    }

    /// The walk's configuration, so a reviewer can see which rules were loaded.
    fn policy_line(project: &ProjectState) -> String {
        format!(
            "policy: {} ignore rules · system dirs: {}",
            project.policy.ignore.len(),
            project.policy.system_dirs.join(", ")
        )
    }
}

#[crate::panel(PROJECT = "project")]
impl Panel for ProjectPanel {
    fn icon(&self) -> Option<&str> {
        Some(TAB_ICON)
    }

    fn init(&mut self, ctx: &Context) {
        ui_image::preload(ctx, TAB_ICON);
    }

    fn title(&self, _frame: &Frame) -> WidgetText {
        "Project".into()
    }

    fn ui(&mut self, ui: &mut Ui, frame: &Frame, _out: &mut UiOut) {
        let project = &frame.state.project;

        ui.heading("Project");
        ui.label(format!("root: {}", project.root.display()));
        ui.label(Self::summary(project));
        ui.label(Self::policy_line(project));
        if let Some(error) = &project.error {
            ui.colored_label(ui.visuals().error_fg_color, error.to_string());
        }

        ui.separator();

        let rows = project.tree.visible_rows(&project.view);
        ui.label(format!(
            "{} visible rows (read-only; M2 draws the real list)",
            rows.len()
        ));

        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for &id in rows.iter().take(ROW_LIMIT) {
                    let Some(node) = project.tree.node(id) else {
                        continue;
                    };
                    let depth = node.path.as_str().matches('/').count();
                    let suffix = if node.is_folder() { "/" } else { "" };
                    let marker = if project.view.selection.contains(&node.path) {
                        ">"
                    } else {
                        " "
                    };
                    ui.monospace(format!(
                        "{marker}{}{}{}",
                        "   ".repeat(depth),
                        node.name(),
                        suffix
                    ));
                }
                if rows.len() > ROW_LIMIT {
                    ui.label(format!("... {} more", rows.len() - ROW_LIMIT));
                }
            });
    }
}