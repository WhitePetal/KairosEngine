//! Preferences panel: every open panel's skin, as editable fields.
//!
//! This is the 0.2.0 rewrite of the legacy kairos_editor PreferencesWindow,
//! and it keeps the capability that window existed for: pick a component on
//! the left, edit its Preferences/Styles/*.ron tokens as widgets on the right.
//!
//! The review that produced this shape — the legacy window's index-based
//! readback, its per-frame TOML write, its pages keyed by position in
//! drawers, and the parts deliberately not carried over — is
//! docs/plan/editor-ui-architecture/preferences-panel.md.
//!
//! # Where the fields come from
//!
//! Each panel owns its style (loaded in Panel::init) and derives StyleFields,
//! so it can describe its own tokens (crate::skin). The app snapshots the live
//! panels' field tables into Frame::skins while this panel is open; this panel
//! draws them and never touches another panel directly.
//!
//! # The field tree
//!
//! Dotted token paths are a tree (icons.console.info), and the page draws it
//! the way Unity draws an inspector: each nested RON struct is a full-width
//! foldout band with a triangle, leaf rows alternate between two greys, field
//! names are left-aligned, and a selected field gets a blue label and a blue
//! value outline.
//!
//! # Live apply and save
//!
//! A control pushes StyleAction::SetField on every change. The app writes it
//! into the owning panel's live style, so the next pass draws with the new
//! value — that is what makes the edit immediate. The file is written once, on
//! StyleAction::Commit: pointer release, the end of a typed edit, page switch,
//! or closing this panel. The legacy window instead pushed the whole page back
//! every frame and let the owning window write its TOML, about 60 times a
//! second.

pub mod style;

use kairos_editor_widgets::color::ColorExt;
use std::collections::HashSet;

use egui::{
    Align, Context, CornerRadius, CursorIcon, Id, Key, Margin, Pos2, Rect, Sense, Shape, Stroke,
    StrokeKind, TextEdit, Ui, UiBuilder, WidgetText, WidgetType, pos2, vec2,
};
use kairos_color::ColorUsage;
use kairos_editor_core::{Action, LayoutAction, PanelId, StyleAction, StyleValue};
use kairos_editor_widgets::button::Button;
use kairos_editor_widgets::color_picker::{
    ColorField, ColorProperty, ColorSwatches, any_color_picker_open, close_all_color_pickers,
    is_color_picker_open,
};
use kairos_editor_widgets::divider::Divider;
use kairos_editor_widgets::paint::labelled_widget_info;
use kairos_editor_widgets::style::StyleFile;
use kairos_editor_widgets::text::TextStyle;

use crate::frame::{Frame, UiOut};
use crate::panel::Panel;
use crate::skin::{PanelSkin, StyleField, StyleKind, humanize};

use self::style::PreferencesPanelStyle;

fn nav_id(panel: PanelId) -> Id {
    Id::new(("kairos.preferences.nav", panel))
}

/// Stable id of a foldout band, keyed by its dotted group path.
fn group_id(panel: PanelId, path: &str) -> Id {
    Id::new(("kairos.preferences.group", panel, path))
}

/// Stable id of a field's name, so tests (and egui's click routing) can find
/// the left-hand label without a layout index.
fn label_id(panel: PanelId, path: &str) -> Id {
    Id::new(("kairos.preferences.label", panel, path))
}

/// Stable id of a field's value box. The box, its drag/click interaction and
/// its inline text editor all share this id: only one of them is live at a
/// time, and keyboard focus must follow the field from one mode to the other.
fn field_id(panel: PanelId, path: &str) -> Id {
    Id::new(("kairos.preferences.field", panel, path))
}

/// Stable id of a boolean checkbox; see field_id.
fn toggle_id(panel: PanelId, path: &str) -> Id {
    Id::new(("kairos.preferences.toggle", panel, path))
}

fn close_id() -> Id {
    Id::new("kairos.preferences.close")
}

/// A numeric token being typed into its value box.
struct FieldEdit {
    path: String,
    text: String,
    /// Value before the edit, restored on Escape.
    original: StyleValue,
    /// Focus the editor and select its contents on the first pass.
    focus: bool,
}

impl FieldEdit {
    fn new(field: &StyleField) -> Self {
        Self {
            path: field.path.clone(),
            text: number_text(&field.value),
            original: field.value.clone(),
            focus: true,
        }
    }
}

/// Fractional pointer movement accumulated by an integer drag. egui reports
/// one-frame deltas; without the remainder a slow drag would never cross a
/// whole unit.
#[derive(Default)]
struct DragState {
    path: Option<String>,
    remainder: f32,
}

/// The page being edited: a snapshot of one panel's field table, plus the
/// bookkeeping that decides when its file is written.
#[derive(Default)]
struct PageState {
    /// The panel the fields belong to.
    panel: Option<PanelId>,
    /// Edited in place: a drag follows the pointer even though the owning
    /// panel's next snapshot is a frame behind.
    fields: Vec<StyleField>,
    title: String,
    file: &'static str,
    /// A SetField was pushed since the last save.
    dirty: bool,
    /// The field whose name or value was clicked last. Its name is blue and
    /// its value box is outlined with the accent.
    selected: Option<String>,
    /// Foldout bands folded shut, by dotted group path.
    collapsed: HashSet<String>,
    /// A numeric token being edited as text.
    edit: Option<FieldEdit>,
    drag: DragState,
    /// Last page scroll offset. While a floating color picker is open we keep
    /// this offset fixed so a TextEdit focus inside the picker cannot scroll
    /// the page behind it.
    scroll_offset: egui::Vec2,
}

impl PageState {
    /// Show the selected panel's field table, committing a pending edit on the
    /// page being left.
    fn select(&mut self, skins: &[PanelSkin], selected: Option<PanelId>, out: &mut UiOut) {
        let same = self.panel == selected && (selected.is_none() || !self.fields.is_empty());
        if same {
            return;
        }

        self.commit(out);

        match selected.and_then(|panel| skins.iter().find(|skin| skin.panel == panel)) {
            Some(skin) => {
                self.fields.clone_from(&skin.fields);
                self.title.clone_from(&skin.title);
                self.file = skin.file;
            }
            None => {
                self.fields.clear();
                self.title.clear();
                self.file = "";
            }
        }
        self.panel = selected;
        // A new page starts unfolded and unselected. Carrying either over
        // would highlight a path that only happens to have the same name.
        self.selected = None;
        self.collapsed.clear();
        self.edit = None;
        self.drag = DragState::default();
    }

    /// Ask the app to persist the page being left, if it has an edit that was
    /// never released (typed text, arrow keys).
    fn commit(&mut self, out: &mut UiOut) {
        if !self.dirty {
            return;
        }
        if let Some(panel) = self.panel {
            out.push(Action::Style(StyleAction::Commit { panel }));
        }
        self.dirty = false;
    }
}

/// The Preferences panel.
pub struct PreferencesPanel {
    style: PreferencesPanelStyle,
    /// Page list selection; falls back to the first live skin.
    selected: Option<PanelId>,
    page: PageState,
    /// A color popup was open when the panel closed; close it on the next
    /// draw so reopening Preferences does not resurrect it.
    close_popups: bool,
    /// Shared swatch groups. Loaded from Library/ColorSwatches.ron in init and
    /// saved by the picker whenever a group or swatch changes.
    swatches: ColorSwatches,
}

impl PreferencesPanel {
    /// Build the panel. The catalog factory above is the only caller.
    pub fn new() -> Self {
        Self {
            style: PreferencesPanelStyle::default(),
            selected: None,
            page: PageState::default(),
            close_popups: false,
            swatches: ColorSwatches::default(),
        }
    }
}

impl Default for PreferencesPanel {
    fn default() -> Self {
        Self::new()
    }
}

#[crate::panel(PREFERENCES = "preferences", skin = PreferencesPanelStyle)]
impl Panel for PreferencesPanel {
    fn title(&self, _frame: &Frame) -> WidgetText {
        "Preferences".into()
    }

