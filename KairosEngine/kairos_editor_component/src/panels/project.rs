//! Project panel: the two-column asset browser (M2).
//!
//! Layout (D1, Unity/Tuanjie Project window): a full-width toolbar, then the
//! folder tree on the left and the content pane on the right, then a full-width
//! status bar. The right pane is a breadcrumb plus a placeholder in v1; the asset
//! grid fills it in M9 without changing this layout.
//!
//! The panel is **read-only** in M2: it selects, expands, navigates and opens,
//! and never creates, renames or deletes (M3). Every intent leaves through
//! [`UiOut`] as a [`ProjectAction`]; the panel never mutates state.
//!
//! What lives where:
//!
//! * core owns the intent that survives a panel close: expansion, the active
//!   directory, selection, sort, hidden files;
//! * this instance owns what only affects a frame: the splitter's in-flight
//!   fraction, the row cache, the row to scroll into view next pass, and whether
//!   the tree had keyboard focus last pass;
//! * [`rows`] is the pure state-to-row projection, [`content`] the right pane and
//!   [`style`] the editable skin.
//!
//! Interaction is Unity's (M2 定稿 N1): the triangle expands, a single click
//! selects, a double click opens (navigate for a folder, the OS default program
//! for a file). The two hit areas are separate on purpose, so clicking a row
//! never navigates.

mod content;
mod rows;
mod style;

use egui::{Align2, Context, CornerRadius, Key, Rect, Sense, Ui, WidgetText, pos2, vec2};
use kairos_editor_core::project::{NodeId, ProjectState, RelPath, ScanReason, is_hidden_name};
use kairos_editor_core::{Action, ProjectAction};
use kairos_editor_widgets::button::{Button, ButtonAlign, ButtonStyle};
use kairos_editor_widgets::color::ColorExt;
use kairos_editor_widgets::splitter::Splitter;
use kairos_editor_widgets::style::StyleFile;
use kairos_editor_widgets::text::TextStyle;
use kairos_editor_widgets::tree::{TreeOutput, TreeView};
use kairos_editor_widgets::ui_image;

use self::rows::{Icons, ProjectRows};
use crate::frame::{Frame, UiOut};
use crate::panel::Panel;
use style::ProjectStyle;

/// Narrowest the folder tree may become, in points. The splitter clamps to this
/// as well, so a narrow panel degrades instead of overflowing.
const MIN_TREE_WIDTH: f32 = 120.0;
/// Narrowest the content pane may become.
const MIN_CONTENT_WIDTH: f32 = 160.0;
/// Toolbar button padding either side of the label.
const BUTTON_PADDING: f32 = 6.0;

/// Panel view state. Everything here is disposable: closing the tab drops it and
/// reopening rebuilds it from core's [`ProjectView`].
pub(crate) struct ProjectPanel {
    /// Loaded from `Preferences/Styles/ProjectStyle.ron` in [`Panel::init`].
    style: ProjectStyle,
    /// Visible rows, rebuilt only when the tree or the view intent changes.
    rows: ProjectRows,
    /// Left column's fraction while the splitter is dragged; `None` follows
    /// `ProjectView::tree_split`. The state is written once, when the drag ends,
    /// so a drag is not sixty actions per second (N3 of the plan's M2 定稿).
    split: Option<f32>,
    /// Row to scroll into view on this pass: keyboard navigation moves the
    /// selection off-screen otherwise.
    scroll_to: Option<RelPath>,
    /// The tree had keyboard focus last pass. The widget reports it, because only
    /// the widget knows whether the last click landed on a row.
    tree_focused: bool,
}

impl Default for ProjectPanel {
    fn default() -> Self {
        Self {
            style: ProjectStyle::default(),
            rows: ProjectRows::default(),
            split: None,
            scroll_to: None,
            tree_focused: false,
        }
    }
}

#[crate::panel(PROJECT = "project", skin = ProjectStyle)]
impl Panel for ProjectPanel {
    fn icon(&self) -> Option<&str> {
        Some(self.style.tab_icon.as_str())
    }

    fn title(&self, _frame: &Frame) -> WidgetText {
        "Project".into()
    }

