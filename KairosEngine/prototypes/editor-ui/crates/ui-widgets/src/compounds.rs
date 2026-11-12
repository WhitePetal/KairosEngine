//! Compounds: themed compositions that map plain-data props in and plain-data
//! events out. Every compound follows the same contract: construct it with
//! plain-data props (Widget::new(props)), call widget.show(ui), then map the
//! returned Output to a domain Action. Rules:
//! - props are plain data (no domain types, no closures carrying control flow);
//! - Output is plain data (clicked ids, booleans, rects) that a feature
//!   component maps to a domain Action;
//! - no state is stored in the compound struct itself across frames: open/
//!   collapsed/text-edit state lives in egui Memory (built-in widgets) or in
//!   the caller's ViewState. show(&mut self) exists because the task spec asked
//!   for one uniform signature; most compounds are stateless (README lesson).
//! - hit rects are returned in Output for (a) host-side overlays such as
//!   context menus and (b) headless interaction tests. egui offers no public
//!   widget-tree query, so this is the pragmatic seam. See README open question 2.

use egui::{
    Align2, Color32, CornerRadius, Id, Key, Layout, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2,
    pos2, vec2,
};

use crate::atoms::{Button, ButtonVariant, IconButton, TextInput};
use crate::icon::IconId;
use crate::theme::Theme;

// ---------------------------------------------------------------------------
// shared painting helpers
// ---------------------------------------------------------------------------

fn row_background(ui: &Ui, rect: Rect, theme: &Theme, selected: bool, hovered: bool) {
    if selected {
        ui.painter()
            .rect_filled(rect, CornerRadius::ZERO, theme.selected_fill());
    } else if hovered {
        ui.painter()
            .rect_filled(rect, CornerRadius::ZERO, theme.hover_fill());
    }
}

fn paint_text(ui: &Ui, pos: Pos2, anchor: Align2, text: &str, size: f32, color: Color32) {
    ui.painter()
        .text(pos, anchor, text, egui::FontId::proportional(size), color);
}

fn paint_glyph(ui: &Ui, center: Pos2, glyph: &str, size: f32, color: Color32) {
    ui.painter().text(
        center,
        Align2::CENTER_CENTER,
        glyph,
        egui::FontId::proportional(size),
        color,
    );
}

// ---------------------------------------------------------------------------
// Toolbar
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolbarItemKind {
    Button,
    /// Stays highlighted while active (e.g. a tool mode).
    Toggle,
}

#[derive(Clone, Copy, Debug)]
pub struct ToolbarItem {
    pub id: &'static str,
    pub kind: ToolbarItemKind,
    pub icon: Option<IconId>,
    pub label: Option<&'static str>,
    pub tooltip: Option<&'static str>,
    pub enabled: bool,
    pub active: bool,
}

impl ToolbarItem {
    pub fn button(id: &'static str) -> Self {
        Self {
            id,
            kind: ToolbarItemKind::Button,
            icon: None,
            label: None,
            tooltip: None,
            enabled: true,
            active: false,
        }
    }

    pub fn toggle(id: &'static str, active: bool) -> Self {
        Self {
            kind: ToolbarItemKind::Toggle,
            active,
            ..Self::button(id)
        }
    }

    pub fn icon(mut self, icon: IconId) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn label(mut self, label: &'static str) -> Self {
        self.label = Some(label);
        self
    }

