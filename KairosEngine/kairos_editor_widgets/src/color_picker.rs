//! Unity/Tuanjie-style color editing.
//!
//! Two widgets live here:
//!
//! * [`ColorField`] is the inline inspector row: a full-width swatch with an
//!   optional HDR badge and the eyedropper affordance, matching the color
//!   property in Unity's Inspector.
//! * [`color_picker`] is the popup it opens: a hue ring around a
//!   saturation/value square, channel sliders, a hex field, an intensity
//!   control for HDR colors, and a swatch palette.
//!
//! The value is a [`ColorProperty`]: a linear eight-byte [`Color`] plus the
//! field's [`ColorUsage`]. HDR is a usage flag, not a second value type; the
//! picker works in sRGB and converts at the boundary, so the wheel and
//! sliders behave the same for LDR and HDR fields.
//!
//! Values are edited in place and the widgets stay domain-free: they return
//! an `egui::Response` whose `changed()` the caller maps to its own action.

use std::f32::consts::TAU;

use egui::epaint::{Mesh, Vertex, WHITE_UV};
use egui::{
    Align, Align2, Color32, CornerRadius, CursorIcon, Id, Margin, Pos2, Rect, Response, Sense,
    Stroke, StrokeKind, TextEdit, Ui, UiBuilder, Widget, WidgetType, pos2, vec2,
};
use kairos_color::{Color, ColorUsage, linear_to_srgb};

use crate::color::ColorExt;
use crate::paint::labelled_widget_info;
use crate::text::TextStyle;

/// A color value plus the field policy that says how to edit it.
///
/// This belongs to the color widget layer, not to the engine value crate: the
/// HDR base/exposure conversions below are picker behavior, while [`Color`]
/// stays a pure eight-byte linear value.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ColorProperty {
    pub color: Color,
    pub usage: ColorUsage,
}

impl ColorProperty {
    pub const fn new(color: Color, usage: ColorUsage) -> Self {
        Self { color, usage }
    }

    pub const fn ldr(color: Color) -> Self {
        Self::new(color, ColorUsage::LDR)
    }

    pub const fn hdr(color: Color) -> Self {
        Self::new(color, ColorUsage::HDR)
    }

    pub fn is_hdr(self) -> bool {
        self.usage.hdr
    }

    pub fn alpha(self) -> f32 {
        self.color.a()
    }

    /// The exposure, in stops, that makes the brightest channel 1.0.
    ///
    /// LDR properties always report zero; their picker has no Intensity row.
    pub fn intensity(self) -> f32 {
        if self.usage.hdr {
            self.color.exposure_stops()
        } else {
            0.0
        }
    }

    /// The sRGB base color at a given exposure and its alpha.
    ///
    /// For HDR this is the color divided by 2^intensity, which is what the
    /// wheel and channel sliders edit. The intensity itself is not part of the
    /// returned color.
    pub fn base_srgb(self, intensity: f32) -> [f32; 4] {
        if self.usage.hdr {
            let scale = 2.0_f32.powf(intensity).max(f32::MIN_POSITIVE);
            [
                linear_to_srgb(self.color.r() / scale).clamp(0.0, 1.0),
                linear_to_srgb(self.color.g() / scale).clamp(0.0, 1.0),
                linear_to_srgb(self.color.b() / scale).clamp(0.0, 1.0),
                self.color.a().clamp(0.0, 1.0),
            ]
        } else {
            let [r, g, b, a] = self.color.to_srgb_f32();
            [
                r.clamp(0.0, 1.0),
                g.clamp(0.0, 1.0),
                b.clamp(0.0, 1.0),
                a.clamp(0.0, 1.0),
            ]
        }
    }

    /// Rebuild the property from an sRGB base and an exposure.
    ///
    /// LDR properties ignore the exposure: they have no room for it.
    pub fn with_base(self, base: [f32; 4], intensity: f32) -> Self {
        let base = [
            base[0].clamp(0.0, 1.0),
            base[1].clamp(0.0, 1.0),
            base[2].clamp(0.0, 1.0),
            base[3].clamp(0.0, 1.0),
        ];
        let color = Color::from_srgb_f32(base);
        let color = if self.usage.hdr {
            color.scaled(intensity)
        } else {
            color
        };
        Self::new(color, self.usage)
    }

    /// The color to paint in a swatch: HDR values clamp to displayable sRGB.
    pub fn display(self) -> Color32 {
        self.color.to_egui()
    }
}

fn channel_u8(channel: f32) -> u8 {
    (channel.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Visual tokens for a color field and its picker.
#[derive(Clone, Debug, PartialEq)]
pub struct ColorPickerStyle {
    pub text: TextStyle,
    pub title: TextStyle,
    pub background: Color32,
    pub header: Color32,
    pub border: Color32,
    pub label: Color32,
    pub value: Color32,
    pub field: Color32,
    pub field_hover: Color32,
    pub field_edge: Color32,
    pub accent: Color32,
    pub checker_light: Color32,
    pub checker_dark: Color32,
    pub border_width: f32,
    pub corner_radius: u8,
    pub popup_width: f32,
    pub header_height: f32,
    pub wheel_size: f32,
    pub row_height: f32,
    pub field_height: f32,
    pub padding: f32,
}

impl Default for ColorPickerStyle {
    fn default() -> Self {
        Self::dark()
    }
}

impl ColorPickerStyle {
    pub fn dark() -> Self {
        Self {
            text: TextStyle::sized(Color32::from_rgb(0xDC, 0xDC, 0xDC), 12.0),
            title: TextStyle::sized(Color32::from_rgb(0xEE, 0xEE, 0xEE), 13.0).strong(),
            background: Color32::from_rgb(0x30, 0x30, 0x30),
            header: Color32::from_rgb(0x3C, 0x3C, 0x3C),
            border: Color32::from_rgb(0x16, 0x16, 0x16),
            label: Color32::from_rgb(0xB4, 0xB4, 0xB4),
            value: Color32::from_rgb(0xE6, 0xE6, 0xE6),
            field: Color32::from_rgb(0x22, 0x22, 0x22),
            field_hover: Color32::from_rgb(0x3A, 0x3A, 0x3A),
            field_edge: Color32::from_rgb(0x18, 0x18, 0x18),
            accent: Color32::from_rgb(0x4A, 0x90, 0xD9),
            checker_light: Color32::from_rgb(0xC8, 0xC8, 0xC8),
            checker_dark: Color32::from_rgb(0x8C, 0x8C, 0x8C),
            border_width: 1.0,
            corner_radius: 4,
            popup_width: 318.0,
            header_height: 24.0,
            wheel_size: 196.0,
            row_height: 22.0,
            field_height: 18.0,
            padding: 10.0,
        }
    }

    pub fn light() -> Self {
        Self {
            text: TextStyle::sized(Color32::from_rgb(0x1E, 0x1E, 0x1E), 12.0),
            title: TextStyle::sized(Color32::from_rgb(0x1E, 0x1E, 0x1E), 13.0).strong(),
            background: Color32::from_rgb(0xEB, 0xEB, 0xEB),
            header: Color32::from_rgb(0xDC, 0xDC, 0xDC),
            border: Color32::from_rgb(0xA8, 0xA8, 0xA8),
            label: Color32::from_rgb(0x3A, 0x3A, 0x3A),
            value: Color32::from_rgb(0x1E, 0x1E, 0x1E),
            field: Color32::WHITE,
            field_hover: Color32::from_rgb(0xF0, 0xF0, 0xF0),
            field_edge: Color32::from_rgb(0xA8, 0xA8, 0xA8),
            accent: Color32::from_rgb(0x2C, 0x5D, 0x87),
            checker_light: Color32::from_rgb(0xE6, 0xE6, 0xE6),
            checker_dark: Color32::from_rgb(0xB4, 0xB4, 0xB4),
            ..Self::dark()
        }
    }
}

/// Stable id of the field's popup, derived from the field id.
pub fn color_popup_id(field: Id) -> Id {
    field.with("popup")
}

/// Stable id of the popup's wheel.
pub fn color_wheel_id(field: Id) -> Id {
    field.with("wheel")
}

/// Stable id of the popup's hex field.
pub fn color_hex_id(field: Id) -> Id {
    field.with("hex")
}

/// Stable id of the popup's eyedropper button.
pub fn color_eye_id(field: Id) -> Id {
    field.with("eye")
}

/// Stable id of the popup's own eyedropper button (the field has its own).
pub fn color_picker_eye_id(field: Id) -> Id {
    field.with("picker-eye")
}

/// Stable id of the picker's channel mode button.
pub fn color_mode_id(field: Id) -> Id {
    field.with("mode")
}

/// Stable id of the popup's close button.
pub fn color_close_id(field: Id) -> Id {
    field.with("close")
}

/// Stable id of one channel track.
pub fn color_channel_id(field: Id, channel: usize) -> Id {
    field.with(("channel", channel))
}

/// Stable id of one channel's numeric box.
pub fn color_channel_value_id(field: Id, channel: usize) -> Id {
    field.with(("channel-value", channel))
}

/// Stable id of one HDR intensity preset.
pub fn color_intensity_preset_id(field: Id, index: usize) -> Id {
    field.with(("intensity-preset", index))
}

/// Stable id of one palette swatch.
pub fn color_swatch_id(field: Id, index: usize) -> Id {
    field.with(("swatch", index))
}

/// The inline color field: a swatch plus the eyedropper slot.
///
/// The widget allocates the available width and the requested height, paints
/// the swatch, and opens [`color_picker`] in a popup when the swatch is
/// clicked. The returned response belongs to the swatch (not the eyedropper
/// slot) and reports `changed()` when an edit in the popup moved the value.
pub struct ColorField<'a> {
    id: Id,
    value: &'a mut ColorProperty,
    style: &'a ColorPickerStyle,
    selected: bool,
    height: Option<f32>,
    eyedropper: bool,
}

impl<'a> ColorField<'a> {
    pub fn new(id: Id, value: &'a mut ColorProperty, style: &'a ColorPickerStyle) -> Self {
        let eyedropper = value.usage.eyedropper;
        Self {
            id,
            value,
            style,
            selected: false,
            height: None,
            eyedropper,
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    pub fn eyedropper(mut self, show: bool) -> Self {
        self.eyedropper = show;
        self
    }
}

impl Widget for ColorField<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let Self {
            id,
            value,
            style,
            selected,
            height,
            eyedropper,
        } = self;

        let width = ui.available_width().max(1.0);
        let height = height.unwrap_or(style.field_height).max(8.0);
        let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());

        let eye_width = if eyedropper { height } else { 0.0 };
        let gap = if eyedropper { 2.0 } else { 0.0 };
        let swatch_right = (rect.right() - eye_width - gap).max(rect.left() + 1.0);
        let swatch_rect = Rect::from_min_max(rect.min, pos2(swatch_right, rect.bottom()));
        let eye_rect = Rect::from_min_max(
            pos2(swatch_right + gap, rect.top()),
            pos2(rect.right(), rect.bottom()),
        );

        let mut swatch_response = ui
            .interact(swatch_rect, id, Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        labelled_widget_info(
            &swatch_response,
            WidgetType::ColorButton,
            if value.is_hdr() { "HDR Color" } else { "Color" },
            None,
        );

        if eyedropper {
            let eye = ui
                .interact(eye_rect, color_eye_id(id), Sense::hover())
                .on_hover_cursor(CursorIcon::Default);
            let _ = eye.on_hover_text(
                "Screen color picking is not available in this build; use the swatch.",
            );
        }

        let was_open = egui::Popup::is_id_open(ui.ctx(), color_popup_id(id));
        let mut changed = false;
        let popup = egui::Popup::from_response(&swatch_response)
            .open_memory(
                swatch_response
                    .clicked()
                    .then_some(egui::SetOpenCommand::Toggle),
            )
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .align(egui::RectAlign::BOTTOM_END)
            .gap(2.0)
            .width(style.popup_width)
            .frame(
                egui::Frame::NONE
                    .fill(style.background)
                    .stroke(Stroke::new(style.border_width, style.border))
                    .corner_radius(CornerRadius::same(style.corner_radius))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 4],
                        blur: 12,
                        spread: 0,
                        color: Color32::from_black_alpha(90),
                    }),
            );

        if let Some(inner) = popup.show(|ui| color_picker(ui, id, value, style, was_open)) {
            changed = inner.inner;
        }
        if changed {
            swatch_response.mark_changed();
        }

        paint_color_field(
            ui,
            swatch_rect,
            eye_rect,
            *value,
            style,
            selected,
            eyedropper,
        );

        swatch_response
    }
}