    fn init(&mut self, ctx: &Context) {
        match StyleFile::<ProjectStyle>::load_or_default() {
            Ok(file) => {
                self.style = if ctx.theme() == egui::Theme::Light {
                    file.light
                } else {
                    file.dark
                };
            }
            Err(error) => {
                kairos_log::warn!(target: "kairos_editor", "project style: {error}");
            }
        }

        ui_image::preload(ctx, &self.style.tab_icon);
        for path in self.style.icons.all() {
            ui_image::preload(ctx, path);
        }
    }

    /// Both panes scroll themselves and the status bar is pinned, so the dock
    /// must not wrap the body in a second scroll area.
    fn scroll_bars(&self) -> [bool; 2] {
        [false, false]
    }

    fn ui(&mut self, ui: &mut Ui, frame: &Frame, out: &mut UiOut) {
        let project = &frame.state.project;
        let Self {
            style,
            rows,
            split,
            scroll_to,
            tree_focused,
        } = self;

        // The three bands are laid out from measured rects, so egui's automatic
        // spacing between them must not shift them (it would leave a 2 px gap
        // under the toolbar and inside the panel's height budget).
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.painter().rect_filled(
            ui.max_rect(),
            CornerRadius::ZERO,
            style.background.to_egui(),
        );
        // The bar reports its own reserved row: the buttons are placed with
        // ui.put, which re-anchors the layout cursor to the last placed widget
        // (20 px) instead of the reserved 21 px band, so the body must not be
        // derived from available_rect_before_wrap.
        let bar = toolbar(ui, style, project, out);
        let body = Rect::from_min_max(pos2(ui.max_rect().left(), bar.bottom()), ui.max_rect().max);
        let status_height = style.status_height.min(body.height() * 0.5).max(0.0);
        let content_rect =
            Rect::from_min_max(body.min, pos2(body.max.x, body.max.y - status_height));
        let status_rect =
            Rect::from_min_max(pos2(body.min.x, body.max.y - status_height), body.max);

        let tree_style = style.tree();
        let icons = Icons {
            folder: &style.icons.folder,
            folder_open: &style.icons.folder_open,
            file: &style.icons.file,
        };
        let row_ids = rows.update(&project.tree, &project.view);

        // Keyboard first: the pass that moves the selection also scrolls it into
        // view, because the widget must be told the target before it draws.
        if *tree_focused {
            keyboard(ui, project, row_ids, scroll_to, out);
        }
        let scroll_to_id = scroll_to.take().and_then(|path| project.tree.find(&path));

        // --- split geometry ---
        let total = content_rect.width();
        let mut left_width = split.unwrap_or(project.view.tree_split) * total;
        left_width = left_width.clamp(0.0, total);
        if total >= MIN_TREE_WIDTH + MIN_CONTENT_WIDTH {
            left_width = left_width.clamp(MIN_TREE_WIDTH, total - MIN_CONTENT_WIDTH);
        }
        let boundary = content_rect.left() + left_width;
        let tree_rect = Rect::from_min_max(content_rect.min, pos2(boundary, content_rect.max.y));
        let content_pane = Rect::from_min_max(pos2(boundary, content_rect.min.y), content_rect.max);

        // --- folder tree ---
        let project_tree = &project.tree;
        let project_view = &project.view;
        ui.scope_builder(egui::UiBuilder::new().max_rect(tree_rect), |ui| {
            let mut view = TreeView::new("project-tree", row_ids, &tree_style, |id| {
                rows::row(project_tree, project_view, icons, id).unwrap_or_else(rows::missing_row)
            });
            if let Some(id) = scroll_to_id {
                view = view.scroll_to(id);
            }
            let output = view.show(ui);
            *tree_focused = output.focused;
            row_actions(project, &output, out);
        });

        // --- right pane ---
        ui.scope_builder(egui::UiBuilder::new().max_rect(content_pane), |ui| {
            content::content(ui, frame, style, out);
        });

        // The splitter overlays the boundary so it takes no room from either pane
        // and its 1 px line sits exactly where the panes were laid out.
        let grab = style.splitter_grab.max(style.splitter_width * 0.5);
        let strip = Rect::from_min_max(
            pos2(boundary - grab, content_rect.top()),
            pos2(boundary + grab, content_rect.bottom()),
        );
        let response = Splitter::vertical()
            .line(style.splitter_width)
            .grab(2.0 * grab)
            .limits(MIN_TREE_WIDTH, MIN_CONTENT_WIDTH)
            .colors(
                style.splitter_idle.to_egui(),
                style.splitter_hover.to_egui(),
                style.splitter_active.to_egui(),
            )
            .show_in(
                ui,
                ui.id().with("project-column-splitter"),
                strip,
                &mut left_width,
                total,
            );
        if response.drag_stopped() && total > 0.0 {
            out.push(Action::Project(ProjectAction::SetTreeSplit(
                left_width / total,
            )));
        }

        status_bar(ui, style, project, status_rect);
    }
}