    pub fn tooltip(mut self, tooltip: &'static str) -> Self {
        self.tooltip = Some(tooltip);
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ToolbarOutput {
    pub clicked: Option<&'static str>,
    pub hovered: Option<&'static str>,
    /// Hit rects, keyed by item id (tests + host-side overlays).
    pub rects: Vec<(&'static str, Rect)>,
}

pub struct Toolbar {
    items: Vec<ToolbarItem>,
}

impl Toolbar {
    pub fn new(items: Vec<ToolbarItem>) -> Self {
        Self { items }
    }

    pub fn show(&mut self, ui: &mut Ui) -> ToolbarOutput {
        let theme = Theme::get(ui);
        let mut output = ToolbarOutput::default();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = theme.space_xs;
            for item in &self.items {
                let response = match item.label {
                    Some(label) => {
                        let mut button = Button::new(label)
                            .variant(if item.active {
                                ButtonVariant::Secondary
                            } else {
                                ButtonVariant::Ghost
                            })
                            .small()
                            .enabled(item.enabled);
                        if let Some(icon) = item.icon {
                            button = button.icon(icon);
                        }
                        ui.add(button)
                    }
                    None => {
                        let mut button = IconButton::new(item.icon.unwrap_or(IconId::Dot))
                            .enabled(item.enabled)
                            .active(item.active);
                        if let Some(tooltip) = item.tooltip {
                            button = button.tooltip(tooltip);
                        }
                        ui.add(button)
                    }
                };
                if response.clicked() {
                    output.clicked = Some(item.id);
                }
                if response.hovered() {
                    output.hovered = Some(item.id);
                }
                output.rects.push((item.id, response.rect));
            }
        });
        output
    }
}

// ---------------------------------------------------------------------------
// PanelChrome
// ---------------------------------------------------------------------------

pub struct PanelChrome<'a> {
    title: &'a str,
    closable: bool,
    toolbar: Option<Toolbar>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PanelChromeOutput {
    pub close_clicked: bool,
    pub toolbar: Option<ToolbarOutput>,
    pub header_rect: Rect,
}

impl Default for PanelChromeOutput {
    fn default() -> Self {
        Self {
            close_clicked: false,
            toolbar: None,
            header_rect: Rect::NOTHING,
        }
    }
}

impl<'a> PanelChrome<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            closable: false,
            toolbar: None,
        }
    }

    pub fn closable(mut self, closable: bool) -> Self {
        self.closable = closable;
        self
    }

    pub fn toolbar(mut self, toolbar: Toolbar) -> Self {
        self.toolbar = Some(toolbar);
        self
    }

    /// Draws the header (title + optional toolbar + close) and then the body.
    pub fn show(&mut self, ui: &mut Ui, body: impl FnOnce(&mut Ui)) -> PanelChromeOutput {
        let theme = Theme::get(ui);
        let mut close_clicked = false;
        let mut toolbar_output = None;
        let frame = egui::Frame::new()
            .fill(theme.bg)
            .inner_margin(egui::Margin::symmetric(
                theme.space_sm as i8,
                theme.space_xs as i8,
            ));
        let header = frame
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.set_height(theme.row_height);
                    ui.add(
                        egui::Label::new(
                            RichText::new(self.title)
                                .size(theme.font_subtitle)
                                .strong()
                                .color(theme.text),
                        )
                        .selectable(false),
                    );
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.closable {
                            close_clicked = ui
                                .add(IconButton::new(IconId::Close).tooltip("Close"))
                                .clicked();
                        }
                        if let Some(toolbar) = &mut self.toolbar {
                            toolbar_output = Some(toolbar.show(ui));
                        }
                    });
                });
            })
            .response;
        ui.painter().hline(
            header.rect.x_range(),
            header.rect.bottom(),
            Stroke::new(1.0, theme.border),
        );
        ui.add_space(theme.space_xs);
        body(ui);
        PanelChromeOutput {
            close_clicked,
            toolbar: toolbar_output,
            header_rect: header.rect,
        }
    }
}

// ---------------------------------------------------------------------------
// ListItem
// ---------------------------------------------------------------------------

pub struct ListItem<'a> {
    label: &'a str,
    icon: Option<IconId>,
    suffix: Option<&'a str>,
    selected: bool,
    disabled: bool,
}

pub struct ListItemOutput {
    pub clicked: bool,
    pub double_clicked: bool,
    pub rect: Rect,
}

impl<'a> ListItem<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            icon: None,
            suffix: None,
            selected: false,
            disabled: false,
        }
    }

    pub fn icon(mut self, icon: IconId) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn suffix(mut self, suffix: &'a str) -> Self {
        self.suffix = Some(suffix);
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn show(&mut self, ui: &mut Ui) -> ListItemOutput {
        let theme = Theme::get(ui);
        let width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(vec2(width, theme.row_height), Sense::hover());
        let id = ui.make_persistent_id(("kairos-ui.list-item", self.label));
        let sense = if self.disabled {
            Sense::hover()
        } else {
            Sense::click()
        };
        let response = ui.interact(rect, id, sense);
        row_background(
            ui,
            rect,
            &theme,
            self.selected,
            response.hovered() && !self.disabled,
        );
        let color = if self.disabled {
            theme.text_muted
        } else {
            theme.text
        };
        let mut x = rect.left() + theme.space_sm;
        if let Some(icon) = self.icon {
            paint_glyph(
                ui,
                pos2(x + theme.icon_md * 0.5, rect.center().y),
                theme.icon(icon),
                theme.icon_md,
                color,
            );
            x += theme.icon_md + theme.space_sm;
        }
        paint_text(
            ui,
            pos2(x, rect.center().y),
            Align2::LEFT_CENTER,
            self.label,
            theme.font_body,
            color,
        );
        if let Some(suffix) = self.suffix {
            paint_text(
                ui,
                pos2(rect.right() - theme.space_sm, rect.center().y),
                Align2::RIGHT_CENTER,
                suffix,
                theme.font_caption,
                theme.text_muted,
            );
        }
        ListItemOutput {
            clicked: response.clicked(),
            double_clicked: response.double_clicked(),
            rect,
        }
    }
}