    /// Load the skin. A missing or broken file is logged and falls back to the
    /// code defaults; the legacy window treated it as fatal and took the editor
    /// down with it.
    fn init(&mut self, ctx: &Context) {
        match StyleFile::<PreferencesPanelStyle>::load_or_default() {
            Ok(file) => {
                self.style = if ctx.theme() == egui::Theme::Light {
                    file.light
                } else {
                    file.dark
                };
            }
            Err(error) => {
                kairos_log::warn!(target: "kairos_editor", "preferences style: {error}");
            }
        }
        match ColorSwatches::load("Library/ColorSwatches.ron") {
            Ok(book) => self.swatches = book,
            Err(error) => {
                kairos_log::warn!(target: "kairos_editor", "color swatches: {error}");
                self.swatches.set_path("Library/ColorSwatches.ron");
            }
        }
    }

    fn ui(&mut self, ui: &mut Ui, frame: &Frame, out: &mut UiOut) {
        let Self {
            style,
            selected,
            page,
            close_popups,
            swatches,
        } = self;

        // The dock builds a body Ui from the context style, so the app's
        // "chrome is not selectable" switch does not reach this panel.
        ui.style_mut().interaction.selectable_labels = false;
        if *close_popups {
            egui::Popup::close_all(ui.ctx());
            close_all_color_pickers(ui.ctx());
            *close_popups = false;
        }

        // The dock paints a body colour behind every tab; a floating window uses
        // the panel's own. Painting it here keeps both cases identical.
        ui.painter().rect_filled(
            ui.max_rect(),
            CornerRadius::ZERO,
            style.background.to_egui(),
        );

        let skins = frame.skins;
        let mut current = match *selected {
            Some(panel) if skins.iter().any(|skin| skin.panel == panel) => Some(panel),
            _ => skins.first().map(|skin| skin.panel),
        };

        egui::Frame::NONE
            .inner_margin(Margin::same(style.padding.round() as i8))
            .show(ui, |ui| {
                // The two regions are sized by hand; egui's default gap between
                // them would make the footer miss the bottom edge by a few px.
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                let available = ui.available_size();
                let body_height = (available.y - style.footer_gap - style.footer_height).max(0.0);
                let (body, _) =
                    ui.allocate_exact_size(vec2(available.x, body_height), Sense::hover());
                let (footer_rect, _) =
                    ui.allocate_exact_size(vec2(available.x, style.footer_height), Sense::hover());

                // The footer is pinned: the page scrolls, the action never does.
                let rule_y = footer_rect.top() - style.footer_gap * 0.5;
                Divider::horizontal()
                    .color(style.divider.to_egui())
                    .paint_in(
                        ui,
                        Rect::from_min_size(pos2(body.left(), rule_y), vec2(body.width(), 1.0)),
                    );

                let nav_rect = Rect::from_min_size(
                    body.min,
                    vec2(style.nav_width.min(body.width()), body.height()),
                );
                let page_rect = Rect::from_min_max(
                    pos2(nav_rect.right() + style.column_gap, body.top()),
                    body.max,
                );

                ui.painter().rect_filled(
                    nav_rect,
                    CornerRadius::same(style.nav_corner_radius as u8),
                    style.nav_background.to_egui(),
                );
                Divider::vertical()
                    .length(body.height())
                    .color(style.divider.to_egui())
                    .paint_in(
                        ui,
                        Rect::from_min_size(
                            pos2(nav_rect.right() + style.column_gap * 0.5, body.top()),
                            vec2(1.0, body.height()),
                        ),
                    );

                ui.scope_builder(UiBuilder::new().max_rect(nav_rect.shrink(4.0)), |ui| {
                    current = page_list(ui, style, skins, current);
                });

                let previous_panel = page.panel;
                page.select(skins, current, out);
                if previous_panel.is_some() && page.panel != previous_panel {
                    // A color popup belongs to the page that opened it; leaving
                    // the page must not leave it floating over the new one.
                    egui::Popup::close_all(ui.ctx());
                    close_all_color_pickers(ui.ctx());
                }

                ui.scope_builder(UiBuilder::new().max_rect(page_rect), |ui| {
                    page_body(ui, style, page, swatches, out);
                });

                footer(ui, style, footer_rect, out);
            });

        // A click ends the interaction: one write per gesture, however many
        // frames the drag took. Keyboard edits have no release; they persist
        // when the page changes or the panel closes, and Enter/Escape ends a
        // color hex or numeric edit immediately.
        let released = ui.input(|input| input.pointer.any_released());
        let keyboard_end =
            ui.input(|input| input.key_pressed(Key::Enter) || input.key_pressed(Key::Escape));
        if page.dirty && (released || keyboard_end) {
            page.commit(out);
        }

        *selected = current;
    }

    /// The page scrolls inside the body and the footer is pinned, so the dock
    /// must not wrap the whole panel in a second scroll area.
    fn scroll_bars(&self) -> [bool; 2] {
        [false, false]
    }

    /// Closing with an edit still pending (a value typed but never released)
    /// saves it before the panel goes away.
    fn on_close(&mut self, _frame: &Frame, out: &mut UiOut) -> bool {
        self.page.commit(out);
        if let Err(error) = self.swatches.save_if_dirty() {
            kairos_log::warn!(target: "kairos_editor", "color swatches: {error}");
        }
        self.close_popups = true;
        true
    }
}

/// The page list: one entry per panel with an editable skin.
fn page_list(
    ui: &mut Ui,
    style: &PreferencesPanelStyle,
    skins: &[PanelSkin],
    selected: Option<PanelId>,
) -> Option<PanelId> {
    let mut chosen = selected;
    ui.spacing_mut().item_spacing = vec2(0.0, 1.0);
    let width = ui.available_width();
    let nav = style.nav_style();

    for skin in skins {
        let is_current = Some(skin.panel) == selected;
        let response = ui.add(
            Button::new(&skin.title, &nav)
                .width(width)
                .selected(is_current)
                .id(nav_id(skin.panel)),
        );
        if is_current {
            // A 2 px accent stripe reads as "this page" even when the pointer
            // is hovering another entry.
            ui.painter().rect_filled(
                Rect::from_min_size(response.rect.min, vec2(2.0, response.rect.height())),
                CornerRadius::ZERO,
                style.accent.to_egui(),
            );
        }
        if response.clicked() {
            chosen = Some(skin.panel);
        }
    }

    chosen
}

/// One page: heading, the file being written, rule, grouped rows.
fn page_body(
    ui: &mut Ui,
    style: &PreferencesPanelStyle,
    page: &mut PageState,
    swatches: &mut ColorSwatches,
    out: &mut UiOut,
) {
    let picker_before = any_color_picker_open(ui.ctx());
    let mut scroll = egui::ScrollArea::vertical()
        .id_salt("kairos-preferences-page")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);

            let Some(panel) = page.panel else {
                TextStyle::sized(style.path.to_egui(), style.body_size).label(
                    ui,
                    "No panel skins to edit yet. Open a panel, then reopen Preferences.",
                );
                return;
            };

            TextStyle::sized(style.title.to_egui(), style.title_size)
                .strong()
                .label(ui, page.title.clone());
            ui.add_space(2.0);
            TextStyle::sized(style.path.to_egui(), style.group_size).label(
                ui,
                format!(
                    "Preferences/Styles/{} - edits apply live, saved on release",
                    page.file
                ),
            );
            ui.add_space(style.section_spacing);
            ui.add(Divider::horizontal().color(style.divider.to_egui()));
            ui.add_space(style.section_spacing);

            let mut writer = PageWriter {
                style,
                panel,
                page,
                swatches,
                out,
                clicked: false,
            };
            writer.rows(ui);
        });
    let picker_after = any_color_picker_open(ui.ctx());
    if picker_before || picker_after {
        // Keep the page under the floating picker still. A TextEdit focus in
        // the picker emits scroll_to_rect, and this ScrollArea would otherwise
        // consume it and slide the Preferences page while the user types.
        scroll.state.offset = page.scroll_offset;
        scroll.state.store(ui.ctx(), scroll.id);
    } else {
        page.scroll_offset = scroll.state.offset;
    }
}

/// Draws one page's field tree.
struct PageWriter<'a> {
    style: &'a PreferencesPanelStyle,
    panel: PanelId,
    page: &'a mut PageState,
    swatches: &'a mut ColorSwatches,
    out: &'a mut UiOut,
    /// A row, a field name or a foldout consumed this pass's click. Without
    /// one, a click elsewhere on the page ends the current selection.
    clicked: bool,
}

