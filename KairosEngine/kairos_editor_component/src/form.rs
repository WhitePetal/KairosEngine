//! The rows a style draws itself into.
//!
//! A style type walks itself here ([`StyleRow`](crate::style_row::StyleRow));
//! this module owns everything the value cannot know: the two-column geometry,
//! foldout bands, list blocks, selection, the edit buffers, and the list of
//! dirty flag to write back. It replaces the page's field table: the caller
//! draws a typed copy of the style itself and hands the whole value back when
//! it changed.
//!
//! The pass owns the values, so nothing here touches `Action`/`Effect`: a page
//! turns `dirty` into one whole-style write and keeps the gesture boundary
//! (commit on release) where it is today.

use std::collections::HashSet;

use egui::{CornerRadius, CursorIcon, Id, LayerId, Order, Rect, Sense, Ui, UiBuilder, pos2, vec2};
use kairos_color::{Color, ColorUsage};
use kairos_editor_widgets::color::ColorExt;
use kairos_editor_widgets::color_picker::{
    ColorField, ColorPickerStyle, ColorProperty, eyedropper, is_color_picker_open,
};
use kairos_editor_widgets::field::{
    FieldLabel, FieldLabelResponse, FieldOutcome, FieldStyle, ListBandIds, ListChrome,
    ListChromeEdit, ListChromeIds, ListChromeOutcome, ListChromeState, ListChromeStyle, Number,
    NumberEdit, NumberField, TextField, ToggleField, VariantMenu,
};
use kairos_editor_widgets::text::TextStyle;

use crate::panels::PreferencesPanelStyle;
use crate::skin::{LIST_MAX, StyleEnum, humanize};
use crate::style_row::{RowSink, StyleRow};

/// A numeric token being typed into, keyed by its dotted path.
pub struct FormEdit {
    pub path: String,
    pub number: NumberEdit,
}

/// The page's state across passes: what is folded, selected and being edited.
#[derive(Default)]
pub struct FormState {
    /// Folded group and element paths.
    pub collapsed: HashSet<String>,
    /// The selected row, by dotted path.
    pub selected: Option<String>,
    /// A numeric token being edited as text.
    pub edit: Option<FormEdit>,
    /// A value was written this pass.
    pub dirty: bool,
    /// Retained list chrome: the count editor and the reorder gesture.
    pub list_chrome: ListChromeState,
}

/// The current list element's block: its visual depth and row parity.
#[derive(Clone, Copy, Debug)]
struct Element {
    depth: usize,
    row: usize,
}

/// One row's geometry, built before the value control draws.
struct Row {
    name: String,
    path: String,
    id: Id,
    value_rect: Rect,
    label_paint: Rect,
    label: FieldLabelResponse,
    active: bool,
}

/// The rows of one page, drawn into one `Ui`.
pub struct FormRows<'a> {
    ui: &'a mut Ui,
    panel: &'a PreferencesPanelStyle,
    field_style: &'a FieldStyle,
    chrome: &'a ListChromeStyle,
    picker: &'a ColorPickerStyle,
    state: &'a mut FormState,
    seed: Id,
    width: f32,
    /// The dotted path of the group being drawn.
    path: Vec<String>,
    /// The visual depth of the rows being drawn.
    depth: usize,
    /// The page-level zebra counter.
    rows: usize,
    /// The visual depth of the enum whose payload is being drawn.
    payload: Option<usize>,
    /// The list element whose fields are being drawn.
    element: Option<Element>,
    /// This pass draws a floating copy of a dragged element block: it paints
    /// the same rows but must not resolve the owner's list drag.
    in_ghost: bool,
    clicked: bool,
    finished: bool,
    /// A value changed this pass (the page sends one whole-style write).
    changed: bool,
}

