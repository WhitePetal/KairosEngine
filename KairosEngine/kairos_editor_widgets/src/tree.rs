//! A virtualized, single-column tree view (the Project panel's left pane).
//!
//! The widget paints rows and reports what the pointer did; it owns no domain
//! state and no state of its own. The caller keeps the row list (a pre-order
//! `&[Id]`, one id per visible row), the expansion/selection/view model, and
//! turns [`TreeOutput`] into its own actions. This module only knows
//! [`TreeStyle`], [`TreeRow`] and [`TreeOutput`] — no project, asset or editor
//! types — so the same tree can draw any id-keyed hierarchy.
//!
//! Three decisions are deliberate:
//!
//! - **Only the visible rows are built.** [`egui::ScrollArea::show_rows`] hands
//!   back the visible index range and the row closure is called only for it, so
//!   a 200k-row project costs the same per frame as a 20-row one. The prototype
//!   materialised every row into a `Vec<TreeRow>` first (MBs per frame on a big
//!   tree); the callback is the seam that avoids that.
//! - **The chevron is its own hit area.** A row is one click target and the
//!   expander is a second, smaller target registered after it so it wins the
//!   pointer. Clicking the chevron toggles expansion and must not select or
//!   navigate — that is Unity/Tuanjie's rule, and it is the reason the two
//!   cannot share a response.
//! - **The tree, not a row, is the keyboard focus unit.** Rows sense clicks and
//!   drags but never take keyboard focus themselves, so a huge tree does not
//!   flood egui's focus bookkeeping with 200k candidates. The tree claims one
//!   focusable rect underneath the rows, requests focus on any click in the tree
//!   and restores it after a context-menu press, then reports
//!   [`TreeOutput::focused`] so the panel can own arrow-key handling.
//!
//! The tree is pure appearance: it allocates, paints and hit-tests rows, and
//! reports single/double/secondary clicks, chevron toggles, the hovered row,
//! the drag lifecycle, the inline editor's outcome and its own keyboard focus.
//!
//! Two interactions are drawn by the widget but decided by the panel, because
//! the row geometry lives here and the domain rules do not:
//!
//! - **Inline rename** ([`TreeView::editing`]): the widget paints the editor in
//!   the row's label rect — that rect is private to this module, which is why the
//!   panel cannot draw the editor itself — and reports Enter / Escape / lost
//!   focus. The buffer, its validation and any error text belong to the panel,
//!   which keeps passing the buffer for as long as the editor must stay open: a
//!   rejected commit must not swallow the user's typing.
//! - **Drag and drop** ([`TreeOutput::drop_target`]): the widget reports which
//!   row a drag started on, that a drag is in progress and which row is under
//!   the pointer. It holds no payload, paints no ghost and never decides whether
//!   a row accepts the drop; the panel owns those rules and calls
//!   [`TreeView::highlight`] on the row it accepts.
//!
//! Context menus and confirmation dialogs stay the panel's own job: they need a
//! [`Ui`] and the panel's actions, not row geometry. The M2 rule stands — the
//! widget knows no project, asset or editor types, only [`TreeStyle`],
//! [`TreeRow`], [`TreeOutput`] and ids.

use std::hash::{DefaultHasher, Hash, Hasher};

use egui::{
    Align, Color32, CornerRadius, EventFilter, FontId, Key, Layout, Margin, Painter, PointerButton,
    Pos2, Rect, Response, ScrollArea, Sense, Shape, Stroke, StrokeKind, TextEdit, TextureId, Ui,
    UiBuilder, pos2, vec2,
};

use crate::ui_image;

/// Left inset of a row's content, before the indent and the chevron column.
const ROW_LEFT_PAD: f32 = 4.0;
/// Gap between one column and the next (chevron -> icon -> label).
const CONTENT_GAP: f32 = 4.0;
/// Right inset of the right-aligned suffix.
const ROW_RIGHT_PAD: f32 = 6.0;
/// Size of the chevron glyph drawn inside its [`TreeStyle::expander`] hit box.
const CHEVRON_GLYPH: f32 = 8.0;
/// Memory key (salted with the tree id) for the row whose inline editor ran
/// last frame; see [`TreeView::show`].
const EDITING_ROW_MEMORY: &str = "editing-row";
/// Memory key for a row drag in flight.
///
/// The dragged row can scroll out of the visible range while the button is
/// held (the pointer sits on an edge row and the tree auto-scrolls): its
/// response no longer exists, so the release would never be reported. The
/// session flag keeps the drag alive until the button is released, whatever
/// happened to the row.
const DRAG_SESSION_MEMORY: &str = "drag-session";
/// Distance from the viewport edge at which a drag starts scrolling it, in
/// points.
const AUTO_SCROLL_EDGE: f32 = 30.0;
/// Fastest auto-scroll, in points per second, reached at the edge itself.
///
/// It is deliberately slow: the point is to let the user stop on the row they
/// want, not to slam the viewport to the end. The speed is scaled by the square
/// of how deep the pointer is in the edge band, so the first pixels creep and
/// only holding at the very edge moves at this rate (about 4 px/frame at 60 Hz).
const AUTO_SCROLL_SPEED: f32 = 260.0;

/// Memory key for the editor whose commit was already reported last frame.
///
/// egui's lost-focus signal spans two frames ("the widget had focus at the start
/// of this frame *or* the previous one"), so an editor the panel keeps open would
/// otherwise commit twice for one blur.
const EDITING_COMMIT_MEMORY: &str = "editing-commit";

/// Arrow keys belong to the tree while it is focused.
///
/// egui's focus system moves focus to a neighbouring focusable widget on an
/// arrow key the focused widget does not "consume"; the panel implements its own
/// selection movement with those keys (only while [`TreeOutput::focused`]),
/// so the tree locks the arrows to itself. Tab and Escape keep their default
/// meaning: leave the tree.
const TREE_KEY_FILTER: EventFilter = EventFilter {
    tab: false,
    horizontal_arrows: true,
    vertical_arrows: true,
    escape: false,
};

/// Visual tokens for one tree: row metrics, text sizes and colours.
///
/// A component builds one from its serializable panel style and passes it to
/// every [`TreeView`] of that panel; the widget never reads egui's `Visuals`,
/// so a panel skin and the tree can never disagree. Plain [`egui::Color32`],
/// [`Stroke`] and `f32` keep the widget crate free of domain and colour-space
/// types.
#[derive(Clone, Debug, PartialEq)]
pub struct TreeStyle {
    pub row_height: f32,
    pub indent: f32,
    /// Chevron hit box width; the glyph itself is 8 x 8 points.
    pub expander: f32,
    /// Icon box size (square).
    pub icon: f32,
    pub text_size: f32,
    pub suffix_size: f32,
    /// Label colour.
    pub text: Color32,
    /// Right-aligned suffix colour (file extensions).
    pub suffix: Color32,
    pub selected_bg: Color32,
    pub selected_radius: u8,
    pub hover_bg: Color32,
    /// Fill of the row under the pointer while a row drag is in progress.
    ///
    /// The panel chooses the drop-target row (the widget only reports where the
    /// pointer is); this is the wash that says "a drop here would be accepted".
    pub drop_fill: Color32,
    /// Outline of the drop-target row, usually 1 px.
    ///
    /// A [`Stroke`] rather than a colour so a skin can drop the outline
    /// entirely by setting its width to zero.
    pub drop_stroke: Stroke,
    pub expander_color: Color32,
    pub separator: Color32,
    pub separator_width: f32,
    /// Background behind the rows. Transparent lets an outer frame show through;
    /// the widget only paints it when it has alpha.
    pub bg: Color32,
}

impl TreeStyle {
    /// The measured dark skin (Unity/Tuanjie 2022.3, the Project panel palette).
    ///
    /// Metrics come from the widget contract in
    /// `docs/plan/project-panel/README.md` section 2.6: 20 px rows, 14 px
    /// indent, 13 px chevron hit box, 18 px icons, 12/11 px text. Colours reuse
    /// the Project panel's measured tokens (`ProjectWindowStyle.toml`
    /// `[hierachy]`) and the Console panel's palette for the tokens that TOML
    /// never had (hover, separator, background).
    pub fn dark() -> Self {
        Self {
            row_height: 20.0,
            indent: 14.0,
            expander: 13.0,
            icon: 18.0,
            text_size: 12.0,
            suffix_size: 11.0,
            text: Color32::from_rgb(0xC8, 0xC8, 0xC8),
            suffix: Color32::from_rgb(0x78, 0x78, 0x78),
            selected_bg: Color32::from_rgb(0x3C, 0x4B, 0x64),
            selected_radius: 4,
            hover_bg: Color32::from_rgb(0x3A, 0x3A, 0x3A),
            drop_fill: Color32::from_rgba_unmultiplied(0x2C, 0x5D, 0x87, 0x80),
            drop_stroke: Stroke::new(1.0, Color32::from_rgb(0x5A, 0x9B, 0xE0)),
            expander_color: Color32::from_rgb(0xA0, 0xA0, 0xA0),
            separator: Color32::from_rgb(0x20, 0x20, 0x20),
            separator_width: 1.0,
            bg: Color32::from_rgb(0x31, 0x31, 0x31),
        }
    }

    /// The light-skin counterpart of [`TreeStyle::dark`]: same metrics, the
    /// Console panel's light palette. A starting point to review against a
    /// light-skin capture, not a measured skin.
    pub fn light() -> Self {
        Self {
            text: Color32::from_rgb(0x1E, 0x1E, 0x1E),
            suffix: Color32::from_rgb(0x6B, 0x6B, 0x6B),
            selected_bg: Color32::from_rgb(0x3B, 0x6E, 0xA5),
            hover_bg: Color32::from_rgb(0xE4, 0xE4, 0xE4),
            drop_fill: Color32::from_rgba_unmultiplied(0x2C, 0x5D, 0x87, 0x59),
            drop_stroke: Stroke::new(1.0, Color32::from_rgb(0x2C, 0x5D, 0x87)),
            expander_color: Color32::from_rgb(0x5A, 0x5A, 0x5A),
            separator: Color32::from_rgb(0xA6, 0xA6, 0xA6),
            bg: Color32::from_rgb(0xEC, 0xEC, 0xEC),
            ..Self::dark()
        }
    }
}