impl PageWriter<'_> {
    fn rows(&mut self, ui: &mut Ui) {
        let style = self.style;
        // The scroll bar floats over the content in egui; reserving its lane
        // keeps the rightmost value box (an icon path, typically) clear of it.
        let row_width = (ui.available_width() - style.scrollbar_gap).max(1.0);
        let mut open: Vec<String> = Vec::new();
        let mut row_number = 0usize;

        for index in 0..self.page.fields.len() {
            let group = group_path(&self.page.fields[index].path);
            let segments: Vec<&str> = group.split('.').filter(|part| !part.is_empty()).collect();

            // Close the groups this field is no longer in, then open the ones
            // it is. Fields are in declaration order, so the group runs stay
            // contiguous.
            let common = open
                .iter()
                .zip(segments.iter())
                .take_while(|(opened, segment)| opened.as_str() == **segment)
                .count();
            open.truncate(common);

            for depth in common..segments.len() {
                let path = segments[..=depth].join(".");
                let hidden = open
                    .iter()
                    .any(|ancestor| self.page.collapsed.contains(ancestor));
                open.push(path.clone());
                if !hidden {
                    self.foldout(ui, &path, depth, humanize(segments[depth]), row_width);
                }
            }

            let hidden = open
                .iter()
                .any(|ancestor| self.page.collapsed.contains(ancestor));
            if !hidden {
                self.row(ui, index, open.len(), row_width, row_number);
                row_number += 1;
                if style.row_gap > 0.0 {
                    ui.add_space(style.row_gap);
                }
            }
        }

        // A click in the empty page (or on its heading) ends the selection;
        // rows, names and foldouts record their own clicks.
        if ui.input(|input| input.pointer.any_click()) && !self.clicked {
            self.page.selected = None;
            self.page.edit = None;
            self.page.drag = DragState::default();
        }
    }

    /// One foldout band: triangle, left-aligned group name, full-width hit
    /// area. Unity calls this a foldout; the RON calls it a nested struct.
    fn foldout(&mut self, ui: &mut Ui, path: &str, depth: usize, label: String, row_width: f32) {
        let style = self.style;
        let (rect, _) = ui.allocate_exact_size(vec2(row_width, style.group_height), Sense::hover());
        let response = ui
            .interact(rect, group_id(self.panel, path), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);

        let collapsed = self.page.collapsed.contains(path);
        let fill = if response.hovered() {
            style.group_hover
        } else {
            style.group_background
        };
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::ZERO, fill.to_egui());

        let arrow = pos2(
            rect.left() + style.row_inset + depth as f32 * style.group_indent + 4.0,
            rect.center().y,
        );
        paint_foldout_arrow(painter, arrow, collapsed, style.group.to_egui());

        let text_left = rect.left() + style.row_inset + 16.0 + depth as f32 * style.group_indent;
        let text = TextStyle::sized(style.group.to_egui(), style.group_size);
        let galley = text.layout(ui, &label);
        let text_pos = pos2(text_left, rect.center().y - galley.size().y * 0.5);
        let color = style.group.to_egui();
        painter.with_clip_rect(rect).galley(text_pos, galley, color);

        labelled_widget_info(
            &response,
            WidgetType::SelectableLabel,
            &label,
            Some(!collapsed),
        );

        if response.clicked() {
            if collapsed {
                self.page.collapsed.remove(path);
            } else {
                self.page.collapsed.insert(path.to_owned());
            }
            self.clicked = true;
        }

        let _ = response.on_hover_text(path.to_owned());
    }

    /// One leaf row: alternating band, left name, value box.
    fn row(&mut self, ui: &mut Ui, index: usize, depth: usize, row_width: f32, row_number: usize) {
        let style = self.style;
        let panel = self.panel;
        let (rect, _) = ui.allocate_exact_size(vec2(row_width, style.row_height), Sense::hover());
        ui.painter().rect_filled(
            rect,
            CornerRadius::ZERO,
            style.row_fill(row_number).to_egui(),
        );

        let path = self.page.fields[index].path.clone();
        let name = self.page.fields[index].name.clone();
        let kind = self.page.fields[index].kind;

        // Unity's layout: one label column, then a value column that fills the
        // rest of the row. The label column shrinks before the value box would
        // run under the scroll bar.
        let content_left = rect.left() + style.row_inset;
        let content_right = (rect.right() - style.row_inset).max(content_left + 1.0);
        let content_width = content_right - content_left;
        let max_label = (content_width - style.label_gap - style.field_width).max(48.0);
        let label_width = style
            .label_width
            .min(max_label)
            .min((content_width - style.label_gap - 1.0).max(0.0));
        let value_left = content_left + label_width + style.label_gap;
        let value_width = (content_right - value_left).max(1.0);
        let value_rect = Rect::from_min_size(
            pos2(value_left, rect.center().y - style.field_height * 0.5),
            vec2(value_width, style.field_height),
        );
        let label_rect = Rect::from_min_max(
            pos2(content_left, rect.top()),
            pos2(value_left - style.label_gap, rect.bottom()),
        );

        // The name is an interaction, not just paint: clicking it selects the
        // field, and a text field takes keyboard focus so typing starts.
        let label_response = ui.interact(label_rect, label_id(panel, &path), Sense::click());
        if label_response.clicked() {
            self.page.selected = Some(path.clone());
            self.clicked = true;
            if matches!(kind, StyleKind::Text) {
                ui.memory_mut(|memory| memory.request_focus(field_id(panel, &path)));
            }
        }

        let PageState {
            fields,
            selected,
            edit,
            drag,
            ..
        } = &mut *self.page;
        let field = &mut fields[index];
        let outcome = draw_value(
            ui,
            style,
            panel,
            field,
            selected,
            edit,
            drag,
            self.swatches,
            &mut self.clicked,
            value_rect,
        );

        if outcome.popup_open {
            // A click in the picker is still an interaction with this row;
            // without this the page's outside-click sweep would clear the
            // selection while the user is editing the color.
            self.clicked = true;
        }

        if outcome.changed {
            let field = &self.page.fields[index];
            self.out.push(Action::Style(StyleAction::SetField {
                panel,
                path: field.path.clone(),
                value: field.value.clone(),
            }));
            self.page.dirty = true;
        }
        if outcome.finished_edit && self.page.dirty {
            self.page.commit(self.out);
        }

        // Paint the name after the value: a click on the value updates the
        // selection, and the blue label should appear in the same pass.
        let focused = ui.memory(|memory| memory.has_focus(field_id(panel, &path)));
        let editing = self
            .page
            .edit
            .as_ref()
            .is_some_and(|edit| edit.path == path);
        let active = self.page.selected.as_deref() == Some(path.as_str()) || focused || editing;
        let color = style.label_color(active).to_egui();
        let text = TextStyle::sized(color, style.body_size);
        let galley = text.layout(ui, &name);
        let text_left = label_rect.left() + 16.0 + depth as f32 * style.group_indent;
        let text_pos = pos2(text_left, rect.center().y - galley.size().y * 0.5);
        ui.painter()
            .with_clip_rect(label_rect)
            .galley(text_pos, galley, color);

        let _ = label_response.on_hover_text(path);
    }
}

/// What one value control did this pass.
#[derive(Clone, Copy, Debug, Default)]
struct ValueOutcome {
    changed: bool,
    /// A typed numeric edit ended (Enter, Escape, or focus loss); the caller
    /// writes the file now instead of waiting for a pointer release.
    finished_edit: bool,
    /// The color picker popup is open. Clicks inside it must not clear the
    /// row selection, so the page treats them as a row interaction.
    popup_open: bool,
}