fn paint_color_field(
    ui: &Ui,
    swatch_rect: Rect,
    eye_rect: Rect,
    value: ColorProperty,
    style: &ColorPickerStyle,
    selected: bool,
    eyedropper: bool,
) {
    let painter = ui.painter();
    let alpha = value.alpha();
    if value.is_hdr() || alpha < 0.999 {
        paint_checkerboard(painter, swatch_rect, style);
    }

    let color = value.display();
    painter.rect_filled(swatch_rect, CornerRadius::ZERO, color);

    if value.is_hdr() {
        let text_color = contrast_color(color);
        let text = TextStyle::sized(text_color, 9.0);
        text.paint(
            ui,
            pos2(swatch_rect.right() - 4.0, swatch_rect.center().y),
            Align2::RIGHT_CENTER,
            "HDR",
        );
    }

    let edge = if selected {
        style.accent
    } else {
        style.field_edge
    };
    painter.rect_stroke(
        swatch_rect,
        CornerRadius::ZERO,
        Stroke::new(style.border_width, edge),
        StrokeKind::Inside,
    );

    if eyedropper {
        let hovered = ui.rect_contains_pointer(eye_rect);
        if hovered {
            painter.rect_filled(eye_rect, CornerRadius::same(2), style.field_hover);
        }
        paint_pipette(
            painter,
            eye_rect.center(),
            if hovered { style.value } else { style.label },
        );
    }
}

fn paint_checkerboard(painter: &egui::Painter, rect: Rect, style: &ColorPickerStyle) {
    let cell = 6.0;
    let painter = painter.with_clip_rect(rect);
    painter.rect_filled(rect, CornerRadius::ZERO, style.checker_light);
    let mut y = rect.top();
    let mut row = 0;
    while y < rect.bottom() {
        let mut x = rect.left() + if row % 2 == 0 { 0.0 } else { cell };
        while x < rect.right() {
            let tile = Rect::from_min_max(
                pos2(x, y),
                pos2((x + cell).min(rect.right()), (y + cell).min(rect.bottom())),
            );
            painter.rect_filled(tile, CornerRadius::ZERO, style.checker_dark);
            x += cell * 2.0;
        }
        y += cell;
        row += 1;
    }
}

fn contrast_color(color: Color32) -> Color32 {
    let luminance =
        0.2126 * color.r() as f32 + 0.7152 * color.g() as f32 + 0.0722 * color.b() as f32;
    if luminance > 140.0 {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}

fn paint_pipette(painter: &egui::Painter, center: Pos2, color: Color32) {
    // A diagonal barrel with a round bulb and a pointed tip, the same
    // silhouette Unity uses for its eyedropper.
    painter.line_segment(
        [center + vec2(-3.8, 3.8), center + vec2(2.4, -2.4)],
        Stroke::new(2.2, color),
    );
    painter.circle_filled(center + vec2(3.8, -3.8), 3.0, color);
    painter.line_segment(
        [center + vec2(1.4, -1.4), center + vec2(6.0, -6.0)],
        Stroke::new(1.2, color),
    );
    painter.add(egui::Shape::convex_polygon(
        vec![
            center + vec2(-6.0, 6.0),
            center + vec2(-1.8, 5.0),
            center + vec2(-5.0, 1.8),
        ],
        color,
        Stroke::NONE,
    ));
}

/// Which channel representation the sliders use.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ChannelMode {
    /// Unity's default: R/G/B/A in 0..1.
    #[default]
    RgbUnit,
    /// R/G/B/A in 0..255, for authors who think in bytes.
    Rgb255,
    /// Hue in degrees, saturation/value/alpha in 0..1.
    Hsv,
}

impl ChannelMode {
    const ALL: [Self; 3] = [Self::RgbUnit, Self::Rgb255, Self::Hsv];

    fn label(self) -> &'static str {
        match self {
            Self::RgbUnit => "RGB 0-1.0",
            Self::Rgb255 => "RGB 0-255",
            Self::Hsv => "HSV 0-1",
        }
    }
}

/// The part of the wheel a drag started on. Captured at press time so a drag
/// that leaves the ring keeps changing hue instead of jumping to saturation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WheelPart {
    Hue,
    SaturationValue,
}