/// Which chevron a folder row draws.
///
/// `None` in [`TreeRow::expander`] means "file": the chevron column is
/// still reserved so icons line up in one column, but the row has no toggle hit
/// area.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expander {
    /// Folder is collapsed: chevron points right.
    Collapsed,
    /// Folder is expanded: chevron points down.
    Expanded,
}

impl Expander {
    /// `true` for [`Expander::Expanded`].
    pub fn is_expanded(self) -> bool {
        self == Self::Expanded
    }
}

/// One row's appearance, built on demand by the caller's row closure.
///
/// Borrowed strings keep the per-frame cost to the rows that are actually
/// visible: nothing is materialised for the other 199,980 rows.
#[derive(Clone, Copy, Debug)]
pub struct TreeRow<'a> {
    /// Nesting level; indented by `depth * TreeStyle::indent`.
    pub depth: u16,
    /// Row text (a file/folder name).
    pub label: &'a str,
    /// Right-aligned trailing text, typically the file extension (`png`).
    pub suffix: Option<&'a str>,
    /// Icon file path relative to the editor root, e.g.
    /// `Preferences/Textures/project/folder.png`.
    pub icon: Option<&'a str>,
    /// A host-owned texture to draw in the icon box instead of [`Self::icon`].
    ///
    /// Texture rows show the file itself (the panel asks the host's thumbnail
    /// cache and passes the resulting texture id); `icon` stays the fallback for
    /// the frames before the decode lands. A `TextureId` and not a file path:
    /// decoding and caching belong to the host, and the widget only paints what
    /// it is handed.
    pub thumbnail: Option<TextureId>,
    /// `None` = a file: no chevron and no toggle hit area.
    pub expander: Option<Expander>,
    /// Selected row: gets the rounded `TreeStyle::selected_bg` fill.
    pub selected: bool,
    /// Paint a `TreeStyle::separator` line on the row's top edge. The panel
    /// uses it above the `Library` system folder, which sorts last.
    pub separator_above: bool,
}

/// What the pointer and keyboard focus did during one [`TreeView::show`].
///
/// At most one row is clicked per frame. Note that egui reports the second
/// press of a double click as *both* a click and a double click; this widget
/// reports both truthfully ([`TreeOutput::clicked`] **and**
/// [`TreeOutput::double_clicked`] are `Some` on that frame) and leaves
/// the precedence to the caller. The panel's rule is "double click wins": a
/// folder navigates, a file opens, and the single-click selection written by the
/// same event targets the same row, so it is harmless.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TreeOutput<Id> {
    /// Single click on a row: select it (the panel's `Select` action).
    pub clicked: Option<Id>,
    /// Double click on a row: open it (folder -> navigate, file -> open).
    pub double_clicked: Option<Id>,
    /// Click on a row's chevron: toggle expansion. Never reported together with
    /// [`TreeOutput::clicked`] — the chevron has its own hit area.
    pub expander_toggled: Option<Id>,
    /// Row under the pointer (the chevron hit box counts as its row).
    pub hovered: Option<Id>,
    /// `true` while the tree owns keyboard focus; the panel only reads arrow
    /// keys, Home/End and Enter in that state.
    pub focused: bool,

    /// Row right-clicked this frame (the primary click is [`Self::clicked`]).
    ///
    /// The panel owns the context menu: the widget only reports which row was
    /// right-clicked and where, and never opens or positions a popup itself.
    pub secondary_clicked: Option<Id>,
    /// Pointer position of [`Self::secondary_clicked`], in screen points.
    ///
    /// The panel anchors its context menu here; `None` when no row was
    /// right-clicked this frame.
    pub secondary_pos: Option<Pos2>,

    /// Row a drag started on this frame: the drag source, and the panel's
    /// hook for capturing the drag payload.
    ///
    /// Primary button only, and only once the pointer has moved past egui's
    /// drag threshold — a click that wobbles a pixel is still a click.
    pub drag_started: Option<Id>,
    /// A row drag is in progress: set on the start frame, cleared on the frame
    /// the button is released.
    ///
    /// The flag is a tree-level session, not one row's response: the source row
    /// may scroll out of the visible range while the pointer holds at an edge,
    /// and the drag must survive that. The panel uses this to show its drag
    /// ghost and to treat [`Self::drop_target`] as meaningful.
    pub dragging: bool,
    /// Row under the pointer while [`Self::dragging`], or on the release frame.
    ///
    /// Includes the dragged row itself; `None` when the pointer is on empty
    /// space below the rows or outside the tree. The widget never decides
    /// whether the row accepts the drop — the panel applies the move rules and,
    /// when it accepts, calls [`TreeView::highlight`] on this row.
    pub drop_target: Option<Id>,
    /// The button was released this frame after a drag: the panel finishes or
    /// cancels the move.
    ///
    /// `true` whether or not the pointer is over a row (the release must end a
    /// drag whose source row has scrolled away); on that frame
    /// [`Self::dragging`] is already `false`.
    pub drag_stopped: bool,

    /// The inline editor ([`TreeView::editing`]) committed this frame: Enter,
    /// or the editor lost keyboard focus.
    ///
    /// The panel validates the buffer and issues its rename action; the editor
    /// only closes when the panel stops passing [`TreeView::editing`], so a
    /// rejected name keeps the editor open.
    pub edit_committed: bool,
    /// The inline editor was cancelled this frame: Escape.
    ///
    /// The buffer is left exactly as typed — the widget never reverts it — so
    /// restoring the original name is the panel's job.
    pub edit_cancelled: bool,
}

impl<Id> Default for TreeOutput<Id> {
    fn default() -> Self {
        Self {
            clicked: None,
            double_clicked: None,
            expander_toggled: None,
            hovered: None,
            focused: false,
            secondary_clicked: None,
            secondary_pos: None,
            drag_started: None,
            dragging: false,
            drop_target: None,
            drag_stopped: false,
            edit_committed: false,
            edit_cancelled: false,
        }
    }
}

/// A row list plus the style and callback needed to paint it, virtualized.
///
/// Construct one per frame; see [`TreeView::new`],
/// [`TreeView::scroll_to`], [`TreeView::highlight`], [`TreeView::editing`]
/// and [`TreeView::show`]. The builder is cheap: one small box for the row
/// closure and a few borrows.
pub struct TreeView<'a, Id> {
    /// Root id folded from the caller's salt; [`TreeView::show`] salts the ui
    /// id with it so row ids stay stable across frames.
    tree_salt: egui::Id,
    rows: &'a [Id],
    style: &'a TreeStyle,
    row: Box<dyn Fn(Id) -> TreeRow<'a> + 'a>,
    scroll_to: Option<Id>,
    highlight: Option<Id>,
    highlight_region: Option<Id>,
    editing: Option<(Id, &'a mut String)>,
}

impl<'a, Id: Copy + Eq + Hash> TreeView<'a, Id> {
    /// A tree over `rows` (pre-order: exactly the rows to display, in display
    /// order), styled by `style` and described on demand by `row`.
    ///
    /// `row` is called **only for visible rows**, once per row per frame; it
    /// must be cheap (derive the row from the caller's model, no I/O).
    ///
    /// `id_salt` identifies the tree within the frame. It only has to be
    /// stable across frames; row ids are derived from it and from each `Id`,
    /// so the same logical tree keeps its ids when rows move or scroll.
    pub fn new(
        id_salt: impl Hash,
        rows: &'a [Id],
        style: &'a TreeStyle,
        row: impl Fn(Id) -> TreeRow<'a> + 'a,
    ) -> Self {
        Self {
            tree_salt: salt_id(id_salt),
            rows,
            style,
            row: Box::new(row),
            scroll_to: None,
            highlight: None,
            highlight_region: None,
            editing: None,
        }
    }

    /// Scroll `id` into view (centred) on this frame, if it is one of
    /// `rows`.
    ///
    /// Used after keyboard selection movement and after a reveal. Works for rows
    /// outside the current visible range: the row index is mapped to a
    /// content-space rect from the range the scroll area reports, so the scroll
    /// target does not require a response for an unrendered row.
    pub fn scroll_to(mut self, id: Id) -> Self {
        self.scroll_to = Some(id);
        self
    }

    /// Paint `id` as the drop-target row: a rounded `drop_fill` wash plus the
    /// `drop_stroke` outline.
    ///
    /// The panel decides which row accepts the drag — the widget only reports
    /// [`TreeOutput::drop_target`] and holds none of the move rules — so this is
    /// how it asks for the highlight.
    pub fn highlight(mut self, id: Id) -> Self {
        self.highlight = Some(id);
        self
    }

    /// Outline the drop *region* of `id`: its row plus, for an expanded folder,
    /// every visible row of its subtree, as one rounded `drop_stroke` box.
    ///
    /// A folder is dropped into by pointing anywhere inside the area it owns,
    /// like Unity/Tuanjie: with a large expanded folder only the folder's own
    /// row would otherwise be reachable. The panel decides the target; the
    /// widget only owns the row geometry and the depth of each visible row.
    pub fn highlight_region(mut self, id: Id) -> Self {
        self.highlight_region = Some(id);
        self
    }

    /// Draw the inline rename editor in `id`'s label area, editing `buffer`.
    ///
    /// The panel owns the buffer, its validation and any error text; the widget
    /// only owns the row geometry — the label rect comes from the private row
    /// layout, which is why the panel cannot draw this editor itself — and the
    /// three endings: Enter or lost focus report [`TreeOutput::edit_committed`],
    /// Escape reports [`TreeOutput::edit_cancelled`].
    ///
    /// The editor stays open for as long as the panel passes this builder: a
    /// commit that fails validation keeps the text and re-focuses the editor on
    /// the next frame. Closing the editor (and restoring the original name after
    /// a cancel) is the panel's call, made by simply not calling this next frame.
    pub fn editing(mut self, id: Id, buffer: &'a mut String) -> Self {
        self.editing = Some((id, buffer));
        self
    }

