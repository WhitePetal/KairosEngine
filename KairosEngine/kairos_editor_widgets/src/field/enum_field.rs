//! The enum field: Unity's popup field, and the menu it opens.
//!
//! Two controls share one menu:
//!
//! * [EnumField] borrows the actual enum mutably, like every other field
//!   borrows its value. It gets the labels from the enum's
//!   [strum::Display](https://docs.rs/strum/latest/strum/derive.Display.html)
//!   and the variants from [strum::IntoEnumIterator]; choosing a row writes the
//!   new variant into the borrowed value. There is no field-side trait to
//!   implement.
//! * [VariantMenu] is the style-token view: the Preferences page has a variant
//!   id and a static variant table, not the owner's typed enum. It edits the id
//!   string and leaves the page to send its own style action.
//!
//! Both use the same trigger and popup: the project's [crate::button::Button]
//! with the color picker's popup-field tokens, and the editor's shared
//! [crate::menu::MenuStyle] menu.
//!
//! A variant with named fields is drawn by the panel as rows after this one:
//! the menu button and its payload form one block, so the enum stays a single
//! widget here.

use std::fmt::Display;

use egui::{
    Color32, CursorIcon, Id, Rect, Response, Sense, Ui, WidgetInfo, WidgetType,
    containers::menu::MenuConfig, vec2,
};
use strum::IntoEnumIterator;

use super::{FieldOutcome, style::FieldStyle};
use crate::button::{Button, ButtonStyle};
use crate::text::TextStyle;

/// Stable id of the menu popup, derived from the field's id.
pub fn enum_popup_id(field: Id) -> Id {
    field.with("popup")
}

/// Stable id of one row in the menu.
pub fn enum_item_id(field: Id, index: usize) -> Id {
    field.with(("enum-item", index))
}

/// A popup field over an actual enum value.
///
/// The value is borrowed mutably, the same way [super::NumberField] borrows a
/// number and [super::TextField] borrows a string: selecting a row assigns the
/// new variant to the source value. Variant labels come from strum's Display
/// (the variant name by default), and data-carrying variants are rebuilt with
/// their default payload by strum's EnumIter.
pub struct EnumField<'a, E> {
    id: Id,
    value: &'a mut E,
    style: &'a FieldStyle,
    selected_row: bool,
    label: &'a str,
}

impl<'a, E> EnumField<'a, E>
where
    E: IntoEnumIterator + Display,
{
    pub fn new(id: Id, value: &'a mut E, style: &'a FieldStyle) -> Self {
        Self {
            id,
            value,
            style,
            selected_row: false,
            label: "",
        }
    }

    /// The row that owns this field is selected: draw it active even without
    /// focus.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected_row = selected;
        self
    }

    /// Accessibility name (the inspector's row label).
    pub fn label(mut self, label: &'a str) -> Self {
        self.label = label;
        self
    }

    pub fn show(self, ui: &mut Ui, rect: Rect) -> FieldOutcome {
        let Self {
            id,
            value,
            style,
            selected_row,
            label,
        } = self;
        // Compare discriminants, not values: a data-carrying variant's payload
        // is edited state, so EnumIter's default payload must not hide which
        // variant is active.
        let current_label = value.to_string();
        let selected = E::iter()
            .position(|variant| std::mem::discriminant(&variant) == std::mem::discriminant(&*value))
            .unwrap_or(0);
        let labels: Vec<String> = E::iter().map(|variant| variant.to_string()).collect();
        let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        let (mut response, chosen) = show_menu(
            ui,
            id,
            &label_refs,
            selected,
            &current_label,
            style,
            selected_row,
            label,
            rect,
        );
        if let Some(index) = chosen
            && index != selected
            && let Some(variant) = E::iter().nth(index)
        {
            *value = variant;
            response.mark_changed();
        }
        FieldOutcome {
            response,
            finished: false,
        }
    }
}