impl<'a> FormRows<'a> {
    /// One page's rows. `seed` salts every widget id; `width` is the row width
    /// the page reserved (scroll bar and padding already removed).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        ui: &'a mut Ui,
        seed: Id,
        panel: &'a PreferencesPanelStyle,
        field_style: &'a FieldStyle,
        chrome: &'a ListChromeStyle,
        picker: &'a ColorPickerStyle,
        state: &'a mut FormState,
        width: f32,
    ) -> Self {
        // One page pass: forget the list bands the previous pass recorded, so
        // a drag resolves against this pass's layout.
        ListChrome::begin(&mut state.list_chrome);
        Self {
            ui,
            panel,
            field_style,
            chrome,
            picker,
            state,
            seed,
            width,
            path: Vec::new(),
            depth: 0,
            rows: 0,
            payload: None,
            element: None,
            in_ghost: false,
            clicked: false,
            finished: false,
            changed: false,
        }
    }

    /// A row consumed this pass's click (the page uses it to clear the
    /// selection only when nothing else did).
    pub fn clicked(&self) -> bool {
        self.clicked
    }

    /// A typed edit ended: the page must persist now instead of waiting for
    /// the pointer to come up.
    pub fn finished(&self) -> bool {
        self.finished
    }

    /// A value changed this pass: the page hands the owner a new copy.
    pub fn changed(&self) -> bool {
        self.changed
    }

    /// This page has unwritten edits (persists across passes until committed).
    pub fn dirty(&self) -> bool {
        self.state.dirty
    }

    fn child(&self, name: &str) -> String {
        if self.path.is_empty() {
            name.to_owned()
        } else if name.is_empty() {
            self.path.join(".")
        } else {
            format!("{}.{}", self.path.join("."), name)
        }
    }

    fn field_id(&self, path: &str) -> Id {
        self.seed.with(("field", path))
    }

    fn label_id(&self, path: &str) -> Id {
        self.seed.with(("label", path))
    }

    fn toggle_id(&self, path: &str) -> Id {
        self.seed.with(("toggle", path))
    }

    fn group_id(&self, path: &str) -> Id {
        self.seed.with(("group", path))
    }

    fn list_id(&self, path: &str) -> Id {
        self.seed.with(("list", path))
    }

    fn add_id(&self, path: &str) -> Id {
        self.seed.with(("list.add", path))
    }

    fn remove_id(&self, path: &str) -> Id {
        self.seed.with(("list.remove", path))
    }

    fn handle_id(&self, path: &str) -> Id {
        self.seed.with(("list.handle", path))
    }

    /// Lay out one leaf row and hand its label gesture back to the caller.
    fn begin_row(&mut self, name: &str, is_number: bool, is_text: bool) -> Row {
        let path = self.child(name);
        let id = self.field_id(&path);
        let in_element = self.element.is_some();
        let height = if in_element {
            self.chrome.row_height
        } else {
            self.panel.row_height
        };
        let (rect, _) = self
            .ui
            .allocate_exact_size(vec2(self.width, height), Sense::hover());

        let fill = if let Some(element) = self.element {
            if element.row.is_multiple_of(2) {
                self.chrome.band_fill
            } else {
                self.chrome.band_alt
            }
        } else {
            self.panel.row_fill(self.rows).to_egui()
        };
        self.ui
            .painter()
            .rect_filled(rect, CornerRadius::ZERO, fill);

        let (row_inset, group_indent) = if in_element {
            (self.chrome.row_inset, self.chrome.group_indent)
        } else {
            (self.panel.row_inset, self.panel.group_indent)
        };
        let content_left = rect.left() + row_inset;
        let content_right = (rect.right() - row_inset).max(content_left + 1.0);
        let content_width = content_right - content_left;
        let max_label = (content_width - self.panel.label_gap - self.panel.field_width).max(48.0);
        let label_width = self
            .panel
            .label_width
            .min(max_label)
            .min((content_width - self.panel.label_gap - 1.0).max(0.0));
        let value_left = content_left + label_width + self.panel.label_gap;
        let (label_left, value_left, value_width) = if self.payload.is_some() {
            let base_width = (content_right - value_left).max(1.0);
            let sub_label = (base_width * 0.42).clamp(48.0, self.panel.label_width);
            let sub_value = value_left + sub_label + self.panel.label_gap;
            (value_left, sub_value, (content_right - sub_value).max(1.0))
        } else {
            (
                content_left,
                value_left,
                (content_right - value_left).max(1.0),
            )
        };
        let value_rect = Rect::from_min_size(
            pos2(value_left, rect.center().y - self.field_style.height * 0.5),
            vec2(value_width, self.field_style.height),
        );
        let label_rect = Rect::from_min_max(
            pos2(label_left, rect.top()),
            pos2(value_left - self.panel.label_gap, rect.bottom()),
        );
        let label_x = if let Some(base) = self.payload {
            label_left + self.depth.saturating_sub(base) as f32 * self.panel.group_indent
        } else if let Some(element) = self.element {
            let x0 =
                rect.left() + row_inset + element.depth.saturating_sub(1) as f32 * group_indent;
            x0 + self.chrome.gutter_width
                + 4.0
                + 16.0
                + self.depth.saturating_sub(element.depth) as f32 * group_indent
        } else {
            content_left + 16.0 + self.depth as f32 * self.panel.group_indent
        };

        let label_text = humanize(name);
        let label = FieldLabel::new(self.label_id(&path), &label_text, self.field_style)
            .draggable(is_number);
        let response = label.interact(self.ui, label_rect);
        if response.clicked() {
            self.state.selected = Some(path.clone());
            self.clicked = true;
            if is_text {
                self.ui.memory_mut(|memory| memory.request_focus(id));
            }
        }

        if self.element.is_none() {
            self.rows += 1;
        } else if let Some(element) = &mut self.element {
            element.row += 1;
        }

        let active = self.state.selected.as_deref() == Some(path.as_str())
            || self.ui.memory(|memory| memory.has_focus(id))
            || self
                .state
                .edit
                .as_ref()
                .is_some_and(|edit| edit.path == path);
        Row {
            name: label_text,
            path,
            id,
            value_rect,
            label_paint: Rect::from_min_max(
                pos2(label_x, rect.top()),
                pos2(label_rect.right(), rect.bottom()),
            ),
            label: response,
            active,
        }
    }

    fn finish_row(&mut self, row: Row, outcome: FieldOutcome, changed: bool) {
        if changed {
            self.state.dirty = true;
            self.changed = true;
            self.state.selected = Some(row.path.clone());
            self.clicked = true;
        } else if outcome.clicked() {
            self.state.selected = Some(row.path.clone());
            self.clicked = true;
        }
        if outcome.finished {
            self.finished = true;
        }

        let active = self.state.selected.as_deref() == Some(row.path.as_str())
            || self.ui.memory(|memory| memory.has_focus(row.id));
        let label = FieldLabel::new(self.label_id(&row.path), &row.name, self.field_style);
        label.paint(self.ui, row.label_paint, active);
        let _ = row.label.response.on_hover_text(row.path);
    }

    fn take_number(&mut self, path: &str) -> Option<NumberEdit> {
        if self
            .state
            .edit
            .as_ref()
            .is_some_and(|edit| edit.path == path)
        {
            return self.state.edit.take().map(|edit| edit.number);
        }
        None
    }

    fn keep_number(&mut self, path: &str, number: Option<NumberEdit>) {
        if let Some(number) = number {
            self.state.edit = Some(FormEdit {
                path: path.to_owned(),
                number,
            });
        }
    }

    fn color(&mut self, name: &str, color: &mut Color, usage: ColorUsage) {
        let row = self.begin_row(name, false, false);
        let mut property = ColorProperty::new(*color, usage);
        let response = self.ui.put(
            row.value_rect,
            ColorField::new(row.id, &mut property, self.picker)
                .selected(row.active)
                .height(row.value_rect.height()),
        );
        // A pick's confirming click never reaches egui (the host masks it), so
        // the eyedropper reports its own end of gesture.
        let committed = eyedropper::take_committed(self.ui.ctx());
        let popup_open = is_color_picker_open(self.ui.ctx(), row.id);
        let changed = response.changed();
        if changed {
            *color = property.color;
        }
        if popup_open {
            self.clicked = true;
        }
        self.finish_row(
            row,
            FieldOutcome {
                response,
                finished: committed,
            },
            changed,
        );
    }
}