    /// Paint the tree and report this frame's interaction.
    pub fn show(self, ui: &mut Ui) -> TreeOutput<Id> {
        let Self {
            tree_salt,
            rows,
            style,
            row,
            scroll_to,
            highlight,
            highlight_region,
            mut editing,
        } = self;
        let tree_id = ui.make_persistent_id(tree_salt);
        let mut output = TreeOutput::default();
        let scroll_to_index =
            scroll_to.and_then(|target| rows.iter().position(|row_id| *row_id == target));
        let editing_row_id = editing.as_ref().map(|(id, _)| *id);

        // Which row's editor ran last frame. It tells "the editor just opened"
        // (request focus and select all) apart from "the panel kept it open
        // after a rejected commit" (re-focus, keep the cursor); it is cleared as
        // soon as no editor is drawn, so a reopened editor starts fresh.
        let editing_memory_id = tree_id.with(EDITING_ROW_MEMORY);
        let previously_editing = ui
            .memory(|memory| memory.data.get_temp::<Option<u64>>(editing_memory_id))
            .flatten();
        let mut drawn_editor = None;

        // The lost-focus signal spans two frames; remember that this editor's
        // commit was already reported so one blur is one action.
        let commit_memory_id = tree_id.with(EDITING_COMMIT_MEMORY);
        let previously_committed = ui
            .memory(|memory| memory.data.get_temp::<Option<u64>>(commit_memory_id))
            .flatten();
        let mut drawn_editor_commit = None;

        // Drag state of the whole tree, filled in as the rows are hit-tested and
        // reported after the loop: the row under the pointer may come before or
        // after the row being dragged.
        let mut drag_started = None;
        let mut pointer_row = None;
        // A drag is a tree-level session, not a row response: the source row can
        // scroll out of the visible range while the pointer holds at an edge, and
        // the release must still end the move. See DRAG_SESSION_MEMORY.
        let drag_memory_id = tree_id.with(DRAG_SESSION_MEMORY);
        let mut drag_session = ui
            .memory(|memory| memory.data.get_temp::<bool>(drag_memory_id))
            .unwrap_or(false);
        // The drop region: the highlighted folder's row plus its visible
        // subtree.
        //
        // The folder row itself can be scrolled above the viewport while some of
        // its descendants are still visible. The box then starts at the viewport
        // edge, so pointing at a child always shows which folder owns it; that is
        // the case the first version missed.
        let region_target = highlight_region.and_then(|id| {
            rows.iter()
                .position(|row| *row == id)
                .map(|index| (index, row(id).depth))
        });
        let mut region: Option<RegionBox> = None;

        // A context-menu press must not cost the tree its keyboard focus: egui
        // surrenders focus on any press that is not on the hovered widget, and
        // the row sits above this tree-wide rect. Snapshot it before that runs.
        let had_keyboard_focus = ui.memory(|memory| memory.has_focus(tree_id));

        // The tree is the keyboard focus unit (the panel only reads arrow keys
        // while focused). This interactive, focusable rect under the rows keeps
        // the focus alive across egui's per-frame "the focused widget is gone"
        // cleanup (a focus id does not survive a frame in which no widget uses
        // it) and the key filter keeps egui's directional focus walk from taking
        // the arrow keys away from the panel.
        let tree_rect = ui.available_rect_before_wrap();
        let tree_response = ui.interact(tree_rect, tree_id, Sense::click());
        ui.memory_mut(|memory| memory.set_focus_lock_filter(tree_id, TREE_KEY_FILTER));

        if style.bg.a() > 0 {
            ui.painter()
                .rect_filled(tree_rect, CornerRadius::ZERO, style.bg);
        }

        ui.scope(|ui| {
            // show_rows measures the row pitch with the item spacing of the ui it
            // is called on. Zeroing it here (and in the content ui) makes the pitch
            // exactly style.row_height, so the painted rows line up with the
            // virtualized index range instead of drifting by spacing * rows.
            ui.spacing_mut().item_spacing.y = 0.0;
            ScrollArea::vertical()
                .id_salt(tree_id)
                .auto_shrink([false, false])
                .show_rows(ui, style.row_height, rows.len(), |ui, range| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let width = ui.available_width();
                    for index in range.clone() {
                        let id = rows[index];
                        // The only place the caller's row data is built: visible
                        // rows only, and never for the other ~200k rows.
                        let data = row(id);
                        let (_, rect) = ui.allocate_space(vec2(width, style.row_height));
                        let layout = row_layout(rect, style, data.depth);
                        let egui_row_id = row_id(tree_id, id);
                        let editing_this_row = editing_row_id == Some(id);

                        // The drop region follows the visible subtree of the
                        // highlighted folder; see `TreeView::highlight_region`.
                        if let Some((target_index, target_depth)) = region_target {
                            if index == target_index {
                                region = Some(RegionBox {
                                    left: rect.left(),
                                    right: rect.right(),
                                    top: rect.top(),
                                    bottom: rect.bottom(),
                                    depth: target_depth,
                                    open: true,
                                    extend: data.expander == Some(Expander::Expanded),
                                });
                            } else if let Some(box_) = region.as_mut() {
                                if box_.open {
                                    if data.depth > box_.depth {
                                        box_.bottom = rect.bottom();
                                    } else {
                                        box_.open = false;
                                    }
                                }
                            } else if index > target_index && data.depth > target_depth {
                                // The folder row is above the viewport: start
                                // the box at the viewport edge and extend it over
                                // the visible descendants.
                                region = Some(RegionBox {
                                    left: rect.left(),
                                    right: rect.right(),
                                    top: ui.clip_rect().top(),
                                    bottom: rect.bottom(),
                                    depth: target_depth,
                                    open: true,
                                    // A visible descendant proves the folder is
                                    // expanded; the run continues below.
                                    extend: true,
                                });
                            }
                        }

                        // A row is one click target and one drag source; the
                        // chevron inside it is a second, smaller click target
                        // registered after the row so it wins the pointer (egui
                        // hit-tests the last widget added first). Clicking the
                        // chevron must not select the row.
                        let response = ui.interact(rect, egui_row_id, Sense::click_and_drag());
                        let expander_response = data.expander.map(|_| {
                            ui.interact(layout.expander, egui_row_id.with("expander"), Sense::CLICK)
                        });

                        let hovered = response.hovered()
                            || expander_response.as_ref().is_some_and(Response::hovered);
                        paint_row(
                            ui,
                            rect,
                            style,
                            &data,
                            &layout,
                            hovered,
                            highlight == Some(id),
                            editing_this_row,
                        );

                        // The editor is registered after the row (and the
                        // chevron) so a click inside it edits text instead of
                        // selecting the row.
                        if editing_this_row && let Some((_, buffer)) = editing.as_mut() {
                            let editor_id = egui_row_id.with("editor");
                            let outcome = draw_editor(
                                ui,
                                style,
                                layout.label,
                                editor_id,
                                &mut **buffer,
                                previously_editing != Some(editor_id.value()),
                                previously_committed == Some(editor_id.value()),
                            );
                            if outcome.committed {
                                output.edit_committed = true;
                                drawn_editor_commit = Some(editor_id.value());
                            }
                            output.edit_cancelled |= outcome.cancelled;
                            drawn_editor = Some(editor_id.value());
                        }

                        // A right click on the chevron is a right click on the row
                        // it belongs to, like hover and selection.
                        let secondary = response.secondary_clicked()
                            || expander_response
                                .as_ref()
                                .is_some_and(Response::secondary_clicked);
                        if secondary {
                            output.secondary_clicked = Some(id);
                            output.secondary_pos = ui.input(|input| input.pointer.interact_pos());
                            if had_keyboard_focus {
                                ui.memory_mut(|memory| memory.request_focus(tree_id));
                            }
                        }

                        if expander_response.as_ref().is_some_and(Response::clicked) {
                            output.expander_toggled = Some(id);
                            ui.memory_mut(|memory| memory.request_focus(tree_id));
                        } else if response.clicked() {
                            output.clicked = Some(id);
                            ui.memory_mut(|memory| memory.request_focus(tree_id));
                        }
                        // A double click also reports clicked (egui semantics);
                        // both are forwarded, the caller decides precedence.
                        if response.double_clicked() {
                            output.double_clicked = Some(id);
                        }
                        if hovered {
                            output.hovered = Some(id);
                        }

                        // Primary only: a right-button press also arms egui's
                        // drag machinery, but the panel's move is a left drag.
                        if response.drag_started_by(PointerButton::Primary) {
                            drag_started = Some(id);
                            drag_session = true;
                        }
                        // contains_pointer (not hovered): during a drag nothing is
                        // "hovered", but a drop target still has to be found.
                        let row_contains_pointer = response.contains_pointer()
                            || expander_response
                                .as_ref()
                                .is_some_and(Response::contains_pointer);
                        if row_contains_pointer {
                            pointer_row = Some(id);
                        }
                    }

                    // While a drag holds at an edge the tree scrolls itself;
                    // egui's scroll area ignores the wheel during a widget drag,
                    // so the tree also translates it here.
                    if drag_session {
                        drag_scroll(ui);
                    }

                    // The drop region's box goes over the rows: outline only, so
                    // the labels stay readable. It spans the folder's own row and
                    // the visible rows of its subtree (the scroll area clips the
                    // part that is out of view).
                    if let Some(box_) = region {
                        let mut bottom = box_.bottom;
                        if box_.open && box_.extend && range.end < rows.len() {
                            // The subtree continues below the viewport: let the
                            // box run off the edge instead of closing halfway.
                            bottom = ui.clip_rect().bottom().max(bottom);
                        }
                        let painted =
                            Rect::from_min_max(pos2(box_.left, box_.top), pos2(box_.right, bottom));
                        ui.painter().rect_stroke(
                            painted,
                            CornerRadius::same(style.selected_radius),
                            style.drop_stroke,
                            StrokeKind::Inside,
                        );
                    }

                    if let Some(index) = scroll_to_index
                        && !range.contains(&index)
                    {
                        // Off-screen row: there is no response to call
                        // scroll_to_me on, so map the index to a content-space rect
                        // using the range the scroll area handed us.
                        let top =
                            ui.max_rect().top() + (index - range.start) as f32 * style.row_height;
                        let rect = Rect::from_min_size(
                            pos2(ui.max_rect().left(), top),
                            vec2(ui.max_rect().width(), style.row_height),
                        );
                        ui.scroll_to_rect(rect, Some(Align::Center));
                    }
                });
        });

