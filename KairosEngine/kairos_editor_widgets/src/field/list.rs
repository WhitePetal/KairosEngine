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
    Color32, CornerRadius, CursorIcon, Id, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui,
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
    shifts: Vec<BandShift>,
    /// Full block heights from the previous pass. A dragged block is hidden
    /// without calling its row closure, so its body is reserved from here.
    heights: Vec<f32>,
}

#[derive(Clone, Copy, Debug)]
struct Drag {
    from: usize,
    /// Pointer offset from the band's top when the drag started: the ghost
    /// keeps this grab point under the pointer.
    grab: f32,
    /// The preview insertion slot while the pointer is held. The vector is
    /// not reordered until the release.
    to: usize,
}

/// One element block sliding to a preview slot, or settling into its new slot
/// after a drop. The typed list applies the offset to the band and the caller's
/// row closure together.
#[derive(Clone, Copy)]
struct BandShift {
    index: usize,
    from: f32,
    to: f32,
    start: f64,
    duration: f64,
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
        self.shifts.retain(|shift| shift.index < len);
        self.heights.truncate(len);
        if self.heights.len() < len {
            self.heights.resize(len, 0.0);
        }
        if self.drag.is_some_and(|drag| drag.from >= len) {
            self.drag = None;
        }
    }

    fn shift(&self, index: usize, now: f64) -> f32 {
        self.shifts
            .iter()
            .find(|shift| shift.index == index)
            .map_or(0.0, |shift| {
                let t = ((now - shift.start) / shift.duration).clamp(0.0, 1.0);
                let eased = (t * t * (3.0 - 2.0 * t)) as f32;
                shift.from + (shift.to - shift.from) * eased
            })
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
/// * an optional [Self::item_label] callback that names the element bands;
/// * the maximum length, if the list has a UI cap.
///
/// The widget draws the header and the element bands and calls the row closure
/// only for open elements. An element row returns [ListItemOutcome] so clicks,
/// live edits and finished edits feed back into the list's selection and
/// [ListOutcome]. While an element is dragged, its band and body keep their
/// layout height but the closure is not called: the ghost carries the block and
/// the other elements slide to their preview slots.
type ItemLabel<'a, T> = Box<dyn Fn(&T, usize) -> String + 'a>;

pub struct ListField<'a, T> {
    id: Id,
    items: &'a mut Vec<T>,
    style: &'a FieldStyle,
    chrome: Option<ListChromeStyle>,
    label: &'a str,
    max: usize,
    make: Box<dyn FnMut() -> T + 'a>,
    item_label: Option<ItemLabel<'a, T>>,
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
            chrome: None,
            label: "",
            max: usize::MAX,
            make: Box::new(make),
            item_label: None,
        }
    }

    /// Draw the chrome from a shared list skin instead of deriving it from
    /// [FieldStyle]. Callers with a shell-owned `ListStyle` project it with its
    /// `tokens(&FieldStyle)` method and pass the result here.
    pub fn chrome(mut self, chrome: ListChromeStyle) -> Self {
        self.chrome = Some(chrome);
        self
    }

    /// Accessibility name of the list, drawn on its header band.
    pub fn label(mut self, label: &'a str) -> Self {
        self.label = label;
        self
    }

    /// Name each element's header band. The default is `Element {index}`.
    ///
    /// The callback sees the element and its current index, so a caller can
    /// show a name from the element itself and keep the index for reordering.
    pub fn item_label(mut self, label: impl Fn(&T, usize) -> String + 'a) -> Self {
        self.item_label = Some(Box::new(label));
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
            chrome,
            label,
            max,
            mut make,
            item_label,
        } = self;
        let chrome = chrome.unwrap_or_else(|| ListChromeStyle::from_field(style));
        let row_width = ui.available_width().max(1.0);
        let left = ui.cursor().left();
        let now = ui.input(|input| input.time);
        state.normalize(items.len());

        let mut pending = None;
        let mut finished = false;
        let mut header = draw_header(
            ui,
            id,
            style,
            &chrome,
            label,
            items.len(),
            max,
            state,
            &mut pending,
            &mut finished,
        );

        let mut changed = false;
        // (index, band rect, slide offset, block bottom in the normal layout)
        let mut bands: Vec<(usize, Rect, f32, f32)> = Vec::new();
        let mut heights: Vec<f32> = Vec::with_capacity(items.len());
        let drag_from = state.drag.map(|drag| drag.from);
        let container_top = ui.cursor().top();

        if !state.collapsed {
            for (index, item) in items.iter_mut().enumerate() {
                let shift = state.shift(index, now);
                if shift != 0.0 {
                    ui.add_space(shift);
                }
                let selected = state.selected == Some(index);
                let item_name = item_label
                    .as_ref()
                    .map_or_else(|| format!("Element {index}"), |label| label(item, index));
                let hidden = drag_from == Some(index);
                let band = draw_item_band(
                    ui,
                    id,
                    index,
                    &chrome,
                    &item_name,
                    selected,
                    state.is_open(index),
                    state.drag.is_some(),
                    hidden,
                );

                if band.pressed {
                    state.selected = Some(index);
                }
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
                    state.selected = Some(index);
                    // A new drag starts from a still list: do not target slots
                    // that are still sliding from the previous drop.
                    state.shifts.clear();
                    state.drag = Some(Drag {
                        from: index,
                        grab: band.grab,
                        to: index,
                    });
                }

                if hidden {
                    // The dragged block is laid out but not drawn; reserve
                    // the body height measured on the previous pass so the
                    // neighbours' preview slots keep their real sizes.
                    if state.is_open(index) {
                        let full = state
                            .heights
                            .get(index)
                            .copied()
                            .unwrap_or(chrome.row_height)
                            .max(chrome.row_height);
                        // The band allocation already paid egui's item spacing;
                        // reserve the rest of the real block height.
                        let spacing = ui.spacing().item_spacing.y;
                        let body = (full - chrome.row_height - spacing).max(0.0);
                        if body > 0.0 {
                            ui.add_space(body);
                        }
                    }
                } else if state.is_open(index) {
                    let item_id = list_item_id(id, index);
                    let selected = state.selected == Some(index);
                    // The body sits on the element's raised band, so the
                    // background is reserved before the caller's rows paint
                    // over it: a placeholder shape is filled once the body
                    // height is known.
                    let body_top = ui.cursor().top();
                    let body_bg = ui.painter().add(egui::Shape::Noop);
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
                    let body_bottom = ui.cursor().top();
                    if body_bottom > body_top + 0.5 {
                        let body = Rect::from_min_max(
                            pos2(left, body_top),
                            pos2(left + row_width, body_bottom),
                        );
                        ui.painter().set(
                            body_bg,
                            egui::Shape::rect_filled(body, CornerRadius::ZERO, chrome.band_fill),
                        );
                        ui.painter().rect_filled(
                            Rect::from_min_size(
                                pos2(body.left(), body.bottom() - 1.0),
                                vec2(body.width(), 1.0),
                            ),
                            CornerRadius::ZERO,
                            chrome.container_border,
                        );
                    }
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
                if shift != 0.0 {
                    ui.add_space(-shift);
                }
                // After the offset is removed this is the block's normal
                // layout bottom, which is what the drop target tests against.
                let bottom = ui.cursor().top();
                heights.push((bottom - (band.rect.top() - shift)).max(1.0));
                bands.push((index, band.rect, shift, bottom));
            }
        }
        state.heights = heights;

        if !state.collapsed {
            // The container closes at the last row; the add/remove pair hangs
            // below it, outside the frame.
            let bottom = ui.cursor().top();
            if bottom > container_top {
                paint_container(
                    ui,
                    &chrome,
                    Rect::from_min_max(pos2(left, container_top), pos2(left + row_width, bottom)),
                );
            }
            draw_footer(ui, id, &chrome, items.len(), max, state, &mut pending);
        }

        if let Some(drag) = state.drag {
            let released = ui.input(|input| input.pointer.any_released());
            let down = ui.input(|input| input.pointer.primary_down());
            if !down && !released {
                // The button came up without a release event (the window lost
                // focus): there is no drag left to resolve.
                state.drag = None;
            } else if state.collapsed || bands.is_empty() {
                state.drag = None;
            } else if let Some(pointer) = ui.ctx().pointer_interact_pos() {
                let count = bands.len();
                // Normal-layout tops and full block heights: the slides are
                // applied to these, not to the animated positions.
                let tops: Vec<f32> = bands
                    .iter()
                    .map(|(_, rect, shift, _)| rect.top() - shift)
                    .collect();
                let block_heights: Vec<f32> = bands
                    .iter()
                    .map(|(_, rect, shift, bottom)| (bottom - (rect.top() - shift)).max(1.0))
                    .collect();
                // The slot under the pointer, counting the other blocks whose
                // current band centre is above it.
                let mut to = 0;
                for (index, (_, rect, _, _)) in bands.iter().enumerate() {
                    if index == drag.from {
                        continue;
                    }
                    if pointer.y > rect.center().y {
                        to += 1;
                    }
                }
                let to = to.min(count - 1);
                let min_top = tops[0];
                let max_top =
                    (tops[count - 1] + block_heights[count - 1] - chrome.row_height).max(min_top);
                let ghost_top = (pointer.y - drag.grab).clamp(min_top, max_top);

                if released {
                    // Blocks still sliding continue from where they are; the
                    // residual offsets are keyed by the slot they land in.
                    let preview = preview_tops(count, drag.from, to, &tops, &block_heights);
                    let starts: Vec<f32> = (0..count)
                        .map(|index| {
                            if index == drag.from {
                                preview[index]
                            } else {
                                bands[index].1.top()
                            }
                        })
                        .collect();
                    state.shifts =
                        drop_shifts(count, drag.from, to, &tops, &block_heights, &starts)
                            .into_iter()
                            .map(|(index, offset)| BandShift {
                                index,
                                from: offset,
                                to: 0.0,
                                start: now,
                                duration: 0.12,
                            })
                            .collect();
                    if to != drag.from && pending.is_none() {
                        pending = Some(Edit::Move {
                            from: drag.from,
                            to,
                        });
                    }
                    state.drag = None;
                    finished = true;
                } else {
                    // The gap preview: the held block is laid out but not
                    // drawn, and every other block slides to its preview slot.
                    if to != drag.to {
                        let preview = preview_tops(count, drag.from, to, &tops, &block_heights);
                        let mut shifts = Vec::new();
                        for (index, (_, rect, shift, _)) in bands.iter().enumerate() {
                            if index == drag.from {
                                continue;
                            }
                            // The layout is still at the current top; animate
                            // the offset from where it is now to the preview
                            // displacement.
                            let from = *shift;
                            let to_offset = preview[index] - (rect.top() - shift);
                            if (to_offset - from).abs() > 0.5 {
                                shifts.push(BandShift {
                                    index,
                                    from,
                                    to: to_offset,
                                    start: now,
                                    duration: 0.16,
                                });
                            }
                        }
                        state.shifts = shifts;
                        if let Some(drag) = state.drag.as_mut() {
                            drag.to = to;
                        }
                    }
                    let label = items
                        .get(drag.from)
                        .map(|item| {
                            item_label.as_ref().map_or_else(
                                || format!("Element {}", drag.from),
                                |label| label(item, drag.from),
                            )
                        })
                        .unwrap_or_default();
                    paint_element_ghost(
                        ui,
                        &chrome,
                        id,
                        Rect::from_min_size(
                            pos2(left, ghost_top),
                            vec2(bands[drag.from].1.width(), chrome.row_height),
                        ),
                        &label,
                        0,
                        state.is_open(drag.from),
                        true,
                    );
                    ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
                }
            } else if !down {
                // The pointer left the window without a release.
                state.drag = None;
            }
        }

        if let Some(edit) = pending
            && apply_edit(items, &mut make, state, edit, max)
        {
            changed = true;
            if !matches!(edit, Edit::Move { .. }) {
                finished = true;
            }
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
    pressed: bool,
    drag_started: bool,
    /// Pointer offset from the band top at the press, used by the drag ghost.
    grab: f32,
}