// ---------------------------------------------------------------------------
// TreeView
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expander {
    /// Leaf: reserves the expander column but draws nothing.
    None,
    Collapsed,
    Expanded,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TreeRow<'a> {
    pub id: u64,
    pub indent: u8,
    pub expander: Expander,
    pub icon: Option<IconId>,
    pub label: &'a str,
    pub suffix: Option<&'a str>,
    pub disabled: bool,
    pub selected: bool,
}

impl<'a> TreeRow<'a> {
    pub fn new(id: u64, label: &'a str) -> Self {
        Self {
            id,
            indent: 0,
            expander: Expander::None,
            icon: None,
            label,
            suffix: None,
            disabled: false,
            selected: false,
        }
    }

    pub fn indent(mut self, indent: u8) -> Self {
        self.indent = indent;
        self
    }

    pub fn expander(mut self, expander: Expander) -> Self {
        self.expander = expander;
        self
    }

    pub fn icon(mut self, icon: IconId) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn suffix(mut self, suffix: &'a str) -> Self {
        self.suffix = Some(suffix);
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TreeOutput {
    pub clicked: Option<u64>,
    pub double_clicked: Option<u64>,
    pub context_menu: Option<u64>,
    pub context_menu_pos: Option<Pos2>,
    pub drag_started: Option<u64>,
    pub drag_stopped: Option<u64>,
    /// Click on the expander chevron (a view-state toggle, never a domain Action).
    pub expander_toggled: Option<u64>,
    pub row_rects: Vec<(u64, Rect)>,
}

pub struct TreeView<'a> {
    salt: &'a str,
    rows: Vec<TreeRow<'a>>,
}

impl<'a> TreeView<'a> {
    pub fn new(salt: &'a str) -> Self {
        Self {
            salt,
            rows: Vec::new(),
        }
    }

    pub fn rows(mut self, rows: Vec<TreeRow<'a>>) -> Self {
        self.rows = rows;
        self
    }

