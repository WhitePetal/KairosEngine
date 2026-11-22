//! Project panel: the two-column asset browser (M2).
//!
//! Layout (D1, Unity/Tuanjie Project window): a full-width toolbar, then the
//! folder tree on the left and the content pane on the right, then a full-width
//! status bar. The right pane is a breadcrumb plus a placeholder in v1; the asset
//! grid fills it in M9 without changing this layout.
//!
//! The panel selects, expands, navigates and opens, and since M3 it also asks
//! for changes: the row context menu starts a create, an inline rename and the
//! delete confirmation, all of which leave through [`UiOut`] as a
//! [`ProjectAction`]. The panel never mutates state itself, and it never touches
//! the file system: the reducer validates, the app executes.
//!
//! What lives where:
//!
//! * core owns the intent that survives a panel close: expansion, the active
//!   directory, selection, sort, hidden files;
//! * this instance owns what only affects a frame: the splitter's in-flight
//!   fraction, the row cache, the row to scroll into view next pass, whether the
//!   tree had keyboard focus last pass, and the transient M3 interaction state
//!   (the row menu, the rename buffer, the delete dialog, the pending create);
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

use egui::{
    Align2, Context, CornerRadius, Key, Pos2, Rect, Sense, Ui, Vec2, WidgetText, pos2, vec2,
};
use kairos_editor_core::project::{
    CreateEntry, CreateKind, Node, NodeId, ProjectState, ProjectTree, RelPath, ScanReason,
    SelectMode, validate_name,
};
use kairos_editor_core::{Action, ProjectAction};
use kairos_editor_widgets::button::{Button, ButtonAlign, ButtonStyle};
use kairos_editor_widgets::color::ColorExt;
use kairos_editor_widgets::menu::{self, MenuStyle};
use kairos_editor_widgets::modal::{Modal, ModalButton, ModalChoice, ModalRole};
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
/// The delete dialog's buttons. The safe choice is first because Enter presses
/// the first non-danger button: a destructive action is only ever a click.
const DELETE_BUTTONS: [ModalButton<'static>; 2] = [
    ModalButton::new("Cancel", ModalRole::Default),
    ModalButton::new("Delete", ModalRole::Danger),
];
/// How long the panel waits for a created row before giving up on starting its
/// inline rename. A create that failed never produces a row.
const PENDING_CREATE_FRAMES: u32 = 180;

/// The row context menu: the row it targets, where it opened, and whether it is
/// still up.
///
/// Panel state, like everything else in this module: a menu must not survive
/// closing the tab, and reopening a tab must not pop a menu for a row nobody
/// clicked.
struct RowMenu {
    path: RelPath,
    pos: Pos2,
    open: bool,
}

/// The panel's half of the inline rename editor.
///
/// The widget owns the row geometry and the key handling (Enter or blur commit,
/// Escape cancel); this owns the target, the buffer and the message that keeps
/// the editor open when the name table rejects the text.
struct RenameEdit {
    path: RelPath,
    buffer: String,
    error: Option<String>,
}

/// The delete confirmation dialog and where its title strip has been dragged.
///
/// The offset is the caller's (a dialog position is view state): opening the
/// dialog again starts at the screen centre rather than at the last drag.
struct DeleteDialog {
    paths: Vec<RelPath>,
    offset: Vec2,
}

/// A tree drag in flight: the entries being moved, the pointer, and the
/// directory the pointer would drop them into (validated against the tree).
///
/// The widget reports the interaction (start, target row, release); the payload,
/// the ghost and the move rules are the panel's, because the widget has no
/// domain types (T2).
struct DragMove {
    sources: Vec<RelPath>,
    pos: Pos2,
    target: Option<RelPath>,
    /// What the ghost shows: the single entry's display name, or the count.
    label: String,
}

/// The move-blocked dialog (T7): the drag ended on a directory that already has
/// an entry with one of the dragged names, so nothing moves.
struct MoveBlockedDialog {
    path: RelPath,
    target: RelPath,
    offset: Vec2,
}

/// A create whose result should start renaming as soon as its row appears (T8).
///
/// The wait is bounded: a create that failed (name taken, folder gone) never
/// produces a row, and a pending rename that outlived the attempt would fire
/// against some future file that happens to have the same path.
struct PendingCreate {
    path: RelPath,
    waited: u32,
}

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
    /// First pass of this panel instance: scroll the row core still has selected
    /// into view. Closing the tab drops the panel (and its scroll position), not
    /// the state; reopening must land where the user left off.
    reveal_selection: bool,
    /// The row context menu, while one is open.
    menu: Option<RowMenu>,
    /// The inline rename in flight, if any.
    rename: Option<RenameEdit>,
    /// A create waiting for its row, so the new entry starts with its name
    /// selected instead of a generic "New Folder".
    pending_create: Option<PendingCreate>,
    /// The delete confirmation dialog; `Some` means it is open.
    delete_confirm: Option<DeleteDialog>,
    /// The tree drag in flight, if any.
    drag: Option<DragMove>,
    /// The move-blocked dialog; `Some` means it is open.
    move_blocked: Option<MoveBlockedDialog>,
}