/// The one numeric box being typed into.
#[derive(Clone, Debug)]
struct NumberEdit {
    target: EditTarget,
    text: String,
    /// Focus and select-all on the first pass.
    focus: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EditTarget {
    Channel(usize),
    Intensity,
}

#[derive(Clone, Debug)]
struct HexEdit {
    text: String,
    focus: bool,
    original: ColorProperty,
}

/// Per-field picker state kept in egui memory.
///
/// The value is the source of truth while the picker is closed. `source`
/// remembers the last value this state produced; a mismatch means something
/// else changed the field, so the next open re-seeds the base color, hue and
/// intensity from it.
#[derive(Clone, Debug)]
struct PickerState {
    source: Option<ColorProperty>,
    /// sRGB RGBA in 0..1: the color the wheel and sliders edit.
    base: [f32; 4],
    /// Exposure in stops; only used when the value is HDR.
    intensity: f32,
    /// Hue is kept separately so a gray value (saturation 0) remembers the
    /// wheel position while the user raises saturation.
    hue: f32,
    mode: ChannelMode,
    drag: Option<WheelPart>,
    edit: Option<NumberEdit>,
    hex: Option<HexEdit>,
    mode_menu_open: bool,
    swatches: Vec<[f32; 4]>,
}

impl Default for PickerState {
    fn default() -> Self {
        Self {
            source: None,
            base: [0.0, 0.0, 0.0, 1.0],
            intensity: 0.0,
            hue: 0.0,
            mode: ChannelMode::RgbUnit,
            drag: None,
            edit: None,
            hex: None,
            mode_menu_open: false,
            swatches: default_swatches(),
        }
    }
}

impl PickerState {
    fn sync(&mut self, value: ColorProperty) {
        self.intensity = value.intensity();
        self.base = value.base_srgb(self.intensity);
        self.hue = rgb_to_hsv(self.base)[0];
        self.source = Some(value);
        self.drag = None;
        self.edit = None;
        self.hex = None;
        self.mode_menu_open = false;
    }

    fn hsv(&self) -> [f32; 3] {
        let hsv = rgb_to_hsv(self.base);
        [
            if hsv[1] > 1e-6 { hsv[0] } else { self.hue },
            hsv[1],
            hsv[2],
        ]
    }

    fn set_base(&mut self, base: [f32; 4]) {
        self.base = [
            base[0].clamp(0.0, 1.0),
            base[1].clamp(0.0, 1.0),
            base[2].clamp(0.0, 1.0),
            base[3].clamp(0.0, 1.0),
        ];
        let hsv = rgb_to_hsv(self.base);
        if hsv[1] > 1e-6 {
            self.hue = hsv[0];
        }
    }

    fn set_hsv(&mut self, hsv: [f32; 3]) {
        let h = hsv[0].rem_euclid(1.0);
        let s = hsv[1].clamp(0.0, 1.0);
        let v = hsv[2].clamp(0.0, 1.0);
        let rgb = hsv_to_rgb(h, s, v);
        self.base[0] = rgb[0];
        self.base[1] = rgb[1];
        self.base[2] = rgb[2];
        self.hue = h;
    }

    /// Push the edited base/intensity into the caller's value.
    fn write(&mut self, value: &mut ColorProperty) -> bool {
        let next = value.with_base(self.base, self.intensity);
        let changed = next != *value;
        if changed {
            *value = next;
        }
        // Keep the state's source in sync even when the step rounded to the
        // same byte: the base may have moved less than one LDR unit.
        self.source = Some(*value);
        changed
    }

    fn channel_max(&self, index: usize) -> f32 {
        if index == 3 {
            if self.mode == ChannelMode::Rgb255 {
                255.0
            } else {
                1.0
            }
        } else {
            match self.mode {
                ChannelMode::RgbUnit => 1.0,
                ChannelMode::Rgb255 => 255.0,
                ChannelMode::Hsv if index == 0 => 360.0,
                ChannelMode::Hsv => 1.0,
            }
        }
    }

    fn channel_value(&self, index: usize) -> f32 {
        match self.mode {
            ChannelMode::RgbUnit => self.base[index],
            ChannelMode::Rgb255 => self.base[index] * 255.0,
            ChannelMode::Hsv => {
                let hsv = self.hsv();
                if index == 0 {
                    hsv[0] * 360.0
                } else if index == 3 {
                    self.base[3]
                } else {
                    hsv[index]
                }
            }
        }
    }

    /// Set one channel from its displayed value (the inverse of
    /// [`Self::channel_value`]).
    fn set_channel(&mut self, index: usize, displayed: f32) {
        match self.mode {
            ChannelMode::RgbUnit => self.base[index] = displayed.clamp(0.0, 1.0),
            ChannelMode::Rgb255 => self.base[index] = (displayed / 255.0).clamp(0.0, 1.0),
            ChannelMode::Hsv if index == 0 => {
                let hsv = self.hsv();
                self.set_hsv([displayed / 360.0, hsv[1], hsv[2]]);
            }
            ChannelMode::Hsv if index == 3 => {
                self.base[3] = displayed.clamp(0.0, 1.0);
            }
            ChannelMode::Hsv => {
                let mut hsv = self.hsv();
                hsv[index] = displayed.clamp(0.0, 1.0);
                self.set_hsv(hsv);
            }
        }
        if self.mode != ChannelMode::Hsv {
            let hsv = rgb_to_hsv(self.base);
            if hsv[1] > 1e-6 {
                self.hue = hsv[0];
            }
        }
    }

    fn channel_text(&self, index: usize) -> String {
        let value = self.channel_value(index);
        match self.mode {
            ChannelMode::Rgb255 => format!("{value:.0}"),
            ChannelMode::Hsv if index == 0 => format!("{value:.0}"),
            _ => format_trimmed(value, 3),
        }
    }
}

fn channel_label(index: usize) -> &'static str {
    match index {
        0 => "R",
        1 => "G",
        2 => "B",
        _ => "A",
    }
}

fn channel_name(index: usize) -> &'static str {
    match index {
        0 => "Red",
        1 => "Green",
        2 => "Blue",
        _ => "Alpha",
    }
}

fn default_swatches() -> Vec<[f32; 4]> {
    vec![
        [1.0, 1.0, 1.0, 1.0],
        [0.75, 0.75, 0.75, 1.0],
        [0.5, 0.5, 0.5, 1.0],
        [0.25, 0.25, 0.25, 1.0],
        [0.0, 0.0, 0.0, 1.0],
        [0.86, 0.08, 0.24, 1.0],
        [0.36, 0.56, 0.94, 1.0],
        [0.23, 0.72, 0.33, 1.0],
    ]
}

fn format_trimmed(value: f32, places: usize) -> String {
    let mut text = format!("{value:.places$}");
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    if text == "-0" {
        text = "0".to_owned();
    }
    text
}

/// sRGB HSV: `h` in 0..1. The wheel and sliders use this space; the value
/// stores sRGB for LDR and linear light for HDR.
fn rgb_to_hsv(rgba: [f32; 4]) -> [f32; 3] {
    let r = rgba[0];
    let g = rgba[1];
    let b = rgba[2];
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let mut h = if delta <= f32::EPSILON {
        0.0
    } else if max == r {
        ((g - b) / delta).rem_euclid(6.0)
    } else if max == g {
        (b - r) / delta + 2.0
    } else {
        (r - g) / delta + 4.0
    };
    h /= 6.0;
    let s = if max <= f32::EPSILON {
        0.0
    } else {
        delta / max
    };
    [h.rem_euclid(1.0), s, max]
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 3] {
    let h = h.rem_euclid(1.0) * 6.0;
    let sector = h.floor();
    let f = h - sector;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    match sector as i32 % 6 {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

fn hsv_color(h: f32) -> Color32 {
    let rgb = hsv_to_rgb(h, 1.0, 1.0);
    Color32::from_rgb(channel_u8(rgb[0]), channel_u8(rgb[1]), channel_u8(rgb[2]))
}

/// Draw the picker body for one field.
///
/// The popup layer and frame come from [`ColorField`]; this function owns the
/// state (mode, in-progress text, wheel drag) and the value updates.
fn color_picker(
    ui: &mut Ui,
    field: Id,
    value: &mut ColorProperty,
    style: &ColorPickerStyle,
    was_open: bool,
) -> bool {
    let state_id = field.with("picker-state");
    let mut state = ui
        .data(|data| data.get_temp::<PickerState>(state_id))
        .unwrap_or_default();

    if !was_open || state.source != Some(*value) {
        state.sync(*value);
    }

    let changed = picker_ui(ui, field, value, style, &mut state);
    ui.data_mut(|data| data.insert_temp(state_id, state));
    changed
}

fn picker_ui(
    ui: &mut Ui,
    field: Id,
    value: &mut ColorProperty,
    style: &ColorPickerStyle,
    state: &mut PickerState,
) -> bool {
    let mut changed = false;
    ui.set_width(style.popup_width);
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);

    picker_header(ui, field, value, style);

    egui::Frame::NONE
        .inner_margin(Margin::same(style.padding.round() as i8))
        .show(ui, |ui| {
            ui.set_width((style.popup_width - 2.0 * style.padding).max(80.0));
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);

            preview_row(ui, field, value, style);
            ui.add_space(6.0);
            changed |= wheel(ui, field, value, style, state);
            ui.add_space(4.0);
            mode_row(ui, field, style, state);
            let channels = if value.usage.alpha { 4 } else { 3 };
            for channel in 0..channels {
                changed |= channel_row(ui, field, value, style, state, channel);
            }
            ui.add_space(6.0);
            changed |= hex_row(ui, field, value, style, state);
            if value.is_hdr() {
                ui.add_space(6.0);
                changed |= intensity_row(ui, field, value, style, state);
                changed |= presets_row(ui, field, value, style, state);
            }
            ui.add_space(8.0);
            changed |= swatches_row(ui, field, value, style, state);
        });

    changed
}

