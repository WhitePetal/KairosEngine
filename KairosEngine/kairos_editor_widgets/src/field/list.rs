//! The list field: a `Vec<T>` edited in place.
//!
//! A list is a compound field. The widget owns its chrome (header, count,
//! add/remove, element bands, folding and drag reordering) and mutates the
//! borrowed vector directly, like [super::NumberField] mutates its number. It
//! does not know how to draw `T`: the caller hands in a `row` closure, which is
//! the same boundary the other field widgets use to stay domain-free.
//!
//! The cross-frame interaction memory is [ListState], owned by the caller and
//! passed back every pass. Its fields are private: a caller may store, reset or
//! read it, but the widget is the only code that interprets or repairs it. That
//! is the [super::NumberEdit] contract applied to a whole list.

use std::cmp::Ordering;
use std::collections::BTreeSet;

use egui::{
    Color32, CornerRadius, CursorIcon, Id, Rect, Response, Sense, Stroke, StrokeKind, Ui,
    WidgetType, pos2, vec2,
};

use super::style::FieldStyle;
use super::{FieldOutcome, NumberEdit, NumberField};
use crate::paint::labelled_widget_info;
use crate::text::TextStyle;

/// Stable id of a list's header band. Clicking it folds the whole list.
pub fn list_header_id(field: Id) -> Id {
    field.with("header")
}

/// Stable id of a list's count box.
pub fn list_count_id(field: Id) -> Id {
    field.with("count")
}

/// Stable id of a list's add cell.
pub fn list_add_id(field: Id) -> Id {
    field.with("add")
}

/// Stable id of a list's remove cell.
pub fn list_remove_id(field: Id) -> Id {
    field.with("remove")
}

/// Stable id of one element's selection band.
pub fn list_item_id(field: Id, index: usize) -> Id {
    field.with(("item", index))
}

/// Stable id of one element's drag handle.
pub fn list_handle_id(field: Id, index: usize) -> Id {
    field.with(("handle", index))
}

/// Stable id of one element's fold caret.
pub fn list_fold_id(field: Id, index: usize) -> Id {
    field.with(("fold", index))
}

/// What one element's row closure did this pass.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ListItemOutcome {
    /// A control inside the element was clicked: select this element. A field
    /// widget's click should be forwarded here so removing the selected entry
    /// targets what the user last touched, not merely the last band they hit.
    pub clicked: bool,
    /// A value in the element changed.
    pub changed: bool,
    /// A typed edit or picker gesture inside the element ended.
    pub finished: bool,
}

impl ListItemOutcome {
    /// Forward one field widget's outcome from an element row. A click, a
    /// scrub or a live value change selects the element, matching how the
    /// Preferences page treats a field row.
    pub fn from_field(outcome: &FieldOutcome) -> Self {
        Self {
            clicked: outcome.clicked() || outcome.changed() || outcome.response.dragged(),
            changed: outcome.changed(),
            finished: outcome.finished,
        }
    }
}

/// What a [ListField] did this pass.
#[derive(Debug)]
pub struct ListOutcome {
    /// The list header's response. It is marked changed when a structural edit
    /// or an element edit landed.
    pub response: Response,
    /// A value or the list's shape changed.
    pub changed: bool,
    /// A typed edit or a discrete structural edit ended. A caller that batches
    /// writes can flush now.
    pub finished: bool,
}

/// Cross-frame interaction memory for one list.
///
/// The caller owns this value between passes, just like [NumberEdit]. Its
/// fields are private so a caller cannot write an out-of-range selection or a
/// dangling drag; the list widget normalizes the state against the current
/// vector every pass.
#[derive(Default)]
pub struct ListState {
    selected: Option<usize>,
    collapsed: bool,
    closed: BTreeSet<usize>,
    count_edit: Option<NumberEdit>,
    drag: Option<Drag>,
}

#[derive(Clone, Copy, Debug)]
struct Drag {
    from: usize,
}

impl ListState {
    /// The selected element, if any.
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// Whether the whole list is folded.
    pub fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    /// Whether one element's body is open. Elements start open.
    pub fn is_open(&self, index: usize) -> bool {
        !self.closed.contains(&index)
    }

    /// Drop every interaction memory: selection, folds, the count editor and
    /// any drag. The next pass draws a fresh list.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    fn normalize(&mut self, len: usize) {
        if self.selected.is_some_and(|index| index >= len) {
            self.selected = None;
        }
        self.closed.retain(|index| *index < len);
        if self.drag.is_some_and(|drag| drag.from >= len) {
            self.drag = None;
        }
    }
}

/// A row descriptor handed to [ListField]'s element closure.
pub struct ListItem {
    /// The element's index in the current vector.
    pub index: usize,
    /// Base id for the element's child widgets. Derive ids from this, not from
    /// the screen position.
    pub id: Id,
    /// This element is the selected one.
    pub selected: bool,
}

/// One list of values, edited in place.
///
/// The list is built from `items` and mutated directly: add, remove, resize and
/// reorder all change the borrowed vector. The caller supplies:
///
/// * a factory for new entries (used by add and by growing the count);
/// * a row closure that draws one element's fields;
/// * the maximum length, if the list has a UI cap.
///
/// The widget draws the header and the element bands and calls the row closure
/// only for open elements. An element row returns [ListItemOutcome] so clicks,
/// live edits and finished edits feed back into the list's selection and
/// [ListOutcome].
pub struct ListField<'a, T> {
    id: Id,
    items: &'a mut Vec<T>,
    style: &'a FieldStyle,
    label: &'a str,
    max: usize,
    make: Box<dyn FnMut() -> T + 'a>,
}