/// One token's editor. Returns whether it changed the field (which is updated
/// in place so the page keeps its value between frames).
#[allow(clippy::too_many_arguments)]
fn draw_value(
    ui: &mut Ui,
    style: &PreferencesPanelStyle,
    panel: PanelId,
    field: &mut StyleField,
    selected: &mut Option<String>,
    edit: &mut Option<FieldEdit>,
    drag: &mut DragState,
    swatches: &mut ColorSwatches,
    clicked: &mut bool,
    rect: Rect,
) -> ValueOutcome {
    let path = field.path.clone();
    let kind = field.kind;
    let id = field_id(panel, &path);
    let hovered = ui.rect_contains_pointer(rect);
    let editing = edit.as_ref().is_some_and(|current| current.path == path);
    let active = selected.as_deref() == Some(path.as_str())
        || editing
        || ui.memory(|memory| memory.has_focus(id));

    match kind {
        StyleKind::Float | StyleKind::Integer { .. } => {
            if let Some(mut editor) = edit.take().filter(|current| current.path == path) {
                let outcome = draw_number_edit(
                    ui,
                    style,
                    kind,
                    field,
                    selected,
                    clicked,
                    rect,
                    &path,
                    id,
                    hovered,
                    &mut editor,
                );
                if !outcome.finished_edit {
                    *edit = Some(editor);
                }
                return outcome;
            }
            draw_number(
                ui, style, kind, field, selected, edit, drag, clicked, rect, id, &path, active,
                hovered,
            )
        }
        StyleKind::Text => draw_text(
            ui, style, field, selected, clicked, rect, id, &path, active, hovered,
        ),
        StyleKind::Toggle => draw_toggle(
            ui, style, field, selected, clicked, rect, panel, &path, active,
        ),
        StyleKind::Color | StyleKind::HdrColor => draw_color(
            ui, style, field, selected, clicked, swatches, rect, id, &path, active,
        ),
    }
}

/// A numeric token at rest: a Unity-style inset box that drags to scrub and
/// clicks into a text editor.
#[allow(clippy::too_many_arguments)]
fn draw_number(
    ui: &mut Ui,
    style: &PreferencesPanelStyle,
    kind: StyleKind,
    field: &mut StyleField,
    selected: &mut Option<String>,
    edit: &mut Option<FieldEdit>,
    drag: &mut DragState,
    clicked: &mut bool,
    rect: Rect,
    id: Id,
    path: &str,
    active: bool,
    hovered: bool,
) -> ValueOutcome {
    let fill = if hovered && !active {
        style.control_hover
    } else {
        style.control_rail
    };
    paint_value_background(ui, style, rect, fill.to_egui());

    let text = number_text(&field.value);
    let text_style = TextStyle::sized(style.control_value.to_egui(), style.body_size);
    let galley = text_style.layout(ui, &text);
    let text_pos = pos2(
        rect.left() + style.field_padding,
        rect.center().y - galley.size().y * 0.5,
    );
    ui.painter()
        .with_clip_rect(rect.shrink2(vec2(style.field_padding, 0.0)))
        .galley(text_pos, galley, style.control_value.to_egui());

    let response = ui
        .interact(rect, id, Sense::click_and_drag())
        .on_hover_cursor(CursorIcon::ResizeHorizontal);
    labelled_widget_info(&response, WidgetType::DragValue, &field.name, None);

    let mut changed = false;
    if response.clicked() {
        *selected = Some(path.to_owned());
        *clicked = true;
        *edit = Some(FieldEdit::new(field));
        response.request_focus();
    }

    let delta = response.drag_delta().x - response.drag_delta().y;
    if response.dragged() && delta != 0.0 {
        *selected = Some(path.to_owned());
        *clicked = true;
        match kind {
            StyleKind::Float => {
                if let StyleValue::Float(current) = &field.value {
                    let next = round_decimal(*current + delta * drag_speed(*current) as f32, 3);
                    if next != *current {
                        field.value = StyleValue::Float(next);
                        changed = true;
                    }
                }
            }
            StyleKind::Integer { max } => {
                if drag.path.as_deref() != Some(path) {
                    drag.path = Some(path.to_owned());
                    drag.remainder = 0.0;
                }
                let total = drag.remainder + delta;
                let steps = total.trunc();
                drag.remainder = total - steps;
                if steps != 0.0
                    && let StyleValue::Integer(current) = &field.value
                {
                    let next = ((*current as i64) + steps as i64).clamp(0, max as i64) as u32;
                    if next != *current {
                        field.value = StyleValue::Integer(next);
                        changed = true;
                    }
                }
            }
            _ => {}
        }
    }
    if response.drag_stopped() {
        *drag = DragState::default();
    }

    let focused = ui.memory(|memory| memory.has_focus(id));
    let edge = style
        .value_edge(
            active || focused || selected.as_deref() == Some(path),
            hovered,
        )
        .to_egui();
    paint_value_outline(ui, style, rect, edge);

    ValueOutcome {
        changed,
        finished_edit: false,
        popup_open: false,
    }
}

/// The text editor a numeric box switches into on click. Selecting all text
/// on entry makes typing replace the number, like Unity and egui's DragValue.
#[allow(clippy::too_many_arguments)]
fn draw_number_edit(
    ui: &mut Ui,
    style: &PreferencesPanelStyle,
    kind: StyleKind,
    field: &mut StyleField,
    selected: &mut Option<String>,
    clicked: &mut bool,
    rect: Rect,
    path: &str,
    id: Id,
    hovered: bool,
    editor: &mut FieldEdit,
) -> ValueOutcome {
    paint_value_background(ui, style, rect, style.control_rail.to_egui());
    let padding = value_padding(ui, style, rect);
    let mut changed = false;

    let response = ui
        .scope_builder(UiBuilder::new().max_rect(rect), |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            ui.visuals_mut().selection.bg_fill = style.accent.to_egui();
            ui.visuals_mut().selection.stroke = Stroke::NONE;
            ui.add(
                TextEdit::singleline(&mut editor.text)
                    .id(id)
                    .font(
                        TextStyle::sized(style.control_value.to_egui(), style.body_size)
                            .font_id(ui.style()),
                    )
                    .text_color(style.control_value.to_egui())
                    .frame(egui::Frame::NONE.inner_margin(padding))
                    .horizontal_align(Align::Min)
                    .vertical_align(Align::Center)
                    .min_size(vec2(rect.width(), rect.height()))
                    .desired_width(rect.width()),
            )
        })
        .inner;

    if editor.focus {
        response.request_focus();
        select_all_text(ui, id, editor.text.chars().count());
        editor.focus = false;
    }

    if response.changed()
        && let Some(value) = parse_number(kind, &editor.text)
        && field.value != value
    {
        field.value = value;
        changed = true;
    }

    let escape = ui.input(|input| input.key_pressed(Key::Escape));
    let enter = ui.input(|input| input.key_pressed(Key::Enter));
    let mut finished = false;
    if escape {
        if field.value != editor.original {
            field.value = editor.original.clone();
            changed = true;
        }
        response.surrender_focus();
        finished = true;
    } else if enter || response.lost_focus() {
        if let Some(value) = parse_number(kind, &editor.text)
            && field.value != value
        {
            field.value = value;
            changed = true;
        }
        finished = true;
    }

    if response.clicked() {
        *selected = Some(path.to_owned());
        *clicked = true;
    }

    paint_value_outline(ui, style, rect, style.value_edge(true, hovered).to_egui());
    editor.focus = false;

    ValueOutcome {
        changed,
        finished_edit: finished,
        popup_open: false,
    }
}

/// A string token (icon path, font family): a box whose whole interior is an
/// editable field.
#[allow(clippy::too_many_arguments)]
fn draw_text(
    ui: &mut Ui,
    style: &PreferencesPanelStyle,
    field: &mut StyleField,
    selected: &mut Option<String>,
    clicked: &mut bool,
    rect: Rect,
    id: Id,
    path: &str,
    active: bool,
    hovered: bool,
) -> ValueOutcome {
    let fill = if hovered && !active {
        style.control_hover
    } else {
        style.control_rail
    };
    paint_value_background(ui, style, rect, fill.to_egui());

    let mut text = match &field.value {
        StyleValue::Text(text) => text.clone(),
        _ => String::new(),
    };
    let padding = value_padding(ui, style, rect);
    let response = ui
        .scope_builder(UiBuilder::new().max_rect(rect), |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            ui.visuals_mut().selection.bg_fill = style.accent.to_egui();
            ui.visuals_mut().selection.stroke = Stroke::NONE;
            ui.add(
                TextEdit::singleline(&mut text)
                    .id(id)
                    .font(
                        TextStyle::sized(style.control_value.to_egui(), style.body_size)
                            .font_id(ui.style()),
                    )
                    .text_color(style.control_value.to_egui())
                    .frame(egui::Frame::NONE.inner_margin(padding))
                    .horizontal_align(Align::Min)
                    .vertical_align(Align::Center)
                    .min_size(vec2(rect.width(), rect.height()))
                    .desired_width(rect.width()),
            )
        })
        .inner;

    let mut changed = false;
    if response.changed() {
        field.value = StyleValue::Text(text);
        changed = true;
    }
    if response.clicked() || response.has_focus() {
        *selected = Some(path.to_owned());
        if response.clicked() {
            *clicked = true;
        }
    }

    let active_now = selected.as_deref() == Some(path) || response.has_focus();
    let edge = style.value_edge(active_now, hovered).to_egui();
    paint_value_outline(ui, style, rect, edge);

    ValueOutcome {
        changed,
        finished_edit: response.lost_focus(),
        popup_open: false,
    }
}