    pub fn show(&mut self, ui: &mut Ui) -> TreeOutput {
        let theme = Theme::get(ui);
        let indent_step = theme.space_md;
        let root = ui.make_persistent_id(("kairos-ui.tree", self.salt));
        let mut output = TreeOutput::default();

        for row in &self.rows {
            let width = ui.available_width();
            let (rect, _) = ui.allocate_exact_size(vec2(width, theme.row_height), Sense::hover());
            let id = root.with(row.id);
            let sense = if row.disabled {
                Sense::hover()
            } else {
                Sense::click_and_drag()
            };
            let response = ui.interact(rect, id, sense);
            row_background(
                ui,
                rect,
                &theme,
                row.selected,
                response.hovered() && !row.disabled,
            );
            let color = if row.disabled {
                theme.text_muted
            } else {
                theme.text
            };

            let mut x = rect.left() + theme.space_sm + row.indent as f32 * indent_step;
            let mut expander_rect = None;
            match row.expander {
                Expander::None => x += theme.icon_sm + theme.space_xs * 2.0,
                Expander::Collapsed | Expander::Expanded => {
                    let expander = Rect::from_center_size(
                        pos2(x + theme.icon_sm * 0.5, rect.center().y),
                        vec2(theme.icon_sm + theme.space_xs * 2.0, theme.row_height),
                    );
                    expander_rect = Some(expander);
                    let glyph = match row.expander {
                        Expander::Collapsed => theme.icon(IconId::ChevronRight),
                        Expander::Expanded => theme.icon(IconId::ChevronDown),
                        Expander::None => "",
                    };
                    paint_glyph(
                        ui,
                        expander.center(),
                        glyph,
                        theme.icon_sm,
                        theme.text_muted,
                    );
                    x += theme.icon_sm + theme.space_xs * 2.0;
                }
            }
            if let Some(icon) = row.icon {
                paint_glyph(
                    ui,
                    pos2(x + theme.icon_md * 0.5, rect.center().y),
                    theme.icon(icon),
                    theme.icon_md,
                    color,
                );
                x += theme.icon_md + theme.space_sm;
            }
            paint_text(
                ui,
                pos2(x, rect.center().y),
                Align2::LEFT_CENTER,
                row.label,
                theme.font_body,
                color,
            );
            if let Some(suffix) = row.suffix {
                paint_text(
                    ui,
                    pos2(rect.right() - theme.space_sm, rect.center().y),
                    Align2::RIGHT_CENTER,
                    suffix,
                    theme.font_caption,
                    theme.text_muted,
                );
            }

            output.row_rects.push((row.id, rect));

            if response.clicked() {
                let pointer = response.interact_pointer_pos().unwrap_or(rect.center());
                if let Some(expander) = expander_rect.filter(|expander| expander.contains(pointer))
                {
                    let _ = expander;
                    output.expander_toggled = Some(row.id);
                } else {
                    output.clicked = Some(row.id);
                }
            }
            if response.double_clicked() {
                output.double_clicked = Some(row.id);
            }
            if response.secondary_clicked() {
                output.context_menu = Some(row.id);
                output.context_menu_pos = response.interact_pointer_pos();
            }
            if response.drag_started() {
                output.drag_started = Some(row.id);
            }
            if response.drag_stopped() {
                output.drag_stopped = Some(row.id);
            }
        }
        output
    }
}

// ---------------------------------------------------------------------------
// PropertyRow
// ---------------------------------------------------------------------------

pub struct PropertyRow<'a> {
    label: &'a str,
    reset_enabled: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PropertyRowOutput {
    /// The editor slot's response reported a change (e.g. TextEdit changed).
    pub changed: bool,
    pub reset_clicked: bool,
    pub editor_rect: Rect,
    pub reset_rect: Option<Rect>,
}

impl Default for PropertyRowOutput {
    fn default() -> Self {
        Self {
            changed: false,
            reset_clicked: false,
            editor_rect: Rect::NOTHING,
            reset_rect: None,
        }
    }
}

impl<'a> PropertyRow<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            reset_enabled: false,
        }
    }

    pub fn reset_enabled(mut self, reset_enabled: bool) -> Self {
        self.reset_enabled = reset_enabled;
        self
    }

    /// Draws label | editor (closure) | optional reset button.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        editor: impl FnOnce(&mut Ui) -> egui::Response,
    ) -> PropertyRowOutput {
        let theme = Theme::get(ui);
        let mut output = PropertyRowOutput::default();
        ui.horizontal(|ui| {
            ui.set_height(theme.row_height);
            let label_width = (ui.available_width() * 0.34).max(56.0);
            ui.add_sized(
                [label_width, theme.row_height],
                egui::Label::new(
                    RichText::new(self.label)
                        .size(theme.font_body)
                        .color(theme.text_muted),
                )
                .selectable(false),
            );
            let reset_width = if self.reset_enabled {
                theme.row_height + theme.space_xs
            } else {
                0.0
            };
            let editor_width = (ui.available_width() - reset_width - theme.space_sm).max(48.0);
            let response = ui
                .allocate_ui_with_layout(
                    vec2(editor_width, theme.row_height),
                    Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.set_width(editor_width);
                        editor(ui)
                    },
                )
                .inner;
            output.changed = response.changed();
            output.editor_rect = response.rect;
            if self.reset_enabled {
                let response = ui.add(IconButton::new(IconId::Revert).tooltip("Reset"));
                output.reset_clicked = response.clicked();
                output.reset_rect = Some(response.rect);
            }
        });
        output
    }
}

// ---------------------------------------------------------------------------
// FormSection
// ---------------------------------------------------------------------------

pub struct FormSection<'a> {
    title: &'a str,
    default_open: bool,
}

pub struct FormSectionOutput {
    pub open: bool,
}

impl<'a> FormSection<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            default_open: true,
        }
    }

    pub fn default_open(mut self, default_open: bool) -> Self {
        self.default_open = default_open;
        self
    }

    pub fn show(&mut self, ui: &mut Ui, body: impl FnOnce(&mut Ui)) -> FormSectionOutput {
        let theme = Theme::get(ui);
        let response = egui::CollapsingHeader::new(
            RichText::new(self.title)
                .size(theme.font_caption)
                .strong()
                .color(theme.text_muted),
        )
        .default_open(self.default_open)
        .show(ui, body);
        FormSectionOutput {
            open: response.body_response.is_some(),
        }
    }
}

// ---------------------------------------------------------------------------
// SearchBox
// ---------------------------------------------------------------------------

pub struct SearchBox<'a> {
    hint: &'a str,
    width: Option<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchBoxOutput {
    pub changed: bool,
    pub submitted: bool,
    pub rect: Rect,
}

impl Default for SearchBoxOutput {
    fn default() -> Self {
        Self {
            changed: false,
            submitted: false,
            rect: Rect::NOTHING,
        }
    }
}