        // The session ends on the primary release, whether or not the row that
        // started it is still drawn: that is what lets a drag finish after the
        // tree scrolled its source away.
        // Not `button_released`: a lost release event (the pointer left the
        // window) would otherwise keep the session and the panel's ghost alive
        // forever. The button state is the truth the next frame sees.
        let released = !ui.input(|input| input.pointer.primary_down());
        let dragging = drag_session && !released;
        let drag_stopped = drag_session && released;
        drag_session = dragging;
        ui.memory_mut(|memory| memory.data.insert_temp(drag_memory_id, drag_session));

        // The drop target is only meaningful while the move is in flight: a
        // plain hover is not a drag, and the release frame still says where it
        // ended (the widget reports empty space as None).
        output.drag_started = drag_started;
        output.dragging = dragging;
        output.drag_stopped = drag_stopped;
        if dragging || drag_stopped {
            output.drop_target = pointer_row;
        }
        ui.memory_mut(|memory| {
            memory.data.insert_temp(editing_memory_id, drawn_editor);
            memory
                .data
                .insert_temp(commit_memory_id, drawn_editor_commit);
        });

        if tree_response.clicked() {
            // Clicking the tree's empty space focuses it, like clicking a row.
            ui.memory_mut(|memory| memory.request_focus(tree_id));
        }
        output.focused = ui.memory(|memory| memory.has_focus(tree_id));
        output
    }
}

/// Scroll the enclosing tree while a row drag holds near the top or bottom edge,
/// and translate the wheel while a drag is in flight.
///
/// Two workarounds in one function, both because egui's `ScrollArea` only acts on
/// the wheel when `Context::dragged_id` is `None`: a tree drag would otherwise
/// freeze the viewport exactly when the user needs to reach a row outside it.
/// The delta is handed to the parent scroll area through `Ui::scroll_with_delta`
/// (positive y moves the content down, the same convention as the wheel), and
/// the wheel input is consumed so no other scroll area sees it too.
fn drag_scroll(ui: &Ui) {
    let viewport = ui.clip_rect();
    let Some(pointer) = ui.input(|input| input.pointer.interact_pos()) else {
        return;
    };

    let mut delta = ui.input(|input| input.smooth_scroll_delta.y);
    if delta != 0.0 {
        ui.input_mut(|input| input.smooth_scroll_delta = egui::Vec2::ZERO);
    }

    // Points per second, scaled by the frame time so the speed is the same at
    // 60 Hz and 144 Hz, and by the squared depth into the band so the first
    // pixels creep and the edge itself scrolls fastest.
    let seconds = ui.input(|input| input.stable_dt).min(0.05);
    if pointer.y < viewport.top() + AUTO_SCROLL_EDGE {
        let t =
            ((viewport.top() + AUTO_SCROLL_EDGE - pointer.y) / AUTO_SCROLL_EDGE).clamp(0.0, 1.0);
        delta += t * t * AUTO_SCROLL_SPEED * seconds;
        ui.ctx().request_repaint();
    } else if pointer.y > viewport.bottom() - AUTO_SCROLL_EDGE {
        let t = ((pointer.y - (viewport.bottom() - AUTO_SCROLL_EDGE)) / AUTO_SCROLL_EDGE)
            .clamp(0.0, 1.0);
        delta -= t * t * AUTO_SCROLL_SPEED * seconds;
        ui.ctx().request_repaint();
    }

    if delta != 0.0 {
        ui.scroll_with_delta_animation(vec2(0.0, delta), egui::style::ScrollAnimation::none());
    }
}

/// The box drawn around a folder's drop region while a drag is in flight.
///
/// The folder row may be outside the viewport while its descendants are visible,
/// so `top` is whichever comes first: the row itself or the viewport edge.
struct RegionBox {
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
    /// Depth of the folder row; its descendants are strictly deeper.
    depth: u16,
    /// Whether the visible run of descendants has not ended yet, i.e. the
    /// subtree continues below the viewport.
    open: bool,
    /// Whether the folder is expanded: only an expanded folder's box may run
    /// past the last visible row. A collapsed folder's box is its own row.
    extend: bool,
}

/// Where one row's parts are painted, in screen points.
struct RowLayout {
    /// Chevron hit box (also its painting box).
    expander: Rect,
    /// Square icon box.
    icon: Rect,
    /// Label area; the text is left-aligned in it and clipped at its right edge.
    label: Rect,
    /// Right edge the suffix is aligned against.
    suffix_right: f32,
}

/// Lay one row out: left pad, chevron (13), icon (18), gap, label, right-aligned
/// suffix.
///
/// The chevron column is reserved even for files (expander: None) so folder and
/// file icons sit in one column at every depth.
fn row_layout(rect: Rect, style: &TreeStyle, depth: u16) -> RowLayout {
    let x = rect.left() + ROW_LEFT_PAD + f32::from(depth) * style.indent;
    let expander = Rect::from_min_size(pos2(x, rect.top()), vec2(style.expander, rect.height()));
    let icon = Rect::from_center_size(
        pos2(x + style.expander + style.icon * 0.5, rect.center().y),
        vec2(style.icon, style.icon),
    );
    let label_left = x + style.expander + style.icon + CONTENT_GAP;
    let suffix_right = rect.right() - ROW_RIGHT_PAD;
    let label = Rect::from_min_max(
        pos2(label_left, rect.top()),
        pos2(suffix_right.max(label_left), rect.bottom()),
    );
    RowLayout {
        expander,
        icon,
        label,
        suffix_right,
    }
}

/// Paint the background, separator, chevron, icon and label of one row.
///
/// hovered is the union of the row's and the chevron's responses: hovering the
/// chevron still highlights the row it belongs to. highlighted is the drop
/// target the panel accepted; editing means the inline editor owns the label, so
/// the label and suffix are left to it (the buffer already holds the full name).
fn paint_row(
    ui: &Ui,
    rect: Rect,
    style: &TreeStyle,
    data: &TreeRow<'_>,
    layout: &RowLayout,
    hovered: bool,
    highlighted: bool,
    editing: bool,
) {
    let radius = CornerRadius::same(style.selected_radius);
    if data.selected {
        ui.painter().rect_filled(rect, radius, style.selected_bg);
    } else if hovered && style.hover_bg.a() > 0 {
        ui.painter().rect_filled(rect, radius, style.hover_bg);
    }

    if highlighted {
        // The panel chose this row as the drop target (the widget only reports
        // where the pointer is); the wash goes over the selection so it wins.
        if style.drop_fill.a() > 0 {
            ui.painter().rect_filled(rect, radius, style.drop_fill);
        }
        ui.painter()
            .rect_stroke(rect, radius, style.drop_stroke, StrokeKind::Inside);
    }

    if data.separator_above && style.separator_width > 0.0 {
        let line = Rect::from_min_max(
            rect.left_top(),
            pos2(rect.right(), rect.top() + style.separator_width),
        );
        ui.painter()
            .rect_filled(line, CornerRadius::ZERO, style.separator);
    }

    if let Some(expander) = data.expander {
        paint_expander(
            ui.painter(),
            layout.expander.center(),
            expander,
            style.expander_color,
        );
    }

    if let Some(texture) = data.thumbnail {
        // The asset's own preview (the host decoded and downscaled it): a
        // texture row shows the texture, not a family glyph.
        ui.painter().image(
            texture,
            layout.icon,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    } else if let Some(icon) = data.icon {
        // Missing or still-loading icon files paint nothing (see ui_image); this
        // is a runtime asset path, so an absent file must never panic.
        ui_image::paint(ui, layout.icon, icon, Color32::WHITE);
    }

    if editing {
        // The inline editor paints the name itself; drawing the label (or the
        // suffix, which the rename buffer already holds) would double-paint it.
        return;
    }

    // The label is clipped to the space the suffix leaves, so a long name
    // truncates against the right-aligned extension instead of overpainting it.
    let mut label_right = layout.label.right();
    if let Some(suffix) = data.suffix {
        let galley = ui.painter().layout_no_wrap(
            suffix.to_owned(),
            FontId::proportional(style.suffix_size),
            style.suffix,
        );
        let pos = pos2(
            layout.suffix_right - galley.size().x,
            rect.center().y - galley.size().y * 0.5,
        );
        ui.painter().galley(pos, galley, style.suffix);
        label_right = (pos.x - CONTENT_GAP).max(layout.label.left());
    }

    let galley = ui.painter().layout_no_wrap(
        data.label.to_owned(),
        FontId::proportional(style.text_size),
        style.text,
    );
    let clip = Rect::from_min_max(
        layout.label.left_top(),
        pos2(label_right, layout.label.bottom()),
    )
    .intersect(ui.clip_rect());
    ui.painter().with_clip_rect(clip).galley(
        pos2(layout.label.left(), rect.center().y - galley.size().y * 0.5),
        galley,
        style.text,
    );
}

/// Outcome of the inline editor during one frame.
#[derive(Clone, Copy, Debug, Default)]
struct EditorOutcome {
    committed: bool,
    cancelled: bool,
}

/// Draw the inline rename editor into a row's label rect and report its ending.
///
/// The panel owns the buffer and its validation; this only owns the row geometry
/// (which is private to this module) and the endings. The buffer is never
/// reverted here: on Escape it is left exactly as typed so the panel can restore
/// the original name. `commit_already_reported` suppresses the second frame of
/// egui's two-frame lost-focus signal, so one blur is one commit.
fn draw_editor(
    ui: &mut Ui,
    style: &TreeStyle,
    label: Rect,
    editor_id: egui::Id,
    buffer: &mut String,
    first_frame: bool,
    commit_already_reported: bool,
) -> EditorOutcome {
    // A child ui placed over the label rect: unlike `Ui::scope_builder` this does
    // not advance the parent's layout cursor, so the virtualized row pitch stays
    // exactly `row_height`.
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(label)
            .layout(Layout::left_to_right(Align::Center)),
    );
    child.spacing_mut().item_spacing = vec2(0.0, 0.0);
    // Selected glyphs are painted with the selection stroke colour, so a NONE
    // stroke would make the just-selected name disappear (the fields' inline
    // editor sets the same).
    child.visuals_mut().selection.stroke = Stroke::new(1.0, style.text);

    let response = TextEdit::singleline(buffer)
        .id(editor_id)
        .font(FontId::proportional(style.text_size))
        .text_color(style.text)
        .frame(egui::Frame::NONE)
        .margin(Margin::ZERO)
        .vertical_align(Align::Center)
        .desired_width(label.width())
        .show(&mut child)
        .response;

    let escape = ui.input(|input| input.key_pressed(Key::Escape));
    let enter = ui.input(|input| input.key_pressed(Key::Enter));
    let mut outcome = EditorOutcome::default();

    if first_frame {
        // A fresh editor opens with the whole name selected, like the inline
        // editors of the number and text fields.
        response.request_focus();
        select_all(ui.ctx(), editor_id, buffer.chars().count());
    }

    if escape {
        // egui's focus layer consumes Escape before the TextEdit sees it, so the
        // editor merely loses focus; report the cancel and hand the restore of
        // the original name to the panel.
        response.surrender_focus();
        outcome.cancelled = true;
    } else if (enter || response.lost_focus()) && !commit_already_reported {
        outcome.committed = true;
    } else if !first_frame
        && !response.has_focus()
        && !ui.input(|input| input.pointer.any_pressed())
    {
        // The panel kept the editor open after a rejected commit: keep it
        // focused instead of dropping the user out of it. A press elsewhere is
        // not turned into a focus steal, so a blur-commit does not fight the
        // widget the user just clicked.
        response.request_focus();
    }

    outcome
}