fn picker_header(ui: &mut Ui, field: Id, value: &ColorProperty, style: &ColorPickerStyle) {
    let (rect, _) = ui.allocate_exact_size(
        vec2(ui.available_width(), style.header_height),
        Sense::hover(),
    );
    let radius = style.corner_radius;
    ui.painter().rect_filled(
        rect,
        CornerRadius {
            nw: radius,
            ne: radius,
            sw: 0,
            se: 0,
        },
        style.header,
    );

    let title = if value.is_hdr() { "HDR Color" } else { "Color" };
    style
        .title
        .paint(ui, rect.center(), Align2::CENTER_CENTER, title);

    let close_rect =
        Rect::from_center_size(pos2(rect.right() - 13.0, rect.center().y), vec2(18.0, 18.0));
    let close = ui
        .interact(close_rect, color_close_id(field), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    if close.hovered() {
        ui.painter()
            .rect_filled(close_rect, CornerRadius::same(2), style.field_hover);
    }
    let x = 4.0;
    let center = close_rect.center();
    let stroke = Stroke::new(1.2, style.value);
    ui.painter()
        .line_segment([center + vec2(-x, -x), center + vec2(x, x)], stroke);
    ui.painter()
        .line_segment([center + vec2(-x, x), center + vec2(x, -x)], stroke);
    if close.clicked() {
        egui::Popup::close_id(ui.ctx(), color_popup_id(field));
    }
}

fn preview_row(ui: &mut Ui, field: Id, value: &ColorProperty, style: &ColorPickerStyle) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::hover());

    if value.usage.eyedropper {
        let eye_rect = Rect::from_min_size(rect.min + vec2(0.0, 2.0), vec2(22.0, 22.0));
        let eye = ui
            .interact(eye_rect, color_picker_eye_id(field), Sense::hover())
            .on_hover_cursor(CursorIcon::Default);
        if eye.hovered() {
            ui.painter()
                .rect_filled(eye_rect, CornerRadius::same(2), style.field_hover);
        }
        paint_pipette(ui.painter(), eye_rect.center(), style.label);
        let _ = eye.on_hover_text(
            "Screen color picking is not available in this build; use the hex field or swatches.",
        );
    }

    let swatch = Rect::from_min_size(
        pos2(rect.right() - 110.0, rect.top() + 2.0),
        vec2(110.0, 22.0),
    );
    if value.is_hdr() || value.alpha() < 0.999 {
        paint_checkerboard(ui.painter(), swatch, style);
    }
    ui.painter()
        .rect_filled(swatch, CornerRadius::ZERO, value.display());
    ui.painter().rect_stroke(
        swatch,
        CornerRadius::ZERO,
        Stroke::new(1.0, style.field_edge),
        StrokeKind::Inside,
    );
}

struct WheelGeometry {
    center: Pos2,
    outer: f32,
    inner: f32,
    square: Rect,
}

impl WheelGeometry {
    fn new(rect: Rect) -> Self {
        let center = rect.center();
        let outer = rect.width().min(rect.height()) * 0.5 - 1.0;
        let thickness = (outer * 0.09).max(10.0);
        let inner = (outer - thickness).max(1.0);
        let square_half = (inner * 0.60).max(12.0);
        Self {
            center,
            outer,
            inner,
            square: Rect::from_center_size(center, vec2(square_half * 2.0, square_half * 2.0)),
        }
    }

    fn hit(&self, pos: Pos2) -> Option<WheelPart> {
        if self.square.contains(pos) {
            return Some(WheelPart::SaturationValue);
        }
        let radius = (pos - self.center).length();
        (radius >= self.inner && radius <= self.outer).then_some(WheelPart::Hue)
    }
}

fn wheel(
    ui: &mut Ui,
    field: Id,
    value: &mut ColorProperty,
    style: &ColorPickerStyle,
    state: &mut PickerState,
) -> bool {
    let (outer_rect, _) =
        ui.allocate_exact_size(vec2(ui.available_width(), style.wheel_size), Sense::hover());
    let rect = Rect::from_center_size(
        outer_rect.center(),
        vec2(style.wheel_size, style.wheel_size),
    );
    let response = ui
        .interact(rect, color_wheel_id(field), Sense::click_and_drag())
        .on_hover_cursor(CursorIcon::Crosshair);
    labelled_widget_info(&response, WidgetType::Other, "Color wheel", None);

    let geometry = WheelGeometry::new(rect);
    paint_wheel(ui, &geometry, state, style);

    let mut changed = false;
    if response.is_pointer_button_down_on() {
        state.mode_menu_open = false;
        let pos = ui
            .input(|input| input.pointer.interact_pos())
            .unwrap_or(geometry.center);
        if state.drag.is_none() {
            state.drag = geometry.hit(pos);
        }
        if let Some(part) = state.drag {
            match part {
                WheelPart::Hue => {
                    let offset = pos - geometry.center;
                    let angle = offset.y.atan2(offset.x);
                    let hue = (-angle / TAU).rem_euclid(1.0);
                    let hsv = state.hsv();
                    state.set_hsv([hue, hsv[1], hsv[2]]);
                }
                WheelPart::SaturationValue => {
                    let s = ((pos.x - geometry.square.left()) / geometry.square.width())
                        .clamp(0.0, 1.0);
                    let v = 1.0
                        - ((pos.y - geometry.square.top()) / geometry.square.height())
                            .clamp(0.0, 1.0);
                    let hue = state.hsv()[0];
                    state.set_hsv([hue, s, v]);
                }
            }
            changed |= state.write(value);
        }
    }
    if response.clicked() || response.drag_stopped() {
        state.drag = None;
    }

    changed
}

fn paint_wheel(ui: &Ui, geometry: &WheelGeometry, state: &PickerState, style: &ColorPickerStyle) {
    let painter = ui.painter().with_clip_rect(Rect::from_center_size(
        geometry.center,
        vec2(geometry.outer * 2.0 + 2.0, geometry.outer * 2.0 + 2.0),
    ));

    let segments = 144;
    let mut ring = Mesh::default();
    for index in 0..segments {
        let t0 = index as f32 / segments as f32;
        let t1 = (index + 1) as f32 / segments as f32;
        let a0 = -t0 * TAU;
        let a1 = -t1 * TAU;
        let c0 = hsv_color(t0);
        let c1 = hsv_color(t1);
        let inner0 = geometry.center + vec2(a0.cos(), a0.sin()) * geometry.inner;
        let outer0 = geometry.center + vec2(a0.cos(), a0.sin()) * geometry.outer;
        let inner1 = geometry.center + vec2(a1.cos(), a1.sin()) * geometry.inner;
        let outer1 = geometry.center + vec2(a1.cos(), a1.sin()) * geometry.outer;
        let base = ring.vertices.len() as u32;
        ring.vertices.push(Vertex {
            pos: inner0,
            uv: WHITE_UV,
            color: c0,
        });
        ring.vertices.push(Vertex {
            pos: outer0,
            uv: WHITE_UV,
            color: c0,
        });
        ring.vertices.push(Vertex {
            pos: outer1,
            uv: WHITE_UV,
            color: c1,
        });
        ring.vertices.push(Vertex {
            pos: inner1,
            uv: WHITE_UV,
            color: c1,
        });
        ring.add_triangle(base, base + 1, base + 2);
        ring.add_triangle(base, base + 2, base + 3);
    }
    painter.add(ring);

    let hue = state.hsv()[0];
    let hue_color = hsv_color(hue);
    let mut square = Mesh::default();
    let base = square.vertices.len() as u32;
    let white = Color32::WHITE;
    let black = Color32::BLACK;
    for (pos, color) in [
        (geometry.square.left_top(), white),
        (geometry.square.right_top(), hue_color),
        (geometry.square.right_bottom(), black),
        (geometry.square.left_bottom(), black),
    ] {
        square.vertices.push(Vertex {
            pos,
            uv: WHITE_UV,
            color,
        });
    }
    square.add_triangle(base, base + 1, base + 2);
    square.add_triangle(base, base + 2, base + 3);
    painter.add(square);
    painter.rect_stroke(
        geometry.square,
        CornerRadius::ZERO,
        Stroke::new(1.0, style.field_edge),
        StrokeKind::Inside,
    );

    let hsv = state.hsv();
    let ring_angle = -hsv[0] * TAU;
    let ring_radius = (geometry.outer + geometry.inner) * 0.5;
    let ring_marker = geometry.center + vec2(ring_angle.cos(), ring_angle.sin()) * ring_radius;
    paint_wheel_marker(&painter, ring_marker, hue_color);

    let marker = pos2(
        geometry.square.left() + hsv[1] * geometry.square.width(),
        geometry.square.top() + (1.0 - hsv[2]) * geometry.square.height(),
    );
    paint_wheel_marker(&painter, marker, hue_color);
}

