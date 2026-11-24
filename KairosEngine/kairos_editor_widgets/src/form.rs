//! The rows of a form: one token per row, drawn by the widget that owns the
//! row grid.
//!
//! A value tree walks itself here ([`StyleRow`]): a token becomes a row, a
//! nested struct a foldout band, an enum a menu row plus its payload rows, a
//! `Vec<T>` a header plus one block per element. The walk knows nothing about
//! layout, selection or editing: it only names rows and hands out its leaves.
//!
//! [`FormRows`] is the drawing half: the two-column geometry, the zebra bands,
//! the foldout bands, the list chrome and its drag ghost, the selection, the
//! edit buffers and the dirty/changed flags the caller turns into writes. The
//! cross-pass state is [`FormState`], owned by the caller and handed back every
//! pass, the way [`NumberEdit`] is.
//!
//! The widget knows no domain types: a form value is a plain Rust value and the
//! colours are the shared [`Color`]. The component layer keeps what is not a
//! widget concern — which owner a page belongs to, when the edited copy is
//! handed back, how the file is persisted: `kairos_editor_component`'s
//! `StylePage`.
//!
//! The appearance is the widget's own skin ([`FormStyle`]), not a panel's:
//! `kairos_editor_component` owns the serializable skin (`EditorFormStyle`) and
//! projects it onto these tokens at the call site, so every form row in the
//! editor looks the same and one file restyles them all.

use std::collections::HashSet;

use egui::{
    Color32, CornerRadius, CursorIcon, Id, LayerId, Order, Rect, Sense, Ui, UiBuilder, pos2, vec2,
};
use kairos_color::{Color, ColorUsage};

use crate::color_picker::{
    ColorField, ColorPickerStyle, ColorProperty, eyedropper, is_color_picker_open,
};
use crate::field::{
    FieldLabel, FieldLabelResponse, FieldOutcome, FieldStyle, ListBandIds, ListChrome,
    ListChromeEdit, ListChromeIds, ListChromeOutcome, ListChromeState, ListChromeStyle, Number,
    NumberEdit, NumberField, TextField, ToggleField, VariantMenu,
};
use crate::text::TextStyle;

/// An enum token: its variants, the active one, and how to rebuild a variant.
///
/// `#[derive(StyleRow)]` generates this for an enum, so the menu labels, the
/// row and the rebuild path cannot drift from the declaration. The enum also
/// derives `strum::VariantNames`: that is the one table of names the
/// `VariantMenu` reads. The id written to RON is the Rust variant ident; do not
/// override the strum name (`#[strum(serialize = ...)]`) on a style enum, or the
/// menu and the stored id would diverge.
///
/// A variant with named fields is a *payload*: `StyleRow` draws its fields under
/// the enum's menu row, in a payload scope. A single-field tuple variant is
/// transparent: its payload's fields are the enum's rows directly, with no
/// header for the unnamed field. Switching variants rebuilds the new one from
/// `Default`, which is the value semantics of a Rust enum.
///
/// A type from another crate cannot take the derive; if a form needs one, declare
/// a parallel enum and project it onto the runtime value (the editor font list's
/// `EditorFontPriority` -> `FontPriority` is the example).
///
/// A style enum that a typed editor should own also derives strum's `Display`
/// and `EnumIter`; the Preferences page needs neither, because `VariantMenu`
/// edits the id against [`StyleEnum::variants`].
pub trait StyleEnum: Sized + strum::VariantNames {
    /// Every variant name, in declaration order.
    fn variants() -> &'static [&'static str] {
        Self::VARIANTS
    }

    /// The active variant's stable id.
    fn variant_id(&self) -> &'static str;

    /// Rebuild the enum as the named variant, with a default payload.
    ///
    /// `None` for an id this enum does not have: an unknown choice leaves the
    /// value untouched rather than panicking on a stale menu.
    fn with_variant(id: &str) -> Option<Self>;
}

/// The drawing side of the row protocol.
///
/// One method per leaf kind, plus the three scopes a form tree can open: a group
/// (a foldout band), an enum payload (rows laid out under the enum's menu
/// button) and a list (a header plus one block per element). The sink keeps the
/// current path, so the generated walk never joins strings.
pub trait RowSink {
    /// One number box, over the token's own type.
    fn number<N: Number>(&mut self, name: &str, value: &mut N);

    /// One single-line text box.
    fn text(&mut self, name: &str, value: &mut String);

    /// One checkbox.
    fn toggle(&mut self, name: &str, value: &mut bool);

    /// One colour swatch. The token is an LDR colour.
    fn color(&mut self, name: &str, value: &mut Color);