/// Select the whole inline editor buffer, the state a fresh editor starts in.
fn select_all(ctx: &egui::Context, editor_id: egui::Id, chars: usize) {
    let mut state = TextEdit::load_state(ctx, editor_id).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::default(),
            egui::text::CCursor::new(chars),
        )));
    state.store(ctx, editor_id);
}

/// Draw the 8 x 8 chevron centred in the given point.
fn paint_expander(painter: &Painter, center: Pos2, expander: Expander, color: Color32) {
    let half = CHEVRON_GLYPH * 0.5;
    let points = match expander {
        Expander::Collapsed => vec![
            pos2(center.x - half * 0.5, center.y - half),
            pos2(center.x + half * 0.5, center.y),
            pos2(center.x - half * 0.5, center.y + half),
        ],
        Expander::Expanded => vec![
            pos2(center.x - half, center.y - half * 0.5),
            pos2(center.x + half, center.y - half * 0.5),
            pos2(center.x, center.y + half * 0.5),
        ],
    };
    painter.add(Shape::convex_polygon(points, color, Stroke::NONE));
}

/// Fold the caller's salt into a root id usable across frames.
///
/// TreeView::new runs before there is a [`Ui`] to salt against, so the salt is
/// hashed once here; TreeView::show then derives the frame-stable tree id from
/// the caller ui's id and this value.
fn salt_id(id_salt: impl Hash) -> egui::Id {
    let mut hasher = DefaultHasher::new();
    id_salt.hash(&mut hasher);
    egui::Id::new(("kairos_editor_widgets.tree", hasher.finish()))
}