impl RowSink for FormRows<'_> {
    fn number<N: Number>(&mut self, name: &str, value: &mut N) {
        let row = self.begin_row(name, true, false);
        let editing = self
            .state
            .edit
            .as_ref()
            .is_some_and(|edit| edit.path == row.path);
        let active = row.active || editing;
        let number_field = NumberField::new(row.id, value, self.field_style)
            .selected(active)
            .label(&row.name)
            .dragged_by(&row.label);
        let mut number = self.take_number(&row.path);
        let outcome = number_field.show(self.ui, row.value_rect, &mut number);
        self.keep_number(&row.path, number);
        let changed = outcome.changed();
        self.finish_row(row, outcome, changed);
    }

    fn text(&mut self, name: &str, value: &mut String) {
        let row = self.begin_row(name, false, true);
        let outcome = TextField::new(row.id, value, self.field_style)
            .selected(row.active)
            .show(self.ui, row.value_rect);
        let changed = outcome.changed();
        self.finish_row(row, outcome, changed);
    }

    fn toggle(&mut self, name: &str, value: &mut bool) {
        let row = self.begin_row(name, false, false);
        let outcome = ToggleField::new(self.toggle_id(&row.path), value, self.field_style)
            .selected(row.active)
            .label(&row.name)
            .show(self.ui, row.value_rect);
        let changed = outcome.changed();
        self.finish_row(row, outcome, changed);
    }

    fn color(&mut self, name: &str, value: &mut Color) {
        FormRows::color(self, name, value, ColorUsage::LDR);
    }

    fn color_property(&mut self, name: &str, value: &mut ColorProperty) {
        let row = self.begin_row(name, false, false);
        let response = self.ui.put(
            row.value_rect,
            ColorField::new(row.id, value, self.picker)
                .selected(row.active)
                .height(row.value_rect.height()),
        );
        let committed = eyedropper::take_committed(self.ui.ctx());
        let popup_open = is_color_picker_open(self.ui.ctx(), row.id);
        let changed = response.changed();
        if popup_open {
            self.clicked = true;
        }
        self.finish_row(
            row,
            FieldOutcome {
                response,
                finished: committed,
            },
            changed,
        );
    }

    fn variant<E: StyleEnum>(&mut self, name: &str, value: &mut E) {
        let row = self.begin_row(name, false, false);
        let mut id = E::variant_id(value).to_owned();
        let outcome = VariantMenu::new(row.id, &mut id, E::variants(), self.field_style)
            .selected(row.active)
            .label(&row.name)
            .show(self.ui, row.value_rect);
        let applied = if outcome.changed() {
            match E::with_variant(&id) {
                Some(next) => {
                    *value = next;
                    true
                }
                // An unknown id is a no-op, exactly like a mismatched write.
                None => false,
            }
        } else {
            false
        };
        self.finish_row(row, outcome, applied);
    }

    fn group(&mut self, name: &str, body: impl FnOnce(&mut Self)) {
        if name.is_empty() {
            return body(self);
        }
        let path = self.child(name);
        let folded = self.state.collapsed.contains(&path);
        let (rect, _) = self
            .ui
            .allocate_exact_size(vec2(self.width, self.panel.group_height), Sense::hover());
        let response = self
            .ui
            .interact(rect, self.group_id(&path), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        let fill = if response.hovered() {
            self.panel.group_hover
        } else {
            self.panel.group_background
        };
        self.ui
            .painter()
            .rect_filled(rect, CornerRadius::ZERO, fill.to_egui());
        let arrow = pos2(
            rect.left() + self.panel.row_inset + self.depth as f32 * self.panel.group_indent + 4.0,
            rect.center().y,
        );
        paint_foldout_arrow(self.ui.painter(), arrow, folded, self.panel.group.to_egui());
        let text_left =
            rect.left() + self.panel.row_inset + 16.0 + self.depth as f32 * self.panel.group_indent;
        let text = TextStyle::sized(self.panel.group.to_egui(), self.panel.group_size);
        let galley = text.layout(self.ui, humanize(name));
        self.ui.painter().with_clip_rect(rect).galley(
            pos2(text_left, rect.center().y - galley.size().y * 0.5),
            galley,
            self.panel.group.to_egui(),
        );
        if response.clicked() {
            if folded {
                self.state.collapsed.remove(&path);
            } else {
                self.state.collapsed.insert(path.clone());
            }
            self.clicked = true;
        }
        let _ = response.on_hover_text(path.clone());
        // Re-read: the click above may have just opened the group.
        if self.state.collapsed.contains(&path) {
            return;
        }
        self.path.push(name.to_owned());
        self.depth += 1;
        body(self);
        self.depth -= 1;
        self.path.pop();
    }

    fn payload(&mut self, name: &str, body: impl FnOnce(&mut Self)) {
        self.path.push(name.to_owned());
        let previous = self.payload.replace(self.depth);
        body(self);
        self.payload = previous;
        self.path.pop();
    }

    fn list<T: StyleRow + Default>(&mut self, name: &str, values: &mut Vec<T>) {
        let path = self.child(name);
        let list_id = self.list_id(&path);
        let ids = ListChromeIds {
            header: self.group_id(&path),
            count: self.field_id(&path),
            add: self.add_id(&path),
            remove: self.remove_id(&path),
        };
        let folded = self.state.collapsed.contains(&path);
        let depth = self.depth;
        // A preview step or a drop slides whole blocks: the pass time drives the
        // easing, and the drag index (when this list is the dragged one) names
        // the block the ghost carries.
        let now = self.ui.input(|input| input.time);
        let drag_index = self.state.list_chrome.drag_index(list_id);
        let link = humanize(name);
        let header = ListChrome::header(
            self.ui,
            &mut self.state.list_chrome,
            self.chrome,
            list_id,
            ids,
            &link,
            values.len(),
            folded,
            depth,
            self.width,
            LIST_MAX as usize,
        );
        if header.clicked {
            self.state.selected = Some(path.clone());
            self.clicked = true;
        }
        if let Some(folded) = header.folded {
            if folded {
                self.state.collapsed.insert(path.clone());
            } else {
                self.state.collapsed.remove(&path);
            }
            self.clicked = true;
        }
        if let Some(edit) = header.edit {
            self.apply_list_edit(edit, values);
        }
        if header.finished {
            self.finished = true;
        }

        // The container opens under the header; the footer closes it at the
        // cursor after the last row, so the frame hugs the rows.
        ListChrome::open(&mut self.state.list_chrome, list_id, self.ui.cursor().top());

        if !self.state.collapsed.contains(&path) {
            // Enter the list's path: an element row is addressed below the list
            // (`proportional.0.name`), the same way the field table did.
            self.path.push(name.to_owned());
            for (index, item) in values.iter_mut().enumerate() {
                let element_path = format!("{path}.{index}");
                let selected = self.state.selected.as_deref() == Some(element_path.as_str());
                let folded = self.state.collapsed.contains(&element_path);
                if drag_index == Some(index) {
                    // The whole block follows the pointer on a floating layer;
                    // the parent reserves the same height, so every other block
                    // stays where the normal layout would put it.
                    self.drag_ghost(list_id, index, &element_path, item, depth, selected, folded);
                    continue;
                }
                // A preview step or a drop slides the whole block (band, rows and
                // ids together), the same way ListField moves a concrete list's
                // block. The band receives the applied offset so the drop math can
                // recover the block's normal layout slot.
                let shift = self.state.list_chrome.shift(list_id, index, now);
                if shift != 0.0 {
                    self.ui.add_space(shift);
                }
                self.element_block(
                    list_id,
                    index,
                    &element_path,
                    item,
                    depth,
                    selected,
                    folded,
                    shift,
                );
                if shift != 0.0 {
                    self.ui.add_space(-shift);
                }
            }
            self.path.pop();
        }

        let selected_element =
            selected_element_index(self.state.selected.as_deref(), &path, values.len());
        let footer = ListChrome::footer(
            self.ui,
            &mut self.state.list_chrome,
            self.chrome,
            list_id,
            ids,
            values.len(),
            selected_element,
            self.width,
            LIST_MAX as usize,
        );
        if footer.clicked {
            self.clicked = true;
        }
        if let Some(edit) = footer.edit {
            self.apply_list_edit(edit, values);
        }

        // Resolve an element drag once every band is laid out: the move edit
        // only exists after the drop, because the data order does not change
        // while the pointer is held. A floating copy of the dragged block is
        // only painted: it must not resolve (or re-target) the owner's drag.
        let outcome = if self.in_ghost {
            ListChromeOutcome::default()
        } else {
            ListChrome::finish(&mut *self.ui, &mut self.state.list_chrome)
        };
        if let Some(edit) = outcome.edit {
            self.apply_list_edit(edit, values);
        }
        if outcome.finished {
            self.finished = true;
        }
    }
}

