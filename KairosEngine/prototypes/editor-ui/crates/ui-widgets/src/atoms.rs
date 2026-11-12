//! Atoms: the smallest themed building blocks.
//!
//! Every atom is a plain data struct implementing [egui::Widget] (so it is
//! used through ui.add(...)) and is painted exclusively with
//! [crate::theme::Theme] tokens. Atoms never own domain state: mutable inputs
//! (&mut String, &mut f32, ...) are caller-owned buffers -- either view state
//! or a frame-local copy that the component turns into an Action.
//!
//! Several atoms intentionally wrap the corresponding egui widget (TextEdit,
//! Slider, ComboBox, Spinner, Checkbox). The theme's apply_egui_style maps
//! tokens onto egui's Visuals, so wrapped widgets still follow the design
//! system. Custom-painted atoms (Button, Badge, ColorSwatch) use tokens
//! directly. This mixed strategy is recorded as a lesson in the README.

use std::ops::RangeInclusive;

use egui::{Color32, FontId, Response, RichText, Sense, Stroke, StrokeKind, Ui, Widget, vec2};

use crate::icon::IconId;
use crate::theme::Theme;

// ============================ Button ============================

/// Visual weight of a button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonVariant {
    Primary,
    Secondary,
    Ghost,
    Danger,
}

impl ButtonVariant {
    fn colors(self, theme: &Theme) -> (Color32, Color32, Stroke) {
        match self {
            Self::Primary => (theme.bg, theme.accent, Stroke::NONE),
            Self::Secondary => (theme.text, theme.panel, Stroke::new(1.0, theme.border)),
            Self::Ghost => (theme.text, Color32::TRANSPARENT, Stroke::NONE),
            Self::Danger => (theme.bg, theme.error, Stroke::NONE),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonSize {
    Small,
    Medium,
}

/// Text (+ optional leading icon glyph) button.
pub struct Button<'a> {
    text: &'a str,
    variant: ButtonVariant,
    size: ButtonSize,
    enabled: bool,
    icon: Option<IconId>,
}

impl<'a> Button<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            variant: ButtonVariant::Secondary,
            size: ButtonSize::Medium,
            enabled: true,
            icon: None,
        }
    }

    pub fn primary(text: &'a str) -> Self {
        Self::new(text).variant(ButtonVariant::Primary)
    }

    pub fn danger(text: &'a str) -> Self {
        Self::new(text).variant(ButtonVariant::Danger)
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn small(mut self) -> Self {
        self.size = ButtonSize::Small;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn icon(mut self, icon: IconId) -> Self {
        self.icon = Some(icon);
        self
    }
}

impl Widget for Button<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::get(ui);
        let (fg, bg, stroke) = self.variant.colors(&theme);
        let fg = if self.enabled { fg } else { theme.text_muted };

        let mut label = String::new();
        if let Some(icon) = self.icon {
            label.push_str(theme.icon(icon));
            label.push_str("  ");
        }
        label.push_str(self.text);

        let font_size = if self.size == ButtonSize::Small {
            theme.font_caption
        } else {
            theme.font_body
        };
        let height = if self.size == ButtonSize::Small {
            theme.row_height * 0.8
        } else {
            theme.row_height
        };
        let button = egui::Button::new(RichText::new(label).size(font_size).color(fg))
            .fill(bg)
            .stroke(stroke)
            .corner_radius(theme.radius_sm)
            .min_size(vec2(0.0, height));
        ui.add_enabled(self.enabled, button)
    }
}

// ============================ IconButton ============================

/// Square, label-less button for a single icon.
pub struct IconButton<'a> {
    icon: IconId,
    tooltip: Option<&'a str>,
    enabled: bool,
    active: bool,
}

impl<'a> IconButton<'a> {
    pub fn new(icon: IconId) -> Self {
        Self {
            icon,
            tooltip: None,
            enabled: true,
            active: false,
        }
    }

    pub fn tooltip(mut self, tooltip: &'a str) -> Self {
        self.tooltip = if tooltip.is_empty() {
            None
        } else {
            Some(tooltip)
        };
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }
}

impl Widget for IconButton<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::get(ui);
        let fg = if !self.enabled {
            theme.text_muted
        } else if self.active {
            theme.accent
        } else {
            theme.text
        };
        let text = RichText::new(theme.icon(self.icon))
            .size(theme.icon_md)
            .color(fg);
        let mut button = egui::Button::new(text)
            .min_size(vec2(theme.row_height, theme.row_height))
            .corner_radius(theme.radius_sm);
        button = if self.active {
            button
                .frame(true)
                .fill(theme.active_fill())
                .stroke(Stroke::NONE)
        } else {
            button.frame(false)
        };
        let response = ui.add_enabled(self.enabled, button);
        match self.tooltip {
            Some(tooltip) => response.on_hover_text(tooltip),
            None => response,
        }
    }
}

// ============================ Checkbox ============================

/// Checkbox. Stateless: reports the click, the caller flips the value.
pub struct Checkbox<'a> {
    checked: bool,
    label: &'a str,
    enabled: bool,
}