#[allow(clippy::too_many_arguments)]
fn draw_header(
    ui: &mut Ui,
    id: Id,
    style: &FieldStyle,
    chrome: &ListChromeStyle,
    label: &str,
    len: usize,
    max: usize,
    state: &mut ListState,
    pending: &mut Option<Edit>,
    finished: &mut bool,
) -> Response {
    let width = ui.available_width().max(1.0);
    let (band, _) = ui.allocate_exact_size(vec2(width, chrome.group_height), Sense::hover());
    let hovered = ui.rect_contains_pointer(band);
    ui.painter().rect_filled(
        band,
        CornerRadius::ZERO,
        if hovered {
            chrome.header_hover
        } else {
            chrome.header_fill
        },
    );

    let title = if label.is_empty() { "List" } else { label };
    let count = Rect::from_min_size(
        pos2(
            band.right() - chrome.count_width,
            band.center().y - style.height * 0.5,
        ),
        vec2(chrome.count_width, style.height),
    );
    let label_rect =
        Rect::from_min_max(band.min, pos2(count.left().max(band.left()), band.bottom()));
    let response = ui
        .interact(label_rect, list_header_id(id), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    response.widget_info(|| {
        egui::WidgetInfo::selected(WidgetType::SelectableLabel, true, !state.collapsed, title)
    });
    paint_arrow(
        ui.painter(),
        pos2(band.left() + chrome.row_inset + 4.0, band.center().y),
        !state.collapsed,
        chrome.header_text.text_color(),
    );
    paint_text_with(
        ui,
        band,
        &chrome.header_text,
        title,
        band.left() + chrome.row_inset + 16.0,
        chrome.header_text.text_color(),
    );
    if response.clicked() {
        state.collapsed = !state.collapsed;
    }

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
    response
}

fn draw_footer(
    ui: &mut Ui,
    id: Id,
    chrome: &ListChromeStyle,
    len: usize,
    max: usize,
    state: &ListState,
    pending: &mut Option<Edit>,
) {
    let width = ui.available_width().max(1.0);
    let (rect, _) = ui.allocate_exact_size(vec2(width, chrome.button_height + 2.0), Sense::hover());
    // The pair is outside the container: it hangs flush under the block's
    // bottom-right corner, on the page background, not in a footer band.
    let group = Rect::from_min_size(
        pos2(rect.right() - 2.0 - chrome.button_cell * 2.0, rect.top()),
        vec2(chrome.button_cell * 2.0, chrome.button_height),
    );
    let add_rect = Rect::from_min_size(group.min, vec2(chrome.button_cell, chrome.button_height));
    let remove_rect = Rect::from_min_size(
        pos2(group.left() + chrome.button_cell, group.top()),
        vec2(chrome.button_cell, chrome.button_height),
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
    paint_footer_group(
        ui,
        chrome,
        group,
        add_rect,
        remove_rect,
        add.hovered() && can_add,
        remove.hovered() && can_remove,
    );
    paint_plus_with(ui, add_rect.center(), chrome.button_text, can_add);
    paint_minus_with(ui, remove_rect.center(), chrome.button_text, can_remove);

    if add.clicked() && can_add && pending.is_none() {
        *pending = Some(Edit::Insert { index: len });
    }
    if remove.clicked() && can_remove && pending.is_none() {
        *pending = Some(Edit::Remove {
            index: state.selected.unwrap_or(len - 1).min(len - 1),
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_item_band(
    ui: &mut Ui,
    id: Id,
    index: usize,
    chrome: &ListChromeStyle,
    label: &str,
    selected: bool,
    open: bool,
    drag_active: bool,
    hidden: bool,
) -> BandOutcome {
    let width = ui.available_width().max(1.0);
    let (rect, _) = ui.allocate_exact_size(vec2(width, chrome.row_height), Sense::hover());
    if hidden {
        // The ghost carries this band; keep its slot, paint and hit-test
        // nothing.
        return BandOutcome {
            rect,
            fold_clicked: false,
            select_clicked: false,
            pressed: false,
            drag_started: false,
            grab: 0.0,
        };
    }
    let x0 = rect.left() + chrome.row_inset;
    let content_left = x0 + chrome.gutter_width + 4.0;
    let label_x = content_left + 16.0;
    let handle_x = rect.left() + 2.0;

    let handle_rect = Rect::from_min_size(
        pos2(handle_x, rect.top()),
        vec2(chrome.gutter_width, rect.height()),
    );
    let caret_rect = Rect::from_min_size(pos2(content_left, rect.top()), vec2(16.0, rect.height()));
    let label_rect = Rect::from_min_max(pos2(label_x, rect.top()), rect.right_bottom());

    let handle = ui
        .interact(
            handle_rect,
            list_handle_id(id, index),
            Sense::click_and_drag(),
        )
        .on_hover_cursor(CursorIcon::Grab);
    let fold = ui
        .interact(caret_rect, list_fold_id(id, index), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    let select = ui.interact(label_rect, list_item_id(id, index), Sense::click());

    // The ghost keeps the press point under the pointer, not the band's
    // centre: grabbing the grip's top edge must not make the row jump.
    let grab = ui
        .input(|input| input.pointer.press_origin())
        .map_or(rect.height() * 0.5, |origin| {
            (origin.y - rect.top()).clamp(0.0, rect.height())
        });

    // Select on mouse-down, but never through another row's handle once a
    // drag is under way: the held row keeps the pointer.
    let pressed = (handle.is_pointer_button_down_on() || handle.clicked()) && !drag_active;
    let drag_started = handle.drag_started();
    let active = selected || pressed || handle.dragged() || drag_started;
    let fill = if active {
        chrome.selection
    } else if !drag_active && ui.rect_contains_pointer(rect) {
        chrome.band_hover
    } else {
        chrome.band_fill
    };
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::ZERO, fill);
    // The band is the block's raised top: a light top edge and a dark bottom
    // edge separate it from the element body and from the block above.
    painter.rect_filled(
        Rect::from_min_size(rect.min, vec2(rect.width(), 1.0)),
        CornerRadius::ZERO,
        lift(fill, 0x18),
    );
    painter.rect_filled(
        Rect::from_min_size(
            pos2(rect.left(), rect.bottom() - 1.0),
            vec2(rect.width(), 1.0),
        ),
        CornerRadius::ZERO,
        chrome.container_border,
    );

    labelled_widget_info(&select, WidgetType::SelectableLabel, label, Some(active));

    let ink = if active {
        chrome.band_text_selected.text_color()
    } else if handle.hovered() {
        chrome.band_text.text_color()
    } else {
        chrome.handle
    };
    let text_style = if active {
        &chrome.band_text_selected
    } else {
        &chrome.band_text
    };
    paint_element_ink(
        ui,
        painter,
        chrome,
        rect,
        label,
        label_x,
        content_left + 4.0,
        open,
        ink,
        text_style,
        text_style.text_color(),
    );

    BandOutcome {
        rect,
        fold_clicked: fold.clicked(),
        select_clicked: select.clicked(),
        pressed,
        drag_started,
        grab,
    }
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

fn apply_edit<T>(
    items: &mut Vec<T>,
    make: &mut dyn FnMut() -> T,
    state: &mut ListState,
    edit: Edit,
    max: usize,
) -> bool {
    if !matches!(edit, Edit::Move { .. }) {
        state.shifts.clear();
    }
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
    /// Element band at rest and under the pointer.
    pub band_fill: Color32,
    pub band_hover: Color32,
    /// The second grey of an element body: its field rows stripe between
    /// [Self::band_fill] and this, on the raised band background.
    pub band_alt: Color32,
    /// Selected element band.
    pub selection: Color32,
    /// Drag-handle ink at rest.
    pub handle: Color32,
    /// Dark rules: the list frame and the band/block bottom edges.
    /// `container_bevel` outlines the floating drag ghost; a band's own top
    /// edge is a lift of its fill, so a selected blue band gets a blue edge.
    pub container_border: Color32,
    pub container_bevel: Color32,
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
}

impl ListChromeStyle {
    /// The chrome a bare [ListField] paints from its field tokens.
    ///
    /// The field style owns the value box; the list greys are derived from it
    /// so the typed widget inherits the caller's theme. A page with a richer
    /// skin (Preferences) builds the same struct from its own tokens.
    pub fn from_field(field: &FieldStyle) -> Self {
        let band_fill = lift(field.fill, 0x1E);
        let row_height = (field.height + 4.0).max(20.0);
        Self {
            field: field.clone(),
            header_text: field.text.clone().color(field.label),
            band_text: field.text.clone().color(field.label),
            band_text_selected: field.text.clone().color(field.label_active),
            header_fill: field.fill,
            header_hover: field.hover_fill,
            band_fill,
            band_hover: lift(field.fill, 0x26),
            band_alt: lift(band_fill, 0x07),
            selection: shade(field.accent, 0.62),
            handle: field.edge_hover,
            container_border: field.edge,
            container_bevel: lift(field.fill, 0x28),
            row_height,
            group_height: row_height,
            group_indent: 16.0,
            row_inset: 4.0,
            gutter_width: 20.0,
            button_cell: 22.0,
            button_height: 16.0,
            count_width: 48.0,
            button_fill: band_fill,
            button_hover: lift(field.fill, 0x26),
            button_border: field.edge,
            button_border_width: field.border_width.max(1.0),
            button_text: field.text.text_color(),
        }
    }
}

fn lift(color: Color32, by: u8) -> Color32 {
    Color32::from_rgb(
        color.r().saturating_add(by),
        color.g().saturating_add(by),
        color.b().saturating_add(by),
    )
}

fn shade(color: Color32, factor: f32) -> Color32 {
    Color32::from_rgb(
        (color.r() as f32 * factor).round() as u8,
        (color.g() as f32 * factor).round() as u8,
        (color.b() as f32 * factor).round() as u8,
    )
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
    /// The pointer went down on this band's handle: Unity selects the element
    /// on mouse-down, before it has moved far enough to become a drag.
    pub pressed: bool,
    /// The drag gesture started on this band this pass. The page selects the
    /// element so its band turns blue while its ghost follows the pointer.
    pub drag_started: bool,
}

/// What a style-token list footer did this pass.
#[derive(Debug, Default)]
pub struct ListFooterOutcome {
    pub clicked: bool,
    pub edit: Option<ListChromeEdit>,
}

/// What [ListChrome::finish] resolved after every band was laid out.
#[derive(Debug, Default)]
pub struct ListChromeOutcome {
    /// A reorder to send to the owner, emitted once when the drag is released.
    pub edit: Option<ListChromeEdit>,
    /// The drag gesture ended this pass.
    pub finished: bool,
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
    /// Container tops recorded by [ListChrome::header], closed by
    /// [ListChrome::footer]. One entry per open list, including empty ones.
    containers: Vec<(Id, f32)>,
    /// Short slides still settling after a drop.
    shifts: Vec<ChromeShift>,
}

#[derive(Clone, Copy, Debug)]
struct ChromeDrag {
    list: Id,
    from: usize,
    /// Pointer offset from the band's top when the drag started.
    grab: f32,
    /// The preview insertion index while the pointer is held. The data order
    /// does not change until release; this only drives the neighbours' slide.
    to: usize,
}

#[derive(Clone, Debug)]
struct ChromeBand {
    list: Id,
    index: usize,
    rect: Rect,
    /// The layout offset the page applied before this band, so drag targeting
    /// can recover its unshifted slot.
    shift: f32,
    /// The block's normal-layout bottom, recorded when the next band or the
    /// footer closes it.
    bottom: Option<f32>,
}

/// A short vertical slide applied to one element block for a preview step or
/// after a drop. The band and its rows are painted at the recorded offset from
/// their new layout slot, then the offset eases to zero.
#[derive(Clone, Copy, Debug)]
struct ChromeShift {
    list: Id,
    index: usize,
    from: f32,
    to: f32,
    start: f64,
    duration: f64,
}

impl ListChromeState {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// The list whose element is being dragged, if any.
    pub fn drag_list(&self) -> Option<Id> {
        self.drag.map(|drag| drag.list)
    }

    /// The data index of the element being dragged in `list`, if it is the
    /// active drag. The caller hides that element's block while the ghost
    /// carries it.
    pub fn drag_index(&self, list: Id) -> Option<usize> {
        self.drag
            .filter(|drag| drag.list == list)
            .map(|drag| drag.from)
    }

    /// Where the dragged block's normal top sits under the pointer: the
    /// visual translation a caller applies to the whole block.
    pub fn drag_offset(&self, list: Id, block_top: f32, pointer: Pos2) -> Option<f32> {
        self.drag
            .filter(|drag| drag.list == list)
            .map(|drag| pointer.y - drag.grab - block_top)
    }

    /// Drop any in-flight drop slides. A structural edit (insert, remove,
    /// resize) changes the indices they refer to.
    pub fn clear_shifts(&mut self) {
        self.shifts.clear();
    }

    /// Record the top of a list's container. The page calls this after the
    /// header and its optional gap; [ListChrome::footer] closes it at the
    /// cursor once the list's last row is laid out.
    pub fn open(&mut self, list: Id, top: f32) {
        self.containers.retain(|(open, _)| *open != list);
        self.containers.push((list, top));
    }

    fn take_container(&mut self, list: Id) -> Option<f32> {
        let index = self.containers.iter().position(|(open, _)| *open == list)?;
        Some(self.containers.remove(index).1)
    }

    /// Vertical paint offset for one element block, eased from `from` to `to`
    /// after a preview step or a drop.
    pub fn shift(&self, list: Id, index: usize, now: f64) -> f32 {
        self.shifts
            .iter()
            .find(|shift| shift.list == list && shift.index == index)
            .map_or(0.0, |shift| {
                let t = ((now - shift.start) / shift.duration).clamp(0.0, 1.0);
                let eased = (t * t * (3.0 - 2.0 * t)) as f32;
                shift.from + (shift.to - shift.from) * eased
            })
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
        state.containers.clear();
    }

    /// Record the top of one list's container, after its header and gap. The
    /// matching [ListChrome::footer] closes the frame at the cursor.
    pub fn open(state: &mut ListChromeState, list: Id, top: f32) {
        state.open(list, top);
    }

    /// Draw the header band: foldout, name, and the inline count box on the
    /// right. The add/remove pair belongs to the footer, at the bottom-right of
    /// the list container.
    #[allow(clippy::too_many_arguments)]
    pub fn header(
        ui: &mut Ui,
        state: &mut ListChromeState,
        style: &ListChromeStyle,
        list: Id,
        ids: ListChromeIds,
        label: &str,
        len: usize,
        folded: bool,
        depth: usize,
        row_width: f32,
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

        // The count is a field box in the header row, flush with the list
        // container's right edge. The rest of the row folds the list; the box
        // owns its own clicks.
        let count = Rect::from_min_size(
            pos2(
                band.right() - style.count_width,
                band.center().y - style.field.height * 0.5,
            ),
            vec2(style.count_width, style.field.height),
        );
        let label_rect =
            Rect::from_min_max(band.min, pos2(count.left().max(band.left()), band.bottom()));
        let response = ui
            .interact(label_rect, ids.header, Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        labelled_widget_info(&response, WidgetType::SelectableLabel, label, Some(!folded));

        let mut len_value = len;
        let mut edit = if state.count_list == Some(list) {
            state.count_edit.take()
        } else {
            None
        };
        let count_outcome = NumberField::new(ids.count, &mut len_value, &style.field)
            .label(label)
            .show(ui, count, &mut edit);
        let mut edit_out = None;
        if count_outcome.changed() {
            edit_out = Some(ListChromeEdit::SetLen {
                list,
                len: len_value,
            });
        }
        if let Some(edit) = edit {
            state.count_edit = Some(edit);
            state.count_list = Some(list);
        } else if state.count_list == Some(list) {
            state.count_list = None;
        }

        let clicked = response.clicked() || count_outcome.clicked();
        let folded = response.clicked().then_some(!folded);
        ListHeaderOutcome {
            response,
            clicked,
            folded,
            edit: edit_out,
            finished: count_outcome.finished,
        }
    }

    /// Draw one element band: the drag handle at the far left, the fold caret,
    /// the selection label, and the hover or selection band behind them.
    #[allow(clippy::too_many_arguments)]
    pub fn band(
        ui: &mut Ui,
        state: &mut ListChromeState,
        style: &ListChromeStyle,
        list: Id,
        ids: ListBandIds,
        index: usize,
        label: &str,
        // The layout offset the caller applied before this band's row.
        offset: f32,
        depth: usize,
        row_width: f32,
        row_fill: Color32,
        selected: bool,
        folded: bool,
    ) -> ListBandOutcome {
        let (rect, _) = ui.allocate_exact_size(vec2(row_width, style.row_height), Sense::hover());
        let x0 = rect.left() + style.row_inset + depth as f32 * style.group_indent;
        let content_left = x0 + style.gutter_width + 4.0;
        let label_x = content_left + 16.0;
        // The grip is the container's first cell, not something indented with
        // the fold caret: it stays at the far left at every nesting depth.
        let handle_x = rect.left() + 2.0;
        let handle = Rect::from_min_size(
            pos2(handle_x, rect.top()),
            vec2(style.gutter_width, rect.height()),
        );
        let caret_rect =
            Rect::from_min_size(pos2(content_left, rect.top()), vec2(16.0, rect.height()));
        let label_rect =
            Rect::from_min_max(pos2(label_x, rect.top()), pos2(rect.right(), rect.bottom()));

        // This band's top closes the block above it, which is the only point
        // where the page knows how tall that block's body turned out. `rect`
        // is this band's shifted layout top; its normal-layout top is where
        // the block above really ends.
        if let Some(previous) = state.bands.iter_mut().rev().find(|band| band.list == list)
            && previous.bottom.is_none()
        {
            previous.bottom = Some(rect.top() - offset);
        }

        let grip = ui
            .interact(handle, ids.handle, Sense::click_and_drag())
            .on_hover_cursor(CursorIcon::Grab);
        let caret = ui
            .interact(caret_rect, ids.fold, Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        let select = ui.interact(label_rect, ids.select, Sense::click());

        // The ghost keeps the press point under the pointer, so grabbing the
        // grip near its edge does not make the row jump when it lifts.
        let grab = ui
            .input(|input| input.pointer.press_origin())
            .map_or(rect.height() * 0.5, |origin| {
                (origin.y - rect.top()).clamp(0.0, rect.height())
            });

        // Select on mouse-down, and keep it on a click release. Only while no
        // row is already being dragged: a second row's handle must not steal
        // the selection from the element under the pointer.
        let pressed = (grip.is_pointer_button_down_on() || grip.clicked()) && state.drag.is_none();
        if grip.drag_started() {
            // A new drag starts from a still list: do not target slots that
            // are still sliding from the previous drop.
            state.shifts.clear();
            state.drag = Some(ChromeDrag {
                list,
                from: index,
                grab,
                to: index,
            });
        }
        let dragging = state
            .drag
            .is_some_and(|drag| drag.list == list && drag.from == index);
        let active = selected || dragging || grip.drag_started() || pressed;
        // While a drag is held, the band under the pointer is the ghost's
        // target, not a hovered row.
        let hovered = state.drag.is_none() && ui.rect_contains_pointer(rect);
        let fill = if active {
            style.selection
        } else if hovered {
            style.band_hover
        } else {
            row_fill
        };
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::ZERO, fill);
        // The band is the top of a raised block: a light top edge and a dark
        // bottom edge against the body's first stripe.
        painter.rect_filled(
            Rect::from_min_size(rect.min, vec2(rect.width(), 1.0)),
            CornerRadius::ZERO,
            lift(fill, 0x18),
        );
        painter.rect_filled(
            Rect::from_min_size(
                pos2(rect.left(), rect.bottom() - 1.0),
                vec2(rect.width(), 1.0),
            ),
            CornerRadius::ZERO,
            style.container_border,
        );

        labelled_widget_info(&select, WidgetType::SelectableLabel, label, Some(active));

        let ink = if active {
            style.band_text_selected.text_color()
        } else if grip.hovered() {
            style.band_text.text_color()
        } else {
            style.handle
        };
        let text_style = if active {
            &style.band_text_selected
        } else {
            &style.band_text
        };
        paint_element_ink(
            ui,
            painter,
            style,
            rect,
            label,
            label_x,
            content_left + 4.0,
            !folded,
            ink,
            text_style,
            text_style.text_color(),
        );

        state.bands.push(ChromeBand {
            list,
            index,
            rect,
            shift: offset,
            bottom: None,
        });
        let selected = select.clicked();
        let folded = caret.clicked().then_some(!folded);
        ListBandOutcome {
            response: select,
            selected,
            folded,
            pressed,
            drag_started: grip.drag_started(),
        }
    }

    /// Draw the add/remove pair at the bottom-right of the list block and close
    /// the container frame opened for this list.
    #[allow(clippy::too_many_arguments)]
    pub fn footer(
        ui: &mut Ui,
        state: &mut ListChromeState,
        style: &ListChromeStyle,
        list: Id,
        ids: ListChromeIds,
        len: usize,
        selected: Option<usize>,
        row_width: f32,
    ) -> ListFooterOutcome {
        let (rect, _) =
            ui.allocate_exact_size(vec2(row_width, style.button_height + 2.0), Sense::hover());
        let container_top = state.take_container(list);
        if let Some(last) = state.bands.iter_mut().rev().find(|band| band.list == list)
            && last.bottom.is_none()
        {
            last.bottom = Some(rect.top());
        }

        // The pair hangs outside the container, flush under its bottom-right
        // corner: no footer band, no fill, and the frame ends above it.
        let group = Rect::from_min_size(
            pos2(rect.right() - 2.0 - style.button_cell * 2.0, rect.top()),
            vec2(style.button_cell * 2.0, style.button_height),
        );
        let add_rect = Rect::from_min_size(group.min, vec2(style.button_cell, style.button_height));
        let remove_rect = Rect::from_min_size(
            pos2(group.left() + style.button_cell, group.top()),
            vec2(style.button_cell, style.button_height),
        );
        let can_add = len < usize::MAX;
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
        paint_footer_group(
            ui,
            style,
            group,
            add_rect,
            remove_rect,
            add.hovered() && can_add,
            remove.hovered() && can_remove,
        );
        paint_plus_with(ui, add_rect.center(), style.button_text, can_add);
        paint_minus_with(ui, remove_rect.center(), style.button_text, can_remove);

        let mut edit = None;
        if add.clicked() && can_add {
            edit = Some(ListChromeEdit::Insert { list, index: len });
        }
        if remove.clicked() && can_remove {
            edit = Some(ListChromeEdit::Remove {
                list,
                index: selected.unwrap_or(len - 1).min(len - 1),
            });
        }
        if let Some(top) = container_top {
            paint_container(
                ui,
                style,
                Rect::from_min_max(
                    pos2(rect.left(), top.min(rect.top())),
                    pos2(rect.right(), rect.top()),
                ),
            );
        }
        ListFooterOutcome {
            clicked: add.clicked() || remove.clicked(),
            edit,
        }
    }

    /// Resolve the active drag after every band is laid out.
    ///
    /// While the pointer is down nothing is reordered: the caller draws the
    /// held block on a floating layer, and the other blocks slide to their
    /// preview slots so a gap opens under the pointer. The move is emitted
    /// once, on release; blocks that are still mid-slide continue from where
    /// they are.
    pub fn finish(ui: &mut Ui, state: &mut ListChromeState) -> ListChromeOutcome {
        let now = ui.input(|input| input.time);
        let Some(drag) = state.drag else {
            return ListChromeOutcome::default();
        };
        let mut bands: Vec<&ChromeBand> = state
            .bands
            .iter()
            .filter(|band| band.list == drag.list)
            .collect();
        bands.sort_by_key(|band| band.index);

        let released = ui.input(|input| input.pointer.any_released());
        let down = ui.input(|input| input.pointer.primary_down());
        if bands.is_empty() || (!down && !released) {
            state.drag = None;
            return ListChromeOutcome {
                edit: None,
                finished: true,
            };
        }
        let Some(pointer) = ui.ctx().pointer_interact_pos() else {
            if !down {
                state.drag = None;
                return ListChromeOutcome {
                    edit: None,
                    finished: true,
                };
            }
            return ListChromeOutcome::default();
        };

        let count = bands.len();
        // Normal-layout tops and heights: the page offsets each block from
        // these by its slide, so the drop math is free of the animation.
        let tops: Vec<f32> = bands
            .iter()
            .map(|band| band.rect.top() - band.shift)
            .collect();
        let heights: Vec<f32> = bands
            .iter()
            .map(|band| {
                (band.bottom.unwrap_or(band.rect.bottom()) - (band.rect.top() - band.shift))
                    .max(1.0)
            })
            .collect();
        // The slot the pointer is over: count the other blocks whose current
        // (animated) centre is above it. Excluding the held block keeps the
        // count stable after the neighbours start to slide.
        let to = preview_index(&bands, drag.from, pointer.y);

        if released {
            let mut edit = None;
            // After the owner applies the move each block is at the slot
            // `drop_shifts` returns; start it where it is on screen now so a
            // release in the middle of a slide does not snap.
            let preview = preview_tops(count, drag.from, to, &tops, &heights);
            let starts: Vec<f32> = (0..count)
                .map(|index| {
                    if index == drag.from {
                        preview[index]
                    } else {
                        bands[index].rect.top()
                    }
                })
                .collect();
            state.shifts = drop_shifts(count, drag.from, to, &tops, &heights, &starts)
                .into_iter()
                .map(|(index, offset)| ChromeShift {
                    list: drag.list,
                    index,
                    from: offset,
                    to: 0.0,
                    start: now,
                    duration: 0.12,
                })
                .collect();
            if to != drag.from {
                edit = Some(ListChromeEdit::Move {
                    list: drag.list,
                    from: drag.from,
                    to,
                });
            }
            state.drag = None;
            return ListChromeOutcome {
                edit,
                finished: true,
            };
        }

        // The pointer crossed into another slot: slide every other block to
        // its preview position. The held block itself is hidden by the page,
        // so the gap follows the pointer while it is held.
        if to != drag.to {
            let preview = preview_tops(count, drag.from, to, &tops, &heights);
            let mut shifts = Vec::new();
            for (index, band) in bands.iter().enumerate() {
                if index == drag.from {
                    continue;
                }
                // The layout is still at the current top; animate the
                // offset from where it is now to the preview displacement.
                let from = band.shift;
                let to_offset = preview[index] - (band.rect.top() - band.shift);
                if (to_offset - from).abs() > 0.5 {
                    shifts.push(ChromeShift {
                        list: drag.list,
                        index,
                        from,
                        to: to_offset,
                        start: now,
                        duration: 0.16,
                    });
                }
            }
            state.shifts = shifts;
            if let Some(drag) = state.drag.as_mut() {
                drag.to = to;
            }
        }

        // The caller draws the dragged block itself on a floating layer;
        // finish only resolves the preview slot and the slides.
        ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        ListChromeOutcome::default()
    }
}

/// The frame around the element bands: one dark outline.
/// The raised bevel belongs to each element block now, not to the list well:
/// painting it here as well would double the edge along the first and last
/// rows.
fn paint_container(ui: &Ui, style: &ListChromeStyle, rect: Rect) {
    if rect.height() < 1.0 {
        return;
    }
    ui.painter().rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, style.container_border),
        StrokeKind::Inside,
    );
}

/// The drag handle ink: two short bars in the container's left cell.
fn paint_handle(painter: &egui::Painter, rect: Rect, ink: Color32) {
    for offset in [-2.0, 2.0] {
        painter.rect_filled(
            Rect::from_min_size(
                pos2(rect.left() + 7.0, rect.center().y + offset - 0.5),
                vec2(10.0, 1.0),
            ),
            CornerRadius::ZERO,
            ink,
        );
    }
}

/// Handle, caret and label of one element band. Both the live band and its
/// floating drag ghost paint through this, so they cannot drift apart.
#[allow(clippy::too_many_arguments)]
fn paint_element_ink(
    ui: &Ui,
    painter: &egui::Painter,
    chrome: &ListChromeStyle,
    rect: Rect,
    label: &str,
    label_x: f32,
    caret_x: f32,
    open: bool,
    handle_ink: Color32,
    text_style: &TextStyle,
    text_ink: Color32,
) {
    let handle = Rect::from_min_size(
        pos2(rect.left() + 2.0, rect.top()),
        vec2(chrome.gutter_width, rect.height()),
    );
    paint_handle(painter, handle, handle_ink);
    paint_arrow(painter, pos2(caret_x, rect.center().y), open, text_ink);
    paint_text_on(ui, painter, rect, text_style, label, label_x, text_ink);
}

/// The band that follows the pointer during a drag. It lives on the tooltip
/// layer so the scroll area's clip cannot cut it off, and carries a drop
/// shadow so it reads as lifted above the list.
#[allow(clippy::too_many_arguments)]
fn paint_element_ghost(
    ui: &Ui,
    chrome: &ListChromeStyle,
    list: Id,
    rect: Rect,
    label: &str,
    depth: usize,
    open: bool,
    selected: bool,
) {
    let painter = ui
        .ctx()
        .layer_painter(egui::LayerId::new(
            egui::Order::Tooltip,
            Id::new(("kairos.list.ghost", list)),
        ))
        // The tooltip layer ignores the parent's clip. Clip the band to itself
        // (plus its drop shadow), not to the scroll viewport: an element that
        // was only half visible must still drag whole.
        .with_clip_rect(rect.expand(8.0));
    let shadow = Rect::from_min_size(rect.min + vec2(3.0, 4.0), rect.size());
    painter.rect_filled(shadow, CornerRadius::ZERO, Color32::from_black_alpha(80));
    let fill = if selected {
        chrome.selection
    } else {
        chrome.band_fill
    };
    painter.rect_filled(rect, CornerRadius::ZERO, fill);
    painter.rect_filled(
        Rect::from_min_size(rect.min, vec2(rect.width(), 1.0)),
        CornerRadius::ZERO,
        lift(fill, 0x20),
    );
    painter.rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, chrome.container_bevel),
        StrokeKind::Inside,
    );
    let content_left = rect.left()
        + chrome.row_inset
        + depth as f32 * chrome.group_indent
        + chrome.gutter_width
        + 4.0;
    let text_style = if selected {
        &chrome.band_text_selected
    } else {
        &chrome.band_text
    };
    let ink = text_style.text_color();
    paint_element_ink(
        ui,
        &painter,
        chrome,
        rect,
        label,
        content_left + 16.0,
        content_left + 4.0,
        open,
        ink,
        text_style,
        ink,
    );
}

/// Where every block starts after a drop, and how far it has to slide to get
/// there. `tops` and `heights` describe the blocks in their old order;
/// `starts` is the screen top each block begins at (the gap position for the
/// dragged one, its current animated top for the rest). The result is keyed by
/// the slot the block occupies in the new order.
fn drop_shifts(
    count: usize,
    from: usize,
    to: usize,
    tops: &[f32],
    heights: &[f32],
    starts: &[f32],
) -> Vec<(usize, f32)> {
    if count == 0 || from >= count || to >= count {
        return Vec::new();
    }
    let mut order: Vec<usize> = (0..count).collect();
    let moved = order.remove(from);
    order.insert(to, moved);
    let mut new_tops = vec![0.0; count];
    let mut y = tops[0];
    for (slot, index) in order.iter().enumerate() {
        new_tops[slot] = y;
        y += heights[*index];
    }
    let mut shifts = Vec::new();
    for (index, start) in starts.iter().enumerate() {
        let slot = move_index(index, from, to);
        let offset = start - new_tops[slot];
        if offset.abs() > 0.5 {
            shifts.push((slot, offset));
        }
    }
    shifts
}
/// The insertion index the pointer is over while an element is held. Count
/// the other blocks whose current (animated) centre is above the pointer; the
/// held block is excluded, so its own old slot does not bias the count.
fn preview_index(bands: &[&ChromeBand], from: usize, pointer_y: f32) -> usize {
    let mut to = 0;
    for band in bands {
        if band.index == from {
            continue;
        }
        if pointer_y > band.rect.center().y {
            to += 1;
        }
    }
    to.min(bands.len().saturating_sub(1))
}

/// The top each block would occupy with the held block removed from its old
/// slot and inserted at `to`, keyed by the block's current data index. The
/// held block's entry is its future slot (the gap).
fn preview_tops(count: usize, from: usize, to: usize, tops: &[f32], heights: &[f32]) -> Vec<f32> {
    if count == 0 || from >= count || to >= count {
        return Vec::new();
    }
    let mut order: Vec<usize> = (0..count).collect();
    let moved = order.remove(from);
    order.insert(to, moved);
    let mut result = vec![0.0; count];
    let mut y = tops[0];
    for index in order {
        result[index] = y;
        y += heights[index];
    }
    result
}

/// The joined add/remove pair at the bottom-right of a list: one outline with
/// a divider, and the hover fill inside the cell under the pointer.
fn paint_footer_group(
    ui: &Ui,
    style: &ListChromeStyle,
    group: Rect,
    add: Rect,
    remove: Rect,
    add_hovered: bool,
    remove_hovered: bool,
) {
    let painter = ui.painter();
    painter.rect_filled(group, CornerRadius::ZERO, style.button_fill);
    if add_hovered {
        painter.rect_filled(add.shrink(1.0), CornerRadius::ZERO, style.button_hover);
    }
    if remove_hovered {
        painter.rect_filled(remove.shrink(1.0), CornerRadius::ZERO, style.button_hover);
    }
    let stroke = Stroke::new(style.button_border_width.max(1.0), style.button_border);
    painter.rect_stroke(group, CornerRadius::ZERO, stroke, StrokeKind::Inside);
    painter.line_segment(
        [
            pos2(group.center().x, group.top() + 1.0),
            pos2(group.center().x, group.bottom() - 1.0),
        ],
        Stroke::new(1.0, style.button_border),
    );
}

fn paint_text_with(ui: &Ui, rect: Rect, style: &TextStyle, text: &str, left: f32, color: Color32) {
    paint_text_on(ui, ui.painter(), rect, style, text, left, color);
}

fn paint_text_on(
    ui: &Ui,
    painter: &egui::Painter,
    rect: Rect,
    style: &TextStyle,
    text: &str,
    left: f32,
    color: Color32,
) {
    let galley = style.layout(ui, text);
    painter.with_clip_rect(rect).galley(
        pos2(left, rect.center().y - galley.size().y * 0.5),
        galley,
        color,
    );
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

    /// Unity's list shape: count in the header row, drag handle at the far
    /// left of a band, and the add/remove pair under the rows.
    #[test]
    fn the_list_chrome_matches_unitys_shape() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-list-chrome");
        let mut items = vec![Item { value: 0 }, Item { value: 1 }];
        let mut state = ListState::default();

        let mut draw = |ui: &mut Ui| {
            ListField::new(id, &mut items, &style, Item::default).show(
                ui,
                &mut state,
                |_, _, _| ListItemOutcome::default(),
            );
        };
        let _ = ctx.run_ui(input(), &mut draw);

        let header = ctx.read_response(list_header_id(id)).unwrap().rect;
        let count = ctx.read_response(list_count_id(id)).unwrap().rect;
        assert!(
            approx(header.center().y, count.center().y),
            "the count box shares the header row: header {header:?}, count {count:?}"
        );
        assert!(
            count.left() > header.left(),
            "the count box is at the far right: {count:?}"
        );

        let add = ctx.read_response(list_add_id(id)).unwrap().rect;
        let remove = ctx.read_response(list_remove_id(id)).unwrap().rect;
        let last = ctx.read_response(list_item_id(id, 1)).unwrap().rect;
        assert!(
            add.top() >= last.bottom(),
            "the add pair is under the rows, not in the header: add {add:?}, last {last:?}"
        );
        assert!(
            approx(add.center().y, remove.center().y) && remove.left() >= add.right(),
            "the pair is two joined cells: add {add:?}, remove {remove:?}"
        );

        let handle = ctx.read_response(list_handle_id(id, 0)).unwrap().rect;
        let band = ctx.read_response(list_item_id(id, 0)).unwrap().rect;
        assert!(
            handle.right() <= band.left(),
            "the drag handle is the leftmost cell: handle {handle:?}, band {band:?}"
        );

        let _ = draw;
    }

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.5
    }

    fn painted_text(output: &egui::FullOutput) -> Vec<String> {
        fn walk(shape: &egui::Shape, out: &mut Vec<String>) {
            match shape {
                egui::Shape::Text(text) => out.push(text.galley.text().to_owned()),
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        walk(shape, out);
                    }
                }
                _ => {}
            }
        }
        let mut out = Vec::new();
        for clipped in &output.shapes {
            walk(&clipped.shape, &mut out);
        }
        out
    }

    /// The caller can name the bands from the element itself; the default is
    /// the index-based `Element N`.
    #[test]
    fn the_item_label_callback_names_each_band() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-list-item-label");
        let mut items = vec![Item { value: 4 }, Item { value: 5 }];
        let mut state = ListState::default();

        let mut draw = |ui: &mut Ui| {
            ListField::new(id, &mut items, &style, Item::default)
                .item_label(|item, index| format!("Bone {index}: {}", item.value))
                .show(ui, &mut state, |_, _, _| ListItemOutcome::default());
        };
        let output = ctx.run_ui(input(), &mut draw);
        let text = painted_text(&output);
        assert!(text.iter().any(|run| run == "Bone 0: 4"), "{text:?}");
        assert!(text.iter().any(|run| run == "Bone 1: 5"), "{text:?}");
        let _ = draw;
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

    /// The drop animation is keyed by the slot a block lands in, not by the
    /// index it came from: otherwise a move would animate the two blocks into
    /// each other's destinations.
    #[test]
    fn a_drop_targets_the_slot_each_block_lands_in() {
        // Two blocks, tall (100) then short (20). Drag the short one above the
        // tall one; its ghost was released at y = 50.
        assert_eq!(
            drop_shifts(2, 1, 0, &[0.0, 100.0], &[100.0, 20.0], &[0.0, 50.0]),
            vec![(1, -20.0), (0, 50.0)],
            "the tall block slides down into slot 1; the dragged one starts at the ghost"
        );

        // Drag the tall first block below the short one; the ghost was released
        // at the tall block's top (100).
        assert_eq!(
            drop_shifts(2, 0, 1, &[0.0, 100.0], &[100.0, 20.0], &[100.0, 100.0]),
            vec![(1, 80.0), (0, 100.0)],
            "the tall block lands in slot 1 from the ghost; the short one rises to slot 0"
        );
    }

    /// Holding a drag previews the result: the vector is untouched, the held
    /// band is hidden and the neighbour slides up into the gap.
    #[test]
    fn holding_a_drag_opens_a_gap_before_the_release() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-list-gap");
        let items = RefCell::new(vec![
            Item { value: 0 },
            Item { value: 1 },
            Item { value: 2 },
        ]);
        let mut state = ListState::default();

        let mut draw = |ui: &mut Ui| {
            let mut items = items.borrow_mut();
            ListField::new(id, &mut items, &style, Item::default).show(
                ui,
                &mut state,
                |_, _, _| ListItemOutcome::default(),
            );
        };
        let _ = ctx.run_ui(input(), &mut draw);
        let start = ctx
            .read_response(list_handle_id(id, 0))
            .unwrap()
            .rect
            .center();
        let before = ctx.read_response(list_item_id(id, 1)).unwrap().rect.top();

        let mut hover = input();
        hover.events = vec![Event::PointerMoved(start)];
        let _ = ctx.run_ui(hover, &mut draw);
        let mut press = input();
        press.events = vec![button(start, true)];
        let _ = ctx.run_ui(press, &mut draw);
        let target = ctx
            .read_response(list_item_id(id, 1))
            .unwrap()
            .rect
            .bottom()
            - 2.0;
        let mut moved = input();
        moved.events = vec![Event::PointerMoved(pos2(start.x, target))];
        let _ = ctx.run_ui(moved, &mut draw);

        assert_eq!(
            values(&items.borrow()),
            vec![0, 1, 2],
            "the vector is not reordered while the pointer is held"
        );

        // The shift is applied on the pass after the target changes;
        // read_response is one pass behind, so run two still frames.
        let _ = ctx.run_ui(input(), &mut draw);
        let _ = ctx.run_ui(input(), &mut draw);
        let after = ctx.read_response(list_item_id(id, 1)).unwrap().rect.top();
        assert!(
            after < before,
            "the neighbour slides up into the preview slot: {before} -> {after}"
        );

        let mut release = input();
        release.events = vec![button(pos2(start.x, target), false)];
        let _ = ctx.run_ui(release, &mut draw);
        let _ = draw;
        assert_eq!(values(&items.borrow()), vec![1, 0, 2]);
        assert_eq!(
            state.selected(),
            Some(1),
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