impl<'a> SearchBox<'a> {
    pub fn new(hint: &'a str) -> Self {
        Self { hint, width: None }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn show(&mut self, ui: &mut Ui, text: &mut String) -> SearchBoxOutput {
        let theme = Theme::get(ui);
        let mut output = SearchBoxOutput::default();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = theme.space_xs;
            ui.add(
                egui::Label::new(
                    RichText::new(theme.icon(IconId::Search))
                        .size(theme.icon_md)
                        .color(theme.text_muted),
                )
                .selectable(false),
            );
            let width = self.width.unwrap_or_else(|| {
                (ui.available_width() - theme.row_height - theme.space_md).max(64.0)
            });
            let response = ui.add(
                TextInput::new(&mut *text)
                    .hint(self.hint)
                    .desired_width(width),
            );
            output.rect = response.rect;
            output.changed = response.changed();
            output.submitted = response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            if !text.is_empty()
                && ui
                    .add(IconButton::new(IconId::Close).tooltip("Clear"))
                    .clicked()
            {
                text.clear();
                output.changed = true;
            }
        });
        output
    }
}

// ---------------------------------------------------------------------------
// Breadcrumb
// ---------------------------------------------------------------------------

pub struct Breadcrumb<'a> {
    segments: Vec<&'a str>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BreadcrumbOutput {
    pub clicked: Option<usize>,
    pub rect: Rect,
}

impl Default for BreadcrumbOutput {
    fn default() -> Self {
        Self {
            clicked: None,
            rect: Rect::NOTHING,
        }
    }
}

impl<'a> Breadcrumb<'a> {
    pub fn new(segments: Vec<&'a str>) -> Self {
        Self { segments }
    }

    pub fn show(&mut self, ui: &mut Ui) -> BreadcrumbOutput {
        let theme = Theme::get(ui);
        let mut output = BreadcrumbOutput::default();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = theme.space_xs;
            let mut min = pos2(f32::INFINITY, f32::INFINITY);
            let mut max = pos2(f32::NEG_INFINITY, f32::NEG_INFINITY);
            for (index, segment) in self.segments.iter().enumerate() {
                if index > 0 {
                    ui.add(
                        egui::Label::new(
                            RichText::new("/")
                                .size(theme.font_caption)
                                .color(theme.text_muted),
                        )
                        .selectable(false),
                    );
                }
                let color = if index + 1 == self.segments.len() {
                    theme.text
                } else {
                    theme.accent
                };
                let response = ui.add(
                    egui::Label::new(RichText::new(*segment).size(theme.font_body).color(color))
                        .selectable(false)
                        .sense(Sense::click()),
                );
                if response.clicked() {
                    output.clicked = Some(index);
                }
                min = min.min(response.rect.min);
                max = max.max(response.rect.max);
            }
            output.rect = Rect::from_min_max(min, max);
        });
        output
    }
}

// ---------------------------------------------------------------------------
// StatusBar
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusTone {
    Normal,
    Error,
    Success,
}

pub struct StatusBar<'a> {
    left: &'a str,
    right: &'a str,
    tone: StatusTone,
}

pub struct StatusBarOutput {
    pub rect: Rect,
}

impl<'a> StatusBar<'a> {
    pub fn new(left: &'a str, right: &'a str) -> Self {
        Self {
            left,
            right,
            tone: StatusTone::Normal,
        }
    }

    pub fn tone(mut self, tone: StatusTone) -> Self {
        self.tone = tone;
        self
    }

    pub fn show(&mut self, ui: &mut Ui) -> StatusBarOutput {
        let theme = Theme::get(ui);
        let width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(vec2(width, theme.row_height), Sense::hover());
        ui.painter().rect_filled(rect, CornerRadius::ZERO, theme.bg);
        ui.painter()
            .hline(rect.x_range(), rect.top(), Stroke::new(1.0, theme.border));
        let color = match self.tone {
            StatusTone::Normal => theme.text_muted,
            StatusTone::Error => theme.error,
            StatusTone::Success => theme.success,
        };
        paint_text(
            ui,
            pos2(rect.left() + theme.space_sm, rect.center().y),
            Align2::LEFT_CENTER,
            self.left,
            theme.font_caption,
            color,
        );
        paint_text(
            ui,
            pos2(rect.right() - theme.space_sm, rect.center().y),
            Align2::RIGHT_CENTER,
            self.right,
            theme.font_caption,
            theme.text_muted,
        );
        StatusBarOutput { rect }
    }
}

// ---------------------------------------------------------------------------
// ConfirmDialog (modal = data, no callbacks)
// ---------------------------------------------------------------------------

pub struct ConfirmDialog<'a> {
    title: &'a str,
    body: &'a str,
    confirm_label: &'a str,
    cancel_label: &'a str,
    danger: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConfirmDialogOutput {
    pub confirm: bool,
    pub cancel: bool,
    pub confirm_rect: Rect,
    pub cancel_rect: Rect,
}

impl Default for ConfirmDialogOutput {
    fn default() -> Self {
        Self {
            confirm: false,
            cancel: false,
            confirm_rect: Rect::NOTHING,
            cancel_rect: Rect::NOTHING,
        }
    }
}