    /// One colour that carries its own usage (HDR or LDR).
    fn color_property(&mut self, name: &str, value: &mut ColorProperty);

    /// One enum row: a menu over `E`'s variants, writing the chosen variant
    /// back into the value.
    fn variant<E: StyleEnum>(&mut self, name: &str, value: &mut E);

    /// One nested group, drawn as a foldout band.
    ///
    /// An empty `name` is a transparent scope: the root of a form, or a
    /// single-field tuple payload whose fields belong to the enum's own path.
    fn group(&mut self, name: &str, body: impl FnOnce(&mut Self));

    /// The active variant's payload rows, under the enum row `name`.
    ///
    /// Rows inside hang under the enum's menu button instead of starting a
    /// new indent level, and they are addressed below the enum's own path
    /// (`mode.radius`). Only variants with rows call in here, so a unit variant
    /// simply draws its menu row.
    fn payload(&mut self, name: &str, body: impl FnOnce(&mut Self));

    /// One list token: a header with the element count, then one block per
    /// element. The vector is resized, reordered and edited in place.
    fn list<T: StyleRow + Default>(&mut self, name: &str, values: &mut Vec<T>);
}

/// One value that draws itself as form rows.
///
/// `#[derive(StyleRow)]` generates the implementation for a style struct or
/// enum. Hand-written implementations are the leaves (numbers, text, toggles,
/// colours), project types of other crates into a local enum, and any type whose
/// tokens are not its own fields.
pub trait StyleRow {
    /// Draw this token — or this group of tokens — under `name`.
    fn row(&mut self, name: &str, sink: &mut impl RowSink);
}

macro_rules! number_rows {
    ($($ty:ty),* $(,)?) => {
        $(
            impl StyleRow for $ty {
                fn row(&mut self, name: &str, sink: &mut impl RowSink) {
                    sink.number(name, self);
                }
            }
        )*
    };
}

// Each integer keeps its own type: the sink draws the concrete `NumberField<N>`,
// so no widened integer and no type-bound clamp is involved here.
number_rows!(f32, f64);
number_rows!(i8, i16, i32, i64, isize);
number_rows!(u8, u16, u32, u64, usize);

impl StyleRow for bool {
    fn row(&mut self, name: &str, sink: &mut impl RowSink) {
        sink.toggle(name, self);
    }
}

impl StyleRow for String {
    fn row(&mut self, name: &str, sink: &mut impl RowSink) {
        sink.text(name, self);
    }
}

impl StyleRow for Color {
    fn row(&mut self, name: &str, sink: &mut impl RowSink) {
        sink.color(name, self);
    }
}

impl StyleRow for ColorProperty {
    fn row(&mut self, name: &str, sink: &mut impl RowSink) {
        sink.color_property(name, self);
    }
}

/// A list token is one row (its header) plus one sub-walk per element; the
/// sink decides where the element blocks sit and how a drag reorders them.
impl<T: StyleRow + Default> StyleRow for Vec<T> {
    fn row(&mut self, name: &str, sink: &mut impl RowSink) {
        sink.list(name, self);
    }
}

/// The visual tokens of one form page.
///
/// The row grid belongs to the widget, not to a panel: the Preferences editor
/// and a future object inspector draw the same rows from these tokens. Field
/// *values* keep [`FieldStyle`] and list element blocks keep [`ListChromeStyle`];
/// this struct owns only the page-level row geometry, the two zebra greys and
/// the foldout header.
pub struct FormStyle {
    /// Row band fill on the first, third, ... (even, zero-based) rows.
    pub row_odd: Color32,
    /// Row band fill on the second, fourth, ... rows.
    pub row_even: Color32,
    /// Height of one field row.
    pub row_height: f32,
    /// Left and right content inset inside a row band.
    pub row_inset: f32,
    /// Ideal width of a row's label column; the value box starts after it.
    pub label_width: f32,
    /// Minimum width left for the value box; below it the label column shrinks
    /// instead of the box clipping off the right edge.
    pub field_width: f32,
    /// Gap between the label column and the value box.
    pub label_gap: f32,
    /// Foldout header text.
    pub group_text: TextStyle,
    /// Foldout header band at rest and under the pointer.
    pub group_fill: Color32,
    pub group_hover: Color32,
    /// Height of a foldout header row.
    pub group_height: f32,
    /// Indent of one nesting level in the form tree.
    pub group_indent: f32,
}

impl Default for FormStyle {
    fn default() -> Self {
        Self::dark()
    }
}