/// A popup field over a style token's variant names.
///
/// The Preferences page owns the variant id, not the enum value (its field
/// table is a style-value snapshot); this control edits the id string in place.
/// It exists so the page can reuse the enum row without pretending it has the
/// typed enum; a typed editor uses [EnumField].
pub struct VariantMenu<'a> {
    id: Id,
    selected: &'a mut String,
    variants: &'static [&'static str],
    style: &'a FieldStyle,
    selected_row: bool,
    label: &'a str,
}

impl<'a> VariantMenu<'a> {
    pub fn new(
        id: Id,
        selected: &'a mut String,
        variants: &'static [&'static str],
        style: &'a FieldStyle,
    ) -> Self {
        Self {
            id,
            selected,
            variants,
            style,
            selected_row: false,
            label: "",
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected_row = selected;
        self
    }

    pub fn label(mut self, label: &'a str) -> Self {
        self.label = label;
        self
    }

    pub fn show(self, ui: &mut Ui, rect: Rect) -> FieldOutcome {
        let Self {
            id,
            selected,
            variants,
            style,
            selected_row,
            label,
        } = self;
        let current = variants
            .iter()
            .position(|variant| *variant == selected.as_str())
            .unwrap_or(0);
        let current_label = variants.get(current).copied().unwrap_or("");
        let (mut response, chosen) = show_menu(
            ui,
            id,
            variants,
            current,
            current_label,
            style,
            selected_row,
            label,
            rect,
        );
        if let Some(index) = chosen
            && index != current
            && let Some(variant) = variants.get(index)
        {
            *selected = (*variant).to_owned();
            response.mark_changed();
        }
        FieldOutcome {
            response,
            finished: false,
        }
    }
}

/// The shared trigger and popup. Returns the trigger response and the row the
/// user chose, if any; the caller decides what a choice writes.
#[allow(clippy::too_many_arguments)]
fn show_menu(
    ui: &mut Ui,
    id: Id,
    labels: &[&str],
    selected: usize,
    current: &str,
    style: &FieldStyle,
    selected_row: bool,
    label: &str,
    rect: Rect,
) -> (Response, Option<usize>) {
    let popup_id = enum_popup_id(id);
    let open = egui::Popup::is_id_open(ui.ctx(), popup_id);
    let active = selected_row || open || ui.memory(|memory| memory.has_focus(id));
    let hovered = ui.rect_contains_pointer(rect);
    let outline = if active {
        style.accent
    } else if hovered {
        style.edge_hover
    } else {
        style.menu_button_edge
    };
    let button_style = style.menu_button_style(outline);

    let selected = selected.min(labels.len().saturating_sub(1));
    // The button paints its label itself; keep it inside the box so a long
    // variant name cannot run under the caret or out of the value column.
    let text_width = (rect.width() - style.padding - 18.0).max(1.0);
    let current = elide(ui, current, &style.text, text_width);

    let response = ui
        .put(
            rect,
            Button::new(&current, &button_style)
                .id(id)
                .width(rect.width())
                .height(rect.height())
                .caret()
                .selected(open),
        )
        .on_hover_cursor(CursorIcon::PointingHand);
    let enabled = ui.is_enabled();
    let current_text = current.clone();
    if !response.clicked() {
        response.widget_info(|| {
            let mut info = WidgetInfo::new(WidgetType::ComboBox);
            info.enabled = enabled;
            info.label = Some(label.to_owned());
            info.current_text_value = Some(current_text.clone());
            info
        });
    }

    let mut chosen = None;
    let width = response.rect.width().max(style.menu.popup_min_width);
    egui::Popup::menu(&response)
        .style(style.menu.style_modifier())
        .info(
            egui::UiStackInfo::new(egui::UiKind::Menu)
                .with_tag_value(MenuConfig::MENU_CONFIG_TAG, style.menu.config()),
        )
        .gap(1.0)
        .width(width)
        .show(|ui| {
            let mut rows = |ui: &mut Ui| {
                for (index, variant) in labels.iter().enumerate() {
                    if show_item(ui, id, index, variant, style, selected) {
                        chosen = Some(index);
                        ui.close();
                    }
                }
            };
            let max_height = 10.0 * style.menu.item_height;
            if labels.len() > 10 {
                egui::ScrollArea::vertical()
                    .max_height(max_height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| rows(ui));
            } else {
                rows(ui);
            }
        });
    (response, chosen)
}