impl<'a> ConfirmDialog<'a> {
    pub fn new(title: &'a str, body: &'a str) -> Self {
        Self {
            title,
            body,
            confirm_label: "Confirm",
            cancel_label: "Cancel",
            danger: false,
        }
    }

    pub fn confirm_label(mut self, label: &'a str) -> Self {
        self.confirm_label = label;
        self
    }

    pub fn cancel_label(mut self, label: &'a str) -> Self {
        self.cancel_label = label;
        self
    }

    pub fn danger(mut self, danger: bool) -> Self {
        self.danger = danger;
        self
    }

    /// Draws the dialog as a centered window. The caller owns ModalState; this
    /// widget only reports which button was clicked this frame.
    pub fn show(&mut self, ui: &mut Ui) -> ConfirmDialogOutput {
        let theme = Theme::get(ui);
        let ctx = ui.ctx().clone();
        let mut output = ConfirmDialogOutput::default();
        egui::Window::new(self.title)
            .id(Id::new(("kairos-ui.confirm-dialog", self.title)))
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .collapsible(false)
            .resizable(false)
            .show(&ctx, |ui| {
                ui.set_min_width(300.0);
                ui.add(
                    egui::Label::new(
                        RichText::new(self.body)
                            .size(theme.font_body)
                            .color(theme.text),
                    )
                    .selectable(false),
                );
                ui.add_space(theme.space_md);
                ui.horizontal(|ui| {
                    let response =
                        ui.add(Button::new(self.confirm_label).variant(if self.danger {
                            ButtonVariant::Danger
                        } else {
                            ButtonVariant::Primary
                        }));
                    output.confirm_rect = response.rect;
                    if response.clicked() {
                        output.confirm = true;
                    }
                    let response = ui.add(Button::new(self.cancel_label));
                    output.cancel_rect = response.rect;
                    if response.clicked() {
                        output.cancel = true;
                    }
                });
            });
        output
    }
}

// ---------------------------------------------------------------------------
// EmptyState
// ---------------------------------------------------------------------------

pub struct EmptyState<'a> {
    title: &'a str,
    hint: &'a str,
    icon: Option<IconId>,
}

pub struct EmptyStateOutput {
    pub rect: Rect,
}

impl<'a> EmptyState<'a> {
    pub fn new(title: &'a str, hint: &'a str) -> Self {
        Self {
            title,
            hint,
            icon: None,
        }
    }

    pub fn icon(mut self, icon: IconId) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn show(&mut self, ui: &mut Ui) -> EmptyStateOutput {
        let theme = Theme::get(ui);
        ui.vertical_centered(|ui| {
            ui.add_space(theme.space_lg);
            if let Some(icon) = self.icon {
                ui.add(
                    egui::Label::new(
                        RichText::new(theme.icon(icon))
                            .size(theme.icon_md * 2.0)
                            .color(theme.text_muted),
                    )
                    .selectable(false),
                );
                ui.add_space(theme.space_sm);
            }
            ui.add(
                egui::Label::new(
                    RichText::new(self.title)
                        .size(theme.font_subtitle)
                        .color(theme.text_muted),
                )
                .selectable(false),
            );
            ui.add(
                egui::Label::new(
                    RichText::new(self.hint)
                        .size(theme.font_body)
                        .color(theme.text_muted),
                )
                .selectable(false),
            );
        });
        EmptyStateOutput {
            rect: ui.min_rect(),
        }
    }
}

// ---------------------------------------------------------------------------
// ContextMenu (data-driven; caller decides when it is open / where it closes)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub struct ContextMenuItem {
    pub id: &'static str,
    pub label: &'static str,
    pub icon: Option<IconId>,
    pub enabled: bool,
    pub danger: bool,
}

impl ContextMenuItem {
    pub fn new(id: &'static str, label: &'static str) -> Self {
        Self {
            id,
            label,
            icon: None,
            enabled: true,
            danger: false,
        }
    }

    pub fn icon(mut self, icon: IconId) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn danger(mut self, danger: bool) -> Self {
        self.danger = danger;
        self
    }
}

pub struct ContextMenu {
    id: &'static str,
    pos: Pos2,
    items: Vec<ContextMenuItem>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ContextMenuOutput {
    pub selected: Option<&'static str>,
    pub rect: Rect,
}

impl Default for ContextMenuOutput {
    fn default() -> Self {
        Self {
            selected: None,
            rect: Rect::NOTHING,
        }
    }
}

impl ContextMenu {
    pub fn new(id: &'static str, pos: Pos2) -> Self {
        Self {
            id,
            pos,
            items: Vec::new(),
        }
    }