fn paint_wheel_marker(painter: &egui::Painter, pos: Pos2, color: Color32) {
    painter.circle_filled(pos, 5.5, Color32::from_black_alpha(150));
    painter.circle_filled(pos, 4.5, color);
    painter.circle_stroke(pos, 4.5, Stroke::new(1.0, Color32::WHITE));
}

/// Stable id of one entry in the channel-mode dropdown.
pub fn color_mode_item_id(field: Id, index: usize) -> Id {
    field.with(("mode-item", index))
}

fn mode_row(ui: &mut Ui, field: Id, style: &ColorPickerStyle, state: &mut PickerState) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0), Sense::hover());
    let button_rect = Rect::from_min_size(
        pos2(rect.right() - 112.0, rect.top() + 1.0),
        vec2(112.0, 18.0),
    );
    let response = ui
        .interact(button_rect, color_mode_id(field), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    paint_inset(ui, button_rect, style, response.hovered());
    let text_style = style.text.clone().color(style.value);
    text_style.paint(
        ui,
        pos2(button_rect.left() + 7.0, button_rect.center().y),
        Align2::LEFT_CENTER,
        state.mode.label(),
    );
    paint_caret(
        ui.painter(),
        pos2(button_rect.right() - 10.0, button_rect.center().y),
        style.label,
    );
    if response.clicked() {
        state.mode_menu_open = !state.mode_menu_open;
    }
    if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
        state.mode_menu_open = false;
    }

    if !state.mode_menu_open {
        return;
    }

    let mut choice = None;
    for (index, mode) in ChannelMode::ALL.iter().enumerate() {
        let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 18.0), Sense::hover());
        let item_rect = Rect::from_min_max(
            pos2(rect.right() - 112.0, row.top() + 1.0),
            pos2(rect.right(), row.bottom() - 1.0),
        );
        let item = ui
            .interact(item_rect, color_mode_item_id(field, index), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        let active = state.mode == *mode;
        let fill = if active {
            style.accent
        } else if item.hovered() {
            style.field_hover
        } else {
            style.field
        };
        ui.painter()
            .rect_filled(item_rect, CornerRadius::ZERO, fill);
        ui.painter().rect_stroke(
            item_rect,
            CornerRadius::ZERO,
            Stroke::new(1.0, style.field_edge),
            StrokeKind::Inside,
        );
        let color = if active {
            Color32::WHITE
        } else {
            style.text.text_color()
        };
        style.text.clone().color(color).paint(
            ui,
            pos2(item_rect.left() + 7.0, item_rect.center().y),
            Align2::LEFT_CENTER,
            mode.label(),
        );
        if item.clicked() {
            choice = Some(*mode);
        }
    }
    if let Some(mode) = choice {
        state.mode = mode;
        state.mode_menu_open = false;
    }
}

fn channel_row(
    ui: &mut Ui,
    field: Id,
    value: &mut ColorProperty,
    style: &ColorPickerStyle,
    state: &mut PickerState,
    index: usize,
) -> bool {
    let (rect, _) =
        ui.allocate_exact_size(vec2(ui.available_width(), style.row_height), Sense::hover());
    let value_width = 44.0;
    let track = Rect::from_min_max(
        pos2(rect.left() + 20.0, rect.center().y - 4.0),
        pos2(rect.right() - value_width - 6.0, rect.center().y + 4.0),
    );
    let value_rect = Rect::from_min_size(
        pos2(rect.right() - value_width, rect.top() + 2.0),
        vec2(value_width, rect.height() - 4.0),
    );

    let label_style = style.text.clone().color(style.label);
    label_style.paint(
        ui,
        pos2(rect.left() + 3.0, rect.center().y),
        Align2::LEFT_CENTER,
        channel_label(index),
    );

    paint_channel_track(ui, track, style, state, index);

    let track_response = ui
        .interact(
            track,
            color_channel_id(field, index),
            Sense::click_and_drag(),
        )
        .on_hover_cursor(CursorIcon::ResizeHorizontal);
    labelled_widget_info(
        &track_response,
        WidgetType::Slider,
        channel_name(index),
        None,
    );

    let mut changed = false;
    if track_response.is_pointer_button_down_on() {
        let pos = ui
            .input(|input| input.pointer.interact_pos())
            .unwrap_or(track.center());
        let t = ((pos.x - track.left()) / track.width()).clamp(0.0, 1.0);
        let displayed = t * state.channel_max(index);
        state.set_channel(index, displayed);
        changed |= state.write(value);
        state.mode_menu_open = false;
    }

    let t = (state.channel_value(index) / state.channel_max(index)).clamp(0.0, 1.0);
    paint_track_handle(ui.painter(), track, t);

    let editing = matches!(
        state.edit.as_ref().map(|edit| edit.target),
        Some(EditTarget::Channel(current)) if current == index
    );
    if editing {
        let mut edit = state.edit.take().expect("checked above");
        paint_inset(ui, value_rect, style, true);
        let response = text_edit_box(
            ui,
            value_rect,
            color_channel_value_id(field, index),
            &mut edit.text,
            style,
            Align::Max,
        );
        if edit.focus {
            response.request_focus();
            select_all_text(ui, response.id, edit.text.chars().count());
            edit.focus = false;
        }
        let escape = ui.input(|input| input.key_pressed(egui::Key::Escape));
        let enter = ui.input(|input| input.key_pressed(egui::Key::Enter));
        if escape {
            // The value is only written on commit, so Escape just drops the
            // typed text.
        } else if enter || response.lost_focus() {
            if let Some(displayed) = parse_number(&edit.text) {
                state.set_channel(index, displayed.clamp(0.0, state.channel_max(index)));
                changed |= state.write(value);
            }
        } else {
            state.edit = Some(edit);
        }
    } else {
        let response = ui
            .interact(
                value_rect,
                color_channel_value_id(field, index),
                Sense::click(),
            )
            .on_hover_cursor(CursorIcon::Text);
        paint_inset(ui, value_rect, style, response.hovered());
        let text_style = style.text.clone().color(style.value);
        text_style.paint(
            ui,
            pos2(value_rect.right() - 5.0, value_rect.center().y),
            Align2::RIGHT_CENTER,
            state.channel_text(index),
        );
        if response.clicked() {
            state.edit = Some(NumberEdit {
                target: EditTarget::Channel(index),
                text: state.channel_text(index),
                focus: true,
            });
            state.mode_menu_open = false;
        }
    }

    changed
}

fn paint_channel_track(
    ui: &Ui,
    rect: Rect,
    style: &ColorPickerStyle,
    state: &PickerState,
    index: usize,
) {
    let painter = ui.painter().with_clip_rect(rect);
    if index == 3 {
        paint_checkerboard(&painter, rect, style);
    }

    match state.mode {
        ChannelMode::RgbUnit | ChannelMode::Rgb255 => match index {
            0 => paint_gradient(
                &painter,
                rect,
                Color32::from_rgb(0, 255, 255),
                Color32::WHITE,
            ),
            1 => paint_gradient(
                &painter,
                rect,
                Color32::from_rgb(255, 0, 255),
                Color32::WHITE,
            ),
            2 => paint_gradient(
                &painter,
                rect,
                Color32::from_rgb(255, 255, 0),
                Color32::WHITE,
            ),
            _ => {
                let color = Color32::from_rgb(
                    channel_u8(state.base[0]),
                    channel_u8(state.base[1]),
                    channel_u8(state.base[2]),
                );
                paint_gradient(
                    &painter,
                    rect,
                    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 0),
                    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 255),
                );
            }
        },
        ChannelMode::Hsv => {
            let hsv = state.hsv();
            match index {
                0 => {
                    let strips = 60;
                    for step in 0..strips {
                        let t0 = step as f32 / strips as f32;
                        let t1 = (step + 1) as f32 / strips as f32;
                        let strip = Rect::from_min_max(
                            pos2(rect.left() + t0 * rect.width(), rect.top()),
                            pos2(rect.left() + t1 * rect.width() + 0.5, rect.bottom()),
                        );
                        painter.rect_filled(strip, CornerRadius::ZERO, hsv_color(t0));
                    }
                }
                1 => {
                    let from = hsv_to_rgb(hsv[0], 0.0, hsv[2]);
                    let to = hsv_to_rgb(hsv[0], 1.0, hsv[2]);
                    paint_gradient(&painter, rect, srgb_color(from), srgb_color(to));
                }
                2 => {
                    let to = hsv_to_rgb(hsv[0], hsv[1], 1.0);
                    paint_gradient(&painter, rect, Color32::BLACK, srgb_color(to));
                }
                _ => {
                    let color = Color32::from_rgb(
                        channel_u8(state.base[0]),
                        channel_u8(state.base[1]),
                        channel_u8(state.base[2]),
                    );
                    paint_gradient(
                        &painter,
                        rect,
                        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 0),
                        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 255),
                    );
                }
            }
        }
    }

    painter.rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, style.field_edge),
        StrokeKind::Inside,
    );
}