/// The style of a toolbar button: transparent until hovered, exact bar height.
fn toolbar_button_style(style: &ProjectStyle) -> ButtonStyle {
    ButtonStyle::new(TextStyle::sized(
        style.toolbar_text.to_egui(),
        style.row_text_size,
    ))
    .fills(
        egui::Color32::TRANSPARENT,
        style.control_hover.to_egui(),
        style.control_hover.to_egui(),
    )
    .corner_radius(CornerRadius::ZERO)
    .height(style.toolbar_height)
    .padding(vec2(BUTTON_PADDING, 0.0))
}

/// Width a toolbar button needs for its label, measured once per pass.
fn label_width(ui: &Ui, style: &ProjectStyle, label: &str) -> f32 {
    let text = TextStyle::sized(style.toolbar_text.to_egui(), style.row_text_size);
    text.layout(ui, label).size().x + 2.0 * BUTTON_PADDING
}

/// The toolbar: collapse all, refresh and the hidden-files toggle.
///
/// Only actions M2 can actually perform are drawn; the create/delete buttons
/// arrive with M3 (N7 of the plan's M2 定稿). Buttons are hand-laid-out on a
/// measured 20 px bar, like the Console's.
fn toolbar(ui: &mut Ui, style: &ProjectStyle, project: &ProjectState, out: &mut UiOut) -> Rect {
    let width = ui.available_width();
    let (row, _) = ui.allocate_exact_size(
        vec2(width, style.toolbar_height + style.separator_width),
        Sense::hover(),
    );
    let painter = ui.painter().clone();
    painter.rect_filled(
        Rect::from_min_size(row.min, vec2(width, style.toolbar_height)),
        CornerRadius::ZERO,
        style.toolbar_bg.to_egui(),
    );
    painter.rect_filled(
        Rect::from_min_size(
            pos2(row.left(), row.top() + style.toolbar_height),
            vec2(width, style.separator_width),
        ),
        CornerRadius::ZERO,
        style.separator.to_egui(),
    );

    let button_style = toolbar_button_style(style);
    let mut x = row.left();
    for (label, action) in [
        ("Collapse all", ProjectAction::CollapseAll),
        (
            "Refresh",
            ProjectAction::ScanRequested {
                reason: ScanReason::Manual,
                focus: None,
            },
        ),
    ] {
        let button_width = label_width(ui, style, label);
        let rect =
            Rect::from_min_size(pos2(x, row.top()), vec2(button_width, style.toolbar_height));
        x = rect.right();
        let response = ui.put(
            rect,
            Button::new(label, &button_style)
                .width(button_width)
                .height(style.toolbar_height)
                .align(ButtonAlign::Center)
                .id(ui.id().with("project-toolbar").with(label)),
        );
        if response.clicked() {
            out.push(Action::Project(action));
        }
    }

    // Hidden files sits at the right edge, where Unity keeps its filters.
    let label = "Hidden files";
    let button_width = label_width(ui, style, label);
    let rect = Rect::from_min_size(
        pos2(row.right() - button_width, row.top()),
        vec2(button_width, style.toolbar_height),
    );
    let response = ui.put(
        rect,
        Button::new(label, &button_style)
            .width(button_width)
            .height(style.toolbar_height)
            .align(ButtonAlign::Center)
            .selected(project.view.show_hidden)
            .id(ui.id().with("project-toolbar-hidden")),
    );
    if response.clicked() {
        out.push(Action::Project(ProjectAction::SetShowHidden(
            !project.view.show_hidden,
        )));
    }

    row
}