    pub fn items(mut self, items: Vec<ContextMenuItem>) -> Self {
        self.items = items;
        self
    }

    pub fn show(&mut self, ui: &mut Ui) -> ContextMenuOutput {
        let theme = Theme::get(ui);
        let ctx = ui.ctx().clone();
        let mut selected = None;
        let frame = egui::Frame::new()
            .fill(theme.panel)
            .stroke(Stroke::new(1.0, theme.border))
            .corner_radius(theme.radius_md)
            .inner_margin(egui::Margin::same(theme.space_xs as i8));
        let area = egui::Area::new(Id::new(("kairos-ui.context-menu", self.id)))
            .order(egui::Order::Foreground)
            .fixed_pos(self.pos)
            .show(&ctx, |ui| {
                frame.show(ui, |ui| {
                    ui.set_min_width(160.0);
                    for item in &self.items {
                        let mut button = Button::new(item.label)
                            .variant(if item.danger {
                                ButtonVariant::Danger
                            } else {
                                ButtonVariant::Ghost
                            })
                            .enabled(item.enabled);
                        if let Some(icon) = item.icon {
                            button = button.icon(icon);
                        }
                        if ui.add(button).clicked() {
                            selected = Some(item.id);
                        }
                    }
                });
            });
        ContextMenuOutput {
            selected,
            rect: area.response.rect,
        }
    }
}

// ---------------------------------------------------------------------------
// Toast / Notification
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastTone {
    Info,
    Success,
    Error,
}

pub struct Toast<'a> {
    id: &'a str,
    message: &'a str,
    tone: ToastTone,
    dismissible: bool,
}

pub struct ToastOutput {
    pub dismissed: bool,
    pub rect: Rect,
}

impl<'a> Toast<'a> {
    pub fn new(id: &'a str, message: &'a str) -> Self {
        Self {
            id,
            message,
            tone: ToastTone::Info,
            dismissible: false,
        }
    }

    pub fn tone(mut self, tone: ToastTone) -> Self {
        self.tone = tone;
        self
    }

    pub fn dismissible(mut self, dismissible: bool) -> Self {
        self.dismissible = dismissible;
        self
    }

    pub fn show(&mut self, ui: &mut Ui) -> ToastOutput {
        let theme = Theme::get(ui);
        let ctx = ui.ctx().clone();
        let (icon, color) = match self.tone {
            ToastTone::Info => (IconId::Info, theme.text),
            ToastTone::Success => (IconId::Check, theme.success),
            ToastTone::Error => (IconId::Warning, theme.error),
        };
        let mut dismissed = false;
        let frame = egui::Frame::new()
            .fill(theme.panel)
            .stroke(Stroke::new(1.0, theme.border))
            .corner_radius(theme.radius_md)
            .inner_margin(egui::Margin::symmetric(
                theme.space_md as i8,
                theme.space_sm as i8,
            ));
        let area = egui::Area::new(Id::new(("kairos-ui.toast", self.id)))
            .order(egui::Order::Tooltip)
            .anchor(Align2::RIGHT_TOP, vec2(-theme.space_lg, theme.space_lg))
            .show(&ctx, |ui| {
                frame.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::Label::new(
                                RichText::new(theme.icon(icon))
                                    .size(theme.icon_md)
                                    .color(color),
                            )
                            .selectable(false),
                        );
                        ui.add(
                            egui::Label::new(
                                RichText::new(self.message)
                                    .size(theme.font_body)
                                    .color(theme.text),
                            )
                            .selectable(false),
                        );
                        if self.dismissible
                            && ui
                                .add(IconButton::new(IconId::Close).tooltip("Dismiss"))
                                .clicked()
                        {
                            dismissed = true;
                        }
                    });
                });
            });
        ToastOutput {
            dismissed,
            rect: area.response.rect,
        }
    }
}

// ---------------------------------------------------------------------------
// TabBar
// ---------------------------------------------------------------------------

/// One tab in a [TabBar].
///
/// Plain data: the bar has no domain knowledge. The feature component maps its
/// panel titles to items and maps the returned indices back to domain ids.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TabBarItem {
    pub title: String,
    /// When false the tab draws no close affordance and never reports
    /// close_clicked for this index.
    pub closable: bool,
}

impl TabBarItem {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            closable: true,
        }
    }

    pub fn closable(mut self, closable: bool) -> Self {
        self.closable = closable;
        self
    }
}