fn paint_track_handle(painter: &egui::Painter, track: Rect, t: f32) {
    let x = track.left() + t * track.width();
    let handle = Rect::from_center_size(pos2(x, track.center().y), vec2(4.0, track.height() + 4.0));
    painter.rect_filled(handle, CornerRadius::ZERO, Color32::WHITE);
    painter.rect_stroke(
        handle,
        CornerRadius::ZERO,
        Stroke::new(1.0, Color32::from_black_alpha(170)),
        StrokeKind::Inside,
    );
}

fn paint_gradient(painter: &egui::Painter, rect: Rect, start: Color32, end: Color32) {
    let mut mesh = Mesh::default();
    let base = 0;
    for (pos, color) in [
        (rect.left_top(), start),
        (rect.right_top(), end),
        (rect.right_bottom(), end),
        (rect.left_bottom(), start),
    ] {
        mesh.vertices.push(Vertex {
            pos,
            uv: WHITE_UV,
            color,
        });
    }
    mesh.add_triangle(base, base + 1, base + 2);
    mesh.add_triangle(base, base + 2, base + 3);
    painter.add(mesh);
}

fn srgb_color(rgb: [f32; 3]) -> Color32 {
    Color32::from_rgb(channel_u8(rgb[0]), channel_u8(rgb[1]), channel_u8(rgb[2]))
}

fn hex_row(
    ui: &mut Ui,
    field: Id,
    value: &mut ColorProperty,
    style: &ColorPickerStyle,
    state: &mut PickerState,
) -> bool {
    let (rect, _) =
        ui.allocate_exact_size(vec2(ui.available_width(), style.row_height), Sense::hover());
    let box_rect = Rect::from_min_size(
        pos2(rect.right() - 100.0, rect.top() + 2.0),
        vec2(100.0, rect.height() - 4.0),
    );
    let label_style = style.text.clone().color(style.label);
    label_style.paint(
        ui,
        pos2(rect.left() + 3.0, rect.center().y),
        Align2::LEFT_CENTER,
        "Hexadecimal",
    );

    let mut changed = false;
    if let Some(mut edit) = state.hex.take() {
        paint_inset(ui, box_rect, style, true);
        let response = text_edit_box(
            ui,
            box_rect,
            color_hex_id(field),
            &mut edit.text,
            style,
            Align::Min,
        );
        if edit.focus {
            response.request_focus();
            select_all_text(ui, response.id, edit.text.chars().count());
            edit.focus = false;
        }

        if response.changed()
            && let Some((rgba, has_alpha)) = parse_hex(&edit.text)
        {
            let alpha = if has_alpha { rgba[3] } else { state.base[3] };
            state.set_base([rgba[0], rgba[1], rgba[2], alpha]);
            changed |= state.write(value);
        }

        let escape = ui.input(|input| input.key_pressed(egui::Key::Escape));
        let enter = ui.input(|input| input.key_pressed(egui::Key::Enter));
        if escape {
            let before = *value;
            *value = edit.original;
            state.sync(edit.original);
            changed |= before != edit.original;
        } else if enter || response.lost_focus() {
            // Live parsing already applied every valid intermediate.
        } else {
            state.hex = Some(edit);
        }
    } else {
        let response = ui
            .interact(box_rect, color_hex_id(field), Sense::click())
            .on_hover_cursor(CursorIcon::Text);
        paint_inset(ui, box_rect, style, response.hovered());
        let text_style = style.text.clone().color(style.value);
        text_style.paint(
            ui,
            pos2(box_rect.left() + 6.0, box_rect.center().y),
            Align2::LEFT_CENTER,
            hex_text(state),
        );
        if response.clicked() {
            state.hex = Some(HexEdit {
                text: hex_text(state),
                focus: true,
                original: *value,
            });
            state.mode_menu_open = false;
        }
    }

    changed
}

fn hex_text(state: &PickerState) -> String {
    let [r, g, b, a] = state.base;
    if a < 0.999 {
        format!(
            "#{:02X}{:02X}{:02X}{:02X}",
            channel_u8(r),
            channel_u8(g),
            channel_u8(b),
            channel_u8(a)
        )
    } else {
        format!(
            "#{:02X}{:02X}{:02X}",
            channel_u8(r),
            channel_u8(g),
            channel_u8(b)
        )
    }
}

fn parse_hex(text: &str) -> Option<([f32; 4], bool)> {
    let trimmed = text.trim();
    let digits = trimmed.strip_prefix('#').unwrap_or(trimmed);
    if !digits.is_ascii() || (digits.len() != 6 && digits.len() != 8) {
        return None;
    }
    let color = Color32::from_hex(trimmed).ok()?;
    Some((
        [
            color.r() as f32 / 255.0,
            color.g() as f32 / 255.0,
            color.b() as f32 / 255.0,
            color.a() as f32 / 255.0,
        ],
        digits.len() == 8,
    ))
}

fn intensity_row(
    ui: &mut Ui,
    field: Id,
    value: &mut ColorProperty,
    style: &ColorPickerStyle,
    state: &mut PickerState,
) -> bool {
    let (rect, _) =
        ui.allocate_exact_size(vec2(ui.available_width(), style.row_height), Sense::hover());
    let value_width = 44.0;
    let track = Rect::from_min_max(
        pos2(rect.left() + 54.0, rect.center().y - 4.0),
        pos2(rect.right() - value_width - 6.0, rect.center().y + 4.0),
    );
    let value_rect = Rect::from_min_size(
        pos2(rect.right() - value_width, rect.top() + 2.0),
        vec2(value_width, rect.height() - 4.0),
    );

    let label_style = style.text.clone().color(style.label);
    label_style.paint(
        ui,
        pos2(rect.left() + 3.0, rect.center().y),
        Align2::LEFT_CENTER,
        "Intensity",
    );

    let painter = ui.painter();
    painter.rect_filled(track, CornerRadius::ZERO, style.field);
    let t = ((state.intensity + 10.0) / 20.0).clamp(0.0, 1.0);
    let x = track.left() + t * track.width();
    let zero = track.left() + 0.5 * track.width();
    let fill = Rect::from_min_max(
        pos2(x.min(zero), track.top()),
        pos2(x.max(zero), track.bottom()),
    );
    painter.rect_filled(fill, CornerRadius::ZERO, style.accent);
    painter.rect_stroke(
        track,
        CornerRadius::ZERO,
        Stroke::new(1.0, style.field_edge),
        StrokeKind::Inside,
    );
    paint_track_handle(painter, track, t);

    let track_response = ui
        .interact(track, color_channel_id(field, 4), Sense::click_and_drag())
        .on_hover_cursor(CursorIcon::ResizeHorizontal);
    labelled_widget_info(&track_response, WidgetType::Slider, "Intensity", None);

    let mut changed = false;
    if track_response.is_pointer_button_down_on() {
        let pos = ui
            .input(|input| input.pointer.interact_pos())
            .unwrap_or(track.center());
        let t = ((pos.x - track.left()) / track.width()).clamp(0.0, 1.0);
        state.intensity = -10.0 + t * 20.0;
        changed |= state.write(value);
        state.mode_menu_open = false;
    }

    let editing = matches!(
        state.edit.as_ref().map(|edit| edit.target),
        Some(EditTarget::Intensity)
    );
    if editing {
        let mut edit = state.edit.take().expect("checked above");
        paint_inset(ui, value_rect, style, true);
        let response = text_edit_box(
            ui,
            value_rect,
            color_channel_value_id(field, 4),
            &mut edit.text,
            style,
            Align::Max,
        );
        if edit.focus {
            response.request_focus();
            select_all_text(ui, response.id, edit.text.chars().count());
            edit.focus = false;
        }
        let escape = ui.input(|input| input.key_pressed(egui::Key::Escape));
        let enter = ui.input(|input| input.key_pressed(egui::Key::Enter));
        if escape {
        } else if enter || response.lost_focus() {
            if let Some(stops) = parse_number(&edit.text) {
                state.intensity = stops.clamp(-10.0, 10.0);
                changed |= state.write(value);
            }
        } else {
            state.edit = Some(edit);
        }
    } else {
        let response = ui
            .interact(value_rect, color_channel_value_id(field, 4), Sense::click())
            .on_hover_cursor(CursorIcon::Text);
        paint_inset(ui, value_rect, style, response.hovered());
        let text_style = style.text.clone().color(style.value);
        text_style.paint(
            ui,
            pos2(value_rect.right() - 5.0, value_rect.center().y),
            Align2::RIGHT_CENTER,
            format_trimmed(state.intensity, 2),
        );
        if response.clicked() {
            state.edit = Some(NumberEdit {
                target: EditTarget::Intensity,
                text: format_trimmed(state.intensity, 2),
                focus: true,
            });
            state.mode_menu_open = false;
        }
    }

    changed
}