/// Row interactions: the triangle toggles, a click selects, a double click opens.
///
/// Order matters: egui reports `clicked` and `double_clicked` on the second press
/// of a double click, so the double click is handled first and the single click
/// only when it is not one.
fn row_actions(project: &ProjectState, output: &TreeOutput<NodeId>, out: &mut UiOut) {
    if let Some(id) = output.expander_toggled
        && let Some(node) = project.tree.node(id)
    {
        out.push(Action::Project(ProjectAction::ToggleFolder {
            path: node.path.clone(),
        }));
    }
    if let Some(id) = output.double_clicked {
        if let Some(node) = project.tree.node(id) {
            if node.is_folder() {
                out.push(Action::Project(ProjectAction::Navigate {
                    path: node.path.clone(),
                }));
            } else {
                out.push(Action::Project(ProjectAction::OpenRequested {
                    path: node.path.clone(),
                }));
            }
        }
    } else if let Some(id) = output.clicked
        && let Some(node) = project.tree.node(id)
    {
        out.push(Action::Project(ProjectAction::Select {
            path: Some(node.path.clone()),
        }));
    }
}

/// Keyboard navigation over the visible rows, Unity's bindings.
///
/// The keyboard cursor *is* the selection: v1 keeps at most one selected path, so
/// there is no second piece of state to keep in sync with it. Handled only while
/// the tree has focus, which the widget reports, so the search box (M6) can own
/// the same keys.
fn keyboard(
    ui: &Ui,
    project: &ProjectState,
    row_ids: &[NodeId],
    scroll_to: &mut Option<RelPath>,
    out: &mut UiOut,
) {
    let tree = &project.tree;
    let view = &project.view;
    let selection = view.selection.iter().next();
    let index = selection
        .and_then(|path| {
            row_ids
                .iter()
                .position(|id| tree.node(*id).is_some_and(|node| &node.path == path))
        })
        .unwrap_or(0);
    let selected = selection.and_then(|path| tree.get(path));
    let last_index = row_ids.len().saturating_sub(1);

    let mut next: Option<NodeId> = None;
    ui.input(|input| {
        if input.key_pressed(Key::ArrowDown) {
            next = row_ids.get((index + 1).min(last_index)).copied();
        } else if input.key_pressed(Key::ArrowUp) {
            next = row_ids.get(index.saturating_sub(1)).copied();
        } else if input.key_pressed(Key::Home) {
            next = row_ids.first().copied();
        } else if input.key_pressed(Key::End) {
            next = row_ids.last().copied();
        } else if input.key_pressed(Key::ArrowLeft) {
            if let Some(node) = selected {
                if node.is_folder() && view.expanded.contains(&node.path) {
                    out.push(Action::Project(ProjectAction::ToggleFolder {
                        path: node.path.clone(),
                    }));
                } else if let Some(parent) = node.path.parent() {
                    out.push(Action::Project(ProjectAction::Select {
                        path: Some(parent),
                    }));
                }
            }
        } else if input.key_pressed(Key::ArrowRight) {
            if let Some(node) = selected
                && node.is_folder()
            {
                if !view.expanded.contains(&node.path) {
                    out.push(Action::Project(ProjectAction::ToggleFolder {
                        path: node.path.clone(),
                    }));
                } else if let Some(parent) = tree.find(&node.path) {
                    next = row_ids.iter().copied().find(|id| {
                        tree.node(*id)
                            .is_some_and(|child| child.parent == Some(parent))
                    });
                }
            }
        } else if input.key_pressed(Key::Enter) {
            if let Some(node) = selected {
                if node.is_folder() {
                    out.push(Action::Project(ProjectAction::Navigate {
                        path: node.path.clone(),
                    }));
                } else {
                    out.push(Action::Project(ProjectAction::OpenRequested {
                        path: node.path.clone(),
                    }));
                }
            }
        }
    });

    if let Some(node) = next.and_then(|id| tree.node(id)) {
        out.push(Action::Project(ProjectAction::Select {
            path: Some(node.path.clone()),
        }));
        *scroll_to = Some(node.path.clone());
    }
}