impl<'a> FormRows<'a> {
    /// One list element block: its band and, unless folded, its rows.
    ///
    /// The live list and the floating copy of a dragged block both draw through
    /// here, so the ghost cannot drift from the block it replaces: only the `Ui`
    /// (tooltip layer, translated, disabled) and the parent-relative placement
    /// differ.
    #[allow(clippy::too_many_arguments)]
    fn element_block<T: StyleRow>(
        &mut self,
        list_id: Id,
        index: usize,
        element_path: &str,
        item: &mut T,
        depth: usize,
        selected: bool,
        folded: bool,
        shift: f32,
    ) {
        let band_ids = ListBandIds {
            fold: self.group_id(element_path),
            select: self.label_id(element_path),
            handle: self.handle_id(element_path),
        };
        let band = ListChrome::band(
            self.ui,
            &mut self.state.list_chrome,
            self.chrome,
            list_id,
            band_ids,
            index,
            &format!("Element {index}"),
            shift,
            depth + 1,
            self.width,
            self.chrome.band_fill,
            selected,
            folded,
        );
        if band.pressed || band.drag_started || band.selected {
            self.state.selected = Some(element_path.to_owned());
            self.clicked = true;
        }
        if let Some(folded) = band.folded {
            if folded {
                self.state.collapsed.insert(element_path.to_owned());
            } else {
                self.state.collapsed.remove(element_path);
            }
            self.clicked = true;
        }
        if self.state.collapsed.contains(element_path) {
            return;
        }
        let previous = self.element.replace(Element {
            depth: depth + 1,
            row: 0,
        });
        let outer = self.depth;
        self.depth = depth + 2;
        // The element is already a band of its own: its struct row is
        // transparent, and its tokens are addressed below the index
        // (`proportional.0.name`). A second group band here would mean
        // two widgets with the same id and a stray header row.
        self.path.push(index.to_string());
        StyleRow::row(item, "", self);
        self.path.pop();
        self.depth = outer;
        self.element = previous;
    }