impl FormStyle {
    /// The editor's dark form tokens, matching the shipped `EditorForm.ron`.
    pub fn dark() -> Self {
        Self {
            row_odd: Color32::from_rgb(0x38, 0x38, 0x38),
            row_even: Color32::from_rgb(0x33, 0x33, 0x33),
            row_height: 20.0,
            row_inset: 8.0,
            label_width: 170.0,
            field_width: 120.0,
            label_gap: 8.0,
            group_text: TextStyle::sized(Color32::from_rgb(0xDC, 0xDC, 0xDC), 12.0),
            group_fill: Color32::from_rgb(0x3C, 0x3C, 0x3C),
            group_hover: Color32::from_rgb(0x46, 0x46, 0x46),
            group_height: 20.0,
            group_indent: 16.0,
        }
    }

    /// The band fill for a zero-based row index: two greys, alternating.
    pub fn row_fill(&self, row: usize) -> Color32 {
        if row.is_multiple_of(2) {
            self.row_odd
        } else {
            self.row_even
        }
    }
}

/// A numeric token being typed into, keyed by its dotted path.
pub struct FormEdit {
    pub path: String,
    pub number: NumberEdit,
}

/// The form's state across passes: what is folded, selected and being edited.
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

/// The rows of one form, drawn into one `Ui`.
pub struct FormRows<'a> {
    ui: &'a mut Ui,
    style: &'a FormStyle,
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
    /// The form-level zebra counter.
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
    /// A value changed this pass (the caller sends one whole-value write).
    changed: bool,
}