/// The item count, the selection and the scan's visible warnings.
///
/// The count is the active directory's directly visible children — the number the
/// user is looking at — not the visible rows and not the whole tree. Warnings
/// outrank the selection path: a truncated scan or a read failure is the thing
/// the user must notice.
fn status_bar(ui: &Ui, style: &ProjectStyle, project: &ProjectState, rect: Rect) {
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::ZERO, style.status_bg.to_egui());
    painter.rect_filled(
        Rect::from_min_size(rect.min, vec2(rect.width(), style.separator_width)),
        CornerRadius::ZERO,
        style.separator.to_egui(),
    );

    let text = TextStyle::sized(style.status_text.to_egui(), style.status_text_size);

    let root = project
        .tree
        .node(project.tree.root())
        .expect("the root always exists");
    let directory = project
        .view
        .active
        .as_ref()
        .and_then(|path| project.tree.get(path))
        .unwrap_or(root);
    let items = directory
        .children
        .iter()
        .filter(|id| {
            project
                .tree
                .node(**id)
                .is_some_and(|node| project.view.show_hidden || !is_hidden_name(node.name()))
        })
        .count();
    let mut left = format!("{items} items");
    if project.scan.truncated || project.tree.truncated() {
        left.push_str(" · truncated");
    }
    text.paint(
        ui,
        pos2(rect.left() + 6.0, rect.center().y),
        Align2::LEFT_CENTER,
        left,
    );

    let right = match (&project.error, project.view.selection.iter().next()) {
        (Some(error), _) => error.to_string(),
        (None, Some(path)) => path.to_string(),
        (None, None) => String::from("no selection"),
    };
    text.paint(
        ui,
        pos2(rect.right() - 6.0, rect.center().y),
        Align2::RIGHT_CENTER,
        right,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::HostView;
    use egui::{Event, FullOutput, Modifiers, PointerButton, Pos2, RawInput};
    use kairos_editor_core::EditorState;
    use kairos_editor_core::project::ScanEntry;
    use kairos_editor_widgets::style::Style;

    /// The panel never reads the host in M2; an empty host keeps the harness
    /// honest about it.
    struct EmptyHost;

    impl HostView for EmptyHost {}

    const CANVAS: egui::Vec2 = egui::vec2(800.0, 600.0);

    /// One filled rect a frame painted, for geometry assertions.
    #[derive(Clone, Copy, Debug)]
    struct PaintedRect {
        rect: Rect,
        fill: egui::Color32,
    }

    fn flatten(shape: &egui::Shape, out: &mut Vec<PaintedRect>) {
        match shape {
            egui::Shape::Rect(rect) => out.push(PaintedRect {
                rect: rect.rect,
                fill: rect.fill,
            }),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| flatten(shape, out)),
            _ => {}
        }
    }

    /// Every text run a frame painted.
    fn painted_texts(output: &FullOutput) -> Vec<String> {
        fn walk(shape: &egui::Shape, out: &mut Vec<String>) {
            match shape {
                egui::Shape::Text(text) => out.push(text.galley.text().to_owned()),
                egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        for clipped in &output.shapes {
            walk(&clipped.shape, &mut out);
        }
        out
    }

    fn entry(path: &str, is_dir: bool) -> ScanEntry {
        ScanEntry {
            path: RelPath::new(path),
            is_dir,
        }
    }

    /// A panel wired to a fixed snapshot:
    ///
    /// `res` (folder) > `res/models` (folder) + `res/readme.md`, then `Library`
    /// (the system directory, which sorts last). Sorted root rows are
    /// `[res, Library]`; expanding `res` appends `models` and `readme.md`.
    struct ProjectHarness {
        ctx: egui::Context,
        state: EditorState,
        panel: ProjectPanel,
        out: UiOut,
        time: f64,
    }

    impl ProjectHarness {
        fn new(expanded: &[&str]) -> Self {
            let mut state = EditorState::default();
            state.project = ProjectState::at("D:/project");
            state.project.tree.assemble(
                &[
                    entry("res", true),
                    entry("res/models", true),
                    entry("res/models/Suzanne.png", false),
                    entry("res/readme.md", false),
                    entry("Library", true),
                    entry("Library/Logs", true),
                ],
                &[String::from("Library")],
                false,
            );
            for path in expanded {
                state.project.view.expanded.insert(RelPath::new(*path));
            }
            let mut harness = Self {
                ctx: egui::Context::default(),
                state,
                panel: ProjectPanel::default(),
                out: UiOut::new(),
                time: 0.0,
            };
            // egui hit-tests against the previous pass's widget rects, so the
            // first pointer event needs a frame of layout behind it.
            let _ = harness.frame_output(Vec::new());
            harness
        }

        fn frame_output(&mut self, events: Vec<Event>) -> FullOutput {
            self.time += 0.016;
            let mut input = RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, CANVAS));
            input.time = Some(self.time);
            input.events = events;
            let Self {
                ctx,
                state,
                panel,
                out,
                ..
            } = self;
            let frame = Frame::with_host(state, &EmptyHost);
            ctx.run_ui(input, |ui| panel.ui(ui, &frame, &mut *out))
        }

        /// One frame's actions.
        fn actions(&mut self, events: Vec<Event>) -> Vec<Action> {
            let _ = self.frame_output(events);
            self.out.take()
        }

        /// One frame's painted rects.
        fn painted(&mut self, events: Vec<Event>) -> Vec<PaintedRect> {
            let output = self.frame_output(events);
            let mut painted = Vec::new();
            for clipped in &output.shapes {
                flatten(&clipped.shape, &mut painted);
            }
            painted
        }

        fn painted_text(&mut self, events: Vec<Event>) -> Vec<String> {
            painted_texts(&self.frame_output(events))
        }

        /// A primary click spread over the press frame and the release frame,
        /// like a real pointer.
        fn click(&mut self, at: Pos2) -> Vec<Action> {
            let mut actions = self.actions(press(at));
            actions.extend(self.actions(release(at)));
            actions
        }

        /// Two clicks in quick succession: a double click on the second release.
        fn double_click(&mut self, at: Pos2) -> Vec<Action> {
            let mut actions = self.click(at);
            actions.extend(self.click(at));
            actions
        }
    }

    fn press(pos: Pos2) -> Vec<Event> {
        vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ]
    }

    fn release(pos: Pos2) -> Vec<Event> {
        vec![Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }]
    }

    fn key(key: egui::Key) -> Vec<Event> {
        vec![Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }]
    }

    /// Vertical centre of row `index`: the toolbar is one row, then 20 px rows.
    fn row_center(index: usize) -> Pos2 {
        let toolbar = 20.0 + 1.0;
        pos2(100.0, toolbar + 20.0 * index as f32 + 10.0)
    }

    /// The chevron hit box of the first row: left pad 4 plus the 13 px box.
    fn chevron(index: usize) -> Pos2 {
        pos2(10.0, row_center(index).y)
    }

    fn has(actions: &[Action], wanted: &ProjectAction) -> bool {
        actions
            .iter()
            .any(|action| action == &Action::Project(wanted.clone()))
    }

    #[test]
    fn a_click_selects_the_row() {
        let mut harness = ProjectHarness::new(&[]);
        let actions = harness.click(row_center(0));
        assert!(has(
            &actions,
            &ProjectAction::Select {
                path: Some(RelPath::new("res"))
            }
        ));
        assert!(
            !has(
                &actions,
                &ProjectAction::Navigate {
                    path: RelPath::new("res")
                }
            ),
            "a single click must not navigate"
        );
    }

    #[test]
    fn a_double_click_opens_a_folder_and_a_file_differs() {
        let mut harness = ProjectHarness::new(&[]);
        let actions = harness.double_click(row_center(0));
        assert!(has(
            &actions,
            &ProjectAction::Navigate {
                path: RelPath::new("res")
            }
        ));

        // With res expanded, row 2 is the file res/readme.md.
        let mut harness = ProjectHarness::new(&["res"]);
        let actions = harness.double_click(row_center(2));
        assert!(has(
            &actions,
            &ProjectAction::OpenRequested {
                path: RelPath::new("res/readme.md")
            }
        ));
        assert!(
            !has(
                &actions,
                &ProjectAction::Navigate {
                    path: RelPath::new("res/readme.md")
                }
            ),
            "a file opens, it does not navigate"
        );
    }

    #[test]
    fn the_chevron_toggles_without_selecting() {
        let mut harness = ProjectHarness::new(&[]);
        let actions = harness.click(chevron(0));
        assert!(has(
            &actions,
            &ProjectAction::ToggleFolder {
                path: RelPath::new("res")
            }
        ));
        assert!(
            !actions
                .iter()
                .any(|action| matches!(action, Action::Project(ProjectAction::Select { .. }))),
            "clicking the chevron must not select the row"
        );
    }

    #[test]
    fn the_toolbar_toggles_hidden_files() {
        let mut harness = ProjectHarness::new(&[]);
        let actions = harness.click(pos2(CANVAS.x - 4.0, 10.0));
        assert!(has(&actions, &ProjectAction::SetShowHidden(true)));
    }

    #[test]
    fn the_toolbar_collapses_all() {
        let mut harness = ProjectHarness::new(&["res"]);
        let actions = harness.click(pos2(4.0, 10.0));
        assert!(has(&actions, &ProjectAction::CollapseAll));
    }

    #[test]
    fn the_library_row_gets_a_separator_above_it() {
        let style = ProjectStyle::dark();
        let mut harness = ProjectHarness::new(&[]);
        let painted = harness.painted(Vec::new());
        let separator = style.separator.to_egui();
        let top = row_center(1).y - 10.0;
        assert!(
            painted.iter().any(|painted| painted.fill == separator
                && (painted.rect.top() - top).abs() < 0.5
                && painted.rect.width() > 10.0),
            "the system directory needs a 1 px rule above it"
        );
    }

    #[test]
    fn the_selected_row_is_filled_with_the_selection_colour() {
        let style = ProjectStyle::dark();
        let mut harness = ProjectHarness::new(&[]);
        harness
            .state
            .project
            .view
            .selection
            .insert(RelPath::new("res"));
        let painted = harness.painted(Vec::new());
        assert!(
            painted
                .iter()
                .any(|painted| painted.fill == style.selected_bg.to_egui()
                    && (painted.rect.top() - row_center(0).y + 10.0).abs() < 0.5),
            "the selected row must be filled"
        );
    }

    #[test]
    fn the_status_bar_counts_the_active_directory() {
        let mut harness = ProjectHarness::new(&[]);
        let texts = harness.painted_text(Vec::new());
        assert!(
            texts.iter().any(|text| text == "2 items"),
            "the root has res and Library: {texts:?}"
        );
        assert!(texts.iter().any(|text| text == "no selection"));

        // res/models holds one file, so the count follows the active directory
        // rather than the root's two children.
        harness.state.project.view.active = Some(RelPath::new("res/models"));
        let texts = harness.painted_text(Vec::new());
        assert!(
            texts.iter().any(|text| text == "1 items"),
            "the count must follow the active directory: {texts:?}"
        );
    }

    #[test]
    fn the_breadcrumb_root_navigates() {
        let mut harness = ProjectHarness::new(&[]);
        harness.state.project.view.active = Some(RelPath::new("res"));
        // The right pane starts at the split fraction; its first crumb is the
        // project root, so clicking just inside the pane navigates to the root.
        let boundary = harness.state.project.view.tree_split * CANVAS.x;
        let actions = harness.click(pos2(boundary + 8.0, 31.0));
        assert!(
            actions.iter().any(|action| matches!(
                action,
                Action::Project(ProjectAction::Navigate { path }) if path.is_root()
            )),
            "clicking the root crumb must navigate to the project root: {actions:?}"
        );
    }

    #[test]
    fn arrow_keys_move_the_selection_while_the_tree_has_focus() {
        let mut harness = ProjectHarness::new(&[]);
        // A click focuses the tree: the widget reports it on the same pass and
        // the panel remembers it for the next one.
        let _ = harness.click(row_center(0));
        let actions = harness.actions(key(egui::Key::ArrowDown));
        assert!(
            has(
                &actions,
                &ProjectAction::Select {
                    path: Some(RelPath::new("Library"))
                }
            ),
            "ArrowDown moves the selection to the next visible row: {actions:?}"
        );
    }

    #[test]
    fn enter_opens_the_selected_file() {
        let mut harness = ProjectHarness::new(&["res"]);
        harness
            .state
            .project
            .view
            .selection
            .insert(RelPath::new("res/readme.md"));
        // Focus the tree first.
        let _ = harness.click(row_center(0));
        let actions = harness.actions(key(egui::Key::Enter));
        assert!(has(
            &actions,
            &ProjectAction::OpenRequested {
                path: RelPath::new("res/readme.md")
            }
        ));
    }

    #[test]
    fn the_panel_owns_its_scroll_areas() {
        assert_eq!(ProjectPanel::default().scroll_bars(), [false, false]);
    }
}