/// Interaction data of one [TabBar] frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TabBarOutput {
    /// Index of the tab clicked by the user. None when nothing was clicked and
    /// when the click landed on a close affordance (close must not also focus).
    pub activated: Option<usize>,
    /// Index of the tab whose close affordance was clicked.
    pub close_clicked: Option<usize>,
    /// Hit rects keyed by tab index (tests + host-side overlay anchoring).
    pub tab_rects: Vec<(usize, Rect)>,
    /// Hit rects of the close affordances keyed by tab index. Only closable tabs
    /// contribute an entry.
    pub close_rects: Vec<(usize, Rect)>,
}

/// Horizontal tab strip: the pure, themed compound used for panel tab bars.
///
/// The bar owns no persistent state: the active tab is passed in each frame
/// (the feature component derives it from domain state) and interaction is
/// returned as plain indices. The calling host owns the tab/title bar, so panel
/// bodies must not repeat the title.
///
/// Interaction ids are derived from the parent Ui id, so two TabBars in the same
/// Ui layout need distinct parent ids (docked panels get that for free). Tabs
/// that exceed the available width are clipped by the host panel; an overflow
/// menu is a caller-level extension, not part of this compound yet.
pub struct TabBar {
    items: Vec<TabBarItem>,
    active: Option<usize>,
}

impl TabBar {
    pub fn new(items: Vec<TabBarItem>) -> Self {
        Self {
            items,
            active: None,
        }
    }

    /// Mark tab index as active. An out-of-range index simply selects nothing.
    pub fn active(mut self, index: usize) -> Self {
        self.active = Some(index);
        self
    }

    pub fn show(&mut self, ui: &mut Ui) -> TabBarOutput {
        let theme = Theme::get(ui);
        let mut output = TabBarOutput::default();

        let bar_height = theme.row_height + theme.space_sm;
        let (bar_rect, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), bar_height), Sense::hover());
        ui.painter()
            .rect_filled(bar_rect, CornerRadius::ZERO, theme.panel);
        ui.painter().hline(
            bar_rect.x_range(),
            bar_rect.bottom(),
            Stroke::new(1.0, theme.border),
        );

        let font = egui::FontId::proportional(theme.font_body);
        let mut left = bar_rect.left();
        for (index, item) in self.items.iter().enumerate() {
            // Measure first so the tab can be sized before it is painted.
            let text_width = ui
                .painter()
                .layout_no_wrap(item.title.clone(), font.clone(), theme.text)
                .size()
                .x;
            let close_width = if item.closable {
                theme.icon_sm + theme.space_xs
            } else {
                0.0
            };
            let tab_width = theme.space_md + text_width + theme.space_md + close_width;
            let tab_rect = Rect::from_min_size(
                pos2(left, bar_rect.top()),
                vec2(tab_width, bar_rect.height()),
            );

            let tab_id = ui.make_persistent_id(("kairos-ui.tabbar.tab", index));
            let response = ui.interact(tab_rect, tab_id, Sense::click());
            let active = self.active == Some(index);

            // Tab surface: active tabs get the panel body color plus an accent
            // underline; hovered inactive tabs get the shared hover token.
            if active {
                ui.painter()
                    .rect_filled(tab_rect, CornerRadius::ZERO, theme.bg);
                ui.painter().hline(
                    tab_rect.x_range(),
                    tab_rect.bottom(),
                    Stroke::new(2.0, theme.accent),
                );
            } else if response.hovered() {
                ui.painter()
                    .rect_filled(tab_rect, CornerRadius::ZERO, theme.hover_fill());
            }
            let text_color = if active || response.hovered() {
                theme.text
            } else {
                theme.text_muted
            };
            paint_text(
                ui,
                pos2(tab_rect.left() + theme.space_md, tab_rect.center().y),
                Align2::LEFT_CENTER,
                &item.title,
                theme.font_body,
                text_color,
            );

            if item.closable {
                let close_rect = Rect::from_center_size(
                    pos2(
                        tab_rect.right() - theme.space_sm - theme.icon_sm * 0.5,
                        tab_rect.center().y,
                    ),
                    vec2(theme.icon_sm + theme.space_xs * 2.0, tab_rect.height()),
                );
                let close_id = ui.make_persistent_id(("kairos-ui.tabbar.close", index));
                let close_response = ui.interact(close_rect, close_id, Sense::click());
                let close_color = if close_response.hovered() {
                    theme.text
                } else {
                    theme.text_muted
                };
                paint_glyph(
                    ui,
                    close_rect.center(),
                    theme.icon(IconId::Close),
                    theme.icon_sm,
                    close_color,
                );
                if close_response.clicked() {
                    output.close_clicked = Some(index);
                }
                output.close_rects.push((index, close_rect));
            }

            // A close click must never be reported as an activation as well.
            if response.clicked() && output.close_clicked != Some(index) {
                output.activated = Some(index);
            }
            output.tab_rects.push((index, tab_rect));
            left += tab_width;
        }
        output
    }
}