fn presets_row(
    ui: &mut Ui,
    field: Id,
    value: &mut ColorProperty,
    style: &ColorPickerStyle,
    state: &mut PickerState,
) -> bool {
    const PRESETS: [f32; 5] = [-2.0, -1.0, 0.0, 1.0, 2.0];
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0), Sense::hover());
    let gap = 1.0;
    let cell_width = (rect.width() - gap * 4.0) / 5.0;
    let mut changed = false;

    for (index, preset) in PRESETS.iter().enumerate() {
        let cell = Rect::from_min_size(
            pos2(
                rect.left() + index as f32 * (cell_width + gap),
                rect.top() + 1.0,
            ),
            vec2(cell_width, rect.height() - 2.0),
        );
        let response = ui
            .interact(
                cell,
                color_intensity_preset_id(field, index),
                Sense::click(),
            )
            .on_hover_cursor(CursorIcon::PointingHand);
        let selected = (state.intensity - preset).abs() < 0.01;
        let fill = if selected {
            style.accent
        } else if response.hovered() {
            style.field_hover
        } else {
            style.field
        };
        ui.painter().rect_filled(cell, CornerRadius::ZERO, fill);
        ui.painter().rect_stroke(
            cell,
            CornerRadius::ZERO,
            Stroke::new(1.0, style.field_edge),
            StrokeKind::Inside,
        );
        let label = match *preset {
            value if value > 0.0 => format!("+{value:.0}"),
            value => format!("{value:.0}"),
        };
        let text_color = if selected {
            Color32::WHITE
        } else {
            style.label
        };
        style
            .text
            .clone()
            .color(text_color)
            .paint(ui, cell.center(), Align2::CENTER_CENTER, label);
        if response.clicked() {
            state.intensity = *preset;
            changed |= state.write(value);
            state.mode_menu_open = false;
        }
    }

    changed
}

fn swatches_row(
    ui: &mut Ui,
    field: Id,
    value: &mut ColorProperty,
    style: &ColorPickerStyle,
    state: &mut PickerState,
) -> bool {
    let (label_rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 16.0), Sense::hover());
    style.text.clone().color(style.label).paint(
        ui,
        pos2(label_rect.left() + 3.0, label_rect.center().y),
        Align2::LEFT_CENTER,
        "Swatches",
    );

    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0), Sense::hover());
    let mut changed = false;
    for (index, swatch) in state.swatches.clone().into_iter().enumerate() {
        let cell = Rect::from_min_size(
            pos2(rect.left() + index as f32 * 22.0, rect.top() + 1.0),
            vec2(18.0, 18.0),
        );
        if cell.right() > rect.right() {
            break;
        }
        let response = ui
            .interact(cell, color_swatch_id(field, index), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        if swatch[3] < 0.999 {
            paint_checkerboard(ui.painter(), cell, style);
        }
        ui.painter().rect_filled(
            cell,
            CornerRadius::ZERO,
            srgb_color([swatch[0], swatch[1], swatch[2]]),
        );
        let selected = [0, 1, 2]
            .iter()
            .all(|channel| (state.base[*channel] - swatch[*channel]).abs() < 0.01);
        let edge = if selected {
            style.accent
        } else if response.hovered() {
            style.value
        } else {
            style.field_edge
        };
        ui.painter().rect_stroke(
            cell,
            CornerRadius::ZERO,
            Stroke::new(if selected { 2.0 } else { 1.0 }, edge),
            StrokeKind::Inside,
        );
        if response.clicked() {
            state.set_base(swatch);
            changed |= state.write(value);
            state.mode_menu_open = false;
        }
    }
    changed
}

fn paint_inset(ui: &Ui, rect: Rect, style: &ColorPickerStyle, hovered: bool) {
    let fill = if hovered {
        style.field_hover
    } else {
        style.field
    };
    ui.painter().rect_filled(rect, CornerRadius::ZERO, fill);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, style.field_edge),
        StrokeKind::Inside,
    );
}

fn text_edit_box(
    ui: &mut Ui,
    rect: Rect,
    id: Id,
    text: &mut String,
    style: &ColorPickerStyle,
    align: Align,
) -> Response {
    let padding = Margin::symmetric(4, 1);
    ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        ui.visuals_mut().selection.bg_fill = style.accent;
        ui.visuals_mut().selection.stroke = Stroke::NONE;
        ui.add(
            TextEdit::singleline(text)
                .id(id)
                .font(style.text.font_id(ui.style()))
                .text_color(style.value)
                .frame(egui::Frame::NONE.inner_margin(padding))
                .horizontal_align(align)
                .vertical_align(Align::Center)
                .min_size(vec2(rect.width(), rect.height()))
                .desired_width(rect.width()),
        )
    })
    .inner
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

fn parse_number(text: &str) -> Option<f32> {
    text.trim()
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite())
}