    /// Draw the dragged element as a floating block and reserve its layout slot.
    ///
    /// The whole block — band and rows — is painted into a child `Ui` on the
    /// tooltip layer at its normal layout position, then translated to the
    /// pointer as a visual transform: layout is untouched, so the following
    /// blocks keep their slots and the drop math still sees the real heights.
    /// The parent advances by the child's rect, exactly what the normal block
    /// would have consumed. The copy is disabled (half-opacity, no input): the
    /// pointer belongs to the drag, not to the widgets under it.
    #[allow(clippy::too_many_arguments)]
    fn drag_ghost<T: StyleRow>(
        &mut self,
        list_id: Id,
        index: usize,
        element_path: &str,
        item: &mut T,
        depth: usize,
        selected: bool,
        folded: bool,
    ) {
        let block_top = self.ui.cursor().top();
        let Some(pointer) = self.ui.ctx().pointer_interact_pos() else {
            self.ui.add_space(self.chrome.row_height);
            return;
        };
        let Some(offset) = self
            .state
            .list_chrome
            .drag_offset(list_id, block_top, pointer)
        else {
            // The drag ended between this pass's input and the draw (a lost
            // window): keep the slot so the list does not jump for a frame.
            self.ui.add_space(self.chrome.row_height);
            return;
        };

        let layer = LayerId::new(Order::Tooltip, Id::new(("kairos.list.block", list_id)));
        let mut child = self.ui.new_child(UiBuilder::new().layer_id(layer));
        // The floating copy escapes the list's scroll viewport: clipping it to
        // the panel/window is what lets a half-scrolled block drag whole.
        child.set_clip_rect(self.ui.ctx().content_rect());
        child.disable();

        let state = &mut *self.state;
        let path = self.path.clone();
        let panel = self.panel;
        let field_style = self.field_style;
        let chrome = self.chrome;
        let picker = self.picker;
        let seed = self.seed;
        let width = self.width;
        let rows = self.rows;
        child.with_visual_transform(
            egui::emath::TSTransform::from_translation(vec2(0.0, offset)),
            |ui| {
                let mut ghost = FormRows {
                    ui,
                    panel,
                    field_style,
                    chrome,
                    picker,
                    state,
                    seed,
                    width,
                    path,
                    depth: depth + 2,
                    rows,
                    payload: None,
                    element: Some(Element {
                        depth: depth + 1,
                        row: 0,
                    }),
                    in_ghost: true,
                    clicked: false,
                    finished: false,
                    changed: false,
                };
                ghost.element_block(
                    list_id,
                    index,
                    element_path,
                    item,
                    depth,
                    selected,
                    folded,
                    0.0,
                );
            },
        );
        // The child laid out exactly where the block would have been drawn;
        // taking its rect keeps the list geometry identical.
        self.ui.advance_cursor_after_rect(child.min_rect());
    }