/// A boolean token: Unity's square checkbox at the start of the value column.
#[allow(clippy::too_many_arguments)]
fn draw_toggle(
    ui: &mut Ui,
    style: &PreferencesPanelStyle,
    field: &mut StyleField,
    selected: &mut Option<String>,
    clicked: &mut bool,
    rect: Rect,
    panel: PanelId,
    path: &str,
    active: bool,
) -> ValueOutcome {
    let on = matches!(&field.value, StyleValue::Toggle(true));
    let side = style.checkbox_size;
    let box_rect = Rect::from_center_size(
        pos2(rect.left() + side * 0.5, rect.center().y),
        vec2(side, side),
    );
    let response = ui.interact(box_rect, toggle_id(panel, path), Sense::click());
    labelled_widget_info(&response, WidgetType::Checkbox, &field.name, Some(on));

    let active_now = active || selected.as_deref() == Some(path);
    let edge = if active_now {
        style.accent
    } else if response.hovered() {
        style.control_edge_hover
    } else {
        style.control_edge
    };
    paint_value_background(ui, style, box_rect, style.control_rail.to_egui());
    paint_value_outline(ui, style, box_rect, edge.to_egui());
    if on {
        paint_check(ui, style, box_rect);
    }

    let mut changed = false;
    if response.clicked() {
        field.value = StyleValue::Toggle(!on);
        *selected = Some(path.to_owned());
        *clicked = true;
        changed = true;
    }

    ValueOutcome {
        changed,
        finished_edit: false,
        popup_open: false,
    }
}

/// A color token: Unity's inspector field, a full-width swatch with the HDR
/// badge when the field's [`ColorProperty`] has HDR usage and the eyedropper
/// slot. Clicking the swatch opens the shared color picker rather than egui's
/// built-in one.
#[allow(clippy::too_many_arguments)]
fn draw_color(
    ui: &mut Ui,
    style: &PreferencesPanelStyle,
    field: &mut StyleField,
    selected: &mut Option<String>,
    clicked: &mut bool,
    swatches: &mut ColorSwatches,
    rect: Rect,
    id: Id,
    path: &str,
    active: bool,
) -> ValueOutcome {
    let usage = match field.kind {
        StyleKind::HdrColor => ColorUsage::HDR,
        _ => ColorUsage::LDR,
    };
    let Some(mut color) = color_property(&field.value, usage) else {
        return ValueOutcome::default();
    };

    let picker_style = style.color_picker_style();
    let response = ui.put(
        rect,
        ColorField::new(id, &mut color, &picker_style)
            .selected(active)
            .height(rect.height())
            .swatches(swatches),
    );

    let mut changed = false;
    if response.changed() {
        field.value = StyleValue::Color(color.color);
        changed = true;
    }
    if response.clicked() {
        *selected = Some(path.to_owned());
        *clicked = true;
    }

    let popup_open = is_color_picker_open(ui.ctx(), id);

    ValueOutcome {
        changed,
        finished_edit: false,
        popup_open,
    }
}

fn color_property(value: &StyleValue, usage: ColorUsage) -> Option<ColorProperty> {
    match value {
        StyleValue::Color(color) => Some(ColorProperty::new(*color, usage)),
        _ => None,
    }
}

/// Horizontal text padding plus the vertical padding that centres one line
/// inside a value box (TextEdit sizes itself by its font's row height, not by
/// the box, so the difference is split above and below).
fn value_padding(ui: &Ui, style: &PreferencesPanelStyle, rect: Rect) -> Margin {
    let font = TextStyle::sized(style.control_value.to_egui(), style.body_size).font_id(ui.style());
    let row_height = ui.fonts_mut(|fonts| fonts.row_height(&font));
    let vertical = ((rect.height() - row_height) * 0.5).round().max(0.0) as i8;
    Margin::symmetric(style.field_padding.round() as i8, vertical)
}

fn paint_value_background(ui: &Ui, style: &PreferencesPanelStyle, rect: Rect, fill: egui::Color32) {
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(style.control_corner_radius as u8),
        fill,
    );
}

fn paint_value_outline(ui: &Ui, style: &PreferencesPanelStyle, rect: Rect, edge: egui::Color32) {
    if style.control_border_width <= 0.0 {
        return;
    }
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(style.control_corner_radius as u8),
        Stroke::new(style.control_border_width, edge),
        StrokeKind::Inside,
    );
}

fn paint_check(ui: &Ui, style: &PreferencesPanelStyle, rect: Rect) {
    let color = style.control_value.to_egui();
    let stroke = Stroke::new(1.5, color);
    let a = pos2(rect.left() + rect.width() * 0.22, rect.center().y);
    let b = pos2(
        rect.left() + rect.width() * 0.42,
        rect.bottom() - rect.height() * 0.22,
    );
    let c = pos2(
        rect.right() - rect.width() * 0.18,
        rect.top() + rect.height() * 0.22,
    );
    ui.painter().line_segment([a, b], stroke);
    ui.painter().line_segment([b, c], stroke);
}

fn paint_foldout_arrow(
    painter: &egui::Painter,
    center: Pos2,
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
    painter.add(Shape::convex_polygon(points, color, Stroke::NONE));
}

/// The dotted group path of a token (icons.console), or an empty string for a
/// root token.
fn group_path(path: &str) -> String {
    path.rsplit_once('.')
        .map_or_else(String::new, |(group, _)| group.to_owned())
}

/// The displayed form of a numeric token.
fn number_text(value: &StyleValue) -> String {
    match value {
        StyleValue::Float(value) => {
            let mut text = format!("{value}");
            if text == "-0" {
                text = "0".to_owned();
            }
            text
        }
        StyleValue::Integer(value) => value.to_string(),
        _ => String::new(),
    }
}

/// Parse a typed number, clamped to the field's own range (a u8 corner radius
/// is not a negative number and does not exceed 255).
fn parse_number(kind: StyleKind, text: &str) -> Option<StyleValue> {
    let text = text.trim();
    match kind {
        StyleKind::Float => text
            .parse::<f32>()
            .ok()
            .filter(|value| value.is_finite())
            .map(StyleValue::Float),
        StyleKind::Integer { max } => text
            .parse::<f32>()
            .ok()
            .filter(|value| value.is_finite())
            .map(|value| StyleValue::Integer(value.round().clamp(0.0, max as f32) as u32)),
        _ => None,
    }
}

fn select_all_text(ui: &Ui, id: Id, chars: usize) {
    let mut state = TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::default(),
            egui::text::CCursor::new(chars),
        )));
    state.store(ui.ctx(), id);
}

fn round_decimal(value: f32, places: i32) -> f32 {
    let factor = 10f32.powi(places);
    (value * factor).round() / factor
}

/// Units per point of drag: a 3 px padding needs a finer hand than a 130 px
/// pane height, and the style knows its own magnitude.
fn drag_speed(value: f32) -> f64 {
    (value.abs() / 50.0).clamp(0.01, 1.0) as f64
}