/// The egui id of one row: stable for a given tree and row id across frames.
fn row_id(tree_id: egui::Id, row: impl Hash) -> egui::Id {
    let mut hasher = DefaultHasher::new();
    row.hash(&mut hasher);
    tree_id.with(hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use egui::{Context, Event, FullOutput, Key, Modifiers, PointerButton, Pos2, RawInput};

    /// The test viewport: 320 x 200 points, exactly ten 20 pt rows.
    const VIEW: egui::Vec2 = egui::Vec2::new(320.0, 200.0);

    fn style() -> TreeStyle {
        TreeStyle::dark()
    }

    /// Rows are plain integers, both as ids and as their row specification.
    fn ids(count: usize) -> Vec<usize> {
        (0..count).collect()
    }

    /// What one test row should look like; the row closure turns it into a
    /// [`TreeRow`]. A plain data struct keeps the helper's row closure created
    /// inside the frame, which a moved closure could not be (the frame callback
    /// only gets `&mut` and may run more than once in principle).
    #[derive(Clone, Copy)]
    struct RowSpec {
        depth: u16,
        label: &'static str,
        suffix: Option<&'static str>,
        icon: Option<&'static str>,
        expander: Option<Expander>,
        selected: bool,
        separator_above: bool,
    }

    impl Default for RowSpec {
        fn default() -> Self {
            Self {
                depth: 0,
                label: "row",
                suffix: None,
                icon: None,
                expander: None,
                selected: false,
                separator_above: false,
            }
        }
    }

    fn input(time: f64, events: Vec<Event>) -> RawInput {
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, VIEW)),
            time: Some(time),
            events,
            ..Default::default()
        }
    }

    fn hover(pos: Pos2) -> Vec<Event> {
        vec![Event::PointerMoved(pos)]
    }

    fn click(pos: Pos2) -> Vec<Event> {
        vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            },
        ]
    }

    fn right_click(pos: Pos2) -> Vec<Event> {
        vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: PointerButton::Secondary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
            Event::PointerButton {
                pos,
                button: PointerButton::Secondary,
                pressed: false,
                modifiers: Modifiers::NONE,
            },
        ]
    }

    /// Press the primary button and keep it down: a drag needs later frames.
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

    /// Move to `pos` and release the primary button in the same frame.
    fn release(pos: Pos2) -> Vec<Event> {
        vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            },
        ]
    }

    fn key(key: Key) -> Vec<Event> {
        vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }]
    }

    fn typed(text: &str) -> Vec<Event> {
        vec![Event::Text(text.to_owned())]
    }

    /// Extra builder state one frame may hand the tree: the M3 channels that are
    /// not part of the plain row list.
    #[derive(Default)]
    struct ViewExtras<'a> {
        highlight: Option<usize>,
        highlight_region: Option<usize>,
        editing: Option<(usize, &'a mut String)>,
    }

    /// Run one headless frame of the tree under test.
    fn draw<'a>(
        ctx: &Context,
        rows: &'a [usize],
        style: &'a TreeStyle,
        describe: &(dyn Fn(usize) -> RowSpec + 'a),
        time: f64,
        scroll_to: Option<usize>,
        events: Vec<Event>,
    ) -> (FullOutput, TreeOutput<usize>) {
        draw_view(
            ctx,
            rows,
            style,
            describe,
            time,
            scroll_to,
            ViewExtras::default(),
            events,
        )
    }

    /// Run one headless frame with the inline editor open on `edit_row`; the
    /// buffer is the panel's, so it lives in the test across frames.
    fn draw_editing<'a>(
        ctx: &Context,
        rows: &'a [usize],
        style: &'a TreeStyle,
        describe: &(dyn Fn(usize) -> RowSpec + 'a),
        edit_row: usize,
        buffer: &'a mut String,
        time: f64,
        events: Vec<Event>,
    ) -> (FullOutput, TreeOutput<usize>) {
        draw_view(
            ctx,
            rows,
            style,
            describe,
            time,
            None,
            ViewExtras {
                highlight: None,
                highlight_region: None,
                editing: Some((edit_row, buffer)),
            },
            events,
        )
    }

    /// Run one headless frame, optionally with a drop-target highlight or the
    /// inline editor.
    fn draw_view<'a>(
        ctx: &Context,
        rows: &'a [usize],
        style: &'a TreeStyle,
        describe: &(dyn Fn(usize) -> RowSpec + 'a),
        time: f64,
        scroll_to: Option<usize>,
        extras: ViewExtras<'a>,
        events: Vec<Event>,
    ) -> (FullOutput, TreeOutput<usize>) {
        let ViewExtras {
            highlight,
            highlight_region,
            editing,
        } = extras;
        let mut editing = editing;
        let mut output: Option<TreeOutput<usize>> = None;
        let full = ctx.run_ui(input(time, events), |ui| {
            let mut view = TreeView::new("tree-tests", rows, style, |id| {
                let spec = describe(id);
                TreeRow {
                    depth: spec.depth,
                    label: spec.label,
                    suffix: spec.suffix,
                    icon: spec.icon,
                    thumbnail: None,
                    expander: spec.expander,
                    selected: spec.selected,
                    separator_above: spec.separator_above,
                }
            });
            if let Some(id) = scroll_to {
                view = view.scroll_to(id);
            }
            if let Some(id) = highlight {
                view = view.highlight(id);
            }
            if let Some(id) = highlight_region {
                view = view.highlight_region(id);
            }
            if let Some((edit_row, buffer)) = editing.take() {
                view = view.editing(edit_row, buffer);
            }
            output = Some(view.show(ui));
        });
        (full, output.expect("run_ui runs the tree callback once"))
    }

    /// Every rect the frame painted, with its fill.
    fn filled_rects(output: &FullOutput) -> Vec<(Rect, Color32)> {
        output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                Shape::Rect(shape) => Some((shape.rect, shape.fill)),
                _ => None,
            })
            .collect()
    }

    /// Every rect a frame outlined with the drop stroke.
    fn outlined_rects(output: &FullOutput, style: &TreeStyle) -> Vec<Rect> {
        output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                Shape::Rect(shape) if shape.stroke == style.drop_stroke => Some(shape.rect),
                _ => None,
            })
            .collect()
    }

    /// The rect of row `index` when the tree is scrolled to the top.
    fn row_rect(index: usize, style: &TreeStyle) -> Rect {
        Rect::from_min_size(
            pos2(0.0, index as f32 * style.row_height),
            vec2(VIEW.x, style.row_height),
        )
    }

    /// The virtualization contract: a 200k-row tree must only describe the rows
    /// the scroll area reports as visible.
    #[test]
    fn only_visible_rows_are_materialized() {
        let ctx = Context::default();
        let style = style();
        let rows = ids(200_000);
        let calls = AtomicUsize::new(0);
        let describe = |_id: usize| {
            calls.fetch_add(1, Ordering::Relaxed);
            RowSpec::default()
        };

        let (_, out) = draw(&ctx, &rows, &style, &describe, 0.0, None, Vec::new());

        let visible = (VIEW.y / style.row_height).ceil() as usize;
        let calls = calls.load(Ordering::Relaxed);
        assert!(
            calls >= visible,
            "the visible rows must be described: {calls} calls for {visible} visible rows"
        );
        assert!(
            calls <= visible + 2,
            "200k rows must not be materialized: {calls} row callback calls for {visible} visible rows"
        );
        assert_eq!(out.clicked, None);
    }

    #[test]
    fn an_empty_tree_reports_nothing() {
        let ctx = Context::default();
        let style = style();
        let rows: Vec<usize> = Vec::new();
        let describe = |_id: usize| RowSpec::default();

        let (_, out) = draw(&ctx, &rows, &style, &describe, 0.0, None, Vec::new());

        assert_eq!(out, TreeOutput::<usize>::default());
    }

    /// Clicking a folder's chevron toggles it and must not select the row: the
    /// chevron is a separate hit area inside the row.
    #[test]
    fn clicking_the_expander_toggles_without_selecting() {
        let ctx = Context::default();
        let style = style();
        let rows = ids(2);
        let describe = |id: usize| RowSpec {
            expander: (id == 0).then_some(Expander::Collapsed),
            ..RowSpec::default()
        };

        // An idle pass registers the rows for hit-testing: egui resolves a click
        // against the previous pass's widget rects.
        let _ = draw(&ctx, &rows, &style, &describe, 0.0, None, Vec::new());
        // The chevron hit box starts at the row's left padding, one row tall.
        let chevron = pos2(ROW_LEFT_PAD + style.expander * 0.5, style.row_height * 0.5);
        let (_, out) = draw(&ctx, &rows, &style, &describe, 0.1, None, click(chevron));

        assert_eq!(out.expander_toggled, Some(0));
        assert_eq!(out.clicked, None, "the chevron must not select its row");
    }

    /// A click on the row itself is a selection and gives the tree keyboard focus.
    #[test]
    fn clicking_a_row_reports_a_click_and_focuses_the_tree() {
        let ctx = Context::default();
        let style = style();
        let rows = ids(3);
        let describe = |_id: usize| RowSpec::default();

        let _ = draw(&ctx, &rows, &style, &describe, 0.0, None, Vec::new());
        // Past the chevron/icon columns, inside row 0's label area.
        let (_, out) = draw(
            &ctx,
            &rows,
            &style,
            &describe,
            0.1,
            None,
            click(pos2(120.0, 10.0)),
        );

        assert_eq!(out.clicked, Some(0));
        assert_eq!(out.double_clicked, None);
        assert_eq!(out.expander_toggled, None);
        assert!(out.focused, "a row click focuses the tree");
    }

    /// egui reports the second press of a double click as a click too; the widget
    /// forwards both and the caller decides which one wins.
    #[test]
    fn clicking_a_row_twice_reports_a_double_click() {
        let ctx = Context::default();
        let style = style();
        let rows = ids(3);
        let describe = |_id: usize| RowSpec::default();
        let at = pos2(120.0, 10.0);

        let _ = draw(&ctx, &rows, &style, &describe, 0.0, None, Vec::new());
        let (_, first) = draw(&ctx, &rows, &style, &describe, 0.1, None, click(at));
        assert_eq!(first.double_clicked, None);

        let (_, second) = draw(&ctx, &rows, &style, &describe, 0.2, None, click(at));
        assert_eq!(second.double_clicked, Some(0));
        assert_eq!(
            second.clicked,
            Some(0),
            "a double click also reports a click (documented in TreeOutput)"
        );
    }

    /// Hovering a row highlights it, and the hovered id is reported.
    #[test]
    fn hovering_a_row_paints_the_hover_fill() {
        let ctx = Context::default();
        let style = style();
        let rows = ids(3);
        let describe = |_id: usize| RowSpec::default();

        let _ = draw(&ctx, &rows, &style, &describe, 0.0, None, Vec::new());
        let (output, out) = draw(
            &ctx,
            &rows,
            &style,
            &describe,
            0.1,
            None,
            hover(pos2(120.0, 10.0)),
        );

        assert_eq!(out.hovered, Some(0));
        let hovered: Vec<Rect> = filled_rects(&output)
            .into_iter()
            .filter(|(_, fill)| *fill == style.hover_bg)
            .map(|(rect, _)| rect)
            .collect();
        assert_eq!(hovered, vec![row_rect(0, &style)]);
    }

    /// The tree keeps the focus it gained on click across later frames: a focus
    /// id only survives a frame in which some widget uses it, which is why the
    /// tree claims a focusable rect of its own.
    #[test]
    fn the_tree_keeps_keyboard_focus_across_frames() {
        let ctx = Context::default();
        let style = style();
        let rows = ids(3);
        let describe = |_id: usize| RowSpec::default();

        let (_, idle) = draw(&ctx, &rows, &style, &describe, 0.0, None, Vec::new());
        assert!(!idle.focused);

        let (_, clicked) = draw(
            &ctx,
            &rows,
            &style,
            &describe,
            0.1,
            None,
            click(pos2(120.0, 10.0)),
        );
        assert!(clicked.focused);

        let (_, later) = draw(&ctx, &rows, &style, &describe, 0.2, None, Vec::new());
        assert!(later.focused, "focus must survive frames without input");
    }

    /// The selected row paints the rounded selection fill over exactly its own
    /// rect, and only that row gets it.
    #[test]
    fn the_selected_row_paints_the_selection_fill() {
        let ctx = Context::default();
        let style = style();
        let rows = ids(3);
        let describe = |id: usize| RowSpec {
            selected: id == 1,
            ..RowSpec::default()
        };

        let (output, _) = draw(&ctx, &rows, &style, &describe, 0.0, None, Vec::new());

        let selected: Vec<Rect> = filled_rects(&output)
            .into_iter()
            .filter(|(_, fill)| *fill == style.selected_bg)
            .map(|(rect, _)| rect)
            .collect();
        assert_eq!(
            selected,
            vec![row_rect(1, &style)],
            "only the selected row is filled with selected_bg"
        );
    }

    /// A separator_above row paints a separator-width line on its top edge.
    #[test]
    fn separator_above_paints_a_one_pixel_line() {
        let ctx = Context::default();
        let style = style();
        let rows = ids(3);
        let describe = |id: usize| RowSpec {
            separator_above: id == 1,
            ..RowSpec::default()
        };

        let (output, _) = draw(&ctx, &rows, &style, &describe, 0.0, None, Vec::new());

        let lines: Vec<Rect> = filled_rects(&output)
            .into_iter()
            .filter(|(_, fill)| *fill == style.separator)
            .map(|(rect, _)| rect)
            .collect();
        let expected = Rect::from_min_max(
            pos2(0.0, style.row_height),
            pos2(VIEW.x, style.row_height + style.separator_width),
        );
        assert_eq!(lines, vec![expected]);
    }

    /// An icon path is a runtime asset: a missing file must not panic and must
    /// not paint a texture (there is nothing to paint).
    #[test]
    fn a_missing_icon_does_not_panic_or_paint_a_texture() {
        let ctx = Context::default();
        let style = style();
        let rows = ids(2);
        let describe = |_id: usize| RowSpec {
            icon: Some("Preferences/Textures/project/does_not_exist.png"),
            ..RowSpec::default()
        };

        let (output, out) = draw(&ctx, &rows, &style, &describe, 0.0, None, Vec::new());

        assert_eq!(out.clicked, None);
        assert!(
            output
                .shapes
                .iter()
                .all(|clipped| !matches!(&clipped.shape, Shape::Mesh(_))),
            "a missing icon file must not paint an image mesh"
        );
    }

    /// scroll_to must work for a row far outside the visible range: the scroll
    /// target is a content-space rect, not a response of an unrendered row.
    #[test]
    fn scroll_to_reaches_rows_outside_the_viewport() {
        let ctx = Context::default();
        let style = style();
        let rows = ids(1_000);
        let first = AtomicUsize::new(usize::MAX);
        let describe = |id: usize| {
            first.fetch_min(id, Ordering::Relaxed);
            RowSpec::default()
        };

        // Frame 1: still at the top; the scroll target is queued for next frame.
        let (_, _) = draw(&ctx, &rows, &style, &describe, 0.0, Some(500), Vec::new());
        assert_eq!(
            first.load(Ordering::Relaxed),
            0,
            "the first frame is at the top"
        );

        // Frame 2: the explicit time jump completes egui's scroll animation.
        first.store(usize::MAX, Ordering::Relaxed);
        let _ = draw(&ctx, &rows, &style, &describe, 1.0, Some(500), Vec::new());

        // Frame 3: the scroll area now paints around row 500.
        first.store(usize::MAX, Ordering::Relaxed);
        let _ = draw(&ctx, &rows, &style, &describe, 1.1, None, Vec::new());
        let first_visible = first.load(Ordering::Relaxed);
        assert!(
            (490..=505).contains(&first_visible),
            "row 500 should be centred in view, first visible row was {first_visible}"
        );
    }

    /// A right click reports the row and the pointer position for the panel's
    /// context menu; it is not a selection and does not move keyboard focus.
    #[test]
    fn a_right_click_reports_the_row_its_position_and_no_selection() {
        let ctx = Context::default();
        let style = style();
        let rows = ids(3);
        let describe = |_id: usize| RowSpec::default();
        let at = pos2(120.0, 10.0);

        let _ = draw(&ctx, &rows, &style, &describe, 0.0, None, Vec::new());
        let (_, out) = draw(&ctx, &rows, &style, &describe, 0.1, None, right_click(at));

        assert_eq!(out.secondary_clicked, Some(0));
        assert_eq!(out.secondary_pos, Some(at));
        assert_eq!(out.clicked, None, "a right click must not select the row");
        assert_eq!(out.double_clicked, None);
        assert!(!out.focused, "a right click must not move keyboard focus");

        // Once the tree owns keyboard focus, a context-menu press keeps it: the
        // panel's arrow keys must survive right-clicking a row.
        let (_, focused) = draw(&ctx, &rows, &style, &describe, 0.2, None, click(at));
        assert!(focused.focused);
        let (_, after) = draw(&ctx, &rows, &style, &describe, 0.3, None, right_click(at));
        assert!(after.focused, "a right click keeps the focus the tree had");
    }

    /// The drag lifecycle, frame by frame: pressing arms the drag, moving past
    /// egui's threshold starts it, and releasing still reports the row under the
    /// pointer so the panel can finish the move.
    #[test]
    fn a_row_drag_reports_start_progress_and_drop_target() {
        let ctx = Context::default();
        let style = style();
        let rows = ids(3);
        let describe = |_id: usize| RowSpec::default();
        let from = pos2(120.0, 10.0);
        let to = pos2(120.0, 50.0);

        let _ = draw(&ctx, &rows, &style, &describe, 0.0, None, Vec::new());

        // Frame 1: press on row 0. A press alone is not a drag yet.
        let (_, pressed) = draw(&ctx, &rows, &style, &describe, 0.1, None, press(from));
        assert_eq!(pressed.drag_started, None);
        assert!(!pressed.dragging);
        assert!(!pressed.drag_stopped);
        assert_eq!(pressed.drop_target, None);

        // Frame 2: still held, pointer moved over row 2. This is the start frame.
        let (_, started) = draw(&ctx, &rows, &style, &describe, 0.2, None, hover(to));
        assert_eq!(started.drag_started, Some(0));
        assert!(started.dragging);
        assert!(!started.drag_stopped);
        assert_eq!(started.drop_target, Some(2));

        // Frame 3: release over row 2. The drag is over, but the drop is still
        // reported so the panel can finish the move.
        let (_, released) = draw(&ctx, &rows, &style, &describe, 0.3, None, release(to));
        assert_eq!(released.drag_started, None);
        assert!(!released.dragging);
        assert!(released.drag_stopped);
        assert_eq!(released.drop_target, Some(2));
        assert_eq!(released.clicked, None, "a drag is not a click");
    }

    /// A pointer wobble below egui's drag threshold stays a click: none of the
    /// drag channels fire.
    #[test]
    fn a_sub_threshold_move_still_uses_the_click_channel() {
        let ctx = Context::default();
        let style = style();
        let rows = ids(3);
        let describe = |_id: usize| RowSpec::default();
        let from = pos2(120.0, 10.0);

        let _ = draw(&ctx, &rows, &style, &describe, 0.0, None, Vec::new());
        let (_, pressed) = draw(&ctx, &rows, &style, &describe, 0.1, None, press(from));
        assert_eq!(pressed.drag_started, None);

        let (_, out) = draw(
            &ctx,
            &rows,
            &style,
            &describe,
            0.2,
            None,
            release(pos2(122.0, 10.0)),
        );

        assert_eq!(out.drag_started, None, "a 2 px move is not a drag");
        assert!(!out.dragging);
        assert!(!out.drag_stopped);
        assert_eq!(out.drop_target, None);
        assert_eq!(
            out.clicked,
            Some(0),
            "a sub-threshold move is still a click"
        );
    }

    /// The drop-target highlight is a rounded fill plus an outline over exactly
    /// the highlighted row, and never over a neighbour.
    #[test]
    fn a_highlighted_row_paints_the_drop_target_fill_and_outline() {
        let ctx = Context::default();
        let style = style();
        let rows = ids(3);
        let describe = |_id: usize| RowSpec::default();

        let (output, out) = draw_view(
            &ctx,
            &rows,
            &style,
            &describe,
            0.0,
            None,
            ViewExtras {
                highlight: Some(1),
                highlight_region: None,
                editing: None,
            },
            Vec::new(),
        );

        assert_eq!(out.clicked, None);
        let washed: Vec<Rect> = filled_rects(&output)
            .into_iter()
            .filter(|(_, fill)| *fill == style.drop_fill)
            .map(|(rect, _)| rect)
            .collect();
        assert_eq!(
            washed,
            vec![row_rect(1, &style)],
            "only the highlighted row gets the drop fill"
        );

        let outlined: Vec<Rect> = output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                Shape::Rect(shape) if shape.stroke == style.drop_stroke => Some(shape.rect),
                _ => None,
            })
            .collect();
        assert_eq!(
            outlined,
            vec![row_rect(1, &style)],
            "the outline sits on the same row as the fill"
        );
    }

    /// Typing goes straight into the panel's buffer: the widget draws the editor
    /// and owns no text of its own.
    #[test]
    fn typing_reaches_the_inline_editor_buffer() {
        let ctx = Context::default();
        let style = style();
        let rows = ids(3);
        let describe = |_id: usize| RowSpec::default();
        let mut buffer = String::from("row");

        let _ = draw(&ctx, &rows, &style, &describe, 0.0, None, Vec::new());
        // Frame 1: the fresh editor takes focus and selects the whole name.
        let (_, opened) = draw_editing(
            &ctx,
            &rows,
            &style,
            &describe,
            1,
            &mut buffer,
            0.1,
            Vec::new(),
        );
        assert!(!opened.edit_committed);
        assert!(!opened.edit_cancelled);

        // Frame 2: one typed character replaces the selected name.
        let (_, out) = draw_editing(
            &ctx,
            &rows,
            &style,
            &describe,
            1,
            &mut buffer,
            0.2,
            typed("x"),
        );
        assert_eq!(buffer, "x");
        assert!(!out.edit_committed);
    }

    /// Clicking inside the editor edits text there: the row is registered below
    /// the editor, so the click must not select the row.
    #[test]
    fn clicking_the_inline_editor_does_not_click_the_row() {
        let ctx = Context::default();
        let style = style();
        let rows = ids(3);
        let describe = |_id: usize| RowSpec::default();
        let mut buffer = String::from("row");

        let _ = draw(&ctx, &rows, &style, &describe, 0.0, None, Vec::new());
        let _ = draw_editing(
            &ctx,
            &rows,
            &style,
            &describe,
            1,
            &mut buffer,
            0.1,
            Vec::new(),
        );
        // Middle of row 1's label area, which the editor covers.
        let (_, out) = draw_editing(
            &ctx,
            &rows,
            &style,
            &describe,
            1,
            &mut buffer,
            0.2,
            click(pos2(150.0, 30.0)),
        );

        assert_eq!(out.clicked, None, "the editor sits above the row response");
        assert!(!out.edit_committed);
    }

    /// Enter commits the edit without the widget touching the buffer.
    #[test]
    fn enter_commits_the_inline_edit() {
        let ctx = Context::default();
        let style = style();
        let rows = ids(3);
        let describe = |_id: usize| RowSpec::default();
        let mut buffer = String::from("row");

        let _ = draw(&ctx, &rows, &style, &describe, 0.0, None, Vec::new());
        let _ = draw_editing(
            &ctx,
            &rows,
            &style,
            &describe,
            1,
            &mut buffer,
            0.1,
            Vec::new(),
        );
        let (_, out) = draw_editing(
            &ctx,
            &rows,
            &style,
            &describe,
            1,
            &mut buffer,
            0.2,
            key(Key::Enter),
        );

        assert!(out.edit_committed);
        assert!(!out.edit_cancelled);
        assert_eq!(buffer, "row", "Enter does not change the buffer");
        assert_eq!(out.clicked, None);
    }

    /// Escape cancels the edit and leaves the buffer to the panel: the widget
    /// neither reverts nor clears it.
    #[test]
    fn escape_cancels_the_inline_edit_without_touching_the_buffer() {
        let ctx = Context::default();
        let style = style();
        let rows = ids(3);
        let describe = |_id: usize| RowSpec::default();
        let mut buffer = String::from("row");

        let _ = draw(&ctx, &rows, &style, &describe, 0.0, None, Vec::new());
        let _ = draw_editing(
            &ctx,
            &rows,
            &style,
            &describe,
            1,
            &mut buffer,
            0.1,
            Vec::new(),
        );
        let (_, typed_out) = draw_editing(
            &ctx,
            &rows,
            &style,
            &describe,
            1,
            &mut buffer,
            0.2,
            typed("x"),
        );
        assert!(!typed_out.edit_cancelled);
        assert_eq!(buffer, "x");

        let (_, out) = draw_editing(
            &ctx,
            &rows,
            &style,
            &describe,
            1,
            &mut buffer,
            0.3,
            key(Key::Escape),
        );

        assert!(out.edit_cancelled);
        assert!(!out.edit_committed);
        assert_eq!(
            buffer, "x",
            "the widget must not restore the original name; the panel does"
        );
    }

    /// Losing focus to another widget commits the edit, like Enter.
    #[test]
    fn clicking_another_row_commits_the_inline_edit() {
        let ctx = Context::default();
        let style = style();
        let rows = ids(3);
        let describe = |_id: usize| RowSpec::default();
        let mut buffer = String::from("row");

        let _ = draw(&ctx, &rows, &style, &describe, 0.0, None, Vec::new());
        let _ = draw_editing(
            &ctx,
            &rows,
            &style,
            &describe,
            1,
            &mut buffer,
            0.1,
            Vec::new(),
        );
        // Row 0 is drawn before the edited row 1, so the focus move and the
        // editor's lost-focus check happen in the same frame.
        let (_, out) = draw_editing(
            &ctx,
            &rows,
            &style,
            &describe,
            1,
            &mut buffer,
            0.2,
            click(pos2(120.0, 10.0)),
        );

        assert!(out.edit_committed, "a lost focus commits like Enter");
        assert!(!out.edit_cancelled);
        assert_eq!(out.clicked, Some(0), "the click still selects its row");
        assert_eq!(buffer, "row");
    }

    /// A commit the panel rejects keeps the editor open: the panel simply passes
    /// the editing state again and the widget re-focuses the box, so the user can
    /// fix the name.
    #[test]
    fn a_rejected_commit_keeps_the_editor_focused() {
        let ctx = Context::default();
        let style = style();
        let rows = ids(3);
        let describe = |_id: usize| RowSpec::default();
        let mut buffer = String::from("row");

        let _ = draw(&ctx, &rows, &style, &describe, 0.0, None, Vec::new());
        let _ = draw_editing(
            &ctx,
            &rows,
            &style,
            &describe,
            1,
            &mut buffer,
            0.1,
            Vec::new(),
        );
        let _ = draw_editing(
            &ctx,
            &rows,
            &style,
            &describe,
            1,
            &mut buffer,
            0.2,
            typed("x"),
        );
        let (_, committed) = draw_editing(
            &ctx,
            &rows,
            &style,
            &describe,
            1,
            &mut buffer,
            0.3,
            key(Key::Enter),
        );
        assert!(committed.edit_committed);
        assert_eq!(buffer, "x");

        // The panel validated `x` (say it is a reserved name) and keeps the
        // editor open by passing the same id and buffer again.
        let (_, kept) = draw_editing(
            &ctx,
            &rows,
            &style,
            &describe,
            1,
            &mut buffer,
            0.4,
            Vec::new(),
        );
        assert!(!kept.edit_committed);
        assert!(!kept.edit_cancelled);

        let _ = draw_editing(
            &ctx,
            &rows,
            &style,
            &describe,
            1,
            &mut buffer,
            0.5,
            typed("y"),
        );
        assert_eq!(
            buffer, "xy",
            "the editor must still be focused after the rejected commit"
        );
    }

    /// The dragged row itself is a drop target: the panel's move rules decide
    /// whether dropping onto the source means anything, not the widget.
    #[test]
    fn the_dragged_row_is_also_a_drop_target() {
        let ctx = Context::default();
        let style = style();
        let rows = ids(3);
        let describe = |_id: usize| RowSpec::default();

        let _ = draw(&ctx, &rows, &style, &describe, 0.0, None, Vec::new());
        let _ = draw(
            &ctx,
            &rows,
            &style,
            &describe,
            0.1,
            None,
            press(pos2(100.0, 5.0)),
        );
        // 40 px sideways, still inside row 0: past the drag threshold, still the
        // dragged row under the pointer.
        let (_, out) = draw(
            &ctx,
            &rows,
            &style,
            &describe,
            0.2,
            None,
            hover(pos2(140.0, 5.0)),
        );
        assert_eq!(out.drag_started, Some(0));
        assert!(out.dragging);
        assert_eq!(out.drop_target, Some(0));
    }

    /// Releasing over empty space below the rows still stops the drag, but there
    /// is no drop target.
    #[test]
    fn a_drag_over_empty_space_has_no_drop_target() {
        let ctx = Context::default();
        let style = style();
        let rows = ids(3);
        let describe = |_id: usize| RowSpec::default();
        let below_rows = pos2(120.0, 190.0);

        let _ = draw(&ctx, &rows, &style, &describe, 0.0, None, Vec::new());
        let _ = draw(
            &ctx,
            &rows,
            &style,
            &describe,
            0.1,
            None,
            press(pos2(120.0, 10.0)),
        );
        let (_, moved) = draw(&ctx, &rows, &style, &describe, 0.2, None, hover(below_rows));
        assert!(moved.dragging);
        assert_eq!(moved.drop_target, None);

        let (_, released) = draw(
            &ctx,
            &rows,
            &style,
            &describe,
            0.3,
            None,
            release(below_rows),
        );
        assert!(released.drag_stopped);
        assert!(!released.dragging);
        assert_eq!(released.drop_target, None);
    }
    /// Every text run a frame painted.
    fn texts(output: &FullOutput) -> Vec<String> {
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

    /// A folder's drop region is its row plus its visible subtree, outlined as
    /// one box: the whole area is the drop target.
    #[test]
    fn a_folder_drop_region_outlines_its_visible_subtree() {
        let ctx = Context::default();
        let style = style();
        let rows = ids(4);
        // rows: folder (depth 0), child, child, sibling (depth 0).
        let describe = |id: usize| RowSpec {
            depth: if id == 0 || id == 3 { 0 } else { 1 },
            expander: (id == 0).then_some(Expander::Expanded),
            ..RowSpec::default()
        };

        let (output, _) = draw_view(
            &ctx,
            &rows,
            &style,
            &describe,
            0.0,
            None,
            ViewExtras {
                highlight: None,
                highlight_region: Some(0),
                editing: None,
            },
            Vec::new(),
        );

        let outlined: Vec<Rect> = output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                Shape::Rect(shape) if shape.stroke == style.drop_stroke => Some(shape.rect),
                _ => None,
            })
            .collect();
        assert_eq!(outlined.len(), 1, "one box for the whole region");
        assert_eq!(
            outlined[0],
            Rect::from_min_max(pos2(0.0, 0.0), pos2(VIEW.x, 3.0 * style.row_height),),
            "the box covers the folder row and its two children, not the sibling"
        );
    }

    /// A row drag that has auto-scrolled its own source row away still ends on
    /// the release: the drag is a tree session, not the source row's response.
    #[test]
    fn a_drag_finishes_after_auto_scrolling_hides_its_source_row() {
        const LABELS: [&str; 20] = [
            "r0", "r1", "r2", "r3", "r4", "r5", "r6", "r7", "r8", "r9", "r10", "r11", "r12", "r13",
            "r14", "r15", "r16", "r17", "r18", "r19",
        ];
        let ctx = Context::default();
        let style = style();
        let rows = ids(20);
        let describe = |id: usize| RowSpec {
            label: LABELS[id],
            ..RowSpec::default()
        };
        let edge = pos2(120.0, VIEW.y - 5.0);

        let _ = draw(&ctx, &rows, &style, &describe, 0.0, None, Vec::new());
        let _ = draw(
            &ctx,
            &rows,
            &style,
            &describe,
            0.1,
            None,
            press(pos2(120.0, 10.0)),
        );
        // Move to the bottom edge: the drag starts and the tree auto-scrolls.
        let (_, started) = draw(&ctx, &rows, &style, &describe, 0.2, None, hover(edge));
        assert_eq!(started.drag_started, Some(0));

        // Hold still: dragging at the edge scrolls the tree by itself until the
        // source row is outside the visible range.
        let mut last = None;
        for frame in 0..30 {
            let (output, out) = draw(
                &ctx,
                &rows,
                &style,
                &describe,
                0.3 + f64::from(frame) * 0.016,
                None,
                Vec::new(),
            );
            assert!(out.dragging, "the drag must stay in flight while held");
            last = Some((output, out));
        }
        let (output, _) = last.expect("frames ran");
        let painted = texts(&output);
        assert!(
            !painted.iter().any(|text| text == "r0"),
            "the source row scrolled away: {painted:?}"
        );

        let (_, released) = draw(&ctx, &rows, &style, &describe, 0.8, None, release(edge));
        assert!(
            released.drag_stopped,
            "the release still ends a drag whose row is gone"
        );
        assert!(!released.dragging);
    }

    /// The wheel scrolls the tree while a row drag is held: egui's scroll area
    /// ignores the wheel during a widget drag, so the tree translates it itself.
    #[test]
    fn the_wheel_scrolls_the_tree_while_a_row_drag_is_held() {
        const LABELS: [&str; 20] = [
            "r0", "r1", "r2", "r3", "r4", "r5", "r6", "r7", "r8", "r9", "r10", "r11", "r12", "r13",
            "r14", "r15", "r16", "r17", "r18", "r19",
        ];
        let ctx = Context::default();
        let style = style();
        let rows = ids(20);
        let describe = |id: usize| RowSpec {
            label: LABELS[id],
            ..RowSpec::default()
        };

        let _ = draw(&ctx, &rows, &style, &describe, 0.0, None, Vec::new());
        let _ = draw(
            &ctx,
            &rows,
            &style,
            &describe,
            0.1,
            None,
            press(pos2(120.0, 10.0)),
        );
        let (_, started) = draw(
            &ctx,
            &rows,
            &style,
            &describe,
            0.2,
            None,
            hover(pos2(120.0, 50.0)),
        );
        assert!(started.dragging);

        // Wheel down while the button is still held.
        let (_, wheel) = draw(
            &ctx,
            &rows,
            &style,
            &describe,
            0.3,
            None,
            vec![Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: vec2(0.0, -240.0),
                phase: egui::TouchPhase::Move,
                modifiers: Modifiers::NONE,
            }],
        );
        assert!(wheel.dragging, "the drag survives the wheel");

        // The scroll is applied at the end of the wheel frame; the following
        // frames paint rows that were below the viewport before.
        let mut painted = Vec::new();
        for frame in 0..3 {
            let (output, _) = draw(
                &ctx,
                &rows,
                &style,
                &describe,
                0.4 + f64::from(frame) * 0.016,
                None,
                Vec::new(),
            );
            painted = texts(&output);
        }
        assert!(
            !painted.iter().any(|text| text == "r0"),
            "the wheel must scroll the dragged tree: {painted:?}"
        );
        assert!(
            painted.iter().any(|text| text == "r19"),
            "the viewport reached the rows that were off screen: {painted:?}"
        );
    }
    /// The folder row can be scrolled above the viewport while its children are
    /// visible; the region box then starts at the top edge instead of vanishing.
    #[test]
    fn a_drop_region_stays_visible_when_its_folder_row_is_scrolled_away() {
        let ctx = Context::default();
        let style = style();
        let rows = ids(20);
        // rows: folder (depth 0), ten children (depth 1), then siblings.
        let describe = |id: usize| RowSpec {
            depth: if id == 0 {
                0
            } else if id <= 10 {
                1
            } else {
                0
            },
            expander: (id == 0).then_some(Expander::Expanded),
            ..RowSpec::default()
        };

        // Scroll row 9 into view so the folder row (0) is above the viewport.
        let _ = draw(&ctx, &rows, &style, &describe, 0.0, Some(9), Vec::new());
        let _ = draw(&ctx, &rows, &style, &describe, 1.0, Some(9), Vec::new());
        let (output, _) = draw_view(
            &ctx,
            &rows,
            &style,
            &describe,
            1.1,
            None,
            ViewExtras {
                highlight: None,
                highlight_region: Some(0),
                editing: None,
            },
            Vec::new(),
        );

        let outlined = outlined_rects(&output, &style);
        assert_eq!(
            outlined.len(),
            1,
            "one box while the folder row is off screen"
        );
        let region = outlined[0];
        assert!(
            region.top() <= 0.0,
            "the box starts at the viewport top, got {region:?}"
        );
        assert!(
            region.height() > 4.0 * style.row_height,
            "the box covers the visible descendants, got {region:?}"
        );
    }

    /// A collapsed folder's region is its own row: there are no visible
    /// descendants to extend over, and the box must not run to the viewport edge.
    #[test]
    fn a_collapsed_folders_region_is_its_own_row() {
        let ctx = Context::default();
        let style = style();
        let rows = ids(3);
        let describe = |id: usize| RowSpec {
            expander: (id == 0).then_some(Expander::Collapsed),
            ..RowSpec::default()
        };

        let (output, _) = draw_view(
            &ctx,
            &rows,
            &style,
            &describe,
            0.0,
            None,
            ViewExtras {
                highlight: None,
                highlight_region: Some(0),
                editing: None,
            },
            Vec::new(),
        );

        assert_eq!(outlined_rects(&output, &style), vec![row_rect(0, &style)]);
    }
}