impl<'a, T> ListField<'a, T> {
    pub fn new(
        id: Id,
        items: &'a mut Vec<T>,
        style: &'a FieldStyle,
        make: impl FnMut() -> T + 'a,
    ) -> Self {
        Self {
            id,
            items,
            style,
            label: "",
            max: usize::MAX,
            make: Box::new(make),
        }
    }

    /// Accessibility name of the list, drawn on its header band.
    pub fn label(mut self, label: &'a str) -> Self {
        self.label = label;
        self
    }

    /// Largest length the count and add controls may reach. The type itself is
    /// still `usize`; this is a UI limit, not a type bound.
    pub fn max(mut self, max: usize) -> Self {
        self.max = max;
        self
    }

    /// Draw the list and mutate `items` directly.
    ///
    /// The row closure draws one element's fields into a vertically laid-out
    /// child `Ui`; it must use `item.id` as the base for child widget ids.
    pub fn show(
        self,
        ui: &mut Ui,
        state: &mut ListState,
        mut row: impl FnMut(&mut T, &mut Ui, ListItem) -> ListItemOutcome,
    ) -> ListOutcome {
        let Self {
            id,
            items,
            style,
            label,
            max,
            mut make,
        } = self;
        let pointer_down = ui.input(|input| input.pointer.primary_down());
        state.normalize(items.len());

        let mut pending = None;
        let mut finished = false;
        let mut header = draw_header(
            ui,
            id,
            style,
            label,
            items.len(),
            max,
            state,
            &mut pending,
            &mut finished,
        );

        let mut changed = false;
        let mut bands: Vec<(usize, Rect)> = Vec::new();

        if !state.collapsed {
            for (index, item) in items.iter_mut().enumerate() {
                let selected = state.selected == Some(index);
                let band = draw_item_band(ui, id, index, style, selected, state.is_open(index));

                if band.fold_clicked {
                    if state.closed.contains(&index) {
                        state.closed.remove(&index);
                    } else {
                        state.closed.insert(index);
                    }
                }
                if band.select_clicked {
                    state.selected = Some(index);
                }
                if band.drag_started {
                    state.drag = Some(Drag { from: index });
                }
                bands.push((index, band.rect));

                if state.is_open(index) {
                    let item_id = list_item_id(id, index);
                    let selected = state.selected == Some(index);
                    let outcome = ui
                        .indent(item_id.with("body"), |ui| {
                            row(
                                item,
                                ui,
                                ListItem {
                                    index,
                                    id: item_id,
                                    selected,
                                },
                            )
                        })
                        .inner;
                    if outcome.clicked {
                        state.selected = Some(index);
                    }
                    if outcome.changed {
                        changed = true;
                    }
                    if outcome.finished {
                        finished = true;
                    }
                }
            }
        }

        if let Some(drag) = state.drag {
            let released = ui.input(|input| input.pointer.any_released());
            if !pointer_down && !released {
                // The button came up without a release event (the window lost
                // focus): there is no drop to apply.
                state.drag = None;
            } else if state.collapsed || bands.is_empty() {
                state.drag = None;
            } else if let Some(pointer) = ui.ctx().pointer_interact_pos() {
                let slot = bands
                    .iter()
                    .take_while(|(_, rect)| pointer.y > rect.center().y)
                    .count();
                let first = bands[0].1;
                let y = if slot == 0 {
                    first.top()
                } else {
                    bands[slot - 1].1.bottom()
                };
                ui.painter().rect_filled(
                    Rect::from_min_size(
                        pos2(first.left() + 2.0, y - 1.0),
                        vec2(first.width() - 4.0, 2.0),
                    ),
                    CornerRadius::ZERO,
                    style.accent,
                );

                if released {
                    let to = if slot > drag.from { slot - 1 } else { slot };
                    if to != drag.from && to < items.len() && pending.is_none() {
                        pending = Some(Edit::Move {
                            from: drag.from,
                            to,
                        });
                    }
                    state.drag = None;
                }
            } else if !pointer_down {
                // The pointer left the window without a release: there is no
                // slot to drop on.
                state.drag = None;
            }
        }

        if let Some(edit) = pending
            && apply_edit(items, &mut make, state, edit, max)
        {
            changed = true;
            finished = true;
        }

        if changed {
            header.mark_changed();
        }
        ListOutcome {
            response: header,
            changed,
            finished,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Edit {
    Insert { index: usize },
    Remove { index: usize },
    Resize(usize),
    Move { from: usize, to: usize },
}

struct BandOutcome {
    rect: Rect,
    fold_clicked: bool,
    select_clicked: bool,
    drag_started: bool,
}

#[allow(clippy::too_many_arguments)]
fn draw_header(
    ui: &mut Ui,
    id: Id,
    style: &FieldStyle,
    label: &str,
    len: usize,
    max: usize,
    state: &mut ListState,
    pending: &mut Option<Edit>,
    finished: &mut bool,
) -> Response {
    let width = ui.available_width().max(1.0);
    let height = style.height.max(18.0);

    let (band, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let hovered = ui.rect_contains_pointer(band);
    ui.painter().rect_filled(
        band,
        CornerRadius::ZERO,
        if hovered {
            style.hover_fill
        } else {
            style.fill
        },
    );
    let response = ui
        .interact(band, list_header_id(id), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    response.widget_info(|| {
        egui::WidgetInfo::selected(WidgetType::SelectableLabel, true, !state.collapsed, label)
    });
    paint_arrow(
        ui.painter(),
        pos2(band.left() + 8.0, band.center().y),
        !state.collapsed,
        style.label,
    );
    let title = if label.is_empty() { "List" } else { label };
    paint_text(
        ui,
        band,
        style,
        title,
        band.left() + 18.0,
        style.label_color(false),
    );
    if response.clicked() {
        state.collapsed = !state.collapsed;
    }

    let (tools, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    ui.painter()
        .rect_filled(tools, CornerRadius::ZERO, style.fill);

    let button_width = 16.0;
    let button_height = (height - 4.0).max(12.0);
    let buttons = Rect::from_min_size(
        pos2(
            tools.right() - 4.0 - button_width * 2.0,
            tools.center().y - button_height * 0.5,
        ),
        vec2(button_width * 2.0, button_height),
    );
    let count_width = 44.0;
    let count = Rect::from_min_size(
        pos2(
            buttons.left() - 4.0 - count_width,
            tools.center().y - height * 0.5,
        ),
        vec2(count_width, height),
    );

    let mut len_value = len;
    let outcome = NumberField::new(list_count_id(id), &mut len_value, style)
        .label("List length")
        .range(0.0, max as f64)
        .show(ui, count, &mut state.count_edit);
    if outcome.changed() {
        *pending = Some(Edit::Resize(len_value.min(max)));
    }
    if outcome.finished {
        *finished = true;
    }

    let add_rect = Rect::from_min_size(buttons.min, vec2(button_width, button_height));
    let remove_rect = Rect::from_min_size(
        pos2(buttons.left() + button_width, buttons.top()),
        vec2(button_width, button_height),
    );
    let can_add = len < max;
    let can_remove = len > 0;
    let add = ui
        .interact(
            add_rect,
            list_add_id(id),
            if can_add {
                Sense::click()
            } else {
                Sense::hover()
            },
        )
        .on_hover_cursor(CursorIcon::PointingHand);
    let remove = ui.interact(
        remove_rect,
        list_remove_id(id),
        if can_remove {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    labelled_widget_info(&add, WidgetType::Button, "Add list entry", None);
    labelled_widget_info(&remove, WidgetType::Button, "Remove list entry", None);
    paint_button(ui, add_rect, style, add.hovered(), can_add);
    paint_button(ui, remove_rect, style, remove.hovered(), can_remove);
    paint_plus(ui, add_rect.center(), style, can_add);
    paint_minus(ui, remove_rect.center(), style, can_remove);

    if add.clicked() && can_add {
        *pending = Some(Edit::Insert { index: len });
    }
    if remove.clicked() && can_remove {
        *pending = Some(Edit::Remove {
            index: state.selected.unwrap_or(len - 1).min(len - 1),
        });
    }

    response
}

fn draw_item_band(
    ui: &mut Ui,
    id: Id,
    index: usize,
    style: &FieldStyle,
    selected: bool,
    open: bool,
) -> BandOutcome {
    let width = ui.available_width().max(1.0);
    let height = style.height.max(18.0);
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    ui.painter().rect_filled(
        rect,
        CornerRadius::ZERO,
        if selected {
            style.hover_fill
        } else {
            style.fill
        },
    );

    let handle_rect = Rect::from_min_size(rect.min, vec2(16.0, rect.height()));
    let caret_rect = Rect::from_min_size(
        pos2(rect.left() + 16.0, rect.top()),
        vec2(16.0, rect.height()),
    );
    let label_rect = Rect::from_min_max(pos2(rect.left() + 32.0, rect.top()), rect.right_bottom());

    let handle = ui
        .interact(handle_rect, list_handle_id(id, index), Sense::drag())
        .on_hover_cursor(CursorIcon::Grab);
    let fold = ui
        .interact(caret_rect, list_fold_id(id, index), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    let select = ui.interact(label_rect, list_item_id(id, index), Sense::click());
    labelled_widget_info(
        &select,
        WidgetType::SelectableLabel,
        &format!("Element {index}"),
        Some(selected),
    );

    let ink = if handle.hovered() || selected {
        style.label
    } else {
        style.edge
    };
    for offset in [-2.0, 2.0] {
        ui.painter().rect_filled(
            Rect::from_min_size(
                pos2(
                    handle_rect.center().x - 5.0,
                    handle_rect.center().y + offset - 0.5,
                ),
                vec2(10.0, 1.0),
            ),
            CornerRadius::ZERO,
            ink,
        );
    }
    paint_arrow(
        ui.painter(),
        pos2(caret_rect.center().x, caret_rect.center().y),
        open,
        style.label,
    );
    paint_text(
        ui,
        label_rect,
        style,
        &format!("Element {index}"),
        label_rect.left() + 4.0,
        style.label_color(selected),
    );

    BandOutcome {
        rect,
        fold_clicked: fold.clicked(),
        select_clicked: select.clicked(),
        drag_started: handle.drag_started(),
    }
}

fn paint_text(ui: &Ui, rect: Rect, style: &FieldStyle, text: &str, left: f32, color: Color32) {
    let galley = style.text.layout(ui, text);
    ui.painter().with_clip_rect(rect).galley(
        pos2(left, rect.center().y - galley.size().y * 0.5),
        galley,
        color,
    );
}

fn paint_arrow(painter: &egui::Painter, center: egui::Pos2, open: bool, color: Color32) {
    let points = if open {
        vec![
            center + vec2(-4.0, -2.5),
            center + vec2(4.0, -2.5),
            center + vec2(0.0, 2.5),
        ]
    } else {
        vec![
            center + vec2(-2.5, -4.0),
            center + vec2(-2.5, 4.0),
            center + vec2(2.5, 0.0),
        ]
    };
    painter.add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
}

fn paint_button(ui: &Ui, rect: Rect, style: &FieldStyle, hovered: bool, enabled: bool) {
    let fill = if hovered && enabled {
        style.hover_fill
    } else {
        style.fill
    };
    ui.painter().rect_filled(rect, CornerRadius::ZERO, fill);
    if style.border_width > 0.0 {
        ui.painter().rect_stroke(
            rect,
            CornerRadius::ZERO,
            Stroke::new(style.border_width, style.edge),
            StrokeKind::Inside,
        );
    }
}

fn paint_plus(ui: &Ui, center: egui::Pos2, style: &FieldStyle, enabled: bool) {
    let color = if enabled { style.label } else { style.edge };
    let stroke = Stroke::new(1.0, color);
    let half = 4.0;
    ui.painter().line_segment(
        [
            pos2(center.x - half, center.y),
            pos2(center.x + half, center.y),
        ],
        stroke,
    );
    ui.painter().line_segment(
        [
            pos2(center.x, center.y - half),
            pos2(center.x, center.y + half),
        ],
        stroke,
    );
}

fn paint_minus(ui: &Ui, center: egui::Pos2, style: &FieldStyle, enabled: bool) {
    let color = if enabled { style.label } else { style.edge };
    let half = 4.0;
    ui.painter().line_segment(
        [
            pos2(center.x - half, center.y),
            pos2(center.x + half, center.y),
        ],
        Stroke::new(1.0, color),
    );
}

fn apply_edit<T>(
    items: &mut Vec<T>,
    make: &mut dyn FnMut() -> T,
    state: &mut ListState,
    edit: Edit,
    max: usize,
) -> bool {
    match edit {
        Edit::Insert { index } => {
            if items.len() >= max {
                return false;
            }
            let at = index.min(items.len());
            items.insert(at, make());
            state.closed = state
                .closed
                .iter()
                .map(|&closed| if closed >= at { closed + 1 } else { closed })
                .collect();
            state.selected = Some(at);
            true
        }
        Edit::Remove { index } => {
            if index >= items.len() {
                return false;
            }
            items.remove(index);
            state.closed = state
                .closed
                .iter()
                .filter_map(|&closed| match closed.cmp(&index) {
                    Ordering::Less => Some(closed),
                    Ordering::Equal => None,
                    Ordering::Greater => Some(closed - 1),
                })
                .collect();
            state.selected = match state.selected {
                Some(selected) if selected == index => None,
                Some(selected) if selected > index => Some(selected - 1),
                selected => selected,
            };
            true
        }
        Edit::Resize(len) => {
            let target = len.min(max);
            match target.cmp(&items.len()) {
                Ordering::Greater => items.resize_with(target, make),
                Ordering::Less => {
                    items.truncate(target);
                    state.closed.retain(|&closed| closed < target);
                    state.selected = state.selected.filter(|&selected| selected < target);
                }
                Ordering::Equal => return false,
            }
            true
        }
        Edit::Move { from, to } => {
            let len = items.len();
            if from >= len || to >= len {
                return false;
            }
            if from == to {
                return false;
            }
            let item = items.remove(from);
            items.insert(to, item);
            state.selected = state
                .selected
                .map(|selected| move_index(selected, from, to));
            state.closed = state
                .closed
                .iter()
                .map(|&closed| move_index(closed, from, to))
                .collect();
            true
        }
    }
}

/// Where an index ends up after moving `from` so it sits at `to` in the final
/// vector. The moves are in final-list coordinates.
fn move_index(index: usize, from: usize, to: usize) -> usize {
    if index == from {
        to
    } else if from < to && index > from && index <= to {
        index - 1
    } else if to < from && index >= to && index < from {
        index + 1
    } else {
        index
    }
}
/// The visual tokens of [ListChrome]. The style page owns these values; a
/// style-token list has no concrete `Vec<T>`, so it draws the same chrome from
/// this snapshot instead of a [ListField].
pub struct ListChromeStyle {
    /// Tokens for the size box and its buttons.
    pub field: FieldStyle,
    /// Header band text.
    pub header_text: TextStyle,
    /// Element band text at rest.
    pub band_text: TextStyle,
    /// Element band text while selected.
    pub band_text_selected: TextStyle,
    /// Header band at rest and under the pointer.
    pub header_fill: Color32,
    pub header_hover: Color32,
    /// Selected element band.
    pub selection: Color32,
    /// Divider between an element's gutter and its fields.
    pub divider: Color32,
    /// Drag-handle ink at rest.
    pub handle: Color32,
    /// Insertion line while dragging.
    pub accent: Color32,
    pub row_height: f32,
    pub group_height: f32,
    pub group_indent: f32,
    pub row_inset: f32,
    pub gutter_width: f32,
    pub button_cell: f32,
    pub button_height: f32,
    pub count_width: f32,
    pub button_fill: Color32,
    pub button_hover: Color32,
    pub button_border: Color32,
    pub button_border_width: f32,
    pub button_text: Color32,
    pub button_corner_radius: u8,
}

/// Ids a style-token list header draws. They belong to the caller because the
/// page uses stable dotted paths, not a vector index, to address its fields.
#[derive(Clone, Copy, Debug)]
pub struct ListChromeIds {
    pub header: Id,
    pub count: Id,
    pub add: Id,
    pub remove: Id,
}

/// Ids one element band draws.
#[derive(Clone, Copy, Debug)]
pub struct ListBandIds {
    pub fold: Id,
    pub select: Id,
    pub handle: Id,
}

/// A structural edit requested by [ListChrome]. It is the style-token view's
/// equivalent of [ListField]'s direct vector mutation: the page sends it back
/// to the owner, which has the concrete `Vec<T>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListChromeEdit {
    SetLen { list: Id, len: usize },
    Insert { list: Id, index: usize },
    Remove { list: Id, index: usize },
    Move { list: Id, from: usize, to: usize },
}

/// What a style-token list header did this pass.
#[derive(Debug)]
pub struct ListHeaderOutcome {
    pub response: Response,
    /// A header control was clicked.
    pub clicked: bool,
    /// A fold click asked for this new value.
    pub folded: Option<bool>,
    pub edit: Option<ListChromeEdit>,
    /// The count box's typed edit ended; a caller that batches writes should
    /// flush now.
    pub finished: bool,
}

/// What one style-token element band did this pass.
#[derive(Debug)]
pub struct ListBandOutcome {
    pub response: Response,
    pub selected: bool,
    pub folded: Option<bool>,
}

/// Cross-frame memory for style-token list chrome.
///
/// The page owns it like [ListState]; the fields are private so the page can
/// clear or query it but cannot write an out-of-range drag.
#[derive(Default)]
pub struct ListChromeState {
    count_edit: Option<NumberEdit>,
    count_list: Option<Id>,
    drag: Option<ChromeDrag>,
    bands: Vec<ChromeBand>,
}

#[derive(Clone, Copy, Debug)]
struct ChromeDrag {
    list: Id,
    from: usize,
}

#[derive(Clone, Copy, Debug)]
struct ChromeBand {
    list: Id,
    index: usize,
    rect: Rect,
}

impl ListChromeState {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Drop a reorder that no longer has a valid list, keeping the count
    /// editor. A structural refresh uses this: the rows changed, but the
    /// in-flight count text still belongs to the same list path.
    pub fn cancel_drag(&mut self) {
        self.drag = None;
    }
}

/// The style-token view of a list: a header, count and button pair, plus the
/// element bands and reorder gesture. It knows a length and stable ids, not a
/// concrete vector; every structural gesture comes back as [ListChromeEdit].
pub struct ListChrome;

impl ListChrome {
    /// Forget the bands from the previous pass. Call once before drawing any
    /// list on a page.
    pub fn begin(state: &mut ListChromeState) {
        state.bands.clear();
    }

    /// Draw the two-row header and its add/remove/count controls.
    #[allow(clippy::too_many_arguments)]
    pub fn header(
        ui: &mut Ui,
        state: &mut ListChromeState,
        style: &ListChromeStyle,
        list: Id,
        ids: ListChromeIds,
        label: &str,
        len: usize,
        selected: Option<usize>,
        folded: bool,
        depth: usize,
        row_width: f32,
        max: usize,
    ) -> ListHeaderOutcome {
        let (band, _) = ui.allocate_exact_size(vec2(row_width, style.group_height), Sense::hover());
        let hovered = ui.rect_contains_pointer(band);
        ui.painter().rect_filled(
            band,
            CornerRadius::ZERO,
            if hovered {
                style.header_hover
            } else {
                style.header_fill
            },
        );
        let response = ui
            .interact(band, ids.header, Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        labelled_widget_info(&response, WidgetType::SelectableLabel, label, Some(!folded));
        let arrow = pos2(
            band.left() + style.row_inset + depth as f32 * style.group_indent + 4.0,
            band.center().y,
        );
        paint_arrow(ui.painter(), arrow, !folded, style.header_text.text_color());
        let text_left = band.left() + style.row_inset + 16.0 + depth as f32 * style.group_indent;
        paint_text_with(
            ui,
            band,
            &style.header_text,
            label,
            text_left,
            style.header_text.text_color(),
        );

        let (tools, _) = ui.allocate_exact_size(vec2(row_width, style.row_height), Sense::hover());
        ui.painter()
            .rect_filled(tools, CornerRadius::ZERO, style.header_fill);
        let buttons = Rect::from_min_size(
            pos2(
                tools.right() - style.row_inset - style.button_cell * 2.0,
                tools.center().y - style.button_height * 0.5,
            ),
            vec2(style.button_cell * 2.0, style.button_height),
        );
        let count = Rect::from_min_size(
            pos2(
                buttons.left() - 4.0 - style.count_width,
                tools.center().y - style.field.height * 0.5,
            ),
            vec2(style.count_width, style.field.height),
        );

        let mut len_value = len;
        let mut edit = if state.count_list == Some(list) {
            state.count_edit.take()
        } else {
            None
        };
        let count_outcome = NumberField::new(ids.count, &mut len_value, &style.field)
            .label(label)
            .range(0.0, max as f64)
            .show(ui, count, &mut edit);
        let mut edit_out = None;
        if count_outcome.changed() {
            edit_out = Some(ListChromeEdit::SetLen {
                list,
                len: len_value.min(max),
            });
        }
        if let Some(edit) = edit {
            state.count_edit = Some(edit);
            state.count_list = Some(list);
        } else if state.count_list == Some(list) {
            state.count_list = None;
        }

        let add_rect =
            Rect::from_min_size(buttons.min, vec2(style.button_cell, style.button_height));
        let remove_rect = Rect::from_min_size(
            pos2(buttons.left() + style.button_cell, buttons.top()),
            vec2(style.button_cell, style.button_height),
        );
        let can_add = len < max;
        let can_remove = len > 0;
        let add = ui
            .interact(
                add_rect,
                ids.add,
                if can_add {
                    Sense::click()
                } else {
                    Sense::hover()
                },
            )
            .on_hover_cursor(CursorIcon::PointingHand);
        let remove = ui.interact(
            remove_rect,
            ids.remove,
            if can_remove {
                Sense::click()
            } else {
                Sense::hover()
            },
        );
        labelled_widget_info(&add, WidgetType::Button, "Add list entry", None);
        labelled_widget_info(&remove, WidgetType::Button, "Remove list entry", None);
        paint_chrome_button(ui, add_rect, style, add.hovered(), can_add);
        paint_chrome_button(ui, remove_rect, style, remove.hovered(), can_remove);
        paint_plus_with(ui, add_rect.center(), style.button_text, can_add);
        paint_minus_with(ui, remove_rect.center(), style.button_text, can_remove);

        if add.clicked() && can_add {
            edit_out = Some(ListChromeEdit::Insert { list, index: len });
        }
        if remove.clicked() && can_remove {
            edit_out = Some(ListChromeEdit::Remove {
                list,
                index: selected.unwrap_or(len - 1).min(len - 1),
            });
        }

        let clicked =
            response.clicked() || count_outcome.clicked() || add.clicked() || remove.clicked();
        let folded = response.clicked().then_some(!folded);
        ListHeaderOutcome {
            response,
            clicked,
            folded,
            edit: edit_out,
            finished: count_outcome.finished,
        }
    }

    /// Draw one element band: drag handle, fold caret and selection label.
    #[allow(clippy::too_many_arguments)]
    pub fn band(
        ui: &mut Ui,
        state: &mut ListChromeState,
        style: &ListChromeStyle,
        list: Id,
        ids: ListBandIds,
        index: usize,
        depth: usize,
        row_width: f32,
        row_fill: Color32,
        selected: bool,
        folded: bool,
    ) -> ListBandOutcome {
        let (rect, _) = ui.allocate_exact_size(vec2(row_width, style.row_height), Sense::hover());
        ui.painter().rect_filled(
            rect,
            CornerRadius::ZERO,
            if selected { style.selection } else { row_fill },
        );

        let x0 = rect.left() + style.row_inset + depth as f32 * style.group_indent;
        let divider_x = x0 + style.gutter_width;
        let content_left = divider_x + 4.0;
        let label_x = content_left + 16.0;
        ui.painter().rect_filled(
            Rect::from_min_size(pos2(divider_x, rect.top()), vec2(1.0, rect.height())),
            CornerRadius::ZERO,
            style.divider,
        );

        let handle = Rect::from_center_size(
            pos2(x0 + style.gutter_width * 0.5, rect.center().y),
            vec2(style.gutter_width, rect.height()),
        );
        let drag = ui
            .interact(handle, ids.handle, Sense::drag())
            .on_hover_cursor(CursorIcon::Grab);
        let ink = if drag.hovered() || selected {
            style.band_text_selected.text_color()
        } else {
            style.handle
        };
        for offset in [-2.0, 2.0] {
            ui.painter().rect_filled(
                Rect::from_min_size(
                    pos2(handle.center().x - 5.0, handle.center().y + offset - 0.5),
                    vec2(10.0, 1.0),
                ),
                CornerRadius::ZERO,
                ink,
            );
        }
        if drag.drag_started() {
            state.drag = Some(ChromeDrag { list, from: index });
        }
        state.bands.push(ChromeBand { list, index, rect });

        let caret_rect =
            Rect::from_min_size(pos2(content_left, rect.top()), vec2(16.0, rect.height()));
        let caret = ui
            .interact(caret_rect, ids.fold, Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        paint_arrow(
            ui.painter(),
            pos2(content_left + 4.0, rect.center().y),
            !folded,
            style.band_text.text_color(),
        );

        let label_rect =
            Rect::from_min_max(pos2(label_x, rect.top()), pos2(rect.right(), rect.bottom()));
        let select = ui.interact(label_rect, ids.select, Sense::click());
        labelled_widget_info(
            &select,
            WidgetType::SelectableLabel,
            &format!("Element {index}"),
            Some(selected),
        );
        let text_style = if selected {
            &style.band_text_selected
        } else {
            &style.band_text
        };
        paint_text_with(
            ui,
            label_rect,
            text_style,
            &format!("Element {index}"),
            label_x,
            text_style.text_color(),
        );

        let selected = select.clicked();
        let folded = caret.clicked().then_some(!folded);
        ListBandOutcome {
            response: select,
            selected,
            folded,
        }
    }

    /// Resolve the active reorder gesture after every band is laid out. It
    /// paints the insertion line and returns the move to send to the owner.
    pub fn finish(
        ui: &mut Ui,
        state: &mut ListChromeState,
        style: &ListChromeStyle,
    ) -> Option<ListChromeEdit> {
        let drag = state.drag?;
        let mut bands: Vec<&ChromeBand> = state
            .bands
            .iter()
            .filter(|band| band.list == drag.list)
            .collect();
        bands.sort_by_key(|band| band.index);
        let Some(first) = bands.first().copied() else {
            state.drag = None;
            return None;
        };
        let released = ui.input(|input| input.pointer.any_released());
        if !ui.input(|input| input.pointer.primary_down()) && !released {
            state.drag = None;
            return None;
        }
        let Some(pointer) = ui.ctx().pointer_interact_pos() else {
            if !ui.input(|input| input.pointer.primary_down()) {
                state.drag = None;
            }
            return None;
        };
        let slot = bands
            .iter()
            .take_while(|band| pointer.y > band.rect.center().y)
            .count();
        let y = if slot == 0 {
            first.rect.top()
        } else {
            bands[slot - 1].rect.bottom()
        };
        let left = first.rect.left() + style.row_inset;
        let right = first.rect.right() - style.row_inset;
        ui.painter().rect_filled(
            Rect::from_min_size(pos2(left, y - 1.0), vec2(right - left, 2.0)),
            CornerRadius::ZERO,
            style.accent,
        );

        if released {
            let to = if slot > drag.from { slot - 1 } else { slot };
            state.drag = None;
            if to != drag.from && to < bands.len() {
                return Some(ListChromeEdit::Move {
                    list: drag.list,
                    from: drag.from,
                    to,
                });
            }
        }
        None
    }
}

fn paint_text_with(ui: &Ui, rect: Rect, style: &TextStyle, text: &str, left: f32, color: Color32) {
    let galley = style.layout(ui, text);
    ui.painter().with_clip_rect(rect).galley(
        pos2(left, rect.center().y - galley.size().y * 0.5),
        galley,
        color,
    );
}

fn paint_chrome_button(ui: &Ui, rect: Rect, style: &ListChromeStyle, hovered: bool, enabled: bool) {
    let fill = if hovered && enabled {
        style.button_hover
    } else {
        style.button_fill
    };
    ui.painter()
        .rect_filled(rect, CornerRadius::same(style.button_corner_radius), fill);
    if style.button_border_width > 0.0 {
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(style.button_corner_radius),
            Stroke::new(style.button_border_width, style.button_border),
            StrokeKind::Inside,
        );
    }
}

fn paint_plus_with(ui: &Ui, center: egui::Pos2, color: Color32, enabled: bool) {
    let color = if enabled {
        color
    } else {
        color.gamma_multiply(0.45)
    };
    let stroke = Stroke::new(1.0, color);
    let half = 4.0;
    ui.painter().line_segment(
        [
            pos2(center.x - half, center.y),
            pos2(center.x + half, center.y),
        ],
        stroke,
    );
    ui.painter().line_segment(
        [
            pos2(center.x, center.y - half),
            pos2(center.x, center.y + half),
        ],
        stroke,
    );
}

fn paint_minus_with(ui: &Ui, center: egui::Pos2, color: Color32, enabled: bool) {
    let color = if enabled {
        color
    } else {
        color.gamma_multiply(0.45)
    };
    let half = 4.0;
    ui.painter().line_segment(
        [
            pos2(center.x - half, center.y),
            pos2(center.x + half, center.y),
        ],
        Stroke::new(1.0, color),
    );
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};

    use egui::{Context, Event, Pos2, Rect, vec2};

    use super::*;
    use crate::field::testing::{button, click, enter, input, style};

    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    struct Item {
        value: u32,
    }

    fn values(items: &[Item]) -> Vec<u32> {
        items.iter().map(|item| item.value).collect()
    }

    #[test]
    fn add_appends_a_factory_entry_and_selects_it() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-list-add");
        let mut items = vec![Item { value: 0 }, Item { value: 1 }];
        let mut state = ListState::default();

        let mut draw = |ui: &mut Ui| {
            ListField::new(id, &mut items, &style, || Item { value: 9 })
                .label("Items")
                .show(ui, &mut state, |_, _, _| ListItemOutcome::default());
        };
        let _ = ctx.run_ui(input(), &mut draw);
        click(
            &ctx,
            ctx.read_response(list_add_id(id)).unwrap().rect.center(),
            &mut draw,
        );

        let _ = draw;
        assert_eq!(values(&items), vec![0, 1, 9]);
        assert_eq!(state.selected(), Some(2), "the new entry becomes selected");
    }

    #[test]
    fn remove_takes_the_selected_entry_not_always_the_last() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-list-remove");
        let mut items = vec![Item { value: 0 }, Item { value: 1 }, Item { value: 2 }];
        let mut state = ListState::default();

        let mut draw = |ui: &mut Ui| {
            ListField::new(id, &mut items, &style, Item::default).show(
                ui,
                &mut state,
                |_, _, _| ListItemOutcome::default(),
            );
        };
        let _ = ctx.run_ui(input(), &mut draw);
        click(
            &ctx,
            ctx.read_response(list_item_id(id, 1))
                .unwrap()
                .rect
                .center(),
            &mut draw,
        );
        click(
            &ctx,
            ctx.read_response(list_remove_id(id)).unwrap().rect.center(),
            &mut draw,
        );
        let _ = draw;
        assert_eq!(values(&items), vec![0, 2]);
        assert_eq!(state.selected(), None, "the removed entry clears selection");
    }

    #[test]
    fn the_count_box_resizes_the_vector_through_the_factory() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-list-count");
        let mut items = vec![Item { value: 0 }, Item { value: 1 }];
        let mut state = ListState::default();

        let mut draw = |ui: &mut Ui| {
            ListField::new(id, &mut items, &style, || Item { value: 7 }).show(
                ui,
                &mut state,
                |_, _, _| ListItemOutcome::default(),
            );
        };
        let _ = ctx.run_ui(input(), &mut draw);
        click(
            &ctx,
            ctx.read_response(list_count_id(id)).unwrap().rect.center(),
            &mut draw,
        );
        let _ = ctx.run_ui(input(), &mut draw);

        let mut typed = input();
        typed.events = vec![Event::Text("4".to_owned()), enter()];
        let _ = ctx.run_ui(typed, &mut draw);

        let _ = draw;
        assert_eq!(values(&items), vec![0, 1, 7, 7]);
    }

    #[test]
    fn a_maximum_stops_the_add_button() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-list-max");
        let mut items = vec![Item { value: 0 }];
        let mut state = ListState::default();

        let mut draw = |ui: &mut Ui| {
            ListField::new(id, &mut items, &style, || Item { value: 9 })
                .max(1)
                .show(ui, &mut state, |_, _, _| ListItemOutcome::default());
        };
        let _ = ctx.run_ui(input(), &mut draw);
        click(
            &ctx,
            ctx.read_response(list_add_id(id)).unwrap().rect.center(),
            &mut draw,
        );

        let _ = draw;
        assert_eq!(values(&items), vec![0]);
    }

    #[test]
    fn dragging_a_handle_moves_the_entry_and_selection_with_it() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-list-move");
        let mut items = vec![Item { value: 0 }, Item { value: 1 }, Item { value: 2 }];
        let mut state = ListState::default();

        let mut draw = |ui: &mut Ui| {
            ListField::new(id, &mut items, &style, Item::default).show(
                ui,
                &mut state,
                |_, _, _| ListItemOutcome::default(),
            );
        };
        let _ = ctx.run_ui(input(), &mut draw);
        click(
            &ctx,
            ctx.read_response(list_item_id(id, 0))
                .unwrap()
                .rect
                .center(),
            &mut draw,
        );

        let handle = ctx
            .read_response(list_handle_id(id, 0))
            .unwrap()
            .rect
            .center();
        let target = pos2(
            handle.x,
            ctx.read_response(list_item_id(id, 2))
                .unwrap()
                .rect
                .bottom()
                - 2.0,
        );
        drag(&ctx, handle, target, &mut draw);
        let _ = draw;

        assert_eq!(values(&items), vec![1, 2, 0]);
        assert_eq!(
            state.selected(),
            Some(2),
            "selection follows the moved entry"
        );
    }

    fn drag(ctx: &Context, from: Pos2, to: Pos2, draw: &mut impl FnMut(&mut Ui)) {
        let mut hover = input();
        hover.events = vec![Event::PointerMoved(from)];
        let _ = ctx.run_ui(hover, &mut *draw);

        let mut press = input();
        press.events = vec![button(from, true)];
        let _ = ctx.run_ui(press, &mut *draw);

        let mut moved = input();
        moved.events = vec![Event::PointerMoved(to)];
        let _ = ctx.run_ui(moved, &mut *draw);

        let mut release = input();
        release.events = vec![button(to, false)];
        let _ = ctx.run_ui(release, &mut *draw);
    }

    #[test]
    fn folding_the_list_keeps_the_header_and_drops_the_bands() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-list-fold");
        let mut items = vec![Item { value: 0 }, Item { value: 1 }];
        let mut state = ListState::default();

        let mut draw = |ui: &mut Ui| {
            ListField::new(id, &mut items, &style, Item::default)
                .label("Items")
                .show(ui, &mut state, |_, _, _| ListItemOutcome::default());
        };
        let _ = ctx.run_ui(input(), &mut draw);
        click(
            &ctx,
            ctx.read_response(list_header_id(id)).unwrap().rect.center(),
            &mut draw,
        );

        let _ = ctx.run_ui(input(), &mut draw);
        assert!(
            ctx.read_response(list_item_id(id, 0)).is_none(),
            "no element band is laid out while folded"
        );
        assert!(
            ctx.read_response(list_count_id(id)).is_some(),
            "the header and its count stay"
        );
        let _ = draw;
        assert!(state.is_collapsed());
    }

    #[test]
    fn folding_one_element_skips_only_its_row() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-list-element-fold");
        let mut items = vec![Item { value: 0 }, Item { value: 1 }];
        let mut state = ListState::default();
        let seen = RefCell::new(Vec::new());

        let mut draw = |ui: &mut Ui| {
            seen.borrow_mut().clear();
            ListField::new(id, &mut items, &style, Item::default).show(
                ui,
                &mut state,
                |_, _, item| {
                    seen.borrow_mut().push(item.index);
                    ListItemOutcome::default()
                },
            );
        };
        let _ = ctx.run_ui(input(), &mut draw);
        assert_eq!(*seen.borrow(), vec![0, 1]);

        click(
            &ctx,
            ctx.read_response(list_fold_id(id, 0))
                .unwrap()
                .rect
                .center(),
            &mut draw,
        );
        let _ = ctx.run_ui(input(), &mut draw);
        let _ = draw;
        assert!(!state.is_open(0));
        assert_eq!(
            *seen.borrow(),
            vec![1],
            "the folded element's fields are not drawn"
        );
    }

    #[test]
    fn a_click_inside_an_element_selects_that_element() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-list-body-click");
        let mut items = vec![Item { value: 0 }, Item { value: 1 }];
        let mut state = ListState::default();
        let body = Cell::new(Rect::NOTHING);

        let mut draw = |ui: &mut Ui| {
            ListField::new(id, &mut items, &style, Item::default).show(
                ui,
                &mut state,
                |_, ui, item| {
                    let (rect, _) = ui.allocate_exact_size(vec2(80.0, 18.0), Sense::hover());
                    if item.index == 0 {
                        body.set(rect);
                    }
                    let response = ui.interact(rect, item.id.with("value"), Sense::click());
                    ListItemOutcome {
                        clicked: response.clicked(),
                        ..Default::default()
                    }
                },
            );
        };
        let _ = ctx.run_ui(input(), &mut draw);
        click(&ctx, body.get().center(), &mut draw);
        let _ = draw;

        assert_eq!(state.selected(), Some(0));
    }