impl<'a> Checkbox<'a> {
    pub fn new(checked: bool, label: &'a str) -> Self {
        Self {
            checked,
            label,
            enabled: true,
        }
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl Widget for Checkbox<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        // egui's Checkbox needs &mut bool; we pass a copy and let the caller
        // treat response.clicked() as "toggle this value".
        let mut checked = self.checked;
        ui.add_enabled(self.enabled, egui::Checkbox::new(&mut checked, self.label))
    }
}

// ============================ TextInput ============================

/// Single-line text input. The buffer is caller-owned (view state).
pub struct TextInput<'a> {
    text: &'a mut String,
    hint: &'a str,
    mono: bool,
    desired_width: Option<f32>,
    enabled: bool,
}

impl<'a> TextInput<'a> {
    pub fn new(text: &'a mut String) -> Self {
        Self {
            text,
            hint: "",
            mono: false,
            desired_width: None,
            enabled: true,
        }
    }

    pub fn hint(mut self, hint: &'a str) -> Self {
        self.hint = hint;
        self
    }

    pub fn mono(mut self, mono: bool) -> Self {
        self.mono = mono;
        self
    }

    pub fn desired_width(mut self, width: f32) -> Self {
        self.desired_width = Some(width);
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl Widget for TextInput<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let mut edit = egui::TextEdit::singleline(self.text)
            .hint_text(self.hint.to_owned())
            .margin(vec2(4.0, 2.0));
        if self.mono {
            edit = edit.font(egui::TextStyle::Monospace);
        }
        if let Some(width) = self.desired_width {
            edit = edit.desired_width(width);
        }
        ui.add_enabled(self.enabled, edit)
    }
}

// ============================ SliderRow / NumberRow ============================

/// Label + slider on one row (used by the inspector).
pub struct SliderRow<'a> {
    label: &'a str,
    value: &'a mut f32,
    range: RangeInclusive<f32>,
    suffix: &'a str,
}

impl<'a> SliderRow<'a> {
    pub fn new(label: &'a str, value: &'a mut f32, range: RangeInclusive<f32>) -> Self {
        Self {
            label,
            value,
            range,
            suffix: "",
        }
    }

    pub fn suffix(mut self, suffix: &'a str) -> Self {
        self.suffix = suffix;
        self
    }
}

impl Widget for SliderRow<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::get(ui);
        ui.horizontal(|ui| {
            if !self.label.is_empty() {
                let width = (ui.available_width() * 0.35).max(48.0);
                ui.add_sized(
                    [width, theme.row_height],
                    egui::Label::new(
                        RichText::new(self.label)
                            .size(theme.font_body)
                            .color(theme.text_muted),
                    )
                    .selectable(false),
                );
            }
            let mut slider = egui::Slider::new(self.value, self.range).show_value(true);
            if !self.suffix.is_empty() {
                slider = slider.suffix(self.suffix.to_owned());
            }
            ui.add(slider)
        })
        .inner
    }
}

/// Label + drag-number on one row.
pub struct NumberRow<'a> {
    label: &'a str,
    value: &'a mut f64,
    range: Option<RangeInclusive<f64>>,
    suffix: &'a str,
    speed: f64,
}

impl<'a> NumberRow<'a> {
    pub fn new(label: &'a str, value: &'a mut f64) -> Self {
        Self {
            label,
            value,
            range: None,
            suffix: "",
            speed: 0.01,
        }
    }

    pub fn range(mut self, range: RangeInclusive<f64>) -> Self {
        self.range = Some(range);
        self
    }

    pub fn suffix(mut self, suffix: &'a str) -> Self {
        self.suffix = suffix;
        self
    }

    pub fn speed(mut self, speed: f64) -> Self {
        self.speed = speed;
        self
    }
}

impl Widget for NumberRow<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::get(ui);
        ui.horizontal(|ui| {
            if !self.label.is_empty() {
                let width = (ui.available_width() * 0.35).max(48.0);
                ui.add_sized(
                    [width, theme.row_height],
                    egui::Label::new(
                        RichText::new(self.label)
                            .size(theme.font_body)
                            .color(theme.text_muted),
                    )
                    .selectable(false),
                );
            }
            let mut drag = egui::DragValue::new(self.value).speed(self.speed);
            if let Some(range) = self.range {
                drag = drag.range(range);
            }
            if !self.suffix.is_empty() {
                drag = drag.suffix(self.suffix.to_owned());
            }
            ui.add(drag)
        })
        .inner
    }
}

// ============================ ComboSelect ============================

/// Dropdown over a fixed option list; writes the selected index in place.
pub struct ComboSelect<'a> {
    options: &'a [&'a str],
    selected: &'a mut usize,
    width: Option<f32>,
    enabled: bool,
}