    fn apply_list_edit<T: Default>(&mut self, edit: ListChromeEdit, values: &mut Vec<T>) {
        match edit {
            ListChromeEdit::SetLen { len, .. } => {
                values.resize_with(len.min(LIST_MAX as usize), T::default);
            }
            ListChromeEdit::Insert { index, .. } => {
                let at = index.min(values.len());
                values.insert(at, T::default());
            }
            ListChromeEdit::Remove { index, .. } => {
                if index < values.len() {
                    values.remove(index);
                }
            }
            ListChromeEdit::Move { from, to, .. } => {
                if from < values.len() && to < values.len() && from != to {
                    let item = values.remove(from);
                    values.insert(to, item);
                }
            }
        }
        self.state.dirty = true;
        self.state.edit = None;
        self.finished = true;
    }
}

/// The element index a selected field path belongs to, if any.
fn selected_element_index(selected: Option<&str>, list: &str, len: usize) -> Option<usize> {
    let rest = selected?
        .strip_prefix(list)?
        .strip_prefix('.')?
        .split('.')
        .next()?;
    let index = rest.parse::<usize>().ok()?;
    (index < len).then_some(index)
}

fn paint_foldout_arrow(
    painter: &egui::Painter,
    center: egui::Pos2,
    collapsed: bool,
    color: egui::Color32,
) {
    let points = if collapsed {
        vec![
            center + vec2(-2.5, -4.0),
            center + vec2(-2.5, 4.0),
            center + vec2(2.5, 0.0),
        ]
    } else {
        vec![
            center + vec2(-4.0, -2.5),
            center + vec2(4.0, -2.5),
            center + vec2(0.0, 2.5),
        ]
    };
    painter.add(egui::Shape::convex_polygon(
        points,
        color,
        egui::Stroke::NONE,
    ));
}

