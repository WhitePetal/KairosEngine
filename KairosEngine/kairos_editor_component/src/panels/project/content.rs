//! The right-hand pane: a breadcrumb and, until the grid lands (M9), an empty
//! state.
//!
//! The breadcrumb is what makes `ProjectView::active` visible: it is the only
//! part of the panel that answers "which directory is the panel on", and every
//! segment is a `Navigate` so the state can be driven from it. The pane owns no
//! state of its own — M9 replaces the empty state with the grid and keeps this
//! header.

use egui::{Align, Color32, CornerRadius, Layout, RichText, Ui, vec2};
use kairos_editor_core::project::RelPath;
use kairos_editor_core::{Action, ProjectAction};
use kairos_editor_widgets::button::{Button, ButtonStyle};
use kairos_editor_widgets::color::ColorExt;
use kairos_editor_widgets::text::TextStyle;

use super::style::ProjectStyle;
use crate::frame::{Frame, UiOut};

/// Draw the pane's header and its body.
pub(crate) fn content(ui: &mut Ui, frame: &Frame, style: &ProjectStyle, out: &mut UiOut) {
    breadcrumb(ui, frame, style, out);
    empty_state(ui, frame, style);
}

/// A breadcrumb segment: transparent until hovered, then the panel's control fill.
fn crumb_style(style: &ProjectStyle) -> ButtonStyle {
    ButtonStyle::new(TextStyle::sized(
        style.breadcrumb_text.to_egui(),
        style.row_text_size,
    ))
    .fills(
        Color32::TRANSPARENT,
        style.control_hover.to_egui(),
        style.control_hover.to_egui(),
    )
    .corner_radius(CornerRadius::same(3))
    .height(style.row_height)
    .padding(vec2(4.0, 0.0))
}

/// The project root followed by every segment of the active directory; each
/// segment navigates to its own path (the root navigates to the project root).
fn breadcrumb(ui: &mut Ui, frame: &Frame, style: &ProjectStyle, out: &mut UiOut) {
    let project = &frame.state.project;
    let button_style = crumb_style(style);

    // (label, path) pairs, root first.
    let mut segments: Vec<(String, RelPath)> = vec![(project.name.clone(), RelPath::default())];
    if let Some(active) = &project.view.active {
        let mut prefix = String::new();
        for segment in active.as_str().split('/').filter(|part| !part.is_empty()) {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(segment);
            segments.push((segment.to_owned(), RelPath::new(prefix.clone())));
        }
    }

    let row_height = style.row_height;
    ui.allocate_ui_with_layout(
        vec2(ui.available_width(), row_height),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (index, (label, target)) in segments.iter().enumerate() {
                if index > 0 {
                    ui.label(
                        RichText::new("/")
                            .size(style.row_text_size)
                            .color(style.breadcrumb_separator.to_egui()),
                    );
                }
                if ui.add(Button::new(label, &button_style)).clicked() {
                    out.push(Action::Project(ProjectAction::Navigate {
                        path: target.clone(),
                    }));
                }
            }
        },
    );
}

/// The placeholder whose area M9 turns into the asset grid.
fn empty_state(ui: &mut Ui, frame: &Frame, style: &ProjectStyle) {
    let active = frame
        .state
        .project
        .view
        .active
        .as_ref()
        .map_or_else(|| String::from("the project root"), |path| path.to_string());
    let hint = style.hint_text.to_egui();

    ui.allocate_ui_with_layout(ui.available_size(), Layout::top_down(Align::Center), |ui| {
        ui.add_space(ui.available_height() * 0.38);
        ui.label(
            RichText::new("Asset grid lands with the type registry")
                .size(style.row_text_size + 1.0)
                .color(hint),
        );
        ui.add_space(2.0);
        ui.label(
            RichText::new(format!(
                "Browsing {active}; the tree on the left is the v1 navigation."
            ))
            .size(style.row_text_size)
            .color(hint),
        );
    });
}