    #[test]
    fn state_normalizes_after_the_vector_shrinks() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-list-normalize");
        let mut items = vec![Item { value: 0 }, Item { value: 1 }];
        let mut state = ListState {
            selected: Some(1),
            closed: BTreeSet::from([1]),
            ..ListState::default()
        };

        {
            let mut draw = |ui: &mut Ui| {
                ListField::new(id, &mut items, &style, Item::default).show(
                    ui,
                    &mut state,
                    |_, _, _| ListItemOutcome::default(),
                );
            };
            let _ = ctx.run_ui(input(), &mut draw);
        }
        items.pop();
        {
            let mut draw = |ui: &mut Ui| {
                ListField::new(id, &mut items, &style, Item::default).show(
                    ui,
                    &mut state,
                    |_, _, _| ListItemOutcome::default(),
                );
            };
            let _ = ctx.run_ui(input(), &mut draw);
        }

        assert_eq!(state.selected(), None);
        assert!(!state.closed.contains(&1));
    }

    #[test]
    fn clear_drops_every_interaction() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-list-clear");
        let mut items = vec![Item { value: 0 }];
        let mut state = ListState::default();

        let mut draw = |ui: &mut Ui| {
            ListField::new(id, &mut items, &style, Item::default).show(
                ui,
                &mut state,
                |_, _, _| ListItemOutcome::default(),
            );
        };
        let _ = ctx.run_ui(input(), &mut draw);
        let _ = draw;
        state.selected = Some(0);
        state.collapsed = true;
        state.closed.insert(0);
        state.clear();

        assert_eq!(state.selected(), None);
        assert!(!state.is_collapsed());
        assert!(state.is_open(0));
    }
}