fn paint_caret(painter: &egui::Painter, center: Pos2, color: Color32) {
    painter.add(egui::Shape::convex_polygon(
        vec![
            pos2(center.x - 3.5, center.y - 1.5),
            pos2(center.x + 3.5, center.y - 1.5),
            pos2(center.x, center.y + 2.5),
        ],
        color,
        Stroke::NONE,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Key, Modifiers};
    use kairos_color::{Color, ColorUsage};

    struct Harness {
        ctx: egui::Context,
        value: ColorProperty,
        style: ColorPickerStyle,
        field: Id,
    }

    impl Harness {
        fn new(value: ColorProperty) -> Self {
            Self {
                ctx: egui::Context::default(),
                value,
                style: ColorPickerStyle::dark(),
                field: Id::new("color-field"),
            }
        }

        fn frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    vec2(520.0, 760.0),
                )),
                events,
                ..Default::default()
            };
            let Self {
                ctx,
                value,
                style,
                field,
            } = self;
            ctx.run_ui(input, |ui| {
                ui.add_space(20.0);
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    ui.add_sized(
                        vec2(240.0, 18.0),
                        ColorField::new(*field, value, style).height(18.0),
                    );
                });
            })
        }

        fn click_at(&mut self, pos: Pos2) -> egui::FullOutput {
            self.frame(vec![egui::Event::PointerMoved(pos)]);
            self.frame(vec![pointer(pos, true)]);
            self.frame(vec![pointer(pos, false)])
        }

        fn click(&mut self, id: Id) -> egui::FullOutput {
            let rect = self
                .ctx
                .read_response(id)
                .unwrap_or_else(|| panic!("{id:?} was not laid out"))
                .rect;
            self.click_at(rect.center())
        }

        fn open(&mut self) {
            self.frame(Vec::new());
            self.click(self.field);
        }
    }

    fn pointer(pos: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        }
    }

    fn key(key: Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }
    }

    fn painted_texts(output: &egui::FullOutput) -> Vec<String> {
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

    fn assert_near(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 0.02,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn hsv_round_trips_primary_colors() {
        for (hue, expected) in [
            (0.0, [1.0, 0.0, 0.0]),
            (1.0 / 3.0, [0.0, 1.0, 0.0]),
            (2.0 / 3.0, [0.0, 0.0, 1.0]),
        ] {
            let rgb = hsv_to_rgb(hue, 1.0, 1.0);
            assert!((rgb[0] - expected[0]).abs() < 1e-4);
            assert!((rgb[1] - expected[1]).abs() < 1e-4);
            assert!((rgb[2] - expected[2]).abs() < 1e-4);
            let back = rgb_to_hsv([rgb[0], rgb[1], rgb[2], 1.0]);
            assert_near(back[1], 1.0);
            assert_near(back[2], 1.0);
        }
    }

    #[test]
    fn translucent_ldr_colors_stay_straight() {
        let value = ColorProperty::ldr(Color::from_srgb8([200, 100, 50, 128]));
        let base = value.base_srgb(0.0);
        assert!((base[0] * 255.0 - 200.0).abs() <= 1.0, "{base:?}");
        assert!((base[1] * 255.0 - 100.0).abs() <= 1.0, "{base:?}");
        assert!((base[2] * 255.0 - 50.0).abs() <= 1.0, "{base:?}");
        assert!((base[3] * 255.0 - 128.0).abs() <= 1.0, "{base:?}");
        assert!((value.alpha() * 255.0 - 128.0).abs() <= 1.0);
    }

    #[test]
    fn hdr_usage_drives_intensity_while_ldr_ignores_it() {
        let hdr = ColorProperty::hdr(Color::new(4.0, 2.0, 1.0, 1.0));
        assert_eq!(hdr.intensity(), 2.0);
        let base = hdr.base_srgb(hdr.intensity());
        assert!((base[0] - 1.0).abs() < 1e-3, "{base:?}");
        assert!((base[1] - linear_to_srgb(0.5)).abs() < 1e-3, "{base:?}");
        assert!((base[2] - linear_to_srgb(0.25)).abs() < 1e-3, "{base:?}");
        assert_eq!(hdr.with_base(base, hdr.intensity()), hdr);

        let ldr = ColorProperty::ldr(Color::from_srgb8([20, 140, 220, 255]));
        assert_eq!(ldr.intensity(), 0.0);
        assert_eq!(
            ldr.with_base(ldr.base_srgb(5.0), 5.0),
            ldr,
            "an LDR property must not acquire an exposure"
        );
    }

    #[test]
    fn hidden_alpha_omits_the_a_channel() {
        let usage = ColorUsage::LDR.without_alpha();
        let mut harness = Harness::new(ColorProperty::new(
            Color::from_srgb8([20, 140, 220, 255]),
            usage,
        ));
        harness.open();
        let output = harness.frame(Vec::new());
        let texts = painted_texts(&output);
        assert!(!texts.iter().any(|text| text == "A"), "{texts:?}");
        assert!(texts.iter().any(|text| text == "R"), "{texts:?}");
    }

    #[test]
    fn the_field_badges_hdr_colors() {
        let mut ldr = Harness::new(ColorProperty::ldr(Color::from_srgb8([20, 140, 220, 255])));
        let output = ldr.frame(Vec::new());
        assert!(!painted_texts(&output).iter().any(|text| text == "HDR"));

        let mut hdr = Harness::new(ColorProperty::hdr(Color::new(3.0, 1.0, 0.5, 1.0)));
        let output = hdr.frame(Vec::new());
        assert!(
            painted_texts(&output).iter().any(|text| text == "HDR"),
            "HDR fields carry Unity's badge: {:?}",
            painted_texts(&output)
        );
    }

    #[test]
    fn clicking_the_field_opens_the_unity_picker() {
        let mut harness = Harness::new(ColorProperty::ldr(Color::from_srgb8([20, 140, 220, 255])));
        harness.open();
        let output = harness.frame(Vec::new());
        let texts = painted_texts(&output);

        for expected in [
            "Color",
            "RGB 0-1.0",
            "R",
            "G",
            "B",
            "A",
            "Hexadecimal",
            "Swatches",
        ] {
            assert!(
                texts.iter().any(|text| text == expected),
                "missing {expected:?}: {texts:?}"
            );
        }
        assert!(
            !texts.iter().any(|text| text == "Intensity"),
            "LDR has no intensity row: {texts:?}"
        );
    }

    #[test]
    fn an_hdr_field_opens_the_hdr_picker() {
        let mut harness = Harness::new(ColorProperty::hdr(Color::new(4.0, 2.0, 1.0, 1.0)));
        harness.open();
        let output = harness.frame(Vec::new());
        let texts = painted_texts(&output);

        assert!(texts.iter().any(|text| text == "HDR Color"), "{texts:?}");
        assert!(texts.iter().any(|text| text == "Intensity"), "{texts:?}");
        assert!(texts.iter().any(|text| text == "+2"), "{texts:?}");
        assert!(texts.iter().any(|text| text == "-2"), "{texts:?}");
    }

    #[test]
    fn a_swatch_click_writes_the_value() {
        let mut harness = Harness::new(ColorProperty::ldr(Color::from_srgb8([20, 140, 220, 255])));
        harness.open();
        harness.frame(Vec::new());
        let swatch = color_swatch_id(harness.field, 4);
        harness.click(swatch);

        let [r, g, b, _] = harness.value.color.to_srgb8();
        assert!(r < 5 && g < 5 && b < 5, "{:?}", harness.value);
    }

    #[test]
    fn an_hdr_preset_scales_the_color() {
        let mut harness = Harness::new(ColorProperty::hdr(Color::new(1.0, 0.5, 0.25, 1.0)));
        harness.open();
        harness.frame(Vec::new());
        harness.click(color_intensity_preset_id(harness.field, 4));

        let color = harness.value.color;
        assert!((color.r() - 4.0).abs() < 0.01, "{color:?}");
        assert!((color.g() - 2.0).abs() < 0.01, "{color:?}");
        assert!((color.b() - 1.0).abs() < 0.01, "{color:?}");
    }

    #[test]
    fn a_channel_number_can_be_typed() {
        let mut harness = Harness::new(ColorProperty::ldr(Color::from_srgb8([20, 140, 220, 255])));
        harness.open();
        harness.frame(Vec::new());
        harness.click(color_channel_value_id(harness.field, 0));
        harness.frame(Vec::new());
        harness.frame(vec![egui::Event::Text("0.5".to_owned())]);
        harness.frame(vec![key(Key::Enter)]);

        let red = harness.value.color.to_srgb8()[0];
        assert!((127..=128).contains(&red), "{red}: {:?}", harness.value);
    }

    #[test]
    fn the_mode_dropdown_switches_to_bytes() {
        let mut harness = Harness::new(ColorProperty::ldr(Color::from_srgb8([20, 140, 220, 255])));
        harness.open();
        harness.frame(Vec::new());
        harness.click(color_mode_id(harness.field));
        harness.frame(Vec::new());
        harness.click(color_mode_item_id(harness.field, 1));
        let output = harness.frame(Vec::new());
        let texts = painted_texts(&output);

        assert!(texts.iter().any(|text| text == "RGB 0-255"), "{texts:?}");
        assert!(texts.iter().any(|text| text == "140"), "{texts:?}");
    }

    #[test]
    fn the_wheel_square_updates_saturation_and_value() {
        let mut harness = Harness::new(ColorProperty::ldr(Color::from_srgb8([128, 128, 128, 255])));
        harness.open();
        harness.frame(Vec::new());
        let rect = harness
            .ctx
            .read_response(color_wheel_id(harness.field))
            .unwrap()
            .rect;
        // The square occupies the middle; this point is in its upper-right.
        let pos = rect.center() + vec2(30.0, -30.0);
        harness.click_at(pos);

        let [r, g, b, _] = harness.value.color.to_srgb8();
        assert!(
            r > g && g <= b,
            "the upper-right of the square is a saturated hue: {r}, {g}, {b}"
        );
        assert!(r > 200, "{r}");
    }

    #[test]
    fn hex_editing_keeps_a_translucent_alpha() {
        let mut harness = Harness::new(ColorProperty::ldr(Color::from_srgb8([20, 140, 220, 128])));
        harness.open();
        harness.frame(Vec::new());
        harness.click(color_hex_id(harness.field));
        harness.frame(Vec::new());
        harness.frame(vec![egui::Event::Text("#FF0000".to_owned())]);
        harness.frame(vec![key(Key::Enter)]);

        assert_eq!(harness.value.color.to_srgb8(), [255, 0, 0, 128]);
    }

    #[test]
    fn hex_editing_replaces_the_color() {
        let mut harness = Harness::new(ColorProperty::ldr(Color::from_srgb8([20, 140, 220, 255])));
        harness.open();
        harness.frame(Vec::new());
        harness.click(color_hex_id(harness.field));
        // The click seeds the edit; one more pass lets the new TextEdit take
        // focus before the text event is delivered.
        harness.frame(Vec::new());
        harness.frame(vec![egui::Event::Text("#00FF00".to_owned())]);
        harness.frame(vec![key(Key::Enter)]);

        let [r, g, b, _] = harness.value.color.to_srgb8();
        assert_eq!((r, g, b), (0, 255, 0));
    }
}