impl Default for ProjectPanel {
    fn default() -> Self {
        Self {
            style: ProjectStyle::default(),
            rows: ProjectRows::default(),
            split: None,
            scroll_to: None,
            tree_focused: false,
            reveal_selection: true,
            menu: None,
            rename: None,
            pending_create: None,
            delete_confirm: None,
            drag: None,
            move_blocked: None,
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
        // Per-type row icons: the table names the files, the skin names the
        // folder (T16). Preloading is best-effort; the widget loads lazily too.
        for kind in kairos_editor_core::project::FileTypeRegistry::builtin().types() {
            if !kind.icon.is_empty() {
                ui_image::preload(ctx, &format!("{}/{}", self.style.type_icon_root, kind.icon));
            }
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
            reveal_selection,
            menu,
            rename,
            pending_create,
            delete_confirm,
            drag,
            move_blocked,
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
        // The project root is the thumbnail key's first half; a lossy conversion
        // is the same one the scanner uses for paths, so a non-UTF-8 root still
        // resolves the same file the tree shows.
        let root = project.root.to_string_lossy();
        // The host decodes and caches texture previews; the row projection only
        // receives the resulting texture ids (see `HostView::project_thumbnail`).
        let host = frame.host;
        let ctx = ui.ctx().clone();
        let thumbnails =
            move |root: &str, relative: &str| host.project_thumbnail(&ctx, root, relative);
        // Resolve the type table's icon file names against the skin's folder (T16).
        // The row closure borrows these paths, so they have to live in this pass;
        // a handful of small strings is cheaper than a per-row lookup chain.
        let type_icon_paths: Vec<(&'static str, String)> = project
            .file_types
            .types()
            .filter(|kind| !kind.icon.is_empty())
            .map(|kind| (kind.id, format!("{}/{}", style.type_icon_root, kind.icon)))
            .collect();
        let type_icons: Vec<(&'static str, &str)> = type_icon_paths
            .iter()
            .map(|(id, path)| (*id, path.as_str()))
            .collect();
        let icons = Icons {
            folder: &style.icons.folder,
            folder_open: &style.icons.folder_open,
            file: &style.icons.file,
            types: &type_icons,
            thumbnails: Some(&thumbnails),
        };
        let row_ids = rows.update(&project.tree, &project.view, &project.file_types);

        // A create asks for the new row to start in rename mode (T8); a failed
        // one never produces the row, and the pending wait is bounded.
        pending_create_ready(project, pending_create, rename);

        // One text cursor at a time: the inline editor, the confirmation dialog
        // and the context menu all own the keys they need before the tree's own
        // bindings run. `egui::Modal` blocks pointer hits but not raw keyboard
        // events, so the dialog must be handled here too (T13 pitfall 2).
        let keyboard_owner = rename.is_some()
            || delete_confirm.is_some()
            || move_blocked.is_some()
            || menu.is_some();

        // Keyboard first: the pass that moves the selection also scrolls it into
        // view, because the widget must be told the target before it draws.
        if *tree_focused && !keyboard_owner {
            keyboard(ui, project, row_ids, scroll_to, rename, delete_confirm, out);
        }
        // Reopening the tab rebuilds this panel but core still holds the
        // selection, so the first pass brings that row back into view. A keyboard
        // move on the same pass wins, because it already set `scroll_to`.
        if *reveal_selection {
            *reveal_selection = false;
            if scroll_to.is_none() {
                *scroll_to = project
                    .view
                    .selection
                    .iter()
                    .next()
                    .cloned()
                    .or_else(|| project.view.active.clone());
            }
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
        let editing_id = rename
            .as_ref()
            .and_then(|edit| project.tree.find(&edit.path));
        // The drop highlight is the panel's decision (the target is validated),
        // so the widget only paints the row it is told about.
        let highlight_id = drag
            .as_ref()
            .and_then(|state| state.target.as_ref())
            .and_then(|path| project.tree.find(path));
        let mut tree_output = TreeOutput::default();
        ui.scope_builder(egui::UiBuilder::new().max_rect(tree_rect), |ui| {
            let mut view = TreeView::new("project-tree", row_ids, &tree_style, |id| {
                rows::row(
                    project_tree,
                    project_view,
                    &project.file_types,
                    &root,
                    icons,
                    id,
                )
                .unwrap_or_else(rows::missing_row)
            });
            if let Some(id) = scroll_to_id {
                view = view.scroll_to(id);
            }
            if let Some(id) = highlight_id {
                // The row wash says what is under the pointer; the region box
                // says which folder owns that row (its whole visible subtree).
                view = view.highlight(id).highlight_region(id);
            }
            if let Some(id) = editing_id
                && let Some(edit) = rename.as_mut()
            {
                view = view.editing(id, &mut edit.buffer);
            }
            let output = view.show(ui);
            *tree_focused = output.focused;
            let modifiers = ui.input(|input| input.modifiers);
            row_actions(project, &output, modifiers, menu, out);
            edit_actions(project, &output, rename, out);
            tree_output = output;
        });
        // The drag is advanced after the pass that reported it: the payload is
        // captured on the start frame, the target follows the pointer, and the
        // release becomes one move action or the blocked dialog (T7).
        drag_actions(ui, project, &tree_output, drag, move_blocked, out);

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
        // The panes above were laid out from this pass's starting width, so a
        // drag's result is carried to the next pass here (M2 acceptance fix:
        // without this the boundary snapped back and the splitter looked
        // undraggable even though it changed the cursor). The action still leaves
        // once, on release (M2 定稿 补齐 3), so a drag is not sixty actions.
        if total > 0.0 {
            if response.changed() {
                *split = Some(left_width / total);
            }
            if response.drag_stopped() {
                out.push(Action::Project(ProjectAction::SetTreeSplit(
                    left_width / total,
                )));
                // Follow core again once the action lands; holding the local
                // value would mask an external change to the stored split.
                *split = None;
            }
        }

        status_bar(
            ui,
            style,
            project,
            status_rect,
            boundary,
            rename.as_ref().and_then(|edit| edit.error.as_deref()),
        );

        // The drag ghost follows the pointer on the tooltip layer, above both
        // panes. It is drawn while the button is held, which is exactly the
        // window in which the drop target is being chosen.
        if let Some(state) = drag.as_ref() {
            drag_ghost(ui, frame, project, style, state);
        }

        // Overlays last: the menu and the dialog are separate layers, but this is
        // the pass that decides their state, and a menu opened by this pass's
        // right click has to be drawn now, not one frame late.
        row_context_menu(
            ui,
            project,
            menu,
            rename,
            pending_create,
            delete_confirm,
            out,
        );
        delete_dialog(ui, frame, delete_confirm, out);
        move_blocked_dialog(ui, frame, move_blocked);
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
        // The project's toolbars close every segment with a 1 px chrome rule
        // (Console, Profiler); without one the labels read as plain text.
        x = toolbar_separator(&painter, row, style, rect.right());
    }

    // Hidden files is Unity's eye toggle (the Project browser's own visibility
    // affordance): icon-only and square, two states. A label would read as a
    // fourth text segment, and the open / crossed-out eye already answers the only
    // question a count could ("are hidden entries shown?"); a count would also go
    // stale with every directory change, so it stays out of the bar.
    let icon = if project.view.show_hidden {
        &style.icons.eye_open
    } else {
        &style.icons.eye_crossed
    };
    // Square: a 16 px eye in Unity's 20 px toolbar cell.
    let side = style.toolbar_height.max(style.hidden_icon_size + 2.0);
    let rect = Rect::from_min_size(
        pos2(row.right() - side, row.top()),
        vec2(side, style.toolbar_height),
    );
    // A rule leads into the right-aligned group, like the Console's counts.
    toolbar_separator(&painter, row, style, rect.left() - style.separator_width);
    let tooltip = if project.view.show_hidden {
        "Hide hidden files"
    } else {
        "Show hidden files"
    };
    let response = ui.put(
        rect,
        Button::icon_only(icon, style.hidden_icon_size, &button_style)
            .width(side)
            .height(style.toolbar_height)
            .selected(project.view.show_hidden)
            .tooltip(tooltip)
            .id(ui.id().with("project-toolbar-hidden")),
    );
    if response.clicked() {
        out.push(Action::Project(ProjectAction::SetShowHidden(
            !project.view.show_hidden,
        )));
    }

    row
}

/// Paint Unity's 1 px chrome separator at `x` and return where the next toolbar
/// segment starts (the Console toolbar's rule).
fn toolbar_separator(painter: &egui::Painter, row: Rect, style: &ProjectStyle, x: f32) -> f32 {
    painter.rect_filled(
        Rect::from_min_size(
            pos2(x, row.top()),
            vec2(style.separator_width, style.toolbar_height),
        ),
        CornerRadius::ZERO,
        style.separator.to_egui(),
    );
    x + style.separator_width
}

/// Row interactions: the triangle toggles, a click selects, a double click opens.
///
/// Order matters: egui reports `clicked` and `double_clicked` on the second press
/// of a double click, so the double click is handled first and the single click
/// only when it is not one.
fn row_actions(
    project: &ProjectState,
    output: &TreeOutput<NodeId>,
    modifiers: egui::Modifiers,
    menu: &mut Option<RowMenu>,
    out: &mut UiOut,
) {
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
        // The desktop selection gestures: Shift extends from core's anchor (the
        // row the last click or keyboard move picked; it lives with the
        // selection so it survives a rescan and a closed/reopened panel), Ctrl
        // toggles one row, a plain click replaces and re-anchors.
        let (paths, mode) = if modifiers.shift {
            let anchor = project
                .view
                .anchor
                .clone()
                .unwrap_or_else(|| node.path.clone());
            (vec![anchor, node.path.clone()], SelectMode::Range)
        } else if modifiers.ctrl || modifiers.command {
            (vec![node.path.clone()], SelectMode::Toggle)
        } else {
            (vec![node.path.clone()], SelectMode::Replace)
        };
        out.push(Action::Project(ProjectAction::Select { paths, mode }));
    }

    // Unity's rule: right-clicking a row selects it unless it is already part of
    // a multi-selection, and opens the menu at the pointer.
    if let Some(id) = output.secondary_clicked
        && let Some(node) = project.tree.node(id)
    {
        if !project.view.selection.contains(&node.path) {
            out.push(Action::Project(ProjectAction::Select {
                paths: vec![node.path.clone()],
                mode: SelectMode::Replace,
            }));
        }
        *menu = Some(RowMenu {
            path: node.path.clone(),
            pos: output.secondary_pos.unwrap_or_else(|| pos2(0.0, 0.0)),
            open: true,
        });
    }
}

/// What a clicked row-menu item asked the panel to do.
///
/// The menu callback runs while the popup owns the draw, so it records one
/// command instead of mutating panel state mid-popup; the panel applies it once
/// the menu has closed.
enum MenuCommand {
    Create {
        parent: RelPath,
        name: &'static str,
        kind: CreateKind,
    },
    Rename {
        path: RelPath,
    },
    CopyPath {
        path: RelPath,
    },
    Reveal {
        path: RelPath,
    },
    Delete {
        paths: Vec<RelPath>,
    },
}

/// The row context menu, drawn where the right click landed.
///
/// The widget reports secondary_clicked and the pointer position; the menu
/// itself is the panel's, because a menu is actions and the widget has no
/// concept of an action.
fn row_context_menu(
    ui: &mut Ui,
    project: &ProjectState,
    menu: &mut Option<RowMenu>,
    rename: &mut Option<RenameEdit>,
    pending_create: &mut Option<PendingCreate>,
    delete_confirm: &mut Option<DeleteDialog>,
    out: &mut UiOut,
) {
    let Some(state) = menu.as_mut() else {
        return;
    };
    let path = state.path.clone();
    let pos = state.pos;
    let Some(node) = project.tree.get(&path) else {
        // The row vanished under the menu (a watcher rescan): drop it whole.
        *menu = None;
        return;
    };
    // A create lands in the right-clicked folder, or next to a right-clicked
    // file: the two answers Unity gives for "new asset here".
    let directory = if node.is_folder() {
        path.clone()
    } else {
        path.parent().unwrap_or_default()
    };

    let mut command: Option<MenuCommand> = None;
    let mut open = state.open;
    let menu_style = MenuStyle::default();
    // Whether the popup was shown this pass is egui's business; the caller only
    // reads the open flag back.
    let _ = menu::context_menu(ui, &menu_style, &path, pos, &mut open, |ui| {
        menu::submenu(ui, "Create", |ui| {
            create_menu(
                ui,
                project.file_types.create_entries(),
                &[],
                &directory,
                &mut command,
            );
        });
        menu::menu_separator(ui);
        if menu::menu_item(ui, "Rename").clicked() {
            command = Some(MenuCommand::Rename { path: path.clone() });
        }
        if menu::menu_item(ui, "Copy Path").clicked() {
            command = Some(MenuCommand::CopyPath { path: path.clone() });
        }
        if menu::menu_item(ui, "Reveal").clicked() {
            command = Some(MenuCommand::Reveal { path: path.clone() });
        }
        menu::menu_separator(ui);
        if menu::menu_item(ui, "Delete").clicked() {
            command = Some(MenuCommand::Delete {
                paths: delete_targets(project, &path),
            });
        }
    });

    // A picked item closes the menu; so does egui (outside click, Escape), which
    // writes false through the open flag.
    state.open = open && command.is_none();
    if !state.open {
        *menu = None;
    }
    if let Some(command) = command {
        apply_menu_command(
            ui,
            project,
            command,
            rename,
            pending_create,
            delete_confirm,
            out,
        );
    }
}

/// One level of the Create submenu, driven by the type table's entries (T15).
///
/// The table owns the labels and the tree of submenus: an entry with an empty
/// menu path is an item of this level, an entry with a longer path is one level
/// under its next segment. Adding a creatable type stays a data change.
fn create_menu(
    ui: &mut Ui,
    entries: &'static [CreateEntry],
    path: &[&'static str],
    directory: &RelPath,
    command: &mut Option<MenuCommand>,
) {
    for entry in entries.iter().filter(|entry| entry.menu == path) {
        if menu::menu_item(ui, entry.label).clicked() {
            *command = Some(MenuCommand::Create {
                parent: directory.clone(),
                name: entry.default_stem,
                kind: entry.target,
            });
        }
    }

    let mut groups: Vec<&'static str> = Vec::new();
    for entry in entries {
        if entry.menu.len() <= path.len() || !entry.menu.starts_with(path) {
            continue;
        }
        let label = entry.menu[path.len()];
        if groups.contains(&label) {
            continue;
        }
        groups.push(label);
        let mut child = path.to_vec();
        child.push(label);
        menu::submenu(ui, label, |ui| {
            create_menu(ui, entries, &child, directory, command);
        });
    }
}

/// Apply one picked menu item.
fn apply_menu_command(
    ui: &mut Ui,
    project: &ProjectState,
    command: MenuCommand,
    rename: &mut Option<RenameEdit>,
    pending_create: &mut Option<PendingCreate>,
    delete_confirm: &mut Option<DeleteDialog>,
    out: &mut UiOut,
) {
    match command {
        MenuCommand::Create { parent, name, kind } => {
            *pending_create = Some(PendingCreate {
                path: created_path(&parent, name, kind),
                waited: 0,
            });
            out.push(Action::Project(ProjectAction::CreateRequested {
                parent,
                name: name.to_owned(),
                kind,
            }));
        }
        MenuCommand::Rename { path } => {
            if let Some(node) = project.tree.get(&path) {
                *rename = Some(rename_edit(node));
            }
        }
        // The panel does not own the OS clipboard: it asks egui to put a string
        // there, the same call a text field's Ctrl+C makes.
        MenuCommand::CopyPath { path } => ui.ctx().copy_text(path.to_string()),
        MenuCommand::Reveal { path } => {
            out.push(Action::Project(ProjectAction::RevealRequested { path }));
        }
        MenuCommand::Delete { paths } => {
            *delete_confirm = Some(DeleteDialog {
                paths,
                offset: Vec2::ZERO,
            });
        }
    }
}

/// The paths a delete will trash: the whole selection when the right-clicked row
/// is part of it, otherwise just that row (Unity's rule).
fn delete_targets(project: &ProjectState, path: &RelPath) -> Vec<RelPath> {
    if project.view.selection.contains(path) {
        project.view.selection.iter().cloned().collect()
    } else {
        vec![path.clone()]
    }
}

/// The path a create of this name will produce: the reducer appends the
/// template's extension exactly when it is missing.
fn created_path(parent: &RelPath, name: &str, kind: CreateKind) -> RelPath {
    parent.child(&match kind {
        CreateKind::Folder => name.to_owned(),
        CreateKind::File { extension, .. } => format!("{name}.{extension}"),
    })
}

/// Start the inline rename of a freshly created row once it exists.
///
/// The reducer's completion rescans; this waits for that scan to put the row in
/// the tree and then hands the name to the row editor. A failed create never
/// produces the row, so the wait is bounded (PENDING_CREATE_FRAMES).
fn pending_create_ready(
    project: &ProjectState,
    pending: &mut Option<PendingCreate>,
    rename: &mut Option<RenameEdit>,
) {
    let Some(create) = pending.as_mut() else {
        return;
    };
    if rename.is_some() {
        // The user is renaming something else; do not steal the editor.
        return;
    }
    if let Some(node) = project.tree.get(&create.path) {
        *rename = Some(rename_edit(node));
        *pending = None;
        return;
    }
    create.waited = create.waited.saturating_add(1);
    if create.waited > PENDING_CREATE_FRAMES {
        *pending = None;
    }
}

/// The rename editor for one node.
///
/// The buffer is the name the row shows, not the entry's whole name: a file's
/// extension is its type, shown in the suffix column, and must not be editable
/// through a rename. The suffix is put back when the commit becomes an action.
fn rename_edit(node: &Node) -> RenameEdit {
    RenameEdit {
        path: node.path.clone(),
        buffer: rows::display_name(node.name(), node.is_folder()).to_owned(),
        error: None,
    }
}

/// The inline editor's three endings, applied to the panel's rename state.
///
/// Commit and cancel come from the widget (Enter or blur, Escape); the panel
/// validates and decides whether the editor closes. A rejected name keeps it
/// open with the message in the status bar, so the user can fix the text instead
/// of retyping the whole gesture.
fn edit_actions(
    project: &ProjectState,
    output: &TreeOutput<NodeId>,
    rename: &mut Option<RenameEdit>,
    out: &mut UiOut,
) {
    let Some(edit) = rename.as_ref() else {
        return;
    };
    let Some(node) = project.tree.get(&edit.path) else {
        // The row vanished under the editor (a watcher rescan): close it.
        *rename = None;
        return;
    };
    if output.edit_cancelled {
        *rename = None;
        return;
    }
    if !output.edit_committed {
        return;
    }
    let path = edit.path.clone();
    let buffer = edit.buffer.clone();
    // The buffer is the name without its extension, so it is validated as a name
    // on its own: an empty buffer would otherwise build ".png", which is a valid
    // hidden file name rather than the error the user made.
    if let Err(error) = validate_name(&buffer) {
        if let Some(edit) = rename.as_mut() {
            edit.error = Some(error.message());
        }
        return;
    }
    // Put the original extension back: the editor never showed it, so a rename
    // cannot drop or change a file's type.
    let name = match (node.is_folder(), rows::suffix(node.name())) {
        (false, Some(extension)) => format!("{buffer}.{extension}"),
        _ => buffer,
    };
    let unchanged = node.name() == name;
    *rename = None;
    if !unchanged {
        out.push(Action::Project(ProjectAction::RenameRequested {
            path,
            name,
        }));
    }
}

/// The delete confirmation dialog (D5: files and folders both ask first).
///
/// It is a panel-local dialog, not core state: a half-open confirmation must not
/// survive closing the tab. Enter presses Cancel, because the danger button is
/// never the default.
fn delete_dialog(
    ui: &mut Ui,
    frame: &Frame,
    delete_confirm: &mut Option<DeleteDialog>,
    out: &mut UiOut,
) {
    let Some(dialog) = delete_confirm.as_mut() else {
        return;
    };
    let paths = dialog.paths.clone();
    let body = if paths.len() == 1 {
        format!(
            "Delete \"{}\"?\n\nIt will be moved to the recycle bin.",
            paths[0].name()
        )
    } else {
        format!(
            "Delete {} items?\n\nThey will be moved to the recycle bin.",
            paths.len()
        )
    };
    let style = frame.modal_style();
    // The dialog is a window: its title strip drags it, and the panel owns the
    // remembered offset so reopening it starts centred again.
    let choice = Modal::new("project-delete", "Delete", &body)
        .buttons(&DELETE_BUTTONS)
        .show_draggable(ui, &style, &mut dialog.offset);
    match choice {
        Some(ModalChoice::Button(1)) => {
            *delete_confirm = None;
            out.push(Action::Project(ProjectAction::DeleteRequested { paths }));
        }
        Some(_) => *delete_confirm = None,
        None => {}
    }
}

/// Keyboard navigation over the visible rows, Unity's bindings.
///
/// Handled only while the tree has focus, which the widget reports, so the
/// search box (M6) can own the same keys. The arrow keys keep the cursor on a
/// visible row; Shift+arrow extends the selection from core's anchor. The
/// command keys are the panel's own (F2 rename, Delete trash, Ctrl+A select all,
/// Escape clear).
fn keyboard(
    ui: &Ui,
    project: &ProjectState,
    row_ids: &[NodeId],
    scroll_to: &mut Option<RelPath>,
    rename: &mut Option<RenameEdit>,
    delete_confirm: &mut Option<DeleteDialog>,
    out: &mut UiOut,
) {
    let tree = &project.tree;
    let view = &project.view;
    // The keyboard starts from the anchor (the row the last click, toggle or
    // arrow picked), falling back to the first selected row when the anchor is
    // scrolled or collapsed out of view.
    let visible = |path: &RelPath| {
        row_ids
            .iter()
            .any(|id| tree.node(*id).is_some_and(|node| &node.path == path))
    };
    let current = view
        .anchor
        .clone()
        .filter(|path| visible(path))
        .or_else(|| view.selection.iter().next().cloned());
    let index = current
        .as_ref()
        .and_then(|path| {
            row_ids
                .iter()
                .position(|id| tree.node(*id).is_some_and(|node| &node.path == path))
        })
        .unwrap_or(0);
    let selected = current.as_ref().and_then(|path| tree.get(path));
    let last_index = row_ids.len().saturating_sub(1);

    let mut next: Option<NodeId> = None;
    let mut extend = false;
    ui.input(|input| {
        // Ctrl on Windows/Linux, Command on macOS; a synthetic RawInput may set
        // only ctrl, so accept either spelling of "the command modifier".
        if (input.modifiers.command || input.modifiers.ctrl) && input.key_pressed(Key::A) {
            let paths: Vec<RelPath> = row_ids
                .iter()
                .filter_map(|id| tree.node(*id))
                .map(|node| node.path.clone())
                .collect();
            if !paths.is_empty() {
                out.push(Action::Project(ProjectAction::Select {
                    paths,
                    mode: SelectMode::Replace,
                }));
            }
        } else if input.key_pressed(Key::F2) {
            if let Some(path) = rename_target(project, view.anchor.as_ref())
                && let Some(node) = tree.get(&path)
            {
                *rename = Some(rename_edit(node));
            }
        } else if input.key_pressed(Key::Delete) {
            let paths: Vec<RelPath> = view.selection.iter().cloned().collect();
            if !paths.is_empty() {
                *delete_confirm = Some(DeleteDialog {
                    paths,
                    offset: Vec2::ZERO,
                });
            }
        } else if input.key_pressed(Key::Escape) {
            if !view.selection.is_empty() {
                out.push(Action::Project(ProjectAction::Select {
                    paths: Vec::new(),
                    mode: SelectMode::Replace,
                }));
            }
        } else if input.key_pressed(Key::ArrowDown) {
            next = row_ids.get((index + 1).min(last_index)).copied();
            extend = input.modifiers.shift;
        } else if input.key_pressed(Key::ArrowUp) {
            next = row_ids.get(index.saturating_sub(1)).copied();
            extend = input.modifiers.shift;
        } else if input.key_pressed(Key::Home) {
            next = row_ids.first().copied();
            extend = input.modifiers.shift;
        } else if input.key_pressed(Key::End) {
            next = row_ids.last().copied();
            extend = input.modifiers.shift;
        } else if input.key_pressed(Key::ArrowLeft) {
            if let Some(node) = selected {
                if node.is_folder() && view.expanded.contains(&node.path) {
                    out.push(Action::Project(ProjectAction::ToggleFolder {
                        path: node.path.clone(),
                    }));
                } else if let Some(parent) = node.path.parent() {
                    out.push(Action::Project(ProjectAction::Select {
                        paths: vec![parent],
                        mode: SelectMode::Replace,
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
        // Shift+arrow extends from the anchor (core keeps it put); a plain arrow
        // replaces the selection and re-anchors on the row it moved to.
        let (paths, mode) = if extend {
            let anchor = view.anchor.clone().unwrap_or_else(|| node.path.clone());
            (vec![anchor, node.path.clone()], SelectMode::Range)
        } else {
            (vec![node.path.clone()], SelectMode::Replace)
        };
        out.push(Action::Project(ProjectAction::Select { paths, mode }));
        *scroll_to = Some(node.path.clone());
    }
}

/// The entry F2 renames: the selection anchor when it is still selected, else
/// the first selected path. A multi-selection renames one entry at a time.
fn rename_target(project: &ProjectState, anchor: Option<&RelPath>) -> Option<RelPath> {
    if let Some(anchor) = anchor
        && project.view.selection.contains(anchor)
        && project.tree.find(anchor).is_some()
    {
        return Some(anchor.clone());
    }
    project
        .view
        .selection
        .iter()
        .find(|path| project.tree.find(path).is_some())
        .cloned()
}

/// Advance the tree drag: capture the payload on the start frame, follow the
/// pointer, and turn the release into one move action or the blocked dialog.
///
/// The widget reports drag_started / dragging / drop_target / drag_stopped; the
/// payload, the ghost and the rules (a directory, not inside the moved entry,
/// not the same parent, no name collision) are the panel's (T2/T7).
fn drag_actions(
    ui: &Ui,
    project: &ProjectState,
    output: &TreeOutput<NodeId>,
    drag: &mut Option<DragMove>,
    move_blocked: &mut Option<MoveBlockedDialog>,
    out: &mut UiOut,
) {
    if let Some(id) = output.drag_started
        && let Some(node) = project.tree.node(id)
    {
        let sources = if project.view.selection.contains(&node.path) {
            let selected: Vec<RelPath> = project.view.selection.iter().cloned().collect();
            top_level_paths(&project.tree, selected)
        } else {
            vec![node.path.clone()]
        };
        let label = match sources.as_slice() {
            [only] => project
                .tree
                .get(only)
                .map(|node| rows::display_name(node.name(), node.is_folder()).to_owned())
                .unwrap_or_else(|| only.name().to_owned()),
            sources => format!("{} items", sources.len()),
        };
        *drag = Some(DragMove {
            sources,
            pos: pointer_position(ui).unwrap_or_else(|| ui.max_rect().center()),
            target: None,
            label,
        });
    }

    if let Some(state) = drag.as_mut() {
        if let Some(pos) = pointer_position(ui) {
            state.pos = pos;
        }
        // The release frame reports where the drag ended, so it has to be
        // resolved as well: otherwise the move would use the row the pointer
        // was over one frame earlier.
        if output.dragging || output.drag_stopped {
            state.target = output
                .drop_target
                .and_then(|id| drop_directory(&project.tree, &state.sources, id));
        }
    }

    if output.drag_stopped
        && let Some(state) = drag.take()
        && let Some(target) = state.target
    {
        if let Some(path) = move_conflict(&project.tree, &state.sources, &target) {
            // T7: a name that already exists in the target is a blocking dialog,
            // not a half-done move; the reducer repeats the check for a stale
            // snapshot, but the panel must not ask it to.
            *move_blocked = Some(MoveBlockedDialog {
                path,
                target,
                offset: Vec2::ZERO,
            });
        } else {
            out.push(Action::Project(ProjectAction::MoveRequested {
                paths: state.sources,
                target,
            }));
        }
    }
}

/// The pointer position in screen points while a drag is in flight.
fn pointer_position(ui: &Ui) -> Option<Pos2> {
    ui.input(|input| input.pointer.interact_pos())
}

/// Drop the entries of a selection that another selected entry contains.
///
/// The reducer applies the same rule; the panel needs it too, because the ghost
/// promises "N items" and the blocked dialog names one entry.
fn top_level_paths(tree: &ProjectTree, paths: Vec<RelPath>) -> Vec<RelPath> {
    let mut kept: Vec<RelPath> = Vec::new();
    for path in paths {
        if tree.find(&path).is_none() {
            continue;
        }
        if kept.iter().any(|kept| path.is_under(kept)) {
            continue;
        }
        kept.retain(|kept| !kept.is_under(&path));
        kept.push(path);
    }
    kept
}

/// The directory a drag would drop into, when the drop is a real move.
///
/// The target is the **nearest folder owning the hovered row**, not the row
/// itself: pointing at a child of an expanded folder drops into that folder.
/// Unity/Tuanjie behave the same way, and it is the only way to reach a folder
/// whose children fill the viewport. Rejected here, before anything is
/// highlighted: no folder above the row (impossible for a row), the dragged
/// entry itself or a descendant of one (that would destroy it), and the
/// directory every entry already lives in (a no-op).
fn drop_directory(tree: &ProjectTree, sources: &[RelPath], hovered: NodeId) -> Option<RelPath> {
    let mut cursor = Some(hovered);
    while let Some(id) = cursor {
        let node = tree.node(id)?;
        if node.is_folder() {
            // `is_under` is inclusive: this also rejects dropping a folder on
            // itself or on one of its own children.
            if sources.iter().any(|source| node.path.is_under(source)) {
                return None;
            }
            if sources
                .iter()
                .all(|source| source.parent().as_ref() == Some(&node.path))
            {
                return None;
            }
            return Some(node.path.clone());
        }
        cursor = node.parent;
    }
    None
}

/// The first dragged entry whose name is already taken in the target directory,
/// case-insensitively (the reducer's rule, applied before the action).
fn move_conflict(tree: &ProjectTree, sources: &[RelPath], target: &RelPath) -> Option<RelPath> {
    let target_id = tree.find(target)?;
    let names: Vec<&str> = tree
        .children(target_id)
        .iter()
        .filter_map(|id| tree.node(*id))
        .map(Node::name)
        .collect();
    sources
        .iter()
        .find(|source| {
            names
                .iter()
                .any(|name| name.eq_ignore_ascii_case(source.name()))
        })
        .cloned()
}

/// What the drag ghost draws in its icon slot.
enum GhostIcon {
    /// The asset's own preview (a texture row, when the host decoded it).
    Texture(egui::TextureId),
    /// A skin icon file (a type icon, the folder, or the generic file icon).
    File(String),
}

/// Draw the drag ghost: one row-like line with the entry's icon and its name, or
/// the number of entries.
///
/// It is deliberately the shape of a tree row (icon + label, one line) instead
/// of a bare text label: a long name used to wrap and made the ghost look like a
/// second tree row had detached. The label is truncated, never wrapped.
///
/// It lives on the tooltip layer at the pointer. The plan mentions a
/// TSTransform-followed sublayer; that exists for multi-viewport layers, and the
/// editor has one viewport, so an Area at the pointer is the same thing without
/// a transform to maintain.
fn drag_ghost(
    ui: &Ui,
    frame: &Frame,
    project: &ProjectState,
    style: &ProjectStyle,
    drag: &DragMove,
) {
    let icon = ghost_icon(ui.ctx(), frame, project, style, drag);
    let label = egui::RichText::new(drag.label.clone())
        .size(style.row_text_size)
        .color(style.toolbar_text.to_egui());
    egui::Area::new(egui::Id::new("project-drag-ghost"))
        .order(egui::Order::Tooltip)
        .fixed_pos(drag.pos + vec2(12.0, 8.0))
        .interactable(false)
        .show(ui.ctx(), |ui| {
            // A cap, so a long name truncates instead of stretching the ghost
            // across the panel.
            ui.set_max_width(240.0);
            egui::Frame::NONE
                .fill(style.header_bg.to_egui())
                .stroke(egui::Stroke::new(
                    style.separator_width,
                    style.separator.to_egui(),
                ))
                .corner_radius(CornerRadius::same(3))
                .inner_margin(egui::Margin::symmetric(6, 2))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.x = 5.0;
                    ui.horizontal(|ui| {
                        let (rect, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                        match &icon {
                            GhostIcon::Texture(texture) => {
                                ui.painter().image(
                                    *texture,
                                    rect,
                                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                                    egui::Color32::WHITE,
                                );
                            }
                            GhostIcon::File(path) => {
                                ui_image::paint(ui, rect, path, egui::Color32::WHITE);
                            }
                        }
                        ui.add(egui::Label::new(label).truncate());
                    });
                });
        });
}

/// The icon the ghost shows for the dragged payload.
///
/// A single entry shows its own icon — the texture's preview when the host has
/// it, otherwise the family icon or the folder icon. A multi-entry drag shows the
/// generic file icon, because there is no one row to imitate.
fn ghost_icon(
    ctx: &egui::Context,
    frame: &Frame,
    project: &ProjectState,
    style: &ProjectStyle,
    drag: &DragMove,
) -> GhostIcon {
    let generic = || GhostIcon::File(style.icons.file.clone());
    let [only] = drag.sources.as_slice() else {
        return generic();
    };
    let Some(node) = project.tree.get(only) else {
        return generic();
    };
    if node.is_folder() {
        return GhostIcon::File(style.icons.folder.clone());
    }
    let Some(kind) = project.file_types.lookup(node.name()) else {
        return generic();
    };
    if kind.thumbnail {
        let root = project.root.to_string_lossy();
        if let Some(texture) = frame.host.project_thumbnail(ctx, &root, only.as_str()) {
            return GhostIcon::Texture(texture);
        }
    }
    if !kind.icon.is_empty() {
        return GhostIcon::File(format!("{}/{}", style.type_icon_root, kind.icon));
    }
    generic()
}

/// The move-blocked dialog (T7): inform, do not move, leave the tree alone.
fn move_blocked_dialog(ui: &mut Ui, frame: &Frame, move_blocked: &mut Option<MoveBlockedDialog>) {
    const BUTTONS: [ModalButton<'static>; 1] = [ModalButton::new("OK", ModalRole::Default)];
    let Some(dialog) = move_blocked.as_mut() else {
        return;
    };
    let body = format!(
        "\"{}\" already exists in \"{}\". Nothing was moved.",
        dialog.path.name(),
        dialog.target.name()
    );
    let style = frame.modal_style();
    let choice = Modal::new("project-move-blocked", "Cannot Move", &body)
        .buttons(&BUTTONS)
        .show_draggable(ui, &style, &mut dialog.offset);
    if choice.is_some() {
        *move_blocked = None;
    }
}

/// The row under the two panes, split where they meet.
///
/// The left half belongs to the hierarchy: it counts the active directory's
/// directly visible children — the number the user is looking at, and the one a
/// single click moves — and appends a truncation warning. The right half belongs
/// to the content pane and is reserved for the icon-size slider M9 adds; until
/// then it only reports a failed host action (an open, a scan), because a failure
/// is the one thing that must not be silent. The selected path used to live there
/// and is gone on purpose: the header now follows the click, so the path repeated
/// what the breadcrumb already said.
fn status_bar(
    ui: &Ui,
    style: &ProjectStyle,
    project: &ProjectState,
    rect: Rect,
    boundary: f32,
    local_error: Option<&str>,
) {
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::ZERO, style.status_bg.to_egui());
    painter.rect_filled(
        Rect::from_min_size(rect.min, vec2(rect.width(), style.separator_width)),
        CornerRadius::ZERO,
        style.separator.to_egui(),
    );
    // The rule under the column splitter: each half of the row belongs to the
    // pane above it, so the boundary's own line continues through the status row.
    // It uses the splitter's idle colour, not `separator`, because the status
    // background already *is* `separator` in the dark skin.
    if boundary > rect.left() && boundary < rect.right() {
        painter.rect_filled(
            Rect::from_min_size(
                pos2(boundary, rect.top()),
                vec2(style.separator_width, rect.height()),
            ),
            CornerRadius::ZERO,
            style.splitter_idle.to_egui(),
        );
    }

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
            project.tree.node(**id).is_some_and(|node| {
                project.view.show_hidden || !project.file_types.is_hidden(node.name())
            })
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

    // A rejected inline rename wins over the last operation's failure: it is the
    // message about the text currently on screen, and the editor is still open.
    if let Some(message) = local_error {
        text.paint(
            ui,
            pos2(rect.right() - 6.0, rect.center().y),
            Align2::RIGHT_CENTER,
            message,
        );
    } else if let Some(error) = &project.error {
        text.paint(
            ui,
            pos2(rect.right() - 6.0, rect.center().y),
            Align2::RIGHT_CENTER,
            error.to_string(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::HostView;
    use egui::{Event, FullOutput, Modifiers, PointerButton, Pos2, RawInput};
    use kairos_editor_core::EditorState;
    use kairos_editor_core::project::ScanEntry;
    use kairos_editor_widgets::modal::ModalStyle;
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
        stroke: egui::Stroke,
    }

    fn flatten(shape: &egui::Shape, out: &mut Vec<PaintedRect>) {
        match shape {
            egui::Shape::Rect(rect) => out.push(PaintedRect {
                rect: rect.rect,
                fill: rect.fill,
                stroke: rect.stroke,
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
            self.frame_output_with_modifiers(events, Modifiers::NONE)
        }

        fn frame_output_with_modifiers(
            &mut self,
            events: Vec<Event>,
            modifiers: Modifiers,
        ) -> FullOutput {
            self.time += 0.016;
            let mut input = RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, CANVAS));
            input.time = Some(self.time);
            input.events = events;
            // egui takes the modifier state from RawInput, not from the pointer
            // events: a Ctrl+click has to set both.
            input.modifiers = modifiers;
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

        /// One frame's actions, with keyboard modifiers held down.
        fn actions_with_modifiers(
            &mut self,
            events: Vec<Event>,
            modifiers: Modifiers,
        ) -> Vec<Action> {
            let _ = self.frame_output_with_modifiers(events, modifiers);
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

        /// A primary click carrying keyboard modifiers (Ctrl/Shift selection).
        fn click_with(&mut self, at: Pos2, modifiers: Modifiers) -> Vec<Action> {
            let mut actions = self.actions_with_modifiers(press_with(at, modifiers), modifiers);
            actions.extend(self.actions_with_modifiers(release_with(at, modifiers), modifiers));
            actions
        }

        /// One key press carrying modifiers.
        fn key_with(&mut self, key: egui::Key, modifiers: Modifiers) -> Vec<Action> {
            self.actions_with_modifiers(
                vec![Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed: true,
                    repeat: false,
                    modifiers,
                }],
                modifiers,
            )
        }

        /// A primary drag: press on `from`, move onto `to`, release there.
        fn drag(&mut self, from: Pos2, to: Pos2) -> Vec<Action> {
            let mut actions = self.actions(press(from));
            actions.extend(self.actions(vec![Event::PointerMoved(to)]));
            actions.extend(self.actions(release(to)));
            actions
        }

        /// A secondary (right) click spread over the press and release frames.
        fn right_click(&mut self, at: Pos2) -> Vec<Action> {
            let mut actions = self.actions(secondary_press(at));
            actions.extend(self.actions(secondary_release(at)));
            actions
        }

        /// The rect of the frame's first text run equal to `label`.
        fn text_rect(&mut self, events: Vec<Event>, label: &str) -> Option<Rect> {
            let output = self.frame_output(events);
            text_rects(&output)
                .into_iter()
                .find(|(text, _)| text == label)
                .map(|(_, rect)| rect)
        }
    }

    fn press_with(pos: Pos2, modifiers: Modifiers) -> Vec<Event> {
        vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: true,
                modifiers,
            },
        ]
    }

    fn release_with(pos: Pos2, modifiers: Modifiers) -> Vec<Event> {
        vec![Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: false,
            modifiers,
        }]
    }

    fn secondary_press(pos: Pos2) -> Vec<Event> {
        vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: PointerButton::Secondary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ]
    }

    fn secondary_release(pos: Pos2) -> Vec<Event> {
        vec![Event::PointerButton {
            pos,
            button: PointerButton::Secondary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }]
    }

    /// Every text run a frame painted, with the rect it occupies.
    fn text_rects(output: &FullOutput) -> Vec<(String, Rect)> {
        fn walk(shape: &egui::Shape, out: &mut Vec<(String, Rect)>) {
            match shape {
                egui::Shape::Text(text) => {
                    out.push((
                        text.galley.text().to_owned(),
                        Rect::from_min_size(text.pos, text.galley.size()),
                    ));
                }
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
                paths: vec![RelPath::new("res")],
                mode: SelectMode::Replace,
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

    /// The hidden-files toggle is Unity's eye icon, not a text segment: the bar
    /// paints no label for it, the on state fills the square, and clicking it
    /// while on asks for the files to be filtered again.
    #[test]
    fn the_hidden_files_toggle_is_an_eye_icon() {
        let style = ProjectStyle::dark();
        let mut harness = ProjectHarness::new(&[]);
        let texts = harness.painted_text(Vec::new());
        assert!(
            !texts.iter().any(|text| text == "Hidden files"),
            "the eye replaces the text button: {texts:?}"
        );

        harness.state.project.view.show_hidden = true;
        let painted = harness.painted(Vec::new());
        assert!(
            painted
                .iter()
                .any(|painted| painted.fill == style.control_hover.to_egui()
                    && painted.rect.right() >= CANVAS.x - 1.0
                    && painted.rect.top() < 1.0
                    && painted.rect.height() >= style.toolbar_height - 0.5),
            "the open eye must sit on the toggle's active fill: {painted:?}"
        );

        let actions = harness.click(pos2(CANVAS.x - 10.0, 10.0));
        assert!(has(&actions, &ProjectAction::SetShowHidden(false)));
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
                && painted.rect.left() < 1.0
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
        assert!(
            !texts.iter().any(|text| text == "no selection"),
            "the selected path left the status bar on purpose: {texts:?}"
        );

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
                    paths: vec![RelPath::new("Library")],
                    mode: SelectMode::Replace,
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

    /// The content pane's header is its own chrome: a darker bar with a 1 px rule
    /// under it, so the breadcrumb reads as a header and not as body text.
    #[test]
    fn the_content_header_paints_its_own_bar_and_rule() {
        let style = ProjectStyle::dark();
        let mut harness = ProjectHarness::new(&[]);
        let painted = harness.painted(Vec::new());
        let boundary = (harness.state.project.view.tree_split * CANVAS.x).round();
        let top = 21.0;
        assert!(
            painted
                .iter()
                .any(|painted| painted.fill == style.header_bg.to_egui()
                    && painted.rect.left() >= boundary - 0.5
                    && (painted.rect.top() - top).abs() < 0.5
                    && painted.rect.height() >= style.header_height - 0.5),
            "the right pane's header must paint its own bar: {painted:?}"
        );
        assert!(
            painted
                .iter()
                .any(|painted| painted.fill == style.separator.to_egui()
                    && painted.rect.left() >= boundary - 0.5
                    && (painted.rect.top() - (top + style.header_height)).abs() < 0.5),
            "a 1 px rule closes the header"
        );
    }

    /// The project's toolbars close each segment with a 1 px rule (Console,
    /// Profiler); without them the labels read as plain text.
    #[test]
    fn the_toolbar_buttons_are_closed_by_chrome_rules() {
        let style = ProjectStyle::dark();
        let mut harness = ProjectHarness::new(&[]);
        let painted = harness.painted(Vec::new());
        let rules: Vec<_> = painted
            .iter()
            .filter(|painted| {
                painted.fill == style.separator.to_egui()
                    && painted.rect.width() <= style.separator_width + 0.5
                    && painted.rect.top() < 1.0
                    && painted.rect.bottom() <= 21.0
            })
            .collect();
        assert!(
            rules.len() >= 3,
            "Collapse all, Refresh and the right-aligned group each need a rule: {rules:?}"
        );
    }

    /// The status row is split where the panes meet: the count belongs to the
    /// hierarchy half, the content half is reserved for the icon-size slider.
    #[test]
    fn the_status_bar_is_split_under_the_two_panes() {
        let style = ProjectStyle::dark();
        let mut harness = ProjectHarness::new(&[]);
        let painted = harness.painted(Vec::new());
        let boundary = (harness.state.project.view.tree_split * CANVAS.x).round();
        let top = CANVAS.y - style.status_height;
        assert!(
            painted
                .iter()
                .any(|painted| painted.fill == style.splitter_idle.to_egui()
                    && (painted.rect.left() - boundary).abs() < 0.5
                    && (painted.rect.top() - top).abs() < 0.5
                    && painted.rect.height() >= style.status_height - 0.5),
            "the status row must be split at the column boundary"
        );
    }

    /// Press the column splitter, drag right, release: the boundary must follow
    /// the pointer and the drag must write exactly one split action carrying the
    /// dragged fraction. Before the acceptance fix the mutated size was thrown
    /// away, so the action carried the old fraction and the boundary snapped
    /// back.
    #[test]
    fn dragging_the_column_splitter_moves_it_and_emits_one_action() {
        let mut harness = ProjectHarness::new(&[]);
        let boundary = 0.28 * CANVAS.x;
        let dragged = boundary + 60.0;
        let y = 200.0;

        let _ = harness.actions(press(pos2(boundary, y)));
        let _ = harness.actions(vec![Event::PointerMoved(pos2(dragged, y))]);

        // The next pass lays both panes out at the dragged width: the content
        // header starts where the splitter now is.
        let style = ProjectStyle::dark();
        let painted = harness.painted(Vec::new());
        assert!(
            painted
                .iter()
                .any(|painted| painted.fill == style.header_bg.to_egui()
                    && (painted.rect.left() - dragged).abs() < 1.0),
            "the panes must follow the drag, not snap back: {painted:?}"
        );

        let actions = harness.actions(release(pos2(dragged, y)));
        let splits: Vec<f32> = actions
            .iter()
            .filter_map(|action| match action {
                Action::Project(ProjectAction::SetTreeSplit(fraction)) => Some(*fraction),
                _ => None,
            })
            .collect();
        assert_eq!(splits.len(), 1, "one action on release: {actions:?}");
        assert!(
            (splits[0] - dragged / CANVAS.x).abs() < 0.01,
            "the action must carry the dragged fraction, got {splits:?}"
        );
    }

    /// A state with more rows than the canvas can show, so scrolling matters.
    fn tall_state(rows: usize) -> EditorState {
        let mut state = EditorState::default();
        state.project = ProjectState::at("D:/project");
        let mut entries = vec![entry("res", true)];
        for index in 0..rows {
            entries.push(entry(&format!("res/item_{index:02}.png"), false));
        }
        state.project.tree.assemble(&entries, &[], false);
        state.project.view.expanded.insert(RelPath::new("res"));
        state
    }

    /// Closing and reopening the tab rebuilds the panel but core keeps the
    /// selection; the fresh instance must scroll that row back into view.
    #[test]
    fn reopening_the_panel_reveals_the_selection() {
        let mut harness = ProjectHarness::new(&[]);
        harness.state = tall_state(80);
        harness
            .state
            .project
            .view
            .selection
            .insert(RelPath::new("res/item_75.png"));
        // A reopened tab is a fresh panel instance with no scroll position.
        harness.panel = ProjectPanel::default();

        // The first pass queues the reveal; egui animates the scroll, so let the
        // clock jump and drain the animation before painting the next frame.
        let _ = harness.frame_output(Vec::new());
        harness.time += 5.0;
        let _ = harness.frame_output(Vec::new());
        let texts = harness.painted_text(Vec::new());
        assert!(
            texts.iter().any(|text| text == "item_75"),
            "reopening must scroll the selected row into view: {texts:?}"
        );
    }

    #[test]
    fn the_panel_owns_its_scroll_areas() {
        assert_eq!(ProjectPanel::default().scroll_bars(), [false, false]);
    }

    /// A right click selects the row it landed on (unless it is already part of
    /// the selection) and opens the row menu at the pointer.
    #[test]
    fn a_right_click_selects_the_row_and_opens_the_menu() {
        let mut harness = ProjectHarness::new(&[]);
        let actions = harness.right_click(row_center(1));

        assert!(has(
            &actions,
            &ProjectAction::Select {
                paths: vec![RelPath::new("Library")],
                mode: SelectMode::Replace,
            }
        ));

        // The menu is its own layer; jump past egui's fade-in before asserting
        // its items are painted.
        harness.time += 2.0;
        let texts = harness.painted_text(Vec::new());
        for label in ["Create", "Rename", "Copy Path", "Reveal", "Delete"] {
            assert!(
                texts.iter().any(|text| text == label),
                "the row menu is missing {label}: {texts:?}"
            );
        }
    }

    /// The Create submenu is the type table's data, not a hardcoded list: the
    /// built-in table offers Folder and Document > Markdown, and picking the
    /// latter asks for a Markdown document in the right-clicked folder.
    #[test]
    fn the_create_submenu_asks_for_the_table_template() {
        let mut harness = ProjectHarness::new(&[]);
        harness.right_click(row_center(0));
        harness.time += 2.0;

        let create = harness
            .text_rect(Vec::new(), "Create")
            .expect("the Create submenu trigger is painted");
        harness.time += 2.0;
        let _ = harness.click(create.center());
        harness.time += 2.0;
        let document = harness
            .text_rect(Vec::new(), "Document")
            .expect("Document is one level under Create");
        harness.time += 2.0;
        let _ = harness.click(document.center());
        harness.time += 2.0;
        let markdown = harness
            .text_rect(Vec::new(), "Markdown")
            .expect("Markdown is the one Document entry");
        let actions = harness.click(markdown.center());

        assert!(
            has(
                &actions,
                &ProjectAction::CreateRequested {
                    parent: RelPath::new("res"),
                    name: String::from("New Document"),
                    kind: CreateKind::File {
                        extension: "md",
                        contents: "# ",
                    },
                }
            ),
            "picking Create > Document > Markdown must ask the reducer: {actions:?}"
        );
    }

    /// Rename opens the inline editor on the row and Enter commits the buffer as
    /// a RenameRequested action, which is the panel's whole rename contract.
    #[test]
    fn the_rename_item_edits_and_commits_the_row_name() {
        let mut harness = ProjectHarness::new(&[]);
        harness.right_click(row_center(0));
        harness.time += 2.0;
        let rename = harness
            .text_rect(Vec::new(), "Rename")
            .expect("the Rename item is painted");
        harness.time += 2.0;
        let _ = harness.click(rename.center());

        let edit = harness.panel.rename.as_ref().expect("the editor is open");
        assert_eq!(edit.path, RelPath::new("res"));
        assert_eq!(edit.buffer, "res");

        // The first editor frame requests focus and selects the name; the next
        // types over it, then Enter commits.
        let _ = harness.actions(Vec::new());
        let _ = harness.actions(vec![Event::Text(String::from("x"))]);
        let actions = harness.actions(key(egui::Key::Enter));

        assert!(
            has(
                &actions,
                &ProjectAction::RenameRequested {
                    path: RelPath::new("res"),
                    name: String::from("x"),
                }
            ),
            "Enter must commit the edited name: {actions:?}"
        );
        assert!(
            harness.panel.rename.is_none(),
            "the editor closes on commit"
        );
    }

    /// The tree shows a file's stem: the extension is the right-aligned suffix
    /// column, not part of the name.
    #[test]
    fn a_file_row_shows_its_stem_and_the_suffix_separately() {
        let mut harness = ProjectHarness::new(&["res"]);
        let texts = harness.painted_text(Vec::new());

        assert!(
            texts.iter().any(|text| text == "readme"),
            "the row label must be the stem: {texts:?}"
        );
        assert!(
            texts.iter().any(|text| text == "md"),
            "the suffix column carries the extension: {texts:?}"
        );
        assert!(
            !texts.iter().any(|text| text == "readme.md"),
            "the extension must not be part of the row name: {texts:?}"
        );
    }

    /// Renaming a file edits the stem only; the extension is put back when the
    /// commit becomes an action, so a rename cannot change a file's type.
    #[test]
    fn renaming_a_file_keeps_its_extension() {
        let mut harness = ProjectHarness::new(&["res"]);
        harness.right_click(row_center(2));
        harness.time += 2.0;
        let rename = harness
            .text_rect(Vec::new(), "Rename")
            .expect("the Rename item is painted");
        harness.time += 2.0;
        let _ = harness.click(rename.center());

        let edit = harness.panel.rename.as_ref().expect("the editor is open");
        assert_eq!(edit.path, RelPath::new("res/readme.md"));
        assert_eq!(
            edit.buffer, "readme",
            "the editor must not include the extension"
        );

        let _ = harness.actions(Vec::new());
        let _ = harness.actions(vec![Event::Text(String::from("notes"))]);
        let actions = harness.actions(key(egui::Key::Enter));

        assert!(
            has(
                &actions,
                &ProjectAction::RenameRequested {
                    path: RelPath::new("res/readme.md"),
                    name: String::from("notes.md"),
                }
            ),
            "the extension must survive the rename: {actions:?}"
        );
    }

    /// A rejected name keeps the editor open and puts the reason on the status
    /// bar: the user fixes the text instead of retyping the gesture.
    #[test]
    fn an_invalid_rename_stays_open_with_its_message() {
        let harness = ProjectHarness::new(&[]);
        let mut rename = Some(RenameEdit {
            path: RelPath::new("res"),
            buffer: String::from("bad/name"),
            error: None,
        });
        let output = TreeOutput::<NodeId> {
            edit_committed: true,
            ..TreeOutput::default()
        };
        let mut out = UiOut::new();

        edit_actions(&harness.state.project, &output, &mut rename, &mut out);

        assert!(out.is_empty(), "a rejected name never reaches the reducer");
        let edit = rename.as_ref().expect("the editor stays open");
        assert!(
            edit.error.is_some(),
            "the reason is kept for the status bar"
        );
    }

    /// Escape closes the editor and changes nothing.
    #[test]
    fn escape_cancels_the_inline_rename() {
        let harness = ProjectHarness::new(&[]);
        let mut rename = Some(RenameEdit {
            path: RelPath::new("res"),
            buffer: String::from("typed"),
            error: None,
        });
        let output = TreeOutput::<NodeId> {
            edit_cancelled: true,
            ..TreeOutput::default()
        };
        let mut out = UiOut::new();

        edit_actions(&harness.state.project, &output, &mut rename, &mut out);

        assert!(rename.is_none());
        assert!(out.is_empty());
    }

    /// A create starts its inline rename only once the row exists, and a create
    /// that never produces one stops waiting.
    #[test]
    fn a_pending_create_starts_renaming_when_its_row_appears() {
        let mut harness = ProjectHarness::new(&[]);
        let mut pending = Some(PendingCreate {
            path: RelPath::new("res/New Folder"),
            waited: 0,
        });
        let mut rename = None;

        pending_create_ready(&harness.state.project, &mut pending, &mut rename);
        assert!(pending.is_some(), "the row is not there yet");
        assert!(rename.is_none());

        harness.state.project.tree.assemble(
            &[
                entry("res", true),
                entry("res/New Folder", true),
                entry("res/readme.md", false),
                entry("Library", true),
            ],
            &[String::from("Library")],
            false,
        );
        pending_create_ready(&harness.state.project, &mut pending, &mut rename);

        assert!(pending.is_none());
        let edit = rename
            .as_ref()
            .expect("the created row starts in rename mode");
        assert_eq!(edit.path, RelPath::new("res/New Folder"));
        assert_eq!(edit.buffer, "New Folder");
    }

    /// Delete asks first (D5): the menu opens the dialog, and only its danger
    /// button emits the trash request.
    #[test]
    fn the_delete_menu_item_confirms_before_trashing() {
        let mut harness = ProjectHarness::new(&[]);
        harness.right_click(row_center(1));
        harness.time += 2.0;
        let delete = harness
            .text_rect(Vec::new(), "Delete")
            .expect("the Delete item is painted");
        harness.time += 2.0;
        let _ = harness.click(delete.center());

        // The dialog is up; it must not have trashed anything yet.
        assert_eq!(
            harness
                .panel
                .delete_confirm
                .as_ref()
                .map(|dialog| dialog.paths.clone()),
            Some(vec![RelPath::new("Library")])
        );
        harness.time += 2.0;
        let _ = harness.actions(Vec::new());

        let danger_fill = ModalStyle::dark().danger_fill;
        let button = harness
            .painted(Vec::new())
            .into_iter()
            .find(|painted| painted.fill == danger_fill)
            .map(|painted| painted.rect)
            .expect("the danger button is painted");
        let actions = harness.click(button.center());

        assert!(
            has(
                &actions,
                &ProjectAction::DeleteRequested {
                    paths: vec![RelPath::new("Library")],
                }
            ),
            "confirming must ask the reducer to trash the row: {actions:?}"
        );
        assert!(harness.panel.delete_confirm.is_none());
    }

    /// Enter is the safe default in the dialog: it cancels, it never deletes.
    #[test]
    fn enter_cancels_the_delete_dialog() {
        let mut harness = ProjectHarness::new(&[]);
        harness.panel.delete_confirm = Some(DeleteDialog {
            paths: vec![RelPath::new("res")],
            offset: egui::Vec2::ZERO,
        });
        harness.time += 2.0;
        let actions = harness.actions(key(egui::Key::Enter));

        assert!(!has(
            &actions,
            &ProjectAction::DeleteRequested {
                paths: vec![RelPath::new("res")],
            }
        ));
        assert!(harness.panel.delete_confirm.is_none());
    }

    /// Copy Path is a panel action: it puts the project-relative path on the
    /// clipboard and asks the reducer for nothing.
    #[test]
    fn copy_path_puts_the_relative_path_on_the_clipboard() {
        let harness = ProjectHarness::new(&[]);
        let mut out = UiOut::new();
        let mut rename = None;
        let mut pending = None;
        let mut confirm = None;
        let project = &harness.state.project;
        let output = harness.ctx.run_ui(RawInput::default(), |ui| {
            apply_menu_command(
                ui,
                project,
                MenuCommand::CopyPath {
                    path: RelPath::new("res/readme.md"),
                },
                &mut rename,
                &mut pending,
                &mut confirm,
                &mut out,
            );
        });

        assert!(
            output
                .platform_output
                .commands
                .iter()
                .any(|command| matches!(
                    command,
                    egui::OutputCommand::CopyText(text) if text == "res/readme.md"
                )),
            "the path must reach the clipboard"
        );
        assert!(out.is_empty());
    }

    /// A menu command carries its data: Create asks the reducer with the
    /// table's default stem and kind, and records the row to start renaming.
    #[test]
    fn a_create_command_carries_the_table_defaults() {
        let harness = ProjectHarness::new(&[]);
        let mut out = UiOut::new();
        let mut rename = None;
        let mut pending = None;
        let mut confirm = None;
        let project = &harness.state.project;
        let _ = harness.ctx.run_ui(RawInput::default(), |ui| {
            apply_menu_command(
                ui,
                project,
                MenuCommand::Create {
                    parent: RelPath::new("res"),
                    name: "New Folder",
                    kind: CreateKind::Folder,
                },
                &mut rename,
                &mut pending,
                &mut confirm,
                &mut out,
            );
        });

        assert!(has(
            out.as_slice(),
            &ProjectAction::CreateRequested {
                parent: RelPath::new("res"),
                name: String::from("New Folder"),
                kind: CreateKind::Folder,
            }
        ));
        assert_eq!(
            pending.as_ref().map(|create| create.path.clone()),
            Some(RelPath::new("res/New Folder"))
        );
    }

    /// Ctrl+click toggles one row into the set without moving the content pane.
    #[test]
    fn ctrl_click_toggles_the_selection() {
        let mut harness = ProjectHarness::new(&[]);
        harness
            .state
            .project
            .view
            .selection
            .insert(RelPath::new("res"));
        let actions = harness.click_with(row_center(1), Modifiers::CTRL);

        assert!(
            has(
                &actions,
                &ProjectAction::Select {
                    paths: vec![RelPath::new("Library")],
                    mode: SelectMode::Toggle,
                }
            ),
            "ctrl+click must toggle: {actions:?}"
        );
    }

    /// Shift+click selects the visible interval from the anchor to the row.
    #[test]
    fn shift_click_selects_the_visible_range() {
        let mut harness = ProjectHarness::new(&[]);
        harness
            .state
            .project
            .view
            .selection
            .insert(RelPath::new("res"));
        harness.state.project.view.anchor = Some(RelPath::new("res"));
        let actions = harness.click_with(row_center(1), Modifiers::SHIFT);

        assert!(
            has(
                &actions,
                &ProjectAction::Select {
                    paths: vec![RelPath::new("res"), RelPath::new("Library")],
                    mode: SelectMode::Range,
                }
            ),
            "shift+click must select the range: {actions:?}"
        );
    }

    /// F2 starts the inline rename on the selected entry.
    #[test]
    fn f2_renames_the_selected_entry() {
        let mut harness = ProjectHarness::new(&[]);
        harness
            .state
            .project
            .view
            .selection
            .insert(RelPath::new("res"));
        let _ = harness.click(row_center(0));
        let actions = harness.key_with(egui::Key::F2, Modifiers::NONE);

        assert!(actions.is_empty(), "F2 only opens the editor: {actions:?}");
        let edit = harness.panel.rename.as_ref().expect("the editor is open");
        assert_eq!(edit.path, RelPath::new("res"));
        assert_eq!(edit.buffer, "res");
    }

    /// Delete opens the confirmation dialog for the whole selection; the trash
    /// request only leaves after the dialog says yes.
    #[test]
    fn delete_opens_the_confirmation_for_the_selection() {
        let mut harness = ProjectHarness::new(&[]);
        harness
            .state
            .project
            .view
            .selection
            .insert(RelPath::new("res"));
        harness
            .state
            .project
            .view
            .selection
            .insert(RelPath::new("Library"));
        let _ = harness.click(row_center(0));
        let actions = harness.key_with(egui::Key::Delete, Modifiers::NONE);

        assert!(
            !actions.iter().any(|action| matches!(
                action,
                Action::Project(ProjectAction::DeleteRequested { .. })
            )),
            "Delete asks first: {actions:?}"
        );
        let dialog = harness
            .panel
            .delete_confirm
            .as_ref()
            .expect("the confirmation is open");
        assert_eq!(dialog.paths.len(), 2);
    }

    /// Ctrl+A puts every visible row into the selection.
    #[test]
    fn ctrl_a_selects_every_visible_row() {
        let mut harness = ProjectHarness::new(&["res"]);
        let _ = harness.click(row_center(0));
        let actions = harness.key_with(egui::Key::A, Modifiers::CTRL);

        assert!(
            has(
                &actions,
                &ProjectAction::Select {
                    paths: vec![
                        RelPath::new("res"),
                        RelPath::new("res/models"),
                        RelPath::new("res/readme.md"),
                        RelPath::new("Library"),
                    ],
                    mode: SelectMode::Replace,
                }
            ),
            "ctrl+a must select every visible row: {actions:?}"
        );
    }

    /// Escape clears the selection once no other owner (editor, dialog, menu)
    /// has the keys.
    #[test]
    fn escape_clears_the_selection() {
        let mut harness = ProjectHarness::new(&[]);
        harness
            .state
            .project
            .view
            .selection
            .insert(RelPath::new("res"));
        let _ = harness.click(row_center(0));
        let actions = harness.key_with(egui::Key::Escape, Modifiers::NONE);

        assert!(has(
            &actions,
            &ProjectAction::Select {
                paths: Vec::new(),
                mode: SelectMode::Replace,
            }
        ));
    }

    /// Dropping a row on a folder emits exactly one move action, with the
    /// selection as the payload and the folder as the target.
    #[test]
    fn dragging_a_row_onto_a_folder_asks_to_move_it() {
        let mut harness = ProjectHarness::new(&[]);
        let actions = harness.drag(row_center(0), row_center(1));

        assert!(
            has(
                &actions,
                &ProjectAction::MoveRequested {
                    paths: vec![RelPath::new("res")],
                    target: RelPath::new("Library"),
                }
            ),
            "the drop must become one move action: {actions:?}"
        );
        assert!(harness.panel.move_blocked.is_none());
    }

    /// While the button is held the ghost shows the dragged name, and the row
    /// under the pointer is highlighted once the panel accepted it.
    #[test]
    fn the_drag_paints_its_ghost_and_drop_highlight() {
        let mut harness = ProjectHarness::new(&["res"]);
        let style = ProjectStyle::dark();
        let tree = style.tree();
        // A two-entry selection drags as one payload: press on one selected row,
        // then move onto the Library row (still held).
        harness
            .state
            .project
            .view
            .selection
            .insert(RelPath::new("res/models"));
        harness
            .state
            .project
            .view
            .selection
            .insert(RelPath::new("res/readme.md"));
        let _ = harness.actions(press(row_center(1)));
        let _ = harness.actions(vec![Event::PointerMoved(row_center(3))]);

        let output = harness.frame_output(Vec::new());
        let texts = painted_texts(&output);
        assert!(
            texts.iter().any(|text| text == "2 items"),
            "the ghost shows how many entries are moving: {texts:?}"
        );
        let mut painted = Vec::new();
        for clipped in &output.shapes {
            flatten(&clipped.shape, &mut painted);
        }
        assert!(
            painted.iter().any(|painted| painted.fill == tree.drop_fill),
            "the accepted target row is highlighted"
        );

        // Finish the gesture so the next test starts clean.
        let _ = harness.actions(release(row_center(3)));
    }

    /// T7: a target that already holds one of the names blocks the move with a
    /// dialog, and nothing is asked of the reducer.
    #[test]
    fn a_move_onto_a_taken_name_is_blocked_with_a_dialog() {
        let mut harness = ProjectHarness::new(&[]);
        harness.state.project.tree.assemble(
            &[
                entry("res", true),
                entry("res/models", true),
                entry("res/models/Suzanne.png", false),
                entry("res/readme.md", false),
                entry("Library", true),
                entry("Library/Logs", true),
                entry("Library/readme.md", false),
            ],
            &[String::from("Library")],
            false,
        );
        harness
            .state
            .project
            .view
            .expanded
            .insert(RelPath::new("res"));
        // The rebuilt tree changes the row ids, and egui hit-tests against the
        // previous pass's rects: one layout frame before the first pointer event.
        let _ = harness.frame_output(Vec::new());

        // Rows: res, models, readme.md, Library; drag the file onto Library.
        let actions = harness.drag(row_center(2), row_center(3));

        assert!(
            !actions.iter().any(|action| matches!(
                action,
                Action::Project(ProjectAction::MoveRequested { .. })
            )),
            "a blocked move must not reach the reducer: {actions:?}"
        );
        let dialog = harness
            .panel
            .move_blocked
            .as_ref()
            .unwrap_or_else(|| panic!("the blocking dialog is open; actions {actions:?}"));
        assert_eq!(dialog.path, RelPath::new("res/readme.md"));
        assert_eq!(dialog.target, RelPath::new("Library"));

        // Escape dismisses it; the tree is untouched.
        let _ = harness.key_with(egui::Key::Escape, Modifiers::NONE);
        assert!(harness.panel.move_blocked.is_none());
    }

    /// Shift+arrow extends the selection from the anchor instead of moving a
    /// single-row cursor.
    #[test]
    fn shift_arrow_extends_the_selection_from_the_anchor() {
        let mut harness = ProjectHarness::new(&["res"]);
        // Rows: res, models, readme.md, Library. The click's action is applied
        // by the app before the next panel pass; the harness applies it here.
        let _ = harness.click(row_center(0));
        harness
            .state
            .project
            .view
            .selection
            .insert(RelPath::new("res"));
        harness.state.project.view.anchor = Some(RelPath::new("res"));
        let actions = harness.key_with(egui::Key::ArrowDown, Modifiers::SHIFT);

        assert!(
            has(
                &actions,
                &ProjectAction::Select {
                    paths: vec![RelPath::new("res"), RelPath::new("res/models")],
                    mode: SelectMode::Range,
                }
            ),
            "shift+down extends from the clicked row: {actions:?}"
        );
    }

    /// Pointing at a row inside an expanded folder drops into that folder: the
    /// target is the folder owning the row, not the row itself. Without this a
    /// folder whose children fill the viewport cannot be reached by a drag.
    #[test]
    fn dragging_onto_a_child_row_targets_its_folder() {
        let mut harness = ProjectHarness::new(&["res"]);
        // Rows: res, models, readme.md, Library. Drag Library onto the file
        // row: the nearest folder is res, so the move goes into res.
        let actions = harness.drag(row_center(3), row_center(2));

        assert!(
            has(
                &actions,
                &ProjectAction::MoveRequested {
                    paths: vec![RelPath::new("Library")],
                    target: RelPath::new("res"),
                }
            ),
            "the folder owning the hovered row is the drop target: {actions:?}"
        );
    }

    /// The accepted drop region is outlined around the whole folder subtree, not
    /// only its row, so the user sees the area a release would move into.
    #[test]
    fn the_drop_region_outlines_the_folder_subtree() {
        let mut harness = ProjectHarness::new(&["res"]);
        let style = ProjectStyle::dark();
        let tree = style.tree();
        // Start on Library (row 3) and hold over the file row (row 2): the
        // nearest folder is res, whose subtree is rows 0..=2.
        let _ = harness.actions(press(row_center(3)));
        let _ = harness.actions(vec![Event::PointerMoved(row_center(2))]);
        let painted = harness.painted(Vec::new());

        let region = painted
            .iter()
            .find(|painted| {
                painted.stroke == tree.drop_stroke
                    && (painted.rect.height() - 3.0 * tree.row_height).abs() < 1.0
            })
            .unwrap_or_else(|| {
                panic!(
                    "the region box must span the folder's three visible rows; painted {painted:?}"
                )
            });
        assert_eq!(region.fill, egui::Color32::TRANSPARENT);

        // Finish the gesture so it does not leak into the next test.
        let _ = harness.actions(release(row_center(1)));
    }

    /// A click on the dim area around a confirmation is not an answer: the
    /// dialog stays up and nothing is deleted.
    #[test]
    fn clicking_outside_the_delete_dialog_does_not_dismiss_it() {
        let mut harness = ProjectHarness::new(&[]);
        harness
            .state
            .project
            .view
            .selection
            .insert(RelPath::new("res"));
        let _ = harness.click(row_center(0));
        let _ = harness.key_with(egui::Key::Delete, Modifiers::NONE);
        assert!(harness.panel.delete_confirm.is_some());

        // A click far from the centred dialog lands on the modal backdrop.
        let actions = harness.click(pos2(760.0, 560.0));
        assert!(
            harness.panel.delete_confirm.is_some(),
            "a backdrop click must not answer the confirmation"
        );
        assert!(
            !actions.iter().any(|action| matches!(
                action,
                Action::Project(ProjectAction::DeleteRequested { .. })
            )),
            "nothing may be deleted: {actions:?}"
        );
    }

    /// The ghost is one row-like line: the dragged entry's display name (without
    /// the extension) next to its icon. It must not add a second line.
    #[test]
    fn the_single_drag_ghost_uses_the_row_display_name() {
        let mut harness = ProjectHarness::new(&["res"]);
        // Rows: res, models, readme.md, Library. Drag the file row to Library.
        let _ = harness.actions(press(row_center(2)));
        let _ = harness.actions(vec![Event::PointerMoved(row_center(3))]);
        let texts = harness.painted_text(Vec::new());
        assert!(
            texts.iter().any(|text| text == "readme"),
            "the ghost shows the stem, like the row does: {texts:?}"
        );
        assert!(
            !texts.iter().any(|text| text == "readme.md"),
            "the ghost must not reflow the full name: {texts:?}"
        );
        let _ = harness.actions(release(row_center(3)));
    }
    /// A drag that would drop a folder into its own subtree is not a move: no
    /// highlight, no action, no dialog.
    #[test]
    fn dragging_a_folder_into_its_own_subtree_does_nothing() {
        let mut harness = ProjectHarness::new(&["res"]);
        // Rows: res, models, readme.md, Library. Drag res onto models.
        let actions = harness.drag(row_center(0), row_center(1));

        assert!(
            !actions.iter().any(|action| matches!(
                action,
                Action::Project(ProjectAction::MoveRequested { .. })
            )),
            "self-nesting is not a move: {actions:?}"
        );
        assert!(harness.panel.move_blocked.is_none());
    }
}