impl<'a> FormRows<'a> {
    /// One form's rows. `seed` salts every widget id; `width` is the row width
    /// the caller reserved (scroll bar and padding already removed); `list_max`
    /// caps how far a list token's count controls may grow it.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        ui: &'a mut Ui,
        seed: Id,
        style: &'a FormStyle,
        field_style: &'a FieldStyle,
        chrome: &'a ListChromeStyle,
        picker: &'a ColorPickerStyle,
        state: &'a mut FormState,
        width: f32,
    ) -> Self {
        // One form pass: forget the list bands the previous pass recorded, so
        // a drag resolves against this pass's layout.
        ListChrome::begin(&mut state.list_chrome);
        Self {
            ui,
            style,
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

    /// A row consumed this pass's click (the caller uses it to clear the
    /// selection only when nothing else did).
    pub fn clicked(&self) -> bool {
        self.clicked
    }

    /// A typed edit ended: the caller must persist now instead of waiting for
    /// the pointer to come up.
    pub fn finished(&self) -> bool {
        self.finished
    }

    /// A value changed this pass: the caller hands the owner a new copy.
    pub fn changed(&self) -> bool {
        self.changed
    }

    /// This form has unwritten edits (persists across passes until committed).
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
            self.style.row_height
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
            self.style.row_fill(self.rows)
        };
        self.ui
            .painter()
            .rect_filled(rect, CornerRadius::ZERO, fill);

        let (row_inset, group_indent) = if in_element {
            (self.chrome.row_inset, self.chrome.group_indent)
        } else {
            (self.style.row_inset, self.style.group_indent)
        };
        let content_left = rect.left() + row_inset;
        let content_right = (rect.right() - row_inset).max(content_left + 1.0);
        let content_width = content_right - content_left;
        let max_label = (content_width - self.style.label_gap - self.style.field_width).max(48.0);
        let label_width = self
            .style
            .label_width
            .min(max_label)
            .min((content_width - self.style.label_gap - 1.0).max(0.0));
        let value_left = content_left + label_width + self.style.label_gap;
        let (label_left, value_left, value_width) = if self.payload.is_some() {
            let base_width = (content_right - value_left).max(1.0);
            let sub_label = (base_width * 0.42).clamp(48.0, self.style.label_width);
            let sub_value = value_left + sub_label + self.style.label_gap;
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
            pos2(value_left - self.style.label_gap, rect.bottom()),
        );
        let label_x = if let Some(base) = self.payload {
            label_left + self.depth.saturating_sub(base) as f32 * self.style.group_indent
        } else if let Some(element) = self.element {
            let x0 =
                rect.left() + row_inset + element.depth.saturating_sub(1) as f32 * group_indent;
            x0 + self.chrome.gutter_width
                + 4.0
                + 16.0
                + self.depth.saturating_sub(element.depth) as f32 * group_indent
        } else {
            content_left + 16.0 + self.depth as f32 * self.style.group_indent
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
            .allocate_exact_size(vec2(self.width, self.style.group_height), Sense::hover());
        let response = self
            .ui
            .interact(rect, self.group_id(&path), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        let fill = if response.hovered() {
            self.style.group_hover
        } else {
            self.style.group_fill
        };
        self.ui
            .painter()
            .rect_filled(rect, CornerRadius::ZERO, fill);
        let arrow = pos2(
            rect.left() + self.style.row_inset + self.depth as f32 * self.style.group_indent + 4.0,
            rect.center().y,
        );
        paint_foldout_arrow(
            self.ui.painter(),
            arrow,
            folded,
            self.style.group_text.text_color(),
        );
        let text_left =
            rect.left() + self.style.row_inset + 16.0 + self.depth as f32 * self.style.group_indent;
        let text = self.style.group_text.clone();
        let galley = text.layout(self.ui, humanize(name));
        self.ui.painter().with_clip_rect(rect).galley(
            pos2(text_left, rect.center().y - galley.size().y * 0.5),
            galley,
            self.style.group_text.text_color(),
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
        let style = self.style;
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
                    style,
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
                values.resize_with(len, T::default);
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

/// `toolbar_bg` -> `"Toolbar bg"`: the label a row is shown under.
///
/// The label comes from the field name, so a row cannot be shown under a
/// hand-written label that drifted; this is the one place the naming rule lives.
pub fn humanize(path: &str) -> String {
    let leaf = path.rsplit('.').next().unwrap_or(path);
    let mut name = String::with_capacity(leaf.len());
    for (index, word) in leaf.split('_').filter(|word| !word.is_empty()).enumerate() {
        if index > 0 {
            name.push(' ');
        }
        if index == 0 {
            // Sentence case: only the first word is capitalized, because these
            // labels sit in a dense form where Title Case on every word reads
            // like an acronym.
            let mut chars = word.chars();
            if let Some(first) = chars.next() {
                name.extend(first.to_uppercase());
                name.push_str(chars.as_str());
            }
        } else {
            name.push_str(word);
        }
    }
    name
}

#[cfg(test)]
mod tests {
    use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect, vec2};

    use super::*;

    // A style tree without the derive: the protocol is hand-written so this
    // crate keeps no dependency on the component crate's macro, and the test
    // pins the walk the derive generates (a group per struct, one row per
    // field, in declaration order).
    #[derive(Clone, Debug, PartialEq)]
    struct Inner {
        size: f32,
    }

    impl StyleRow for Inner {
        fn row(&mut self, name: &str, sink: &mut impl RowSink) {
            sink.group(name, |sink| {
                StyleRow::row(&mut self.size, "size", sink);
            });
        }
    }

    #[derive(Clone, Debug, PartialEq)]
    struct TestStyle {
        height: f32,
        enabled: bool,
        name: String,
        inner: Inner,
        items: Vec<f32>,
    }

    impl StyleRow for TestStyle {
        fn row(&mut self, name: &str, sink: &mut impl RowSink) {
            sink.group(name, |sink| {
                StyleRow::row(&mut self.height, "height", sink);
                StyleRow::row(&mut self.enabled, "enabled", sink);
                StyleRow::row(&mut self.name, "name", sink);
                StyleRow::row(&mut self.inner, "inner", sink);
                StyleRow::row(&mut self.items, "items", sink);
            });
        }
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
        form: FormStyle,
        field: FieldStyle,
        chrome: ListChromeStyle,
        picker: ColorPickerStyle,
        state: FormState,
        seed: Id,
    }

    impl Rig {
        fn new() -> Self {
            let field = FieldStyle::default();
            let chrome = ListChromeStyle::from_field(&field);
            Self {
                ctx: egui::Context::default(),
                form: FormStyle::dark(),
                field,
                chrome,
                picker: ColorPickerStyle::default(),
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
                form,
                field,
                chrome,
                picker,
                state,
                seed,
            } = self;
            let _ = ctx.run_ui(input, |ui| {
                let mut rows =
                    FormRows::new(ui, *seed, form, field, chrome, picker, state, 360.0);
                StyleRow::row(style, "", &mut rows);
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

    /// The whole pass is a widget: the caller keeps the state, the same walk
    /// draws the same rows and the same ids every pass.
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
        assert!(rig.state.dirty, "the caller is told the copy changed");
    }

    #[test]
    fn row_bands_alternate_between_two_greys() {
        let form = FormStyle::dark();

        assert_eq!(form.row_fill(0), form.row_odd);
        assert_eq!(form.row_fill(1), form.row_even);
        assert_eq!(form.row_fill(2), form.row_odd);
        assert_ne!(form.row_odd, form.row_even);
    }

    #[test]
    fn the_label_is_derived_from_the_field_name() {
        assert_eq!(humanize("toolbar_bg"), "Toolbar bg");
        assert_eq!(humanize("icons.warning"), "Warning");
        assert_eq!(humanize("x"), "X");
    }
}