impl<'a> ComboSelect<'a> {
    pub fn new(options: &'a [&'a str], selected: &'a mut usize) -> Self {
        Self {
            options,
            selected,
            width: None,
            enabled: true,
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl Widget for ComboSelect<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::get(ui);
        let Self {
            options,
            selected,
            width,
            enabled,
        } = self;
        let current = options.get(*selected).copied().unwrap_or_default();
        ui.add_enabled_ui(enabled, |ui| {
            egui::ComboBox::from_id_salt(ui.next_auto_id())
                .width(width.unwrap_or(theme.space_lg * 10.0))
                .selected_text(current)
                .show_ui(ui, |ui| {
                    for (index, option) in options.iter().enumerate() {
                        ui.selectable_value(selected, index, *option);
                    }
                })
                .response
        })
        .inner
    }
}

// ============================ ColorSwatch ============================

/// Small readonly color chip with a hex tooltip.
pub struct ColorSwatch {
    color: Color32,
    size: Option<f32>,
}

impl ColorSwatch {
    pub fn new(color: Color32) -> Self {
        Self { color, size: None }
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = Some(size);
        self
    }
}

impl Widget for ColorSwatch {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::get(ui);
        let size = self.size.unwrap_or(theme.row_height);
        let (rect, response) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
        ui.painter().rect_filled(rect, theme.radius_sm, self.color);
        ui.painter().rect_stroke(
            rect,
            theme.radius_sm,
            Stroke::new(1.0, theme.border),
            StrokeKind::Inside,
        );
        let hex = format!(
            "#{:02x}{:02x}{:02x}",
            self.color.r(),
            self.color.g(),
            self.color.b()
        );
        response.on_hover_text(hex)
    }
}

// ============================ Badge ============================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BadgeTone {
    Neutral,
    Accent,
    Success,
    Error,
}

impl BadgeTone {
    fn colors(self, theme: &Theme) -> (Color32, Color32) {
        match self {
            Self::Neutral => (theme.text_muted, theme.hover_fill()),
            Self::Accent => (theme.accent, theme.active_fill()),
            Self::Success => (
                theme.success,
                crate::theme::mix(theme.panel, theme.success, 0.2),
            ),
            Self::Error => (
                theme.error,
                crate::theme::mix(theme.panel, theme.error, 0.2),
            ),
        }
    }
}

/// Tiny rounded status pill.
pub struct Badge<'a> {
    text: &'a str,
    tone: BadgeTone,
}

impl<'a> Badge<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            tone: BadgeTone::Neutral,
        }
    }

    pub fn tone(mut self, tone: BadgeTone) -> Self {
        self.tone = tone;
        self
    }
}

impl Widget for Badge<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::get(ui);
        let (fg, bg) = self.tone.colors(&theme);
        let galley = ui.painter().layout_no_wrap(
            self.text.to_owned(),
            FontId::proportional(theme.font_caption),
            fg,
        );
        let size = galley.size() + vec2(theme.space_sm * 2.0, theme.space_xs);
        let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
        ui.painter().rect_filled(rect, theme.radius_sm, bg);
        ui.painter()
            .galley(rect.center() - galley.size() * 0.5, galley, fg);
        response
    }
}

// ============================ Spinner / Separator ============================

pub struct Spinner {
    size: Option<f32>,
}

impl Spinner {
    pub fn new() -> Self {
        Self { size: None }
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = Some(size);
        self
    }
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for Spinner {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::get(ui);
        ui.add(
            egui::Spinner::new()
                .size(self.size.unwrap_or(theme.icon_md))
                .color(theme.accent),
        )
    }
}

/// Thin horizontal rule.
pub struct Separator;

impl Widget for Separator {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::get(ui);
        ui.add(egui::Separator::default().spacing(theme.space_sm))
    }
}

// ============================ Text: Heading / Label ============================

/// Panel/section title.
pub struct Heading<'a> {
    text: &'a str,
}

impl<'a> Heading<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text }
    }
}

impl Widget for Heading<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::get(ui);
        ui.add(
            egui::Label::new(
                RichText::new(self.text)
                    .size(theme.font_subtitle)
                    .strong()
                    .color(theme.text),
            )
            .selectable(false),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LabelTone {
    Normal,
    Muted,
    Error,
    Success,
}

/// Themed label; mono=true for paths/ids.
pub struct Label<'a> {
    text: &'a str,
    tone: LabelTone,
    mono: bool,
}

impl<'a> Label<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            tone: LabelTone::Normal,
            mono: false,
        }
    }

    pub fn muted(mut self) -> Self {
        self.tone = LabelTone::Muted;
        self
    }

    pub fn tone(mut self, tone: LabelTone) -> Self {
        self.tone = tone;
        self
    }

    pub fn mono(mut self, mono: bool) -> Self {
        self.mono = mono;
        self
    }
}

impl Widget for Label<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::get(ui);
        let color = match self.tone {
            LabelTone::Normal => theme.text,
            LabelTone::Muted => theme.text_muted,
            LabelTone::Error => theme.error,
            LabelTone::Success => theme.success,
        };
        let mut text = RichText::new(self.text).size(theme.font_body).color(color);
        if self.mono {
            text = text.monospace();
        }
        ui.add(egui::Label::new(text).selectable(false))
    }
}
