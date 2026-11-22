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
//! - **The tree, not a row, is the keyboard focus unit.** Rows sense clicks but
//!   are not focusable widgets, so a huge tree does not flood egui's focus
//!   bookkeeping with 200k candidates. The tree claims one focusable rect
//!   underneath the rows, requests focus on any click in the tree, and reports
//!   [`TreeOutput::focused`] so the panel can own arrow-key handling.
//!
//! The tree is pure appearance: it allocates, paints and hit-tests rows, and
//! reports single/double clicks, chevron toggles, the hovered row and its own
//! keyboard focus. Renaming, dragging and context menus are the panel's job and
//! are deliberately not part of this API yet (M2's scope decision).

use std::hash::{DefaultHasher, Hash, Hasher};

use egui::{
    Align, Color32, CornerRadius, EventFilter, FontId, Painter, Pos2, Rect, Response, ScrollArea,
    Sense, Shape, Stroke, Ui, pos2, vec2,
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
/// so a panel skin and the tree can never disagree. Plain [`egui::Color32`]
/// and `f32` keep the widget crate free of domain and colour-space types.
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
}

impl<Id> Default for TreeOutput<Id> {
    fn default() -> Self {
        Self {
            clicked: None,
            double_clicked: None,
            expander_toggled: None,
            hovered: None,
            focused: false,
        }
    }
}

/// A row list plus the style and callback needed to paint it, virtualized.
///
/// Construct one per frame; see [`TreeView::new`],
/// [`TreeView::scroll_to`] and [`TreeView::show`]. The builder is cheap:
/// one small box for the row closure and two borrows.
pub struct TreeView<'a, Id> {
    /// Root id folded from the caller's salt; [`TreeView::show`] salts the ui
    /// id with it so row ids stay stable across frames.
    tree_salt: egui::Id,
    rows: &'a [Id],
    style: &'a TreeStyle,
    row: Box<dyn Fn(Id) -> TreeRow<'a> + 'a>,
    scroll_to: Option<Id>,
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

    /// Paint the tree and report this frame's interaction.
    pub fn show(self, ui: &mut Ui) -> TreeOutput<Id> {
        let Self {
            tree_salt,
            rows,
            style,
            row,
            scroll_to,
        } = self;
        let tree_id = ui.make_persistent_id(tree_salt);
        let mut output = TreeOutput::default();
        let scroll_to_index =
            scroll_to.and_then(|target| rows.iter().position(|row_id| *row_id == target));

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

                        // A row is one click target; the chevron inside it is a
                        // second, smaller target registered after the row so it
                        // wins the pointer (egui hit-tests the last widget added
                        // first). Clicking the chevron must not select the row.
                        let response = ui.interact(rect, egui_row_id, Sense::CLICK);
                        let expander_response = data.expander.map(|_| {
                            ui.interact(layout.expander, egui_row_id.with("expander"), Sense::CLICK)
                        });

                        let hovered = response.hovered()
                            || expander_response.as_ref().is_some_and(Response::hovered);
                        paint_row(ui, rect, style, &data, &layout, hovered);

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

        if tree_response.clicked() {
            // Clicking the tree's empty space focuses it, like clicking a row.
            ui.memory_mut(|memory| memory.request_focus(tree_id));
        }
        output.focused = ui.memory(|memory| memory.has_focus(tree_id));
        output
    }
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

/// Paint the background, separator, chevron, icon, label and suffix of one row.
///
/// hovered is the union of the row's and the chevron's responses: hovering the
/// chevron still highlights the row it belongs to.
fn paint_row(
    ui: &Ui,
    rect: Rect,
    style: &TreeStyle,
    data: &TreeRow<'_>,
    layout: &RowLayout,
    hovered: bool,
) {
    let radius = CornerRadius::same(style.selected_radius);
    if data.selected {
        ui.painter().rect_filled(rect, radius, style.selected_bg);
    } else if hovered && style.hover_bg.a() > 0 {
        ui.painter().rect_filled(rect, radius, style.hover_bg);
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

    if let Some(icon) = data.icon {
        // Missing or still-loading icon files paint nothing (see ui_image); this
        // is a runtime asset path, so an absent file must never panic.
        ui_image::paint(ui, layout.icon, icon, Color32::WHITE);
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

    use egui::{Context, Event, FullOutput, Modifiers, PointerButton, Pos2, RawInput};

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
        let mut output = TreeOutput::default();
        let full = ctx.run_ui(input(time, events), |ui| {
            let mut view = TreeView::new("tree-tests", rows, style, |id| {
                let spec = describe(id);
                TreeRow {
                    depth: spec.depth,
                    label: spec.label,
                    suffix: spec.suffix,
                    icon: spec.icon,
                    expander: spec.expander,
                    selected: spec.selected,
                    separator_above: spec.separator_above,
                }
            });
            if let Some(id) = scroll_to {
                view = view.scroll_to(id);
            }
            output = view.show(ui);
        });
        (full, output)
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
}