/// Close, right-aligned. Every edit is already written when the interaction
/// ends, so there is nothing to apply or cancel here.
fn footer(ui: &mut Ui, style: &PreferencesPanelStyle, rect: Rect, out: &mut UiOut) {
    ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            if ui
                .add(Button::new("Close", &style.button_style()).id(close_id()))
                .clicked()
            {
                out.push(Action::Layout(LayoutAction::ClosePanel {
                    panel: PREFERENCES,
                }));
            }
        });
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::HostView;
    use crate::panels;
    use egui::{Color32 as EguiColor32, Key, Modifiers};
    use kairos_color::Color;
    use kairos_editor_core::EditorState;
    use kairos_editor_widgets::color_picker::color_swatch_id;
    use kairos_editor_widgets::style::Style;

    struct NoHost;

    impl HostView for NoHost {}

    fn fields() -> Vec<StyleField> {
        vec![
            StyleField {
                path: "height".to_owned(),
                name: "Height".to_owned(),
                value: StyleValue::Float(20.0),
                kind: StyleKind::Float,
            },
            StyleField {
                path: "enabled".to_owned(),
                name: "Enabled".to_owned(),
                value: StyleValue::Toggle(false),
                kind: StyleKind::Toggle,
            },
            StyleField {
                path: "icons.warning".to_owned(),
                name: "Warning".to_owned(),
                value: StyleValue::Text("warning.png".to_owned()),
                kind: StyleKind::Text,
            },
            StyleField {
                path: "icons.error".to_owned(),
                name: "Error".to_owned(),
                value: StyleValue::Text("error.png".to_owned()),
                kind: StyleKind::Text,
            },
            StyleField {
                path: "icons.textures.tint".to_owned(),
                name: "Tint".to_owned(),
                value: StyleValue::Text("tint.png".to_owned()),
                kind: StyleKind::Text,
            },
        ]
    }

    fn skins() -> Vec<PanelSkin> {
        vec![
            PanelSkin {
                panel: panels::CONSOLE,
                title: "Console".to_owned(),
                file: "ConsoleStyle.ron",
                fields: fields(),
            },
            PanelSkin {
                panel: panels::PROFILER,
                title: "Profiler".to_owned(),
                file: "Profiler.ron",
                fields: fields(),
            },
        ]
    }

    fn color_fields() -> Vec<StyleField> {
        vec![
            StyleField {
                path: "background".to_owned(),
                name: "Background".to_owned(),
                value: StyleValue::Color(Color::from_srgb8([20, 140, 220, 255])),
                kind: StyleKind::Color,
            },
            StyleField {
                path: "glow".to_owned(),
                name: "Glow".to_owned(),
                value: StyleValue::Color(Color::new(2.0, 1.0, 0.5, 1.0)),
                kind: StyleKind::HdrColor,
            },
        ]
    }

    fn color_skins() -> Vec<PanelSkin> {
        vec![PanelSkin {
            panel: panels::CONSOLE,
            title: "Console".to_owned(),
            file: "ConsoleStyle.ron",
            fields: color_fields(),
        }]
    }

    /// Enough rows to force the page's scroll bar, so the right-hand gutter
    /// can be measured.
    fn long_skin() -> Vec<PanelSkin> {
        let mut fields = fields();
        for index in 0..40 {
            fields.push(StyleField {
                path: format!("token_{index}"),
                name: format!("Token {index}"),
                value: StyleValue::Float(index as f32),
                kind: StyleKind::Float,
            });
        }
        vec![PanelSkin {
            panel: panels::CONSOLE,
            title: "Console".to_owned(),
            file: "ConsoleStyle.ron",
            fields,
        }]
    }

    /// Headless panel harness: one egui context, one frame's worth of skins and
    /// one panel. Actions are collected, not applied: they are the panel's only
    /// output.
    struct Harness {
        ctx: Context,
        size: egui::Vec2,
        state: EditorState,
        skins: Vec<PanelSkin>,
        panel: PreferencesPanel,
    }

    impl Harness {
        fn new(skins: Vec<PanelSkin>) -> Self {
            // The default floating rect is 42 % of a 1280x820 editor (537x344);
            // the dock's title bar takes 28 px, so the body sees about 316.
            Self::with_size(skins, vec2(537.0, 316.0))
        }

        fn with_size(skins: Vec<PanelSkin>, size: egui::Vec2) -> Self {
            Self {
                ctx: Context::default(),
                size,
                state: EditorState::new(),
                skins,
                panel: PreferencesPanel::new(),
            }
        }

        fn frame(&mut self, events: Vec<egui::Event>) -> (egui::FullOutput, Vec<Action>) {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)),
                events,
                ..Default::default()
            };

            let host = NoHost;
            let Self {
                ctx,
                state,
                skins,
                panel,
                ..
            } = self;
            let frame = Frame::with_host(state, &host).with_skins(skins);
            let mut out = UiOut::new();
            let output = ctx.run_ui(input, |ui| panel.ui(ui, &frame, &mut out));
            (output, out.take())
        }

        fn rect_of(&self, id: Id) -> egui::Rect {
            self.ctx
                .read_response(id)
                .unwrap_or_else(|| panic!("{id:?} was laid out in a previous pass"))
                .rect
        }

        /// Press and release on a widget across passes, the way a real click
        /// arrives; returns the pass that handled the release.
        fn click(&mut self, id: Id) -> (egui::FullOutput, Vec<Action>) {
            let rect = self.rect_of(id);
            self.click_at(rect.center())
        }

        fn click_at(&mut self, pos: egui::Pos2) -> (egui::FullOutput, Vec<Action>) {
            self.frame(vec![egui::Event::PointerMoved(pos)]);
            self.frame(vec![pointer(pos, true)]);
            self.frame(vec![pointer(pos, false)])
        }
    }

    fn pointer(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    #[derive(Clone, Debug)]
    struct PaintedText {
        text: String,
        rect: egui::Rect,
        color: EguiColor32,
    }

    #[derive(Clone, Debug)]
    struct PaintedRect {
        rect: egui::Rect,
        fill: EguiColor32,
        stroke: Stroke,
    }

    fn painted_texts(output: &egui::FullOutput) -> Vec<PaintedText> {
        fn walk(shape: &egui::Shape, out: &mut Vec<PaintedText>) {
            match shape {
                egui::Shape::Text(text) => out.push(PaintedText {
                    text: text.galley.text().to_owned(),
                    rect: egui::Rect::from_min_size(text.pos, text.galley.size()),
                    color: text.override_text_color.unwrap_or(text.fallback_color),
                }),
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

    fn painted_rects(output: &egui::FullOutput) -> Vec<PaintedRect> {
        fn walk(shape: &egui::Shape, out: &mut Vec<PaintedRect>) {
            match shape {
                egui::Shape::Rect(rect) => out.push(PaintedRect {
                    rect: rect.rect,
                    fill: rect.fill,
                    stroke: rect.stroke,
                }),
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

    /// Every text run the pass painted, flattened out of the shape tree.
    fn painted_text(output: &egui::FullOutput) -> String {
        painted_texts(output)
            .into_iter()
            .map(|run| run.text)
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn run<'a>(runs: &'a [PaintedText], text: &str) -> &'a PaintedText {
        runs.iter().find(|run| run.text == text).unwrap_or_else(|| {
            panic!(
                "no {text:?} text run: {:?}",
                runs.iter().map(|run| &run.text).collect::<Vec<_>>()
            )
        })
    }

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn the_page_list_shows_every_styled_panel_and_its_field_tree() {
        let mut harness = Harness::new(skins());

        let (output, actions) = harness.frame(Vec::new());
        let text = painted_text(&output);

        assert!(text.contains("Console"), "{text}");
        assert!(text.contains("Profiler"), "{text}");
        // The first page's heading, file and rows.
        assert!(
            text.contains("Preferences/Styles/ConsoleStyle.ron"),
            "{text}"
        );
        assert!(text.contains("Height"), "{text}");
        assert!(text.contains("Enabled"), "{text}");
        // Nested tokens are foldout bands, not dim labels.
        assert!(text.contains("Icons"), "{text}");
        assert!(text.contains("Warning"), "{text}");
        assert!(text.contains("Textures"), "{text}");
        assert!(text.contains("Tint"), "{text}");
        assert!(
            actions.is_empty(),
            "a plain pass emits nothing: {actions:?}"
        );
    }

    #[test]
    fn clicking_another_panel_shows_its_file() {
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());

        let (output, actions) = harness.click(nav_id(panels::PROFILER));
        let text = painted_text(&output);

        assert!(text.contains("Preferences/Styles/Profiler.ron"), "{text}");
        assert!(!text.contains("ConsoleStyle.ron"), "{text}");
        assert!(
            actions.is_empty(),
            "choosing a page is view state, not an action: {actions:?}"
        );
    }

    /// A click ends the interaction: the value is set, then the save is asked
    /// for — once, not once per frame.
    #[test]
    fn a_toggle_click_sets_the_field_and_commits_on_release() {
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());

        let (_, actions) = harness.click(toggle_id(panels::CONSOLE, "enabled"));

        assert_eq!(
            actions,
            vec![
                Action::Style(StyleAction::SetField {
                    panel: panels::CONSOLE,
                    path: "enabled".to_owned(),
                    value: StyleValue::Toggle(true),
                }),
                Action::Style(StyleAction::Commit {
                    panel: panels::CONSOLE,
                }),
            ]
        );
        assert!(
            matches!(harness.panel.page.fields[1].value, StyleValue::Toggle(true)),
            "the page keeps the edited value between frames"
        );
    }

    /// Switching pages saves the page being left, if it has an edit that was
    /// never released.
    #[test]
    fn a_pending_edit_is_saved_when_the_page_changes() {
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());
        harness.panel.page.dirty = true;

        let (_, actions) = harness.click(nav_id(panels::PROFILER));

        assert_eq!(
            actions,
            vec![Action::Style(StyleAction::Commit {
                panel: panels::CONSOLE,
            })],
            "the old page's file is written once, the new page adds nothing"
        );
        assert!(!harness.panel.page.dirty);
    }

    #[test]
    fn closing_with_a_pending_edit_saves_it() {
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());
        harness.panel.page.dirty = true;

        let state = EditorState::new();
        let host = NoHost;
        let frame = Frame::with_host(&state, &host);
        let mut out = UiOut::new();

        assert!(
            harness.panel.on_close(&frame, &mut out),
            "close is not vetoed"
        );
        assert_eq!(
            out.as_slice(),
            &[Action::Style(StyleAction::Commit {
                panel: panels::CONSOLE,
            })]
        );
        assert!(
            !harness.panel.page.dirty,
            "the same edit is not committed twice"
        );
    }

    #[test]
    fn close_asks_the_layout_to_close_the_panel() {
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());

        let (_, actions) = harness.click(close_id());

        assert_eq!(
            actions,
            vec![Action::Layout(LayoutAction::ClosePanel {
                panel: PREFERENCES
            })]
        );
    }

    #[test]
    fn an_empty_skin_list_explains_itself() {
        let mut harness = Harness::new(Vec::new());

        let (output, _) = harness.frame(Vec::new());

        assert!(
            painted_text(&output).contains("No panel skins to edit"),
            "{}",
            painted_text(&output)
        );
    }

    /// A docked Preferences tab can be much shorter than the floating default.
    /// The footer is pinned, so the action stays on screen while the page
    /// scrolls behind it.
    #[test]
    fn the_footer_survives_a_short_window() {
        let mut harness = Harness::with_size(skins(), vec2(460.0, 180.0));
        harness.frame(Vec::new());

        let rect = harness.rect_of(close_id());
        assert!(
            rect.max.y <= 180.0 && rect.max.x <= 460.0,
            "Close is outside the panel: {rect:?}"
        );
    }

    /// The editor edits its own skin too: it is just another panel in the list.
    #[test]
    fn the_preferences_panel_lists_its_own_skin() {
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());

        let state = EditorState::new();
        let host = NoHost;
        let frame = Frame::with_host(&state, &host);
        let own = harness.panel.skin(&frame).expect("preferences has a skin");
        assert_eq!(own.panel, PREFERENCES);
        assert_eq!(own.file, "PreferencesPanel.ron");
        assert_eq!(
            own.title,
            harness.panel.title(&frame).text(),
            "the page is named by the panel's own title, not a second literal"
        );
        assert!(own.fields.iter().any(|field| field.path == "nav_width"));
        assert!(
            own.fields.iter().any(|field| field.path == "row_odd"),
            "the new zebra tokens are editable too"
        );
    }

    #[test]
    fn color_fields_open_our_picker_and_write_the_style() {
        let mut harness = Harness::with_size(color_skins(), vec2(700.0, 800.0));
        harness.frame(Vec::new());

        let field = field_id(panels::CONSOLE, "background");
        let (_, actions) = harness.click(field);
        assert!(
            actions.is_empty(),
            "opening the picker is not a value change: {actions:?}"
        );
        let (output, _) = harness.frame(Vec::new());
        let text = painted_text(&output);
        assert!(text.contains("Swatches"), "{text}");
        assert!(text.contains("Hexadecimal"), "{text}");

        let swatch = color_swatch_id(field, 4);
        assert!(
            harness.ctx.read_response(swatch).is_some(),
            "the picker paints its palette"
        );
        let (_, actions) = harness.click(swatch);
        assert_eq!(
            actions,
            vec![
                Action::Style(StyleAction::SetField {
                    panel: panels::CONSOLE,
                    path: "background".to_owned(),
                    value: StyleValue::Color(Color::from_srgb8([0, 0, 0, 255])),
                }),
                Action::Style(StyleAction::Commit {
                    panel: panels::CONSOLE,
                }),
            ]
        );
        assert_eq!(
            harness.panel.page.fields[0].value,
            StyleValue::Color(Color::from_srgb8([0, 0, 0, 255]))
        );
        assert_eq!(
            harness.panel.page.selected.as_deref(),
            Some("background"),
            "working inside the popup keeps the row selected"
        );
    }

    #[test]
    fn an_hdr_field_shows_the_badge_and_the_intensity_picker() {
        let mut harness = Harness::with_size(color_skins(), vec2(700.0, 800.0));
        harness.frame(Vec::new());

        let field = field_id(panels::CONSOLE, "glow");
        let (_, actions) = harness.click(field);
        assert!(actions.is_empty(), "{actions:?}");
        let (output, _) = harness.frame(Vec::new());
        let text = painted_text(&output);
        assert!(text.contains("HDR"), "{text}");
        assert!(text.contains("HDR Color"), "{text}");
        assert!(text.contains("Intensity"), "{text}");
        assert!(text.contains("+2"), "{text}");
    }

    #[test]
    fn field_rows_alternate_between_two_greys() {
        let style = PreferencesPanelStyle::dark();
        let mut harness = Harness::new(skins());

        let (output, _) = harness.frame(Vec::new());

        let odd = style.row_odd.to_egui();
        let even = style.row_even.to_egui();
        let mut bands: Vec<PaintedRect> = painted_rects(&output)
            .into_iter()
            .filter(|rect| {
                (rect.fill == odd || rect.fill == even)
                    && approx(rect.rect.height(), style.row_height)
            })
            .collect();
        bands.sort_by(|a, b| a.rect.top().total_cmp(&b.rect.top()));

        assert_eq!(bands.len(), 5, "one band per leaf row: {bands:?}");
        let expected = [odd, even, odd, even, odd];
        let actual: Vec<EguiColor32> = bands.iter().map(|band| band.fill).collect();
        assert_eq!(
            actual, expected,
            "the five leaf rows should stripe odd/even/odd/even/odd"
        );
    }

    #[test]
    fn labels_are_left_aligned_and_indented_by_group_depth() {
        let style = PreferencesPanelStyle::dark();
        let mut harness = Harness::new(skins());

        let (output, _) = harness.frame(Vec::new());
        let runs = painted_texts(&output);
        let height = run(&runs, "Height");
        let enabled = run(&runs, "Enabled");
        let warning = run(&runs, "Warning");
        let tint = run(&runs, "Tint");

        assert!(
            approx(height.rect.left(), enabled.rect.left()),
            "left-aligned labels share an edge regardless of text width: Height {:?}, Enabled {:?}",
            height.rect,
            enabled.rect
        );
        assert!(
            approx(warning.rect.left(), height.rect.left() + style.group_indent),
            "a group's leaf is indented one level: Warning {:?} vs Height {:?}",
            warning.rect,
            height.rect
        );
        assert!(
            approx(
                tint.rect.left(),
                height.rect.left() + 2.0 * style.group_indent
            ),
            "a nested group's leaf is indented two levels: Tint {:?} vs Height {:?}",
            tint.rect,
            height.rect
        );

        // The value column does not move with the label's depth.
        let height_box = harness.rect_of(field_id(panels::CONSOLE, "height"));
        let warning_box = harness.rect_of(field_id(panels::CONSOLE, "icons.warning"));
        assert!(
            approx(height_box.left(), warning_box.left()),
            "value boxes share one column: {height_box:?} vs {warning_box:?}"
        );
        assert!(
            height.rect.right() < height_box.left(),
            "the name column ends before the value column: {:?} vs {:?}",
            height.rect,
            height_box
        );
    }

    #[test]
    fn clicking_a_foldout_collapses_its_rows() {
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());

        let (output, actions) = harness.click(group_id(panels::CONSOLE, "icons"));
        let text = painted_text(&output);
        assert!(text.contains("Icons"), "{text}");
        assert!(!text.contains("Warning"), "{text}");
        assert!(!text.contains("Error"), "{text}");
        assert!(
            !text.contains("Textures"),
            "nested headers fold too: {text}"
        );
        assert!(!text.contains("Tint"), "{text}");
        assert!(
            actions.is_empty(),
            "folding is view state, not an action: {actions:?}"
        );

        let (output, _) = harness.click(group_id(panels::CONSOLE, "icons"));
        let text = painted_text(&output);
        assert!(text.contains("Warning"), "{text}");
        assert!(text.contains("Textures"), "{text}");
        assert!(text.contains("Tint"), "{text}");
    }

    #[test]
    fn a_nested_foldout_collapses_only_its_own_rows() {
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());

        let (output, _) = harness.click(group_id(panels::CONSOLE, "icons.textures"));
        let text = painted_text(&output);

        assert!(text.contains("Warning"), "{text}");
        assert!(!text.contains("Tint"), "{text}");
        assert!(text.contains("Textures"), "{text}");
    }

    #[test]
    fn clicking_a_field_name_selects_it_and_outside_clicks_clear_it() {
        let style = PreferencesPanelStyle::dark();
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());

        let (output, _) = harness.click(label_id(panels::CONSOLE, "height"));

        assert_eq!(harness.panel.page.selected.as_deref(), Some("height"));
        let runs = painted_texts(&output);
        assert_eq!(
            run(&runs, "Height").color,
            style.label_active.to_egui(),
            "the selected field's name turns blue"
        );
        let box_rect = harness.rect_of(field_id(panels::CONSOLE, "height"));
        let accent = style.accent.to_egui();
        assert!(
            painted_rects(&output).iter().any(|painted| {
                painted.stroke.color == accent
                    && approx(painted.rect.left(), box_rect.left())
                    && approx(painted.rect.top(), box_rect.top())
                    && approx(painted.rect.width(), box_rect.width())
            }),
            "the selected value box is outlined with the accent: {box_rect:?}"
        );

        // Clicking the page's heading (outside every row) ends the selection.
        let (_, _) = harness.click_at(egui::pos2(200.0, 40.0));
        assert_eq!(harness.panel.page.selected, None);
        // The click's release pass painted the still-selected colour; the next
        // pass is the one that shows the cleared state.
        let (output, _) = harness.frame(Vec::new());
        let runs = painted_texts(&output);
        assert_eq!(
            run(&runs, "Height").color,
            style.label.to_egui(),
            "the name returns to its normal colour"
        );
    }

    #[test]
    fn clicking_a_number_opens_an_editor_and_enter_commits() {
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());

        harness.click(field_id(panels::CONSOLE, "height"));
        assert!(
            harness.panel.page.edit.is_some(),
            "clicking a numeric box starts an inline edit"
        );

        // One pass draws the editor and focuses it; the next pass accepts text.
        harness.frame(Vec::new());
        harness.frame(Vec::new());
        let (_, actions) = harness.frame(vec![egui::Event::Text("30".to_owned())]);

        assert_eq!(
            harness.panel.page.fields[0].value,
            StyleValue::Float(30.0),
            "typing replaces the selected number"
        );
        assert!(
            actions.contains(&Action::Style(StyleAction::SetField {
                panel: panels::CONSOLE,
                path: "height".to_owned(),
                value: StyleValue::Float(30.0),
            })),
            "the live value follows the text: {actions:?}"
        );

        let (_, actions) = harness.frame(vec![egui::Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }]);
        assert!(harness.panel.page.edit.is_none(), "Enter ends the edit");
        assert!(
            actions.contains(&Action::Style(StyleAction::Commit {
                panel: panels::CONSOLE,
            })),
            "the end of a typed edit writes the file: {actions:?}"
        );
    }

    /// The value box is Unity's inset rail: dark fill, one quiet outline, and
    /// a left-aligned value.
    #[test]
    fn value_boxes_use_the_unity_fill_and_outline() {
        let style = PreferencesPanelStyle::dark();
        let mut harness = Harness::new(skins());

        let (output, _) = harness.frame(Vec::new());
        let rect = harness.rect_of(field_id(panels::CONSOLE, "icons.warning"));
        let painted = painted_rects(&output);

        assert!(
            painted.iter().any(|shape| {
                shape.fill == style.control_rail.to_egui()
                    && approx(shape.rect.left(), rect.left())
                    && approx(shape.rect.top(), rect.top())
                    && approx(shape.rect.width(), rect.width())
                    && approx(shape.rect.height(), rect.height())
            }),
            "the icon-path box uses the control rail: {rect:?}\nnear: {:?}",
            painted
                .iter()
                .filter(|shape| approx(shape.rect.top(), rect.top()))
                .map(|shape| (shape.rect, shape.fill, shape.stroke))
                .collect::<Vec<_>>()
        );
        assert!(
            painted.iter().any(|shape| {
                shape.stroke.color == style.control_edge.to_egui()
                    && approx(shape.rect.width(), rect.width())
                    && approx(shape.rect.height(), rect.height())
            }),
            "the icon-path box has one quiet outline"
        );
    }

    /// The icon-path box used to run under the scroll bar. The page reserves
    /// its gutter before laying the value column out.
    #[test]
    fn the_value_column_stays_clear_of_the_scroll_bar() {
        let mut harness = Harness::new(long_skin());
        harness.frame(Vec::new());
        let (_, _) = harness.frame(Vec::new());

        let rect = harness.rect_of(field_id(panels::CONSOLE, "icons.warning"));
        let panel_right = 537.0 - 14.0;
        assert!(
            rect.right() <= panel_right - 8.0 + 0.01,
            "the value box leaves the scroll-bar gutter clear: {rect:?}"
        );
        assert!(
            rect.right() > 400.0,
            "the box is still the wide value column, not a squeezed stump: {rect:?}"
        );
    }

    /// A narrow dock still keeps the Unity two-column row; it does not stack
    /// the value under the name.
    #[test]
    fn a_narrow_page_keeps_two_columns_inside_the_panel() {
        let mut harness = Harness::with_size(skins(), vec2(460.0, 316.0));
        let (output, _) = harness.frame(Vec::new());
        let runs = painted_texts(&output);
        let height = run(&runs, "Height");
        let rect = harness.rect_of(field_id(panels::CONSOLE, "height"));

        assert!(
            height.rect.left() < rect.left(),
            "the name stays to the left of its value: {:?} vs {rect:?}",
            height.rect
        );
        assert!(
            rect.right() <= 460.0 - 14.0 - 8.0 + 0.01,
            "the value box stays inside the dock: {rect:?}"
        );
        assert!(
            rect.width() >= 100.0,
            "the label column shrank instead of the value box: {rect:?}"
        );
    }

    #[test]
    fn numeric_text_parses_and_clamps() {
        assert_eq!(
            parse_number(StyleKind::Float, " 1.5 "),
            Some(StyleValue::Float(1.5))
        );
        assert_eq!(parse_number(StyleKind::Float, "NaN"), None);
        assert_eq!(parse_number(StyleKind::Float, ""), None);
        assert_eq!(
            parse_number(StyleKind::Integer { max: 255 }, "300"),
            Some(StyleValue::Integer(255))
        );
        assert_eq!(
            parse_number(StyleKind::Integer { max: 255 }, "-4"),
            Some(StyleValue::Integer(0))
        );
        assert_eq!(number_text(&StyleValue::Float(20.0)), "20");
        assert_eq!(number_text(&StyleValue::Float(-0.0)), "0");
        assert_eq!(number_text(&StyleValue::Integer(3)), "3");
        assert_eq!(group_path("icons.console.info"), "icons.console");
        assert_eq!(group_path("height"), "");
    }
}