/// The color field's usage, without exposing the widget's type at every call.
pub use kairos_color::ColorUsage as FormColorUsage;
#[cfg(test)]
mod tests {
    use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect, vec2};

    use super::*;
    use kairos_editor_widgets::style::Style;

    use crate::shared_style::{ColorPickerSkin, EditorFieldStyle};
    use crate::style_row::StyleRow;

    #[derive(Clone, Debug, PartialEq, StyleRow)]
    struct Inner {
        size: f32,
    }

    #[derive(Clone, Debug, PartialEq, StyleRow)]
    struct TestStyle {
        height: f32,
        enabled: bool,
        name: String,
        inner: Inner,
        items: Vec<f32>,
    }

    fn style() -> TestStyle {
        TestStyle {
            height: 20.0,
            enabled: true,
            name: "x".to_owned(),
            inner: Inner { size: 3.0 },
            items: vec![1.0, 2.0],
        }
    }

    struct Rig {
        ctx: egui::Context,
        panel: PreferencesPanelStyle,
        field: FieldStyle,
        chrome: ListChromeStyle,
        picker: ColorPickerStyle,
        state: FormState,
        seed: Id,
    }

    impl Rig {
        fn new() -> Self {
            let field = EditorFieldStyle::dark().tokens();
            let chrome = ListChromeStyle::from_field(&field);
            Self {
                ctx: egui::Context::default(),
                panel: PreferencesPanelStyle::default(),
                field,
                chrome,
                picker: ColorPickerSkin::dark().tokens(),
                state: FormState::default(),
                seed: Id::new("kairos.form.test"),
            }
        }

        fn frame(&mut self, style: &mut TestStyle, events: Vec<Event>) {
            let input = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(420.0, 620.0))),
                events,
                ..Default::default()
            };
            let Self {
                ctx,
                panel,
                field,
                chrome,
                picker,
                state,
                seed,
            } = self;
            let _ = ctx.run_ui(input, |ui| {
                let mut rows = FormRows::new(ui, *seed, panel, field, chrome, picker, state, 360.0);
                style.row("", &mut rows);
            });
        }
    }

    fn press(pos: Pos2) -> Event {
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::default(),
        }
    }

    fn release(pos: Pos2) -> Event {
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::default(),
        }
    }

    #[test]
    fn a_style_draws_its_rows_into_the_form() {
        let mut rig = Rig::new();
        let mut style = style();
        rig.frame(&mut style, Vec::new());
        rig.frame(&mut style, Vec::new());

        assert!(!rig.state.dirty, "nothing changed without input");
    }

    /// The page model: one page slot, `dyn StylePage`, no type at the call
    /// site. The same generated walk drives it.
    #[test]
    fn a_boxed_page_draws_the_same_rows() {
        let mut rig = Rig::new();
        let mut page: Box<dyn crate::style_row::StylePage> = Box::new(style());
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(420.0, 620.0))),
            ..Default::default()
        };
        let Rig {
            ctx,
            panel,
            field,
            chrome,
            picker,
            state,
            seed,
        } = &mut rig;
        let _ = ctx.run_ui(input, |ui| {
            let mut rows = FormRows::new(ui, *seed, panel, field, chrome, picker, state, 360.0);
            page.draw_page(&mut rows);
        });
    }

    #[test]
    fn clicking_a_toggle_writes_back_its_token() {
        let mut rig = Rig::new();
        let mut style = style();
        rig.frame(&mut style, Vec::new());

        let id = rig.seed.with(("toggle", "enabled"));
        let rect = rig
            .ctx
            .read_response(id)
            .expect("the toggle was placed")
            .rect;
        let at = rect.center();
        rig.frame(&mut style, vec![Event::PointerMoved(at)]);
        rig.frame(&mut style, vec![press(at)]);
        rig.frame(&mut style, vec![release(at)]);

        assert!(!style.enabled, "the copy took the click");
        assert!(rig.state.dirty, "the page is told the copy changed");
    }
}