/// One menu row, full width. The row is a [Button] so it has the project's
/// hover/press/selected states, an explicit id for tests and hit-testing, and
/// the menu's own tokens rather than the value box's.
fn show_item(
    ui: &mut Ui,
    field: Id,
    index: usize,
    label: &str,
    style: &FieldStyle,
    selected: usize,
) -> bool {
    let menu = &style.menu;
    let height = menu.item_height;
    let width = ui.available_width();
    let (row, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let hovered = ui.rect_contains_pointer(row);
    let active = index == selected;
    let color = if active || hovered {
        Color32::WHITE
    } else {
        menu.text
    };
    let item_style = ButtonStyle::new(TextStyle::sized(color, menu.text_size))
        .fills(menu.item_fill, menu.item_hover_fill, menu.item_active_fill)
        .corner_radius(menu.item_corner_radius)
        .padding(menu.item_padding)
        .height(height);
    ui.put(
        row,
        Button::new(label, &item_style)
            .id(enum_item_id(field, index))
            .width(width)
            .height(height)
            .selected(active),
    )
    .on_hover_cursor(CursorIcon::PointingHand)
    .clicked()
}

/// Shorten a label with an ellipsis until it fits the button's text column.
fn elide(ui: &Ui, text: &str, style: &TextStyle, max_width: f32) -> String {
    if style.layout(ui, text).size().x <= max_width {
        return text.to_owned();
    }
    let mut end = text.len();
    while end > 0 {
        end = text[..end]
            .char_indices()
            .next_back()
            .map_or(0, |(index, _)| index);
        let candidate = format!("{}…", &text[..end]);
        if style.layout(ui, &candidate).size().x <= max_width {
            return candidate;
        }
    }
    "…".to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::testing::{button, click, input, rect, style};
    use egui::{Context, Event, Pos2, Stroke, pos2};

    static VARIANTS: &[&str] = &["First", "Push"];

    #[derive(Debug, Default, PartialEq, strum::EnumIter, strum::Display)]
    enum TestMode {
        #[default]
        First,
        Push,
    }

    #[derive(Debug, Default, PartialEq)]
    struct TestPayload {
        value: u32,
    }

    #[derive(Debug, Default, PartialEq, strum::EnumIter, strum::Display)]
    enum TestPayloadMode {
        #[default]
        First,
        Push(TestPayload),
    }

    fn click_in(ctx: &Context, at: Pos2, draw: &mut impl FnMut(&mut Ui)) {
        let mut moved = input();
        moved.events = vec![Event::PointerMoved(at)];
        let _ = ctx.run_ui(moved, &mut *draw);
        let mut press = input();
        press.events = vec![button(at, true)];
        let _ = ctx.run_ui(press, &mut *draw);
        let mut release = input();
        release.events = vec![button(at, false)];
        let _ = ctx.run_ui(release, &mut *draw);
    }

    fn painted_text(output: &egui::FullOutput) -> String {
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
        out.join("\n")
    }

    fn painted_fill(output: &egui::FullOutput, fill: Color32, at: Pos2) -> bool {
        fn walk(shape: &egui::Shape, fill: Color32, at: Pos2) -> bool {
            match shape {
                egui::Shape::Rect(rect) => rect.fill == fill && rect.rect.contains(at),
                egui::Shape::Vec(shapes) => shapes.iter().any(|shape| walk(shape, fill, at)),
                _ => false,
            }
        }
        output
            .shapes
            .iter()
            .any(|clipped| walk(&clipped.shape, fill, at))
    }

    #[test]
    fn the_box_shows_the_active_variant_label() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-field-enum-label");
        let mut value = TestMode::Push;

        let mut draw = |ui: &mut Ui| {
            EnumField::new(id, &mut value, &style).show(ui, rect());
        };
        let output = ctx.run_ui(input(), &mut draw);

        assert!(painted_text(&output).contains("Push"));
    }

    #[test]
    fn the_trigger_reads_as_the_project_popup_field() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-field-enum-trigger");
        let mut value = TestMode::First;

        let output = ctx.run_ui(input(), |ui| {
            EnumField::new(id, &mut value, &style).show(ui, rect());
        });

        assert!(
            painted_fill(&output, style.menu_button_fill, rect().center()),
            "the trigger should paint the popup-field fill"
        );
        let outlined = output.shapes.iter().any(|clipped| {
            matches!(
                &clipped.shape,
                egui::Shape::Rect(shape)
                    if shape.rect == rect() && shape.stroke == Stroke::new(1.0, style.menu_button_edge)
            )
        });
        assert!(
            outlined,
            "the trigger should be outlined like the color picker's"
        );
    }

    #[test]
    fn clicking_the_box_opens_a_menu_with_every_variant() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-field-enum-open");
        let mut value = TestMode::First;

        let mut draw = |ui: &mut Ui| {
            EnumField::new(id, &mut value, &style).show(ui, rect());
        };
        let _ = ctx.run_ui(input(), &mut draw);
        click(&ctx, rect().center(), &mut draw);
        let _ = ctx.run_ui(input(), &mut draw);
        let output = ctx.run_ui(input(), &mut draw);

        let text = painted_text(&output);
        assert!(text.contains("First"), "{text}");
        assert!(text.contains("Push"), "{text}");
        assert!(
            ctx.read_response(enum_item_id(id, 1)).is_some(),
            "both rows are real widgets"
        );
    }

    #[test]
    fn choosing_a_variant_writes_the_borrowed_enum() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-field-enum-choose");
        let mut value = TestMode::First;
        let mut changes = Vec::new();

        {
            let mut draw = |ui: &mut Ui| {
                let outcome = EnumField::new(id, &mut value, &style).show(ui, rect());
                changes.push(outcome.changed());
            };
            let _ = ctx.run_ui(input(), &mut draw);
            click(&ctx, rect().center(), &mut draw);
            // The menu settles its position on the pass after it opens, so let
            // it land before reading a row's rect.
            let _ = ctx.run_ui(input(), &mut draw);
            let _ = ctx.run_ui(input(), &mut draw);

            let item = ctx
                .read_response(enum_item_id(id, 1))
                .expect("the second row was laid out")
                .rect;
            click_in(&ctx, item.center(), &mut draw);
        }
        assert_eq!(value, TestMode::Push, "the choice lands in the source enum");
        assert!(
            !egui::Popup::is_id_open(&ctx, enum_popup_id(id)),
            "the menu closes on a choice"
        );
    }

    /// A payload is edited state: the active variant is matched by its variant
    /// name, not by comparing the whole value with EnumIter's default payload.
    #[test]
    fn a_modified_payload_still_shows_its_variant() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-field-enum-payload-state");
        let mut value = TestPayloadMode::Push(TestPayload { value: 7 });

        let output = ctx.run_ui(input(), |ui| {
            EnumField::new(id, &mut value, &style).show(ui, rect());
        });

        assert!(
            painted_text(&output).contains("Push"),
            "the active variant is found by name: {:?}",
            value
        );
    }

    /// Data-carrying variants are rebuilt from Default by strum's iterator;
    /// the widget writes the whole variant, payload included.
    #[test]
    fn choosing_a_tuple_variant_writes_its_default_payload() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-field-enum-payload");
        let mut value = TestPayloadMode::First;

        {
            let mut draw = |ui: &mut Ui| {
                EnumField::new(id, &mut value, &style).show(ui, rect());
            };
            let _ = ctx.run_ui(input(), &mut draw);
            click(&ctx, rect().center(), &mut draw);
            let _ = ctx.run_ui(input(), &mut draw);
            let _ = ctx.run_ui(input(), &mut draw);
            let item = ctx
                .read_response(enum_item_id(id, 1))
                .expect("the tuple variant row was laid out")
                .rect;
            click_in(&ctx, item.center(), &mut draw);
        }
        assert_eq!(value, TestPayloadMode::Push(TestPayload::default()));
    }

    #[test]
    fn choosing_the_active_variant_is_not_an_edit() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-field-enum-same");
        let mut value = TestMode::First;
        let mut changes = Vec::new();

        {
            let mut draw = |ui: &mut Ui| {
                let outcome = EnumField::new(id, &mut value, &style).show(ui, rect());
                changes.push(outcome.changed());
            };
            let _ = ctx.run_ui(input(), &mut draw);
            click(&ctx, rect().center(), &mut draw);
            let _ = ctx.run_ui(input(), &mut draw);
            let _ = ctx.run_ui(input(), &mut draw);
            let item = ctx
                .read_response(enum_item_id(id, 0))
                .expect("the first row was laid out")
                .rect;
            click_in(&ctx, item.center(), &mut draw);
        }

        assert_eq!(value, TestMode::First);
        assert!(!changes.iter().any(|changed| *changed));
        assert!(!egui::Popup::is_id_open(&ctx, enum_popup_id(id)));
    }

    #[test]
    fn clicking_outside_closes_the_menu() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-field-enum-outside");
        let mut value = TestMode::First;

        let mut draw = |ui: &mut Ui| {
            EnumField::new(id, &mut value, &style).show(ui, rect());
        };
        let _ = ctx.run_ui(input(), &mut draw);
        click(&ctx, rect().center(), &mut draw);
        let _ = ctx.run_ui(input(), &mut draw);
        assert!(egui::Popup::is_id_open(&ctx, enum_popup_id(id)));

        click(&ctx, pos2(10.0, 70.0), &mut draw);
        let _ = ctx.run_ui(input(), &mut draw);
        assert!(!egui::Popup::is_id_open(&ctx, enum_popup_id(id)));
    }

    #[test]
    fn the_menu_paints_the_shared_menu_skin() {
        let ctx = Context::default();
        ctx.all_styles_mut(|style| style.animation_time = 0.0);
        let style = style();
        let id = Id::new("kairos-field-enum-skin");
        let mut value = TestMode::First;

        let mut draw = |ui: &mut Ui| {
            EnumField::new(id, &mut value, &style).show(ui, rect());
        };
        let _ = ctx.run_ui(input(), &mut draw);
        click(&ctx, rect().center(), &mut draw);
        let _ = ctx.run_ui(input(), &mut draw);
        let output = ctx.run_ui(input(), &mut draw);

        assert!(
            painted_fill(&output, style.menu.popup_fill, pos2(70.0, 20.0)),
            "the popup should paint the menu fill"
        );
        assert!(
            painted_fill(&output, style.menu.item_active_fill, pos2(100.0, 30.0)),
            "the active row should use the menu highlight"
        );
    }

    #[test]
    fn hovering_the_box_shows_the_hand_cursor() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-field-enum-cursor");
        let mut value = TestMode::First;

        let _ = ctx.run_ui(input(), |ui| {
            EnumField::new(id, &mut value, &style).show(ui, rect());
        });
        let mut hover = input();
        hover.events = vec![Event::PointerMoved(rect().center())];
        let output = ctx.run_ui(hover, |ui| {
            EnumField::new(id, &mut value, &style).show(ui, rect());
        });

        assert_eq!(
            output.platform_output.cursor_icon,
            egui::CursorIcon::PointingHand
        );
    }

    /// The style-token control edits the id string in place, without pretending
    /// it owns the typed enum.
    #[test]
    fn the_variant_menu_writes_the_selected_id() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-field-enum-variant-menu");
        let mut selected = "First".to_owned();

        {
            let mut draw = |ui: &mut Ui| {
                VariantMenu::new(id, &mut selected, VARIANTS, &style).show(ui, rect());
            };
            let _ = ctx.run_ui(input(), &mut draw);
            click(&ctx, rect().center(), &mut draw);
            let _ = ctx.run_ui(input(), &mut draw);
            let _ = ctx.run_ui(input(), &mut draw);
            let item = ctx
                .read_response(enum_item_id(id, 1))
                .expect("the second row was laid out")
                .rect;
            click_in(&ctx, item.center(), &mut draw);
        }
        assert_eq!(selected, "Push");
    }
}
