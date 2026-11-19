//! Unity/Tuanjie-style color editing.
//!
//! Two widgets live here:
//!
//! * [`ColorField`] is the inline inspector row: a full-width swatch with an
//!   optional HDR badge and the eyedropper affordance, matching the color
//!   property in Unity's Inspector.
//! * [`color_picker`] is the floating window it opens: a hue ring around a
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
use std::path::PathBuf;

use egui::epaint::{Mesh, Vertex, WHITE_UV};
use egui::{
    Align, Align2, Color32, ColorImage, CornerRadius, CursorIcon, Id, Key, Margin, Pos2, Rect,
    Response, Sense, Stroke, StrokeKind, TextEdit, TextureHandle, TextureOptions, Ui, UiBuilder,
    Widget, WidgetType, Window, pos2, vec2,
};
use kairos_color::{Color, ColorUsage, linear_to_srgb, srgb_to_linear};
use serde::{Deserialize, Serialize};

use crate::button::{Button, ButtonAlign, ButtonStyle};
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

    /// Unity's HDR base reference: 191/255, the largest displayable channel
    /// value that still leaves room for the exposure slider.
    pub const HDR_BASE_MAX: f32 = 191.0 / 255.0;

    /// The exposure, in stops, that maps the brightest channel onto
    /// [`Self::HDR_BASE_MAX`].
    ///
    /// LDR properties always report zero; their picker has no Intensity row.
    /// This matches Unity's `ColorMutator.DecomposeHdrColor`, including its
    /// 191/255 base: a color whose max channel is 4.0 opens at about 2.42
    /// stops, not 2.0.
    pub fn intensity(self) -> f32 {
        if !self.usage.hdr {
            return 0.0;
        }
        let max = self.color.max_rgb();
        if max > 1.0 || (max > 0.0 && max < 1.0 / 255.0) {
            (max / Self::HDR_BASE_MAX).log2()
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
    /// Row label while its channel is the active edit target. Unity uses a
    /// softer blue than the selection accent so the label does not glow.
    pub label_active: Color32,
    pub value: Color32,
    pub field: Color32,
    pub field_hover: Color32,
    pub field_edge: Color32,
    pub accent: Color32,
    /// Popup-field trigger: a raised control that reads as a button against
    /// the picker body, like Unity's enum/object fields.
    pub menu_button_fill: Color32,
    pub menu_button_hover: Color32,
    pub close: Color32,
    pub close_hover: Color32,
    pub checker_light: Color32,
    pub checker_dark: Color32,
    /// Runtime image under `Preferences/Textures`, tinted by the style.
    pub eyedropper_icon: &'static str,
    pub border_width: f32,
    pub corner_radius: u8,
    pub window_width: f32,
    pub title_height: f32,
    pub padding: f32,
    pub section_gap: f32,
    pub row_height: f32,
    pub row_gap: f32,
    pub field_height: f32,
    pub wheel_size: f32,
    pub ring_thickness: f32,
    pub square_padding: f32,
    pub mode_width: f32,
    pub hex_width: f32,
    pub channel_label_width: f32,
    pub channel_value_width: f32,
    pub channel_track_height: f32,
    pub preview_swatch_width: f32,
    pub preview_swatch_height: f32,
    pub eyedropper_width: f32,
    pub swatch_size: f32,
    pub swatch_gap: f32,
    pub swatch_header_height: f32,
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
            title: TextStyle::sized(Color32::from_rgb(0xEE, 0xEE, 0xEE), 12.0).strong(),
            background: Color32::from_rgb(0x30, 0x30, 0x30),
            header: Color32::from_rgb(0x3C, 0x3C, 0x3C),
            border: Color32::from_rgb(0x16, 0x16, 0x16),
            label: Color32::from_rgb(0xB4, 0xB4, 0xB4),
            label_active: Color32::from_rgb(0x8F, 0xB4, 0xD9),
            value: Color32::from_rgb(0xE6, 0xE6, 0xE6),
            field: Color32::from_rgb(0x22, 0x22, 0x22),
            field_hover: Color32::from_rgb(0x3A, 0x3A, 0x3A),
            field_edge: Color32::from_rgb(0x18, 0x18, 0x18),
            accent: Color32::from_rgb(0x4A, 0x90, 0xD9),
            menu_button_fill: Color32::from_rgb(0x4A, 0x4A, 0x4A),
            menu_button_hover: Color32::from_rgb(0x56, 0x56, 0x56),
            close: Color32::from_rgb(0xC4, 0x3B, 0x3B),
            close_hover: Color32::from_rgb(0xD9, 0x4A, 0x4A),
            checker_light: Color32::from_rgb(0xC8, 0xC8, 0xC8),
            checker_dark: Color32::from_rgb(0x8C, 0x8C, 0x8C),
            eyedropper_icon: "Preferences/Textures/color_picker/eyedropper.png",
            border_width: 1.0,
            corner_radius: 0,
            window_width: 260.0,
            title_height: 22.0,
            padding: 6.0,
            section_gap: 10.0,
            row_height: 20.0,
            row_gap: 6.0,
            field_height: 18.0,
            wheel_size: 232.0,
            ring_thickness: 26.0,
            square_padding: 6.0,
            mode_width: 94.0,
            hex_width: 94.0,
            channel_label_width: 14.0,
            channel_value_width: 42.0,
            channel_track_height: 14.0,
            preview_swatch_width: 42.0,
            preview_swatch_height: 26.0,
            eyedropper_width: 42.0,
            swatch_size: 14.0,
            swatch_gap: 1.0,
            swatch_header_height: 20.0,
        }
    }

    pub fn light() -> Self {
        Self {
            text: TextStyle::sized(Color32::from_rgb(0x1E, 0x1E, 0x1E), 12.0),
            title: TextStyle::sized(Color32::from_rgb(0x1E, 0x1E, 0x1E), 12.0).strong(),
            background: Color32::from_rgb(0xEB, 0xEB, 0xEB),
            header: Color32::from_rgb(0xDC, 0xDC, 0xDC),
            border: Color32::from_rgb(0xA8, 0xA8, 0xA8),
            label: Color32::from_rgb(0x3A, 0x3A, 0x3A),
            label_active: Color32::from_rgb(0x2C, 0x5D, 0x87),
            value: Color32::from_rgb(0x1E, 0x1E, 0x1E),
            field: Color32::WHITE,
            field_hover: Color32::from_rgb(0xF0, 0xF0, 0xF0),
            field_edge: Color32::from_rgb(0xA8, 0xA8, 0xA8),
            accent: Color32::from_rgb(0x2C, 0x5D, 0x87),
            menu_button_fill: Color32::from_rgb(0xE0, 0xE0, 0xE0),
            menu_button_hover: Color32::WHITE,
            checker_light: Color32::from_rgb(0xE6, 0xE6, 0xE6),
            checker_dark: Color32::from_rgb(0xB4, 0xB4, 0xB4),
            ..Self::dark()
        }
    }
}

/// Unity's dark-skin menu highlight. Menus are chrome, not color values: they
/// keep the project menu blue even though the picker's accent is brighter.
const MENU_HIGHLIGHT: Color32 = Color32::from_rgb(0x2C, 0x5D, 0x87);

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

/// Stable id of the original-color preview swatch (click to reset).
pub fn color_original_id(field: Id) -> Id {
    field.with("original")
}

/// Stable id of the current-color preview swatch.
pub fn color_current_id(field: Id) -> Id {
    field.with("current")
}

/// Stable id of the Swatches foldout header.
pub fn color_swatches_fold_id(field: Id) -> Id {
    field.with("swatches-fold")
}

/// Stable id of the swatch-group menu button.
pub fn color_swatch_group_id(field: Id) -> Id {
    field.with("swatch-group")
}

/// Stable id of the floating swatch-group menu.
pub fn color_swatch_group_menu_id(field: Id) -> Id {
    field.with("swatch-group-menu")
}

/// Stable id of one group row in the group menu.
pub fn color_swatch_group_item_id(field: Id, index: usize) -> Id {
    field.with(("swatch-group-item", index))
}

/// Stable id of the New Group menu item.
pub fn color_swatch_group_new_id(field: Id) -> Id {
    field.with("swatch-group-new")
}

/// Stable id of the Rename Group menu item.
pub fn color_swatch_group_rename_id(field: Id) -> Id {
    field.with("swatch-group-rename")
}

/// Stable id of the Delete Group menu item.
pub fn color_swatch_group_delete_id(field: Id) -> Id {
    field.with("swatch-group-delete")
}

/// Stable id of the New swatch tile.
pub fn color_new_swatch_id(field: Id) -> Id {
    field.with("new-swatch")
}

/// Stable id of the group-name text field.
pub fn color_group_name_id(field: Id) -> Id {
    field.with("group-name")
}

/// The one floating picker, if any. Unity keeps a single ColorPicker window;
/// the editor follows that instead of allowing one picker per field.
#[derive(Clone, Debug)]
struct OpenPicker {
    field: Id,
    pos: Pos2,
    anchor: Rect,
    rect: Rect,
}

fn open_picker_id() -> Id {
    Id::new("kairos.color_picker.open")
}

/// Whether the floating picker is open for `field`.
pub fn is_color_picker_open(ctx: &egui::Context, field: Id) -> bool {
    ctx.data(|data| {
        data.get_temp::<OpenPicker>(open_picker_id())
            .is_some_and(|open| open.field == field)
    })
}

/// Whether any color picker is open.
pub fn any_color_picker_open(ctx: &egui::Context) -> bool {
    ctx.data(|data| data.get_temp::<OpenPicker>(open_picker_id()).is_some())
}

/// Close the floating picker, if one is open.
pub fn close_color_picker(ctx: &egui::Context) {
    ctx.data_mut(|data| data.remove::<OpenPicker>(open_picker_id()));
}

/// Alias used by panels that close every transient popup when leaving a page.
pub fn close_all_color_pickers(ctx: &egui::Context) {
    close_color_picker(ctx);
}

fn picker_position(ctx: &egui::Context, anchor: Rect, style: &ColorPickerStyle) -> Pos2 {
    let screen = ctx.content_rect();
    let width = style.window_width;
    let x = (anchor.right() - width).clamp(
        screen.left() + 4.0,
        (screen.right() - width - 4.0).max(screen.left() + 4.0),
    );
    // The picker is about 520 px tall with the HDR rows; if it does not fit
    // below the field, open it above. The window remains draggable afterwards.
    let estimate = 620.0;
    let below = anchor.bottom() + 2.0;
    let y = if below + estimate <= screen.bottom() {
        below
    } else {
        (anchor.top() - estimate - 2.0).max(screen.top() + 4.0)
    };
    pos2(x, y)
}

fn open_picker(ctx: &egui::Context, field: Id, anchor: Rect, style: &ColorPickerStyle) {
    let pos = picker_position(ctx, anchor, style);
    ctx.data_mut(|data| {
        data.insert_temp(
            open_picker_id(),
            OpenPicker {
                field,
                pos,
                anchor,
                rect: Rect::NOTHING,
            },
        );
    });
    ctx.request_repaint();
}

/// The inline color field: a swatch plus the eyedropper slot.
///
/// The widget allocates the available width and the requested height, paints
/// the swatch, and opens the floating Unity-style picker when the swatch is
/// clicked. The returned response belongs to the swatch (not the eyedropper
/// slot) and reports `changed()` when an edit in the picker moved the value.
pub struct ColorField<'a> {
    id: Id,
    value: &'a mut ColorProperty,
    style: &'a ColorPickerStyle,
    selected: bool,
    height: Option<f32>,
    eyedropper: bool,
    swatches: Option<&'a mut ColorSwatches>,
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
            swatches: None,
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

    /// Use the panel-owned preset book. Without one the picker still opens,
    /// but the Swatches section is omitted rather than keeping private,
    /// non-persistent state per field.
    pub fn swatches(mut self, swatches: &'a mut ColorSwatches) -> Self {
        self.swatches = Some(swatches);
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
            swatches,
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
                "Screen color picking is not available yet; use the wheel, sliders or hex.",
            );
        }

        let was_open = is_color_picker_open(ui.ctx(), id);
        if swatch_response.clicked() {
            if was_open {
                close_color_picker(ui.ctx());
            } else {
                open_picker(ui.ctx(), id, swatch_rect, style);
            }
        }

        let mut changed = false;
        if is_color_picker_open(ui.ctx(), id) {
            changed = show_color_picker(
                ui,
                id,
                value,
                style,
                swatches,
                was_open && !swatch_response.clicked(),
            );
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
        let icon = Rect::from_center_size(
            eye_rect.center(),
            vec2(eye_rect.height() - 2.0, eye_rect.height() - 2.0),
        );
        crate::ui_image::paint(
            ui,
            icon,
            style.eyedropper_icon,
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

/// Which channel representation the sliders use.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ChannelMode {
    /// Unity's default: R/G/B/A in 0..255.
    #[default]
    Rgb255,
    /// R/G/B/A in 0..1.
    RgbUnit,
    /// HDR-only: absolute linear channels with a dynamic maximum.
    RgbHdr,
    /// Hue in degrees, saturation/value/alpha in 0..100.
    Hsv,
}

impl ChannelMode {
    fn options(hdr: bool) -> &'static [Self] {
        const LDR: [ChannelMode; 3] = [ChannelMode::Rgb255, ChannelMode::RgbUnit, ChannelMode::Hsv];
        const HDR: [ChannelMode; 3] = [ChannelMode::Rgb255, ChannelMode::RgbHdr, ChannelMode::Hsv];
        if hdr { &HDR } else { &LDR }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Rgb255 => "RGB 0-255",
            Self::RgbUnit => "RGB 0-1.0",
            Self::RgbHdr => "RGB 0-Inf",
            Self::Hsv => "HSV",
        }
    }

    fn valid_for(self, hdr: bool) -> bool {
        match self {
            Self::RgbHdr => hdr,
            Self::RgbUnit => !hdr,
            _ => true,
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

/// One named group of saved colors. Unity calls these preset libraries;
/// Kairos calls them swatch groups because they are also used outside the
/// color picker.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColorSwatchGroup {
    pub name: String,
    pub colors: Vec<Color>,
}

impl ColorSwatchGroup {
    pub fn new(name: impl Into<String>, colors: Vec<Color>) -> Self {
        Self {
            name: name.into(),
            colors,
        }
    }
}

fn default_swatch_group() -> ColorSwatchGroup {
    ColorSwatchGroup::new("Default", default_swatch_colors())
}

fn default_swatches() -> Vec<ColorSwatchGroup> {
    vec![default_swatch_group()]
}

/// The shared, persisted preset book used by every color picker.
///
/// The widget layer owns the data and its RON format; the host owns the file
/// path (Library/ColorSwatches.ron in the editor). The path is skipped by
/// serde so a book loaded from disk stays portable; dirty tracks whether the
/// next save_if_dirty has work to do.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColorSwatches {
    #[serde(default = "default_swatches")]
    pub groups: Vec<ColorSwatchGroup>,
    #[serde(default)]
    pub active: usize,
    #[serde(skip)]
    path: Option<PathBuf>,
    #[serde(skip)]
    dirty: bool,
}

impl Default for ColorSwatches {
    fn default() -> Self {
        Self {
            groups: default_swatches(),
            active: 0,
            path: None,
            dirty: false,
        }
    }
}

impl ColorSwatches {
    /// Load a book from disk. A missing file is not an error: it returns the
    /// default palette with the path attached so the first edit persists.
    pub fn load(path: impl Into<PathBuf>) -> Result<Self, String> {
        let path = path.into();
        let mut book = match std::fs::read_to_string(&path) {
            Ok(text) => ron::from_str::<Self>(&text)
                .map_err(|error| format!("parse {}: {error}", path.display()))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(error) => return Err(format!("read {}: {error}", path.display())),
        };
        book.normalize();
        book.path = Some(path);
        book.dirty = false;
        Ok(book)
    }

    /// Attach the destination used by save_if_dirty.
    pub fn set_path(&mut self, path: impl Into<PathBuf>) {
        self.path = Some(path.into());
    }

    pub fn path(&self) -> Option<&std::path::Path> {
        self.path.as_deref()
    }

    pub fn active_group(&self) -> &ColorSwatchGroup {
        let index = self.active.min(self.groups.len().saturating_sub(1));
        &self.groups[index]
    }

    pub fn active_group_mut(&mut self) -> &mut ColorSwatchGroup {
        let index = self.active.min(self.groups.len().saturating_sub(1));
        &mut self.groups[index]
    }

    pub fn active_index(&self) -> usize {
        self.active.min(self.groups.len().saturating_sub(1))
    }

    pub fn select_group(&mut self, index: usize) -> bool {
        if index >= self.groups.len() || index == self.active_index() {
            return false;
        }
        self.active = index;
        self.dirty = true;
        true
    }

    /// Create a group and make it active. Returns false for a blank or
    /// duplicate name.
    pub fn create_group(&mut self, name: &str) -> bool {
        let name = name.trim();
        if name.is_empty() || self.groups.iter().any(|group| group.name == name) {
            return false;
        }
        self.groups.push(ColorSwatchGroup::new(name, Vec::new()));
        self.active = self.groups.len() - 1;
        self.dirty = true;
        true
    }

    pub fn rename_group(&mut self, index: usize, name: &str) -> bool {
        let name = name.trim();
        if name.is_empty()
            || index >= self.groups.len()
            || self
                .groups
                .iter()
                .enumerate()
                .any(|(other, group)| other != index && group.name == name)
        {
            return false;
        }
        if self.groups[index].name == name {
            return true;
        }
        self.groups[index].name = name.to_owned();
        self.dirty = true;
        true
    }

    pub fn delete_group(&mut self, index: usize) -> bool {
        if self.groups.len() <= 1 || index >= self.groups.len() {
            return false;
        }
        self.groups.remove(index);
        if self.active >= self.groups.len() {
            self.active = self.groups.len() - 1;
        } else if self.active > index {
            self.active -= 1;
        }
        self.dirty = true;
        true
    }

    pub fn add_color(&mut self, color: Color) -> bool {
        let group = self.active_group_mut();
        if group.colors.last() == Some(&color) {
            return false;
        }
        group.colors.push(color);
        self.dirty = true;
        true
    }

    /// Persist pending edits. No-op until a path is attached, which keeps unit
    /// tests and read-only previews from writing the user's Library.
    pub fn save_if_dirty(&mut self) -> Result<(), String> {
        if !self.dirty {
            return Ok(());
        }
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("create {}: {error}", parent.display()))?;
        }
        let text = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(|error| format!("serialize swatches: {error}"))?;
        std::fs::write(path, text).map_err(|error| format!("write {}: {error}", path.display()))?;
        self.dirty = false;
        Ok(())
    }

    fn normalize(&mut self) {
        if self.groups.is_empty() {
            self.groups = default_swatches();
        }
        self.active = self.active.min(self.groups.len() - 1);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GroupEditKind {
    New,
    Rename,
}

#[derive(Clone, Debug)]
struct GroupEdit {
    kind: GroupEditKind,
    text: String,
    focus: bool,
    error: String,
}

/// Per-field picker state kept in egui memory.
///
/// The value is the source of truth while the picker is closed. source
/// remembers the last value this state produced; a mismatch means something
/// else changed the field, so the next open re-seeds the base color, hue and
/// intensity from it.
#[derive(Clone, Debug)]
struct PickerState {
    source: Option<ColorProperty>,
    /// The color captured when the picker opened; the left preview swatch
    /// resets to it, and Escape reverts to it before closing.
    original: Option<ColorProperty>,
    /// sRGB RGBA in 0..1: the color the wheel and sliders edit.
    base: [f32; 4],
    /// Exposure in stops; only used when the value is HDR.
    intensity: f32,
    /// Dynamic maximum for HDR 0-Inf channel sliders.
    hdr_max: f32,
    /// Hue is kept separately so a gray value (saturation 0) remembers the
    /// wheel position while the user raises saturation.
    hue: f32,
    mode: ChannelMode,
    drag: Option<WheelPart>,
    edit: Option<NumberEdit>,
    hex: Option<HexEdit>,
    /// The channel row (0..3) or intensity row (4) the user last edited.
    active_channel: Option<usize>,
    show_swatches: bool,
    group_menu_open: bool,
    group_edit: Option<GroupEdit>,
}

impl Default for PickerState {
    fn default() -> Self {
        Self {
            source: None,
            original: None,
            base: [0.0, 0.0, 0.0, 1.0],
            intensity: 0.0,
            hdr_max: 1.0,
            hue: 0.0,
            mode: ChannelMode::Rgb255,
            drag: None,
            edit: None,
            hex: None,
            active_channel: None,
            show_swatches: true,
            group_menu_open: false,
            group_edit: None,
        }
    }
}

impl PickerState {
    fn sync(&mut self, value: ColorProperty) {
        self.intensity = value.intensity();
        self.base = value.base_srgb(self.intensity);
        self.hdr_max = value.color.max_rgb().max(1.0);
        let hsv = rgb_to_hsv(self.base);
        if hsv[1] > 1e-6 {
            self.hue = hsv[0];
        }
        if !self.mode.valid_for(value.is_hdr()) {
            self.mode = if value.is_hdr() {
                ChannelMode::RgbHdr
            } else {
                ChannelMode::Rgb255
            };
        }
        self.source = Some(value);
        self.drag = None;
        self.edit = None;
        self.hex = None;
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
        if value.is_hdr() {
            self.hdr_max = self.hdr_max.max(value.color.max_rgb());
        }
        changed
    }

    /// Re-seed the base and exposure after an absolute HDR channel edit,
    /// without dropping an in-progress text edit or drag.
    fn refresh_from_value(&mut self, value: ColorProperty) {
        self.intensity = value.intensity();
        self.base = value.base_srgb(self.intensity);
        self.hdr_max = self.hdr_max.max(value.color.max_rgb());
        let hsv = rgb_to_hsv(self.base);
        if hsv[1] > 1e-6 {
            self.hue = hsv[0];
        }
        self.source = Some(value);
    }

    fn linear_rgba(&self) -> [f32; 4] {
        let scale = 2.0_f32.powf(self.intensity);
        [
            srgb_to_linear(self.base[0]) * scale,
            srgb_to_linear(self.base[1]) * scale,
            srgb_to_linear(self.base[2]) * scale,
            self.base[3],
        ]
    }

    fn channel_max(&self, index: usize) -> f32 {
        match self.mode {
            ChannelMode::Rgb255 => 255.0,
            ChannelMode::RgbUnit => 1.0,
            ChannelMode::RgbHdr => self.hdr_max.max(1.0),
            ChannelMode::Hsv if index == 0 => 360.0,
            ChannelMode::Hsv => 100.0,
        }
    }

    fn channel_value(&self, index: usize) -> f32 {
        match self.mode {
            ChannelMode::Rgb255 => self.base[index] * 255.0,
            ChannelMode::RgbUnit => self.base[index],
            ChannelMode::RgbHdr => {
                if index == 3 {
                    self.base[3]
                } else {
                    self.linear_rgba()[index]
                }
            }
            ChannelMode::Hsv => {
                let hsv = self.hsv();
                if index == 0 {
                    hsv[0] * 360.0
                } else if index == 3 {
                    self.base[3] * 100.0
                } else {
                    hsv[index] * 100.0
                }
            }
        }
    }

    /// Set one channel from its displayed value (the inverse of
    /// channel_value). HDR 0-Inf is handled separately because it edits an
    /// absolute linear value and re-derives the base/exposure.
    fn set_channel(&mut self, index: usize, displayed: f32) {
        match self.mode {
            ChannelMode::RgbUnit => self.base[index] = displayed.clamp(0.0, 1.0),
            ChannelMode::Rgb255 => self.base[index] = (displayed / 255.0).clamp(0.0, 1.0),
            ChannelMode::RgbHdr => {}
            ChannelMode::Hsv if index == 0 => {
                let hsv = self.hsv();
                self.set_hsv([displayed / 360.0, hsv[1], hsv[2]]);
            }
            ChannelMode::Hsv if index == 3 => {
                self.base[3] = (displayed / 100.0).clamp(0.0, 1.0);
            }
            ChannelMode::Hsv => {
                let mut hsv = self.hsv();
                hsv[index] = (displayed / 100.0).clamp(0.0, 1.0);
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

    /// Set an absolute linear HDR channel (Unity RGB 0-Inf).
    fn set_hdr_channel(&mut self, value: &mut ColorProperty, index: usize, displayed: f32) {
        let mut linear = self.linear_rgba();
        linear[index] = displayed.max(0.0);
        let next = ColorProperty::new(
            Color::new(linear[0], linear[1], linear[2], linear[3]),
            value.usage,
        );
        *value = next;
        self.refresh_from_value(next);
    }

    fn channel_text(&self, index: usize) -> String {
        let value = self.channel_value(index);
        match self.mode {
            ChannelMode::Rgb255 | ChannelMode::Hsv => format!("{value:.0}"),
            ChannelMode::RgbHdr => format_trimmed(value, 3),
            ChannelMode::RgbUnit => format_trimmed(value, 3),
        }
    }
}

fn channel_label(mode: ChannelMode, index: usize) -> &'static str {
    match mode {
        ChannelMode::Hsv => match index {
            0 => "H",
            1 => "S",
            2 => "V",
            _ => "A",
        },
        _ => match index {
            0 => "R",
            1 => "G",
            2 => "B",
            _ => "A",
        },
    }
}

fn channel_name(mode: ChannelMode, index: usize) -> &'static str {
    match mode {
        ChannelMode::Hsv => match index {
            0 => "Hue",
            1 => "Saturation",
            2 => "Value",
            _ => "Alpha",
        },
        _ => match index {
            0 => "Red",
            1 => "Green",
            2 => "Blue",
            _ => "Alpha",
        },
    }
}

fn default_swatch_colors() -> Vec<Color> {
    [
        [1.0, 1.0, 1.0],
        [0.75, 0.75, 0.75],
        [0.5, 0.5, 0.5],
        [0.25, 0.25, 0.25],
        [0.0, 0.0, 0.0],
        [0.86, 0.08, 0.24],
        [0.36, 0.56, 0.94],
        [0.23, 0.72, 0.33],
    ]
    .into_iter()
    .map(|[r, g, b]| Color::from_srgb_f32([r, g, b, 1.0]))
    .collect()
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

/// What one picker pass produced.
#[derive(Clone, Copy, Debug, Default)]
struct PickerResponse {
    changed: bool,
    close: bool,
    /// A small editor (new/rename group) is open outside the window rect.
    keep_open: bool,
}

fn show_color_picker(
    ui: &mut Ui,
    field: Id,
    value: &mut ColorProperty,
    style: &ColorPickerStyle,
    swatches: Option<&mut ColorSwatches>,
    was_open: bool,
) -> bool {
    let Some(mut open) = ui.data(|data| data.get_temp::<OpenPicker>(open_picker_id())) else {
        return false;
    };
    if open.field != field {
        return false;
    }

    let mut drag_delta = egui::Vec2::ZERO;
    let mut header_close = false;
    let frame = egui::Frame::NONE
        .fill(style.background)
        .stroke(Stroke::new(style.border_width, style.border))
        .corner_radius(CornerRadius::same(style.corner_radius))
        .shadow(egui::epaint::Shadow {
            offset: [0, 4],
            blur: 12,
            spread: 0,
            color: Color32::from_black_alpha(90),
        });

    let window = Window::new("")
        .id(color_popup_id(field))
        .title_bar(false)
        .movable(false)
        .resizable(false)
        .collapsible(false)
        .order(egui::Order::Foreground)
        .fixed_pos(open.pos)
        .auto_sized()
        .fade_in(false)
        .frame(frame);

    let Some(window_response) = window.show(ui.ctx(), |ui| {
        ui.set_width(style.window_width);
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let header = picker_header(ui, field, value, style);
        header_close = header.close;
        drag_delta = header.drag_delta;
        egui::Frame::NONE
            .inner_margin(Margin::same(style.padding.round() as i8))
            .show(ui, |ui| {
                ui.set_width((style.window_width - 2.0 * style.padding).max(80.0));
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                color_picker(ui, field, value, style, swatches, was_open)
            })
            .inner
    }) else {
        return false;
    };

    let picker = window_response.inner.unwrap_or_default();
    let changed = picker.changed;
    open.rect = window_response.response.rect;
    open.pos = window_response.response.rect.min + drag_delta;
    let screen = ui.ctx().content_rect();
    open.pos.x = open.pos.x.clamp(
        screen.left() + 2.0,
        (screen.right() - style.window_width - 2.0).max(screen.left() + 2.0),
    );
    open.pos.y = open.pos.y.max(ui.ctx().content_rect().top());

    let mut close = header_close || picker.close;
    if !close
        && !picker.keep_open
        && ui.input(|input| input.pointer.any_pressed())
        && let Some(pos) = ui.input(|input| input.pointer.interact_pos())
        && !open.rect.contains(pos)
        && !open.anchor.contains(pos)
        && !egui::Popup::is_any_open(ui.ctx())
    {
        close = true;
    }

    if close {
        close_color_picker(ui.ctx());
    } else {
        ui.data_mut(|data| data.insert_temp(open_picker_id(), open));
    }
    changed
}

/// Draw the picker body for one field.
///
/// The floating window and frame come from show_color_picker; this function
/// owns the per-field state (mode, in-progress text, wheel drag) and the value
/// updates.
fn color_picker(
    ui: &mut Ui,
    field: Id,
    value: &mut ColorProperty,
    style: &ColorPickerStyle,
    swatches: Option<&mut ColorSwatches>,
    was_open: bool,
) -> PickerResponse {
    let state_id = field.with("picker-state");
    let mut state = ui
        .data(|data| data.get_temp::<PickerState>(state_id))
        .unwrap_or_default();

    if !was_open || state.source.is_none() {
        state.sync(*value);
        state.original = Some(*value);
        state.active_channel = None;
        // Transient chrome belongs to one opening of the picker. Closing the
        // window must not make the group menu/dialog reappear next time.
        state.group_menu_open = false;
        state.group_edit = None;
    } else if state.source != Some(*value) {
        state.sync(*value);
    }

    let editing_at_start = state.edit.is_some() || state.hex.is_some();
    let menu_at_start = egui::Popup::is_any_open(ui.ctx());
    let group_menu_at_start = state.group_menu_open;
    let group_editing_at_start = state.group_edit.is_some();
    let mut response = PickerResponse {
        changed: picker_ui_body(ui, field, value, style, &mut state, swatches),
        close: false,
        keep_open: false,
    };
    let menu_open = menu_at_start || egui::Popup::is_any_open(ui.ctx());
    let group_editing = group_editing_at_start || state.group_edit.is_some();
    let group_menu_open = group_menu_at_start || state.group_menu_open;
    response.keep_open = group_editing || menu_open || group_menu_open;

    if ui.input(|input| input.key_pressed(Key::Escape))
        && !editing_at_start
        && !menu_open
        && !group_editing
    {
        if group_menu_open {
            state.group_menu_open = false;
        } else {
            if let Some(original) = state.original
                && *value != original
            {
                *value = original;
                state.sync(original);
                response.changed = true;
            }
            response.close = true;
        }
    } else if ui.input(|input| input.key_pressed(Key::Enter))
        && !group_editing
        && !menu_open
        && !group_menu_open
    {
        response.close = true;
    }

    ui.data_mut(|data| data.insert_temp(state_id, state));
    response
}

fn picker_ui_body(
    ui: &mut Ui,
    field: Id,
    value: &mut ColorProperty,
    style: &ColorPickerStyle,
    state: &mut PickerState,
    swatches: Option<&mut ColorSwatches>,
) -> bool {
    let mut changed = false;
    changed |= preview_row(
        ui,
        field,
        value,
        state.original.unwrap_or(*value),
        style,
        state,
    );
    ui.add_space(style.section_gap);
    changed |= wheel(ui, field, value, style, state);
    ui.add_space(style.section_gap);
    changed |= mode_row(ui, field, value, style, state);
    ui.add_space(style.row_gap);
    let channels = if value.usage.alpha { 4 } else { 3 };
    for channel in 0..channels {
        changed |= channel_row(ui, field, value, style, state, channel);
        if channel + 1 < channels {
            ui.add_space(style.row_gap);
        }
    }
    if !value.is_hdr() {
        ui.add_space(style.row_gap);
        changed |= hex_row(ui, field, value, style, state);
    }
    if value.is_hdr() {
        ui.add_space(style.row_gap);
        changed |= intensity_row(ui, field, value, style, state);
        ui.add_space(style.row_gap);
        changed |= presets_row(ui, field, value, style, state);
    }
    if let Some(swatches) = swatches {
        ui.add_space(style.section_gap);
        changed |= swatches_row(ui, field, value, style, state, swatches);
    }
    changed
}

/// The picker header is the only drag handle. The body must never move the
/// window while the user drags a slider or the wheel.
struct HeaderResponse {
    close: bool,
    drag_delta: egui::Vec2,
}

fn picker_header(
    ui: &mut Ui,
    field: Id,
    value: &ColorProperty,
    style: &ColorPickerStyle,
) -> HeaderResponse {
    let (rect, _) = ui.allocate_exact_size(
        vec2(ui.available_width(), style.title_height),
        Sense::hover(),
    );
    ui.painter()
        .rect_filled(rect, CornerRadius::ZERO, style.header);

    let close_rect =
        Rect::from_center_size(pos2(rect.right() - 12.0, rect.center().y), vec2(16.0, 16.0));
    let drag_rect = Rect::from_min_max(rect.min, pos2(close_rect.left() - 2.0, rect.bottom()));
    let drag = ui
        .interact(drag_rect, field.with("title-drag"), Sense::click_and_drag())
        .on_hover_cursor(if ui.input(|input| input.pointer.any_down()) {
            CursorIcon::Grabbing
        } else {
            CursorIcon::Grab
        });
    let drag_delta = if drag.dragged() {
        drag.drag_delta()
    } else {
        egui::Vec2::ZERO
    };

    let title = if value.is_hdr() { "HDR Color" } else { "Color" };
    style.title.paint(
        ui,
        pos2(rect.left() + 8.0, rect.center().y),
        Align2::LEFT_CENTER,
        title,
    );

    let close = ui
        .interact(close_rect, color_close_id(field), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    let close_fill = if close.hovered() {
        style.close_hover
    } else {
        style.close
    };
    ui.painter()
        .rect_filled(close_rect, CornerRadius::same(2), close_fill);
    let center = close_rect.center();
    let x = 3.5;
    let stroke = Stroke::new(1.2, Color32::WHITE);
    ui.painter()
        .line_segment([center + vec2(-x, -x), center + vec2(x, x)], stroke);
    ui.painter()
        .line_segment([center + vec2(-x, x), center + vec2(x, -x)], stroke);

    HeaderResponse {
        close: close.clicked(),
        drag_delta,
    }
}

fn preview_row(
    ui: &mut Ui,
    field: Id,
    value: &mut ColorProperty,
    original: ColorProperty,
    style: &ColorPickerStyle,
    state: &mut PickerState,
) -> bool {
    let (rect, _) = ui.allocate_exact_size(
        vec2(ui.available_width(), style.preview_swatch_height + 2.0),
        Sense::hover(),
    );
    let mut changed = false;

    if value.usage.eyedropper {
        let eye_rect = Rect::from_min_size(rect.min, vec2(style.eyedropper_width, rect.height()));
        let eye = ui
            .interact(eye_rect, color_picker_eye_id(field), Sense::hover())
            .on_hover_cursor(CursorIcon::Default);
        if eye.hovered() {
            ui.painter()
                .rect_filled(eye_rect, CornerRadius::same(2), style.field_hover);
        }
        let icon = Rect::from_center_size(
            eye_rect.center(),
            vec2(
                style.preview_swatch_height - 2.0,
                style.preview_swatch_height - 2.0,
            ),
        );
        crate::ui_image::paint(
            ui,
            icon,
            style.eyedropper_icon,
            if eye.hovered() {
                style.value
            } else {
                style.label
            },
        );
        let _ = eye.on_hover_text(
            "Screen color picking is not available yet; use the wheel, sliders or hex.",
        );
    }

    let current = Rect::from_min_size(
        pos2(rect.right() - style.preview_swatch_width, rect.top() + 1.0),
        vec2(style.preview_swatch_width, style.preview_swatch_height),
    );
    let original_rect = current.translate(vec2(-style.preview_swatch_width, 0.0));
    let reset = ui
        .interact(original_rect, color_original_id(field), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    let _ = reset
        .clone()
        .on_hover_text("Original color. Click to reset the picker.");
    paint_swatch(ui, original_rect, original, style);
    if reset.clicked() {
        *value = original;
        state.sync(original);
        changed = true;
    }
    paint_swatch(ui, current, *value, style);
    changed
}

fn paint_swatch(ui: &Ui, rect: Rect, value: ColorProperty, style: &ColorPickerStyle) {
    if value.is_hdr() || value.alpha() < 0.999 {
        paint_checkerboard(ui.painter(), rect, style);
    }
    ui.painter()
        .rect_filled(rect, CornerRadius::ZERO, value.display());
    ui.painter().rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(style.border_width, style.field_edge),
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
    fn new(rect: Rect, style: &ColorPickerStyle) -> Self {
        let center = rect.center();
        let outer = rect.width().min(rect.height()) * 0.5;
        let thickness = style.ring_thickness.clamp(1.0, outer * 0.5);
        let inner = (outer - thickness).max(1.0);
        // Unity: square side = floor(sqrt(2) * (outer - thumb size)), then the
        // six-pixel colorBoxPadding is removed on every side.
        let square_half =
            ((outer - thickness) * std::f32::consts::SQRT_2 * 0.5 - style.square_padding).max(12.0);
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

    let geometry = WheelGeometry::new(rect, style);
    paint_wheel(ui, &geometry, state, style);

    let mut changed = false;
    if response.is_pointer_button_down_on() {
        state.active_channel = None;
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
        vec2(geometry.outer * 2.0 + 3.0, geometry.outer * 2.0 + 3.0),
    ));

    let ring = color_ring_texture(ui.ctx(), style);
    painter.image(
        ring.id(),
        Rect::from_center_size(
            geometry.center,
            vec2(geometry.outer * 2.0, geometry.outer * 2.0),
        ),
        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
        Color32::WHITE,
    );

    let hsv = state.hsv();
    paint_sv_square(ui, &painter, geometry.square, hsv[0]);
    painter.rect_stroke(
        geometry.square,
        CornerRadius::ZERO,
        Stroke::new(1.0, style.field_edge),
        StrokeKind::Inside,
    );

    let hue_color = hsv_color(hsv[0]);
    let ring_angle = -hsv[0] * TAU;
    let ring_radius = (geometry.outer + geometry.inner) * 0.5;
    let ring_marker = geometry.center + vec2(ring_angle.cos(), ring_angle.sin()) * ring_radius;
    paint_hue_marker(&painter, ring_marker, hue_color, style);

    let marker = pos2(
        geometry.square.left() + hsv[1] * geometry.square.width(),
        geometry.square.top() + (1.0 - hsv[2]) * geometry.square.height(),
    );
    paint_sv_marker(&painter, marker, hsv[0], hsv[1], hsv[2]);
}

/// A 512 px RGBA ring with one-pixel alpha feathering. Unity uses a 216 px
/// (432 px at 2x) texture; generating it at 2x and letting the painter scale it
/// keeps the edge smooth at every DPI without a 144-segment polygon silhouette.
fn color_ring_texture(ctx: &egui::Context, style: &ColorPickerStyle) -> TextureHandle {
    let key = Id::new((
        "kairos.color_picker.ring",
        style.ring_thickness.round() as i32,
        style.wheel_size.round() as i32,
    ));
    if let Some(texture) = ctx.data(|data| data.get_temp::<TextureHandle>(key)) {
        return texture;
    }

    let size = 512usize;
    let outer = size as f32 * 0.5 - 1.0;
    let ratio = style.ring_thickness / (style.wheel_size * 0.5).max(1.0);
    let inner = (outer - ratio * outer).max(1.0);
    let mut pixels = vec![Color32::TRANSPARENT; size * size];
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 + 0.5 - size as f32 * 0.5;
            let dy = y as f32 + 0.5 - size as f32 * 0.5;
            let radius = (dx * dx + dy * dy).sqrt();
            let coverage = (outer - radius).min(radius - inner).clamp(0.0, 1.0);
            if coverage <= 0.0 {
                continue;
            }
            let hue = (-dy.atan2(dx) / TAU).rem_euclid(1.0);
            let color = hsv_color(hue);
            pixels[y * size + x] = Color32::from_rgba_unmultiplied(
                color.r(),
                color.g(),
                color.b(),
                (coverage * 255.0).round() as u8,
            );
        }
    }
    let texture = ctx.load_texture(
        "kairos.color_picker.ring",
        ColorImage::new([size, size], pixels),
        TextureOptions::LINEAR,
    );
    ctx.data_mut(|data| data.insert_temp(key, texture.clone()));
    texture
}

fn paint_sv_square(ui: &Ui, painter: &egui::Painter, rect: Rect, hue: f32) {
    let bucket = (hue.rem_euclid(1.0) * 360.0).round() as u16;
    let key = Id::new("kairos.color_picker.sv-square");
    let cached = ui
        .ctx()
        .data(|data| data.get_temp::<(TextureHandle, u16)>(key));
    let texture = if let Some((mut texture, cached_bucket)) = cached {
        if cached_bucket != bucket {
            texture.set(sv_square_image(hue), TextureOptions::LINEAR);
        }
        texture
    } else {
        ui.ctx().load_texture(
            "kairos.color_picker.sv-square",
            sv_square_image(hue),
            TextureOptions::LINEAR,
        )
    };
    painter.image(
        texture.id(),
        rect,
        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
        Color32::WHITE,
    );
    ui.ctx()
        .data_mut(|data| data.insert_temp(key, (texture, bucket)));
}

fn sv_square_image(hue: f32) -> ColorImage {
    const SIZE: usize = 128;
    let mut pixels = Vec::with_capacity(SIZE * SIZE);
    for y in 0..SIZE {
        let v = 1.0 - y as f32 / (SIZE - 1) as f32;
        for x in 0..SIZE {
            let s = x as f32 / (SIZE - 1) as f32;
            pixels.push(hsv_color_sv(hue, s, v));
        }
    }
    ColorImage::new([SIZE, SIZE], pixels)
}

fn paint_hue_marker(painter: &egui::Painter, pos: Pos2, color: Color32, style: &ColorPickerStyle) {
    let outer = (style.ring_thickness * 0.5).clamp(6.0, 14.0);
    let inner = (outer * 0.75).max(4.0);
    painter.circle_filled(pos, outer + 1.0, Color32::from_black_alpha(130));
    painter.circle_filled(pos, outer, Color32::from_rgb(0xF1, 0xF1, 0xF1));
    painter.circle_filled(pos, inner, color);
}

fn paint_sv_marker(painter: &egui::Painter, pos: Pos2, hue: f32, s: f32, v: f32) {
    let foreground = contrast_color(hsv_color_sv(hue, s, v));
    painter.circle_stroke(pos, 6.0, Stroke::new(2.0, Color32::from_black_alpha(120)));
    painter.circle_stroke(pos, 5.0, Stroke::new(1.5, foreground));
}

fn hsv_color_sv(h: f32, s: f32, v: f32) -> Color32 {
    let rgb = hsv_to_rgb(h, s, v);
    Color32::from_rgb(channel_u8(rgb[0]), channel_u8(rgb[1]), channel_u8(rgb[2]))
}

/// Stable id of one entry in the channel-mode dropdown.
pub fn color_mode_item_id(field: Id, index: usize) -> Id {
    field.with(("mode-item", index))
}

/// Unity's popup field: a raised, outlined control whose text starts at the
/// left and whose caret sits at the right. Shared by the channel-mode and
/// swatch-group triggers so they cannot drift apart.
fn menu_button_style(style: &ColorPickerStyle, height: f32) -> ButtonStyle {
    ButtonStyle {
        text: style.text.clone().color(style.value),
        fill: style.menu_button_fill,
        hover_fill: style.menu_button_hover,
        active_fill: style.menu_button_hover,
        stroke: Stroke::new(style.border_width, style.field_edge),
        corner_radius: CornerRadius::ZERO,
        padding: vec2(6.0, 0.0),
        height,
        caret_inset: 8.0,
    }
}

fn mode_row(
    ui: &mut Ui,
    field: Id,
    value: &ColorProperty,
    style: &ColorPickerStyle,
    state: &mut PickerState,
) -> bool {
    let (rect, _) =
        ui.allocate_exact_size(vec2(ui.available_width(), style.row_height), Sense::hover());
    let button_rect = Rect::from_min_size(
        pos2(rect.right() - style.mode_width, rect.top()),
        vec2(style.mode_width, style.row_height),
    );
    let button_style = menu_button_style(style, style.row_height);
    let open = egui::Popup::is_id_open(ui.ctx(), color_mode_id(field).with("popup"));
    let response = ui.put(
        button_rect,
        Button::new(state.mode.label(), &button_style)
            .width(style.mode_width)
            .height(style.row_height)
            .id(color_mode_id(field))
            .caret()
            .selected(open),
    );
    if response.clicked() {
        state.active_channel = None;
    }

    let mut changed = false;
    let popup = egui::Popup::from_response(&response)
        .open_memory(response.clicked().then_some(egui::SetOpenCommand::Toggle))
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .align(egui::RectAlign::BOTTOM_END)
        .gap(1.0)
        .width(style.mode_width)
        .frame(
            egui::Frame::NONE
                .fill(style.field)
                .stroke(Stroke::new(style.border_width, style.border))
                .corner_radius(CornerRadius::ZERO)
                .shadow(egui::epaint::Shadow {
                    offset: [1, 2],
                    blur: 6,
                    spread: 0,
                    color: Color32::from_black_alpha(128),
                }),
        );
    popup.show(|ui| {
        ui.set_width(style.mode_width - 2.0);
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        for (index, mode) in ChannelMode::options(value.is_hdr()).iter().enumerate() {
            let (row, _) = ui
                .allocate_exact_size(vec2(ui.available_width(), style.row_height), Sense::hover());
            let item = ui
                .interact(row, color_mode_item_id(field, index), Sense::click())
                .on_hover_cursor(CursorIcon::PointingHand);
            let active = state.mode == *mode;
            let highlighted = active || item.hovered();
            if highlighted {
                ui.painter()
                    .rect_filled(row, CornerRadius::ZERO, MENU_HIGHLIGHT);
            }
            if active {
                style.text.clone().color(Color32::WHITE).paint(
                    ui,
                    pos2(row.left() + 7.0, row.center().y),
                    Align2::LEFT_CENTER,
                    "✓",
                );
            }
            let color = if highlighted {
                Color32::WHITE
            } else {
                style.value
            };
            style.text.clone().color(color).paint(
                ui,
                pos2(row.left() + 22.0, row.center().y),
                Align2::LEFT_CENTER,
                mode.label(),
            );
            if item.clicked() {
                if state.mode != *mode {
                    state.mode = *mode;
                    if *mode == ChannelMode::RgbHdr {
                        state.hdr_max = value.color.max_rgb().max(1.0);
                    }
                    changed = true;
                }
                ui.close();
            }
        }
    });
    changed
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
    let value_width = style.channel_value_width;
    let half_track = style.channel_track_height * 0.5;
    let track = Rect::from_min_max(
        pos2(
            rect.left() + style.channel_label_width + 4.0,
            rect.center().y - half_track,
        ),
        pos2(
            rect.right() - value_width - 4.0,
            rect.center().y + half_track,
        ),
    );
    let value_rect = Rect::from_min_size(
        pos2(rect.right() - value_width, rect.top() + 1.0),
        vec2(value_width, rect.height() - 2.0),
    );

    let active = state.active_channel == Some(index);
    let label_style = style.text.clone().color(if active {
        style.label_active
    } else {
        style.label
    });
    label_style.paint(
        ui,
        pos2(rect.left() + 2.0, rect.center().y),
        Align2::LEFT_CENTER,
        channel_label(state.mode, index),
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
        channel_name(state.mode, index),
        None,
    );

    let mut changed = false;
    if track_response.is_pointer_button_down_on() {
        state.active_channel = Some(index);
        let pos = ui
            .input(|input| input.pointer.interact_pos())
            .unwrap_or(track.center());
        let t = ((pos.x - track.left()) / track.width()).clamp(0.0, 1.0);
        let displayed = t * state.channel_max(index);
        if state.mode == ChannelMode::RgbHdr && index < 3 {
            state.set_hdr_channel(value, index, displayed);
            changed = true;
        } else {
            state.set_channel(index, displayed);
            changed |= state.write(value);
        }
    }

    let max = state.channel_max(index).max(f32::MIN_POSITIVE);
    let t = (state.channel_value(index) / max).clamp(0.0, 1.0);
    paint_track_handle(ui.painter(), track, t);

    let editing = matches!(
        state.edit.as_ref().map(|edit| edit.target),
        Some(EditTarget::Channel(current)) if current == index
    );
    if editing {
        let mut edit = state.edit.take().expect("checked above");
        paint_value_box(ui, value_rect, style, true, true);
        let response = text_edit_box(
            ui,
            value_rect,
            color_channel_value_id(field, index),
            &mut edit.text,
            style,
            Align::Min,
        );
        if edit.focus {
            response.request_focus();
            select_all_text(ui, response.id, edit.text.chars().count());
            edit.focus = false;
        }
        let escape = ui.input(|input| input.key_pressed(Key::Escape));
        let enter = ui.input(|input| input.key_pressed(Key::Enter));
        if escape {
            // The value is only written on commit, so Escape just drops the
            // typed text.
        } else if enter || response.lost_focus() {
            if let Some(displayed) = parse_number(&edit.text) {
                if state.mode == ChannelMode::RgbHdr && index < 3 {
                    state.set_hdr_channel(value, index, displayed);
                    changed = true;
                } else {
                    state.set_channel(index, displayed.clamp(0.0, state.channel_max(index)));
                    changed |= state.write(value);
                }
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
        paint_value_box(ui, value_rect, style, response.hovered(), active);
        let text_style = style.text.clone().color(style.value);
        text_style.paint(
            ui,
            pos2(value_rect.left() + 5.0, value_rect.center().y),
            Align2::LEFT_CENTER,
            state.channel_text(index),
        );
        if response.clicked() {
            state.active_channel = Some(index);
            state.edit = Some(NumberEdit {
                target: EditTarget::Channel(index),
                text: state.channel_text(index),
                focus: true,
            });
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
    let [r, g, b] = [
        channel_u8(state.base[0]),
        channel_u8(state.base[1]),
        channel_u8(state.base[2]),
    ];

    match state.mode {
        ChannelMode::RgbUnit | ChannelMode::Rgb255 | ChannelMode::RgbHdr => match index {
            0 => paint_gradient(
                &painter,
                rect,
                Color32::from_rgb(0, g, b),
                Color32::from_rgb(255, g, b),
            ),
            1 => paint_gradient(
                &painter,
                rect,
                Color32::from_rgb(r, 0, b),
                Color32::from_rgb(r, 255, b),
            ),
            2 => paint_gradient(
                &painter,
                rect,
                Color32::from_rgb(r, g, 0),
                Color32::from_rgb(r, g, 255),
            ),
            _ => paint_alpha_gradient(&painter, rect, Color32::from_rgb(r, g, b)),
        },
        ChannelMode::Hsv => {
            let hsv = state.hsv();
            match index {
                0 => paint_hue_gradient(&painter, rect),
                1 => {
                    let from = hsv_to_rgb(hsv[0], 0.0, hsv[2]);
                    let to = hsv_to_rgb(hsv[0], 1.0, hsv[2]);
                    paint_gradient(&painter, rect, srgb_color(from), srgb_color(to));
                }
                2 => {
                    let to = hsv_to_rgb(hsv[0], hsv[1], 1.0);
                    paint_gradient(&painter, rect, Color32::BLACK, srgb_color(to));
                }
                _ => paint_alpha_gradient(&painter, rect, Color32::from_rgb(r, g, b)),
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

fn paint_alpha_gradient(painter: &egui::Painter, rect: Rect, color: Color32) {
    paint_gradient(
        painter,
        rect,
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 0),
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 255),
    );
}

fn paint_hue_gradient(painter: &egui::Painter, rect: Rect) {
    let stops = 6;
    let mut mesh = Mesh::default();
    for step in 0..=stops {
        let t = step as f32 / stops as f32;
        let x = rect.left() + t * rect.width();
        let color = hsv_color(t);
        mesh.vertices.push(Vertex {
            pos: pos2(x, rect.top()),
            uv: WHITE_UV,
            color,
        });
        mesh.vertices.push(Vertex {
            pos: pos2(x, rect.bottom()),
            uv: WHITE_UV,
            color,
        });
    }
    for step in 0..stops {
        let base = (step * 2) as u32;
        mesh.add_triangle(base, base + 1, base + 3);
        mesh.add_triangle(base, base + 3, base + 2);
    }
    painter.add(mesh);
}

fn paint_track_handle(painter: &egui::Painter, track: Rect, t: f32) {
    let x = track.left() + t * track.width();
    let handle = Rect::from_center_size(pos2(x, track.center().y), vec2(7.0, track.height() + 4.0));
    painter.rect_filled(handle, CornerRadius::same(2), Color32::WHITE);
    painter.rect_stroke(
        handle,
        CornerRadius::same(2),
        Stroke::new(1.0, Color32::from_black_alpha(170)),
        StrokeKind::Inside,
    );
}

fn paint_gradient(painter: &egui::Painter, rect: Rect, start: Color32, end: Color32) {
    let mut mesh = Mesh::default();
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
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
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
        pos2(rect.right() - style.hex_width, rect.top() + 1.0),
        vec2(style.hex_width, rect.height() - 2.0),
    );
    let label_style = style.text.clone().color(style.label);
    label_style.paint(
        ui,
        pos2(rect.left() + 2.0, rect.center().y),
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
            state.active_channel = None;
            state.hex = Some(HexEdit {
                text: hex_text(state),
                focus: true,
                original: *value,
            });
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
    let value_width = style.channel_value_width;
    let half_track = style.channel_track_height * 0.5;
    let track = Rect::from_min_max(
        pos2(rect.left() + 52.0, rect.center().y - half_track),
        pos2(
            rect.right() - value_width - 4.0,
            rect.center().y + half_track,
        ),
    );
    let value_rect = Rect::from_min_size(
        pos2(rect.right() - value_width, rect.top() + 1.0),
        vec2(value_width, rect.height() - 2.0),
    );

    let active = state.active_channel == Some(4);
    let label_style = style.text.clone().color(if active {
        style.label_active
    } else {
        style.label
    });
    label_style.paint(
        ui,
        pos2(rect.left() + 2.0, rect.center().y),
        Align2::LEFT_CENTER,
        "Intensity",
    );

    let painter = ui.painter();
    painter.rect_filled(track, CornerRadius::same(3), style.field);
    painter.rect_stroke(
        track,
        CornerRadius::same(3),
        Stroke::new(1.0, style.field_edge),
        StrokeKind::Inside,
    );
    let t = ((state.intensity + 10.0) / 20.0).clamp(0.0, 1.0);
    paint_round_handle(painter, track, t);

    let track_response = ui
        .interact(track, color_channel_id(field, 4), Sense::click_and_drag())
        .on_hover_cursor(CursorIcon::ResizeHorizontal);
    labelled_widget_info(&track_response, WidgetType::Slider, "Intensity", None);

    let mut changed = false;
    if track_response.is_pointer_button_down_on() {
        state.active_channel = Some(4);
        let pos = ui
            .input(|input| input.pointer.interact_pos())
            .unwrap_or(track.center());
        let t = ((pos.x - track.left()) / track.width()).clamp(0.0, 1.0);
        state.intensity = -10.0 + t * 20.0;
        changed |= state.write(value);
    }

    let editing = matches!(
        state.edit.as_ref().map(|edit| edit.target),
        Some(EditTarget::Intensity)
    );
    if editing {
        let mut edit = state.edit.take().expect("checked above");
        paint_value_box(ui, value_rect, style, true, true);
        let response = text_edit_box(
            ui,
            value_rect,
            color_channel_value_id(field, 4),
            &mut edit.text,
            style,
            Align::Min,
        );
        if edit.focus {
            response.request_focus();
            select_all_text(ui, response.id, edit.text.chars().count());
            edit.focus = false;
        }
        let escape = ui.input(|input| input.key_pressed(Key::Escape));
        let enter = ui.input(|input| input.key_pressed(Key::Enter));
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
        paint_value_box(ui, value_rect, style, response.hovered(), active);
        let text_style = style.text.clone().color(style.value);
        text_style.paint(
            ui,
            pos2(value_rect.left() + 5.0, value_rect.center().y),
            Align2::LEFT_CENTER,
            format_trimmed(state.intensity, 2),
        );
        if response.clicked() {
            state.active_channel = Some(4);
            state.edit = Some(NumberEdit {
                target: EditTarget::Intensity,
                text: format_trimmed(state.intensity, 2),
                focus: true,
            });
        }
    }

    changed
}

fn paint_round_handle(painter: &egui::Painter, track: Rect, t: f32) {
    let x = track.left() + t * track.width();
    painter.circle_filled(
        pos2(x, track.center().y),
        7.0,
        Color32::from_black_alpha(100),
    );
    painter.circle_filled(
        pos2(x, track.center().y),
        6.0,
        Color32::from_rgb(0xC8, 0xC8, 0xC8),
    );
    painter.circle_stroke(
        pos2(x, track.center().y),
        6.0,
        Stroke::new(1.0, Color32::from_black_alpha(160)),
    );
}

fn presets_row(
    ui: &mut Ui,
    field: Id,
    value: &mut ColorProperty,
    style: &ColorPickerStyle,
    state: &mut PickerState,
) -> bool {
    const PRESETS: [f32; 5] = [-2.0, -1.0, 0.0, 1.0, 2.0];
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::hover());
    let gap = 1.0;
    let cell_width = (rect.width() - gap * 4.0) / 5.0;
    let mut changed = false;

    for (index, preset) in PRESETS.iter().enumerate() {
        let cell = Rect::from_min_size(
            pos2(rect.left() + index as f32 * (cell_width + gap), rect.top()),
            vec2(cell_width, rect.height()),
        );
        let response = ui
            .interact(
                cell,
                color_intensity_preset_id(field, index),
                Sense::click(),
            )
            .on_hover_cursor(CursorIcon::PointingHand);
        let fill = value.color.scaled(*preset).to_egui();
        ui.painter().rect_filled(cell, CornerRadius::ZERO, fill);
        let edge = if *preset == 0.0 || response.hovered() {
            style.accent
        } else {
            style.field_edge
        };
        ui.painter().rect_stroke(
            cell,
            CornerRadius::ZERO,
            Stroke::new(1.0, edge),
            StrokeKind::Inside,
        );
        if *preset != 0.0 {
            let label = if *preset > 0.0 {
                format!("+{preset:.0}")
            } else {
                format!("{preset:.0}")
            };
            style.text.clone().color(contrast_color(fill)).paint(
                ui,
                cell.center(),
                Align2::CENTER_CENTER,
                label,
            );
        }
        if response.clicked() && *preset != 0.0 {
            state.intensity = (state.intensity + preset).clamp(-10.0, 10.0);
            changed |= state.write(value);
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
    swatches: &mut ColorSwatches,
) -> bool {
    let mut changed = false;
    let (header, _) = ui.allocate_exact_size(
        vec2(ui.available_width(), style.swatch_header_height),
        Sense::hover(),
    );

    let fold_rect = Rect::from_min_size(header.min, vec2(70.0, header.height()));
    let fold = ui
        .interact(fold_rect, color_swatches_fold_id(field), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    let triangle = if state.show_swatches {
        vec![
            pos2(header.left() + 4.0, header.center().y - 2.0),
            pos2(header.left() + 10.0, header.center().y - 2.0),
            pos2(header.left() + 7.0, header.center().y + 2.5),
        ]
    } else {
        vec![
            pos2(header.left() + 5.0, header.center().y - 3.0),
            pos2(header.left() + 10.0, header.center().y),
            pos2(header.left() + 5.0, header.center().y + 3.0),
        ]
    };
    ui.painter().add(egui::Shape::convex_polygon(
        triangle,
        style.label,
        Stroke::NONE,
    ));
    style.text.clone().color(style.label).paint(
        ui,
        pos2(header.left() + 15.0, header.center().y),
        Align2::LEFT_CENTER,
        "Swatches",
    );
    if fold.clicked() {
        state.show_swatches = !state.show_swatches;
    }

    let group_name = swatches.active_group().name.clone();
    let group_button_width = 118.0;
    let group_button_rect = Rect::from_min_size(
        pos2(header.right() - group_button_width, header.top()),
        vec2(group_button_width, header.height()),
    );
    let group_button = ui.put(
        group_button_rect,
        Button::new(
            &truncate(&group_name, 14),
            &menu_button_style(style, header.height()),
        )
        .width(group_button_width)
        .height(header.height())
        .id(color_swatch_group_id(field))
        .caret()
        .selected(state.group_menu_open),
    );
    if group_button.clicked() {
        state.group_menu_open = !state.group_menu_open;
    }

    // The group menu floats above the picker window. It must not allocate a
    // row in this layout: that moved every swatch down while the menu was open
    // and made the action items paint over one another.
    if state.group_menu_open {
        const MENU_WIDTH: f32 = 176.0;
        let menu_groups = swatches.groups.clone();
        let active_index = swatches.active_index();
        let screen = ui.ctx().content_rect();
        let x = group_button_rect.left().clamp(
            screen.left() + 4.0,
            (screen.right() - MENU_WIDTH - 4.0).max(screen.left() + 4.0),
        );
        let area = egui::Area::new(color_swatch_group_menu_id(field))
            .order(egui::Order::Foreground)
            .fixed_pos(pos2(x, group_button_rect.bottom() + 1.0))
            .constrain_to(screen)
            .show(ui.ctx(), |ui| {
                egui::Frame::NONE
                    .fill(style.field)
                    .stroke(Stroke::new(style.border_width, style.border))
                    .corner_radius(CornerRadius::ZERO)
                    .shadow(egui::epaint::Shadow {
                        offset: [1, 2],
                        blur: 6,
                        spread: 0,
                        color: Color32::from_black_alpha(128),
                    })
                    .inner_margin(Margin::symmetric(0, 4))
                    .show(ui, |ui| {
                        ui.set_width(MENU_WIDTH - 2.0);
                        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                        for (index, group) in menu_groups.iter().enumerate() {
                            let (row, _) = ui.allocate_exact_size(
                                vec2(ui.available_width(), style.row_height),
                                Sense::hover(),
                            );
                            let item = ui
                                .interact(
                                    row,
                                    color_swatch_group_item_id(field, index),
                                    Sense::click(),
                                )
                                .on_hover_cursor(CursorIcon::PointingHand);
                            let active = index == active_index;
                            let highlighted = active || item.hovered();
                            if highlighted {
                                ui.painter()
                                    .rect_filled(row, CornerRadius::ZERO, MENU_HIGHLIGHT);
                            }
                            if active {
                                style.text.clone().color(Color32::WHITE).paint(
                                    ui,
                                    pos2(row.left() + 7.0, row.center().y),
                                    Align2::LEFT_CENTER,
                                    "✓",
                                );
                            }
                            style
                                .text
                                .clone()
                                .color(if highlighted {
                                    Color32::WHITE
                                } else {
                                    style.value
                                })
                                .paint(
                                    ui,
                                    pos2(row.left() + 22.0, row.center().y),
                                    Align2::LEFT_CENTER,
                                    truncate(&group.name, 20),
                                );
                            if item.clicked() {
                                if swatches.select_group(index) {
                                    let _ = swatches.save_if_dirty();
                                }
                                state.group_menu_open = false;
                            }
                        }
                        ui.add_space(4.0);
                        let (rule, _) =
                            ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
                        ui.painter()
                            .rect_filled(rule, CornerRadius::ZERO, style.field_hover);
                        ui.add_space(4.0);
                        let new = group_menu_item(
                            ui,
                            color_swatch_group_new_id(field),
                            "New Group...",
                            true,
                            style,
                        );
                        if new.clicked() {
                            state.group_edit = Some(GroupEdit {
                                kind: GroupEditKind::New,
                                text: String::new(),
                                focus: true,
                                error: String::new(),
                            });
                            state.group_menu_open = false;
                        }
                        let rename = group_menu_item(
                            ui,
                            color_swatch_group_rename_id(field),
                            "Rename Group...",
                            true,
                            style,
                        );
                        if rename.clicked() {
                            state.group_edit = Some(GroupEdit {
                                kind: GroupEditKind::Rename,
                                text: group_name.clone(),
                                focus: true,
                                error: String::new(),
                            });
                            state.group_menu_open = false;
                        }
                        let delete = group_menu_item(
                            ui,
                            color_swatch_group_delete_id(field),
                            "Delete Group",
                            swatches.groups.len() > 1,
                            style,
                        );
                        if delete.clicked() {
                            if swatches.delete_group(swatches.active_index()) {
                                let _ = swatches.save_if_dirty();
                            }
                            state.group_menu_open = false;
                        }
                    });
            });
        let menu_rect = area.response.rect;
        if ui.input(|input| input.pointer.any_pressed())
            && let Some(pos) = ui.input(|input| input.pointer.interact_pos())
            && !menu_rect.contains(pos)
            && !group_button_rect.contains(pos)
        {
            state.group_menu_open = false;
        }
    }

    if let Some(mut edit) = state.group_edit.take() {
        let title = match edit.kind {
            GroupEditKind::New => "New Swatch Group",
            GroupEditKind::Rename => "Rename Swatch Group",
        };
        let mut confirmed = false;
        let mut cancelled = false;
        const EDIT_WIDTH: f32 = 220.0;
        let screen = ui.ctx().content_rect();
        let x = (group_button_rect.right() - EDIT_WIDTH).clamp(
            screen.left() + 4.0,
            (screen.right() - EDIT_WIDTH - 4.0).max(screen.left() + 4.0),
        );
        egui::Area::new(field.with("group-edit"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos2(x, group_button_rect.bottom() + 1.0))
            .constrain_to(screen)
            .show(ui.ctx(), |ui| {
                egui::Frame::NONE
                    .fill(style.background)
                    .stroke(Stroke::new(style.border_width, style.border))
                    .corner_radius(CornerRadius::ZERO)
                    .shadow(egui::epaint::Shadow {
                        offset: [1, 2],
                        blur: 6,
                        spread: 0,
                        color: Color32::from_black_alpha(128),
                    })
                    .inner_margin(Margin::same(8))
                    .show(ui, |ui| {
                        ui.set_width(EDIT_WIDTH - 16.0);
                        ui.spacing_mut().item_spacing = vec2(0.0, 5.0);
                        ui.label(
                            egui::RichText::new(title)
                                .color(style.value)
                                .size(12.0)
                                .strong(),
                        );

                        let (field_rect, _) = ui
                            .allocate_exact_size(vec2(ui.available_width(), 20.0), Sense::hover());
                        let focused =
                            ui.memory(|memory| memory.has_focus(color_group_name_id(field)));
                        paint_value_box(ui, field_rect, style, false, focused || edit.focus);
                        let response = text_edit_box(
                            ui,
                            field_rect,
                            color_group_name_id(field),
                            &mut edit.text,
                            style,
                            Align::Min,
                        );
                        if edit.focus {
                            response.request_focus();
                            select_all_text(ui, response.id, edit.text.chars().count());
                            edit.focus = false;
                        }
                        if !edit.error.is_empty() {
                            ui.label(
                                egui::RichText::new(edit.error.clone())
                                    .color(style.close)
                                    .size(11.0),
                            );
                        }

                        ui.horizontal(|ui| {
                            let gap = (ui.available_width() - 110.0).max(0.0);
                            ui.add_space(gap);
                            let button_style = menu_button_style(style, 20.0);
                            if ui
                                .add(
                                    Button::new("OK", &button_style)
                                        .width(52.0)
                                        .height(20.0)
                                        .align(ButtonAlign::Center),
                                )
                                .clicked()
                            {
                                confirmed = true;
                            }
                            ui.add_space(6.0);
                            if ui
                                .add(
                                    Button::new("Cancel", &button_style)
                                        .width(52.0)
                                        .height(20.0)
                                        .align(ButtonAlign::Center),
                                )
                                .clicked()
                            {
                                cancelled = true;
                            }
                        });
                    });
            });
        if ui.input(|input| input.key_pressed(Key::Enter)) {
            confirmed = true;
        }
        if ui.input(|input| input.key_pressed(Key::Escape)) {
            cancelled = true;
        }
        if confirmed {
            let ok = match edit.kind {
                GroupEditKind::New => swatches.create_group(&edit.text),
                GroupEditKind::Rename => swatches.rename_group(swatches.active_index(), &edit.text),
            };
            if ok {
                let _ = swatches.save_if_dirty();
            } else {
                edit.error = if edit.text.trim().is_empty() {
                    "Name cannot be empty"
                } else {
                    "That group already exists"
                }
                .to_owned();
                edit.focus = true;
                state.group_edit = Some(edit);
            }
        } else if !cancelled {
            state.group_edit = Some(edit);
        }
    }

    if !state.show_swatches {
        return changed;
    }

    let colors = swatches.active_group().colors.clone();
    let cell = style.swatch_size;
    let gap = style.swatch_gap;
    let columns = (((ui.available_width() + gap) / (cell + gap)).floor() as usize).max(1);
    let items = colors.len() + 1;
    let rows = items.div_ceil(columns);
    let grid_height = rows as f32 * (cell + gap) - gap;
    let (grid, _) = ui.allocate_exact_size(vec2(ui.available_width(), grid_height), Sense::hover());

    for (index, swatch) in colors.iter().enumerate() {
        let column = index % columns;
        let row = index / columns;
        let cell_rect = Rect::from_min_size(
            pos2(
                grid.left() + column as f32 * (cell + gap),
                grid.top() + row as f32 * (cell + gap),
            ),
            vec2(cell, cell),
        );
        let response = ui
            .interact(cell_rect, color_swatch_id(field, index), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        let _ = response.clone().on_hover_text(swatch.to_hex());
        if swatch.a() < 0.999 {
            paint_checkerboard(ui.painter(), cell_rect, style);
        }
        ui.painter()
            .rect_filled(cell_rect, CornerRadius::ZERO, swatch.to_egui());
        let selected = [0, 1, 2]
            .iter()
            .all(|channel| (state.base[*channel] - swatch.to_srgb_f32()[*channel]).abs() < 0.01);
        let edge = if selected {
            style.accent
        } else if response.hovered() {
            style.value
        } else {
            style.field_edge
        };
        ui.painter().rect_stroke(
            cell_rect,
            CornerRadius::ZERO,
            Stroke::new(if selected { 2.0 } else { 1.0 }, edge),
            StrokeKind::Inside,
        );
        if response.clicked() {
            let mut next = *swatch;
            if !value.is_hdr() {
                let [r, g, b, a] = next.to_linear_f32();
                next = Color::new(
                    r.clamp(0.0, 1.0),
                    g.clamp(0.0, 1.0),
                    b.clamp(0.0, 1.0),
                    a.clamp(0.0, 1.0),
                );
            }
            *value = ColorProperty::new(next, value.usage);
            state.sync(*value);
            state.active_channel = None;
            changed = true;
        }
    }

    let new_index = colors.len();
    let column = new_index % columns;
    let row = new_index / columns;
    let new_rect = Rect::from_min_size(
        pos2(
            grid.left() + column as f32 * (cell + gap),
            grid.top() + row as f32 * (cell + gap),
        ),
        vec2(cell, cell),
    );
    let new = ui
        .interact(new_rect, color_new_swatch_id(field), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    let _ = new
        .clone()
        .on_hover_text("Save the current color as a swatch");
    if value.alpha() < 0.999 {
        paint_checkerboard(ui.painter(), new_rect, style);
    }
    ui.painter()
        .rect_filled(new_rect, CornerRadius::ZERO, value.display());
    ui.painter().rect_stroke(
        new_rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, style.value),
        StrokeKind::Inside,
    );
    let plus_color = contrast_color(value.display());
    ui.painter().line_segment(
        [
            pos2(new_rect.center().x - 3.0, new_rect.center().y),
            pos2(new_rect.center().x + 3.0, new_rect.center().y),
        ],
        Stroke::new(1.0, plus_color),
    );
    ui.painter().line_segment(
        [
            pos2(new_rect.center().x, new_rect.center().y - 3.0),
            pos2(new_rect.center().x, new_rect.center().y + 3.0),
        ],
        Stroke::new(1.0, plus_color),
    );
    if new.clicked() && swatches.add_color(value.color) {
        let _ = swatches.save_if_dirty();
    }
    if colors.is_empty() && grid.width() > 70.0 {
        style.text.clone().color(style.label).paint(
            ui,
            pos2(new_rect.right() + 5.0, new_rect.center().y),
            Align2::LEFT_CENTER,
            "Click to add new preset",
        );
    }

    changed
}

fn group_menu_item(
    ui: &mut Ui,
    id: Id,
    label: &str,
    enabled: bool,
    style: &ColorPickerStyle,
) -> Response {
    let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0), Sense::hover());
    let response = ui
        .interact(
            row,
            id,
            if enabled {
                Sense::click()
            } else {
                Sense::hover()
            },
        )
        .on_hover_cursor(if enabled {
            CursorIcon::PointingHand
        } else {
            CursorIcon::NotAllowed
        });
    let highlighted = response.hovered() && enabled;
    if highlighted {
        ui.painter()
            .rect_filled(row, CornerRadius::ZERO, MENU_HIGHLIGHT);
    }
    style
        .text
        .clone()
        .color(if highlighted {
            Color32::WHITE
        } else if enabled {
            style.value
        } else {
            style.label
        })
        .paint(
            ui,
            pos2(row.left() + 8.0, row.center().y),
            Align2::LEFT_CENTER,
            label,
        );
    response
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn paint_value_box(ui: &Ui, rect: Rect, style: &ColorPickerStyle, hovered: bool, active: bool) {
    let fill = if hovered {
        style.field_hover
    } else {
        style.field
    };
    ui.painter().rect_filled(rect, CornerRadius::ZERO, fill);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(
            style.border_width,
            if active {
                style.accent
            } else {
                style.field_edge
            },
        ),
        StrokeKind::Inside,
    );
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

/// Unity's selected-number highlight: a translucent blue wash rather than a
/// solid block that hides the digits being edited.
fn text_selection_fill(accent: Color32) -> Color32 {
    // Unity's value-field selection is a solid-looking blue block (~62%)
    // behind white digits; the digits stay legible thanks to the opaque
    // `selection.stroke` colour set by `text_edit_box`.
    Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 160)
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
        ui.visuals_mut().selection.bg_fill = text_selection_fill(style.accent);
        // egui paints selected glyphs with the selection stroke colour, so a
        // `NONE` stroke would make the selected number disappear entirely.
        ui.visuals_mut().selection.stroke = Stroke::new(1.0, style.value);
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
        swatches: ColorSwatches,
    }

    impl Harness {
        fn new(value: ColorProperty) -> Self {
            Self {
                ctx: egui::Context::default(),
                value,
                style: ColorPickerStyle::dark(),
                field: Id::new("color-field"),
                swatches: ColorSwatches::default(),
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
                swatches,
            } = self;
            ctx.run_ui(input, |ui| {
                ui.add_space(20.0);
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    ui.add_sized(
                        vec2(240.0, 18.0),
                        ColorField::new(*field, value, style)
                            .height(18.0)
                            .swatches(swatches),
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
        let scale = 4.0 / ColorProperty::HDR_BASE_MAX;
        assert!((hdr.intensity() - scale.log2()).abs() < 1e-4);
        let base = hdr.base_srgb(hdr.intensity());
        assert!(
            (base[0] - linear_to_srgb(ColorProperty::HDR_BASE_MAX)).abs() < 1e-3,
            "{base:?}"
        );
        assert!(
            (base[1] - linear_to_srgb(0.5 * ColorProperty::HDR_BASE_MAX)).abs() < 1e-3,
            "{base:?}"
        );
        assert!(
            (base[2] - linear_to_srgb(0.25 * ColorProperty::HDR_BASE_MAX)).abs() < 1e-3,
            "{base:?}"
        );
        assert!(
            (hdr.with_base(base, hdr.intensity()).color.r() - 4.0).abs() < 0.01,
            "{:?}",
            hdr.with_base(base, hdr.intensity())
        );

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
            "RGB 0-255",
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
        assert!(
            !texts.iter().any(|text| text == "Hexadecimal"),
            "HDR has no hex row: {texts:?}"
        );
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
        harness.frame(vec![egui::Event::Text("128".to_owned())]);
        harness.frame(vec![key(Key::Enter)]);

        let red = harness.value.color.to_srgb8()[0];
        assert!((127..=128).contains(&red), "{red}: {:?}", harness.value);
    }

    #[test]
    fn the_mode_dropdown_switches_to_unit_floats() {
        let mut harness = Harness::new(ColorProperty::ldr(Color::from_srgb8([20, 140, 220, 255])));
        harness.open();
        harness.frame(Vec::new());
        harness.click(color_mode_id(harness.field));
        harness.frame(Vec::new());
        harness.click(color_mode_item_id(harness.field, 1));
        let output = harness.frame(Vec::new());
        let texts = painted_texts(&output);

        assert!(texts.iter().any(|text| text == "RGB 0-1.0"), "{texts:?}");
        assert!(texts.iter().any(|text| text == "0.078"), "{texts:?}");
    }

    #[test]
    fn clicking_a_channel_value_marks_it_active() {
        let mut harness = Harness::new(ColorProperty::ldr(Color::from_srgb8([20, 140, 220, 255])));
        harness.open();
        harness.frame(Vec::new());
        harness.click(color_channel_value_id(harness.field, 1));
        harness.frame(Vec::new());
        let state = harness
            .ctx
            .data(|data| data.get_temp::<PickerState>(harness.field.with("picker-state")))
            .unwrap();
        assert_eq!(state.active_channel, Some(1));
    }

    #[test]
    fn hsv_mode_uses_h_s_v_labels_and_percent_ranges() {
        let mut harness = Harness::new(ColorProperty::ldr(Color::from_srgb8([20, 140, 220, 255])));
        harness.open();
        harness.frame(Vec::new());
        harness.click(color_mode_id(harness.field));
        harness.frame(Vec::new());
        harness.click(color_mode_item_id(harness.field, 2));
        let output = harness.frame(Vec::new());
        let texts = painted_texts(&output);
        for expected in ["HSV", "H", "S", "V", "A"] {
            assert!(
                texts.iter().any(|text| text == expected),
                "missing {expected}: {texts:?}"
            );
        }
        assert!(!texts.iter().any(|text| text == "R"), "{texts:?}");
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
        let pos = rect.center() + vec2(38.0, -38.0);
        harness.click_at(pos);

        let [r, g, b, _] = harness.value.color.to_srgb8();
        assert!(
            r > g && g <= b,
            "the upper-right of the square is a saturated hue: {r}, {g}, {b}"
        );
        assert!(r > 195, "{r}");
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

    #[test]
    fn the_picker_uses_the_unity_metrics() {
        let style = ColorPickerStyle::dark();
        assert_eq!(style.window_width, 260.0);
        assert_eq!(style.wheel_size, 232.0);
        assert_eq!(style.ring_thickness, 26.0);
        assert_eq!(style.padding, 6.0);
        assert_eq!(style.row_height, 20.0);
        assert_eq!(style.mode_width, 94.0);
        assert_eq!(style.hex_width, 94.0);
        assert_eq!(style.channel_track_height, 14.0);
        assert_eq!(style.swatch_size, 14.0);
    }

    #[test]
    fn channel_rows_use_the_widened_track() {
        let mut harness = Harness::new(ColorProperty::ldr(Color::from_srgb8([20, 140, 220, 255])));
        harness.open();
        harness.frame(Vec::new());
        let track = harness
            .ctx
            .read_response(color_channel_id(harness.field, 0))
            .expect("channel track")
            .rect;
        assert!(
            track.width() > 180.0,
            "the channel bar should span the widened row, got {track:?}"
        );
        assert_eq!(track.height(), harness.style.channel_track_height);
    }

    #[test]
    fn menu_triggers_read_as_outlined_popup_fields() {
        let mut harness = Harness::new(ColorProperty::ldr(Color::from_srgb8([20, 140, 220, 255])));
        harness.open();
        harness.frame(Vec::new());
        let output = harness.frame(Vec::new());
        let style = harness.style.clone();
        let mode = harness
            .ctx
            .read_response(color_mode_id(harness.field))
            .expect("mode trigger")
            .rect;
        let group = harness
            .ctx
            .read_response(color_swatch_group_id(harness.field))
            .expect("group trigger")
            .rect;
        for (name, rect) in [("mode", mode), ("group", group)] {
            assert!(
                painted_fill(&output, style.menu_button_fill, rect.center()),
                "the {name} trigger should have a field fill, not the panel background"
            );
            assert!(
                output.shapes.iter().any(|clipped| matches!(
                    &clipped.shape,
                    egui::Shape::Rect(shape)
                        if shape.rect == rect && shape.stroke.width > 0.0
                )),
                "the {name} trigger should be outlined like Unity's popup field"
            );
        }
    }

    #[test]
    fn the_group_menu_floats_and_keeps_its_actions_apart() {
        let mut harness = Harness::new(ColorProperty::ldr(Color::from_srgb8([20, 140, 220, 255])));
        harness.open();
        // `Window::auto_sized` can take two passes to settle after opening.
        for _ in 0..3 {
            harness.frame(Vec::new());
        }
        let before = harness
            .ctx
            .read_response(color_new_swatch_id(harness.field))
            .expect("new swatch tile")
            .rect;

        harness.click(color_swatch_group_id(harness.field));
        // Let the AutoSized picker window settle before measuring the popup.
        harness.frame(Vec::new());
        harness.frame(Vec::new());
        let after = harness
            .ctx
            .read_response(color_new_swatch_id(harness.field))
            .expect("new swatch tile")
            .rect;
        assert_eq!(
            before, after,
            "opening the group menu must not push the swatch grid down"
        );

        let new = harness
            .ctx
            .read_response(color_swatch_group_new_id(harness.field))
            .expect("New Group item")
            .rect;
        let rename = harness
            .ctx
            .read_response(color_swatch_group_rename_id(harness.field))
            .expect("Rename Group item")
            .rect;
        let delete = harness
            .ctx
            .read_response(color_swatch_group_delete_id(harness.field))
            .expect("Delete Group item")
            .rect;
        assert!(
            new.width() > 150.0,
            "menu rows should span the menu: {new:?}"
        );
        assert!(
            new.bottom() <= rename.top() && rename.bottom() <= delete.top(),
            "menu actions must occupy separate rows: {new:?} {rename:?} {delete:?}"
        );
    }

    #[test]
    fn the_selected_value_mask_is_heavy_but_still_translucent() {
        let fill = text_selection_fill(Color32::from_rgb(0x4A, 0x90, 0xD9));
        assert!(
            fill.a() > 128 && fill.a() < 220,
            "Unity's selected field is a heavy blue block, got alpha {}",
            fill.a()
        );
    }

    #[test]
    fn clicking_outside_the_floating_picker_closes_it() {
        let mut harness = Harness::new(ColorProperty::ldr(Color::from_srgb8([20, 140, 220, 255])));
        harness.open();
        harness.frame(Vec::new());
        assert!(is_color_picker_open(&harness.ctx, harness.field));

        harness.click_at(pos2(500.0, 740.0));
        assert!(!is_color_picker_open(&harness.ctx, harness.field));
    }

    #[test]
    fn the_hdr_mode_menu_offers_absolute_channels() {
        let mut harness = Harness::new(ColorProperty::hdr(Color::new(4.0, 2.0, 1.0, 1.0)));
        harness.open();
        harness.frame(Vec::new());
        harness.click(color_mode_id(harness.field));
        let output = harness.frame(Vec::new());
        let texts = painted_texts(&output);
        assert!(texts.iter().any(|text| text == "RGB 0-Inf"), "{texts:?}");

        harness.click(color_mode_item_id(harness.field, 1));
        let output = harness.frame(Vec::new());
        assert!(
            painted_texts(&output)
                .iter()
                .any(|text| text == "RGB 0-Inf"),
            "{:?}",
            painted_texts(&output)
        );

        harness.click(color_channel_value_id(harness.field, 0));
        harness.frame(Vec::new());
        harness.frame(vec![egui::Event::Text("6".to_owned())]);
        harness.frame(vec![key(Key::Enter)]);
        assert!(
            (harness.value.color.r() - 6.0).abs() < 0.01,
            "{:?}",
            harness.value
        );
    }

    #[test]
    fn dragging_the_title_bar_moves_the_floating_picker() {
        let mut harness = Harness::new(ColorProperty::ldr(Color::from_srgb8([20, 140, 220, 255])));
        harness.open();
        harness.frame(Vec::new());
        let before = harness
            .ctx
            .data(|data| data.get_temp::<OpenPicker>(open_picker_id()).unwrap().pos);
        let start = before + vec2(40.0, 10.0);
        harness.frame(vec![egui::Event::PointerMoved(start)]);
        harness.frame(vec![pointer(start, true)]);
        harness.frame(vec![egui::Event::PointerMoved(start + vec2(30.0, 20.0))]);
        harness.frame(vec![pointer(start + vec2(30.0, 20.0), false)]);
        let after = harness
            .ctx
            .data(|data| data.get_temp::<OpenPicker>(open_picker_id()).unwrap().pos);
        assert!(
            after.x > before.x + 10.0,
            "before {before:?}, after {after:?}"
        );
        assert!(
            after.y > before.y + 5.0,
            "before {before:?}, after {after:?}"
        );
    }

    #[test]
    fn the_original_swatch_resets_the_current_color() {
        let original = ColorProperty::ldr(Color::from_srgb8([20, 140, 220, 255]));
        let mut harness = Harness::new(original);
        harness.open();
        harness.frame(Vec::new());
        let wheel = harness
            .ctx
            .read_response(color_wheel_id(harness.field))
            .unwrap()
            .rect;
        harness.click_at(wheel.center() + vec2(30.0, -30.0));
        assert_ne!(harness.value, original);
        harness.click(color_original_id(harness.field));
        assert_eq!(harness.value, original);
        assert!(is_color_picker_open(&harness.ctx, harness.field));
    }

    #[test]
    fn the_close_button_closes_without_reverting() {
        let mut harness = Harness::new(ColorProperty::ldr(Color::from_srgb8([20, 140, 220, 255])));
        harness.open();
        harness.frame(Vec::new());
        harness.click(color_close_id(harness.field));
        assert!(!is_color_picker_open(&harness.ctx, harness.field));
    }

    #[test]
    fn escape_reverts_to_the_color_captured_on_open() {
        let original = ColorProperty::ldr(Color::from_srgb8([20, 140, 220, 255]));
        let mut harness = Harness::new(original);
        harness.open();
        harness.frame(Vec::new());
        let rect = harness
            .ctx
            .read_response(color_wheel_id(harness.field))
            .unwrap()
            .rect;
        harness.click_at(rect.center() + vec2(30.0, -30.0));
        assert_ne!(harness.value, original);
        harness.frame(vec![key(Key::Escape)]);
        assert_eq!(harness.value, original);
        assert!(!is_color_picker_open(&harness.ctx, harness.field));
    }

    #[test]
    fn the_group_menu_offers_create_rename_and_delete() {
        let mut harness = Harness::new(ColorProperty::ldr(Color::from_srgb8([20, 140, 220, 255])));
        harness.open();
        harness.frame(Vec::new());
        harness.click(color_swatch_group_id(harness.field));
        let output = harness.frame(Vec::new());
        let texts = painted_texts(&output);
        assert!(texts.iter().any(|text| text == "New Group..."), "{texts:?}");
        assert!(
            texts.iter().any(|text| text == "Rename Group..."),
            "{texts:?}"
        );
        assert!(texts.iter().any(|text| text == "Delete Group"), "{texts:?}");
    }

    #[test]
    fn a_new_group_can_be_named_from_the_picker() {
        let mut harness = Harness::new(ColorProperty::ldr(Color::from_srgb8([20, 140, 220, 255])));
        harness.open();
        harness.frame(Vec::new());
        harness.click(color_swatch_group_id(harness.field));
        harness.frame(Vec::new());
        // `Window::auto_sized` settles its width/height on the pass after the
        // menu first appears; click the stable rect, not the first-pass one.
        harness.frame(Vec::new());
        harness.click(color_swatch_group_new_id(harness.field));
        harness.frame(Vec::new());
        let state = harness
            .ctx
            .data(|data| data.get_temp::<PickerState>(harness.field.with("picker-state")))
            .unwrap();
        assert!(state.group_edit.is_some(), "new group editor did not open");
        harness.click(color_group_name_id(harness.field));
        harness.click(color_group_name_id(harness.field));
        harness.frame(Vec::new());
        harness.frame(vec![egui::Event::Text("Warm".to_owned())]);
        harness.frame(vec![key(Key::Enter)]);
        assert!(
            harness
                .swatches
                .groups
                .iter()
                .any(|group| group.name == "Warm"),
            "{:?}",
            harness.swatches.groups
        );
        assert!(is_color_picker_open(&harness.ctx, harness.field));
    }

    #[test]
    fn the_rename_action_opens_the_group_editor() {
        let mut harness = Harness::new(ColorProperty::ldr(Color::from_srgb8([20, 140, 220, 255])));
        harness.open();
        for _ in 0..3 {
            harness.frame(Vec::new());
        }
        harness.click(color_swatch_group_id(harness.field));
        harness.frame(Vec::new());
        harness.frame(Vec::new());
        harness.click(color_swatch_group_rename_id(harness.field));

        let state = harness
            .ctx
            .data(|data| data.get_temp::<PickerState>(harness.field.with("picker-state")))
            .unwrap();
        assert_eq!(
            state.group_edit.as_ref().map(|edit| edit.kind),
            Some(GroupEditKind::Rename),
            "the Rename Group action should open its editor"
        );
        assert!(
            is_color_picker_open(&harness.ctx, harness.field),
            "working inside the group menu must not dismiss the picker"
        );
    }

    #[test]
    fn reopening_the_picker_drops_a_stale_group_menu() {
        let mut harness = Harness::new(ColorProperty::ldr(Color::from_srgb8([20, 140, 220, 255])));
        harness.open();
        for _ in 0..3 {
            harness.frame(Vec::new());
        }
        harness.click(color_swatch_group_id(harness.field));
        harness.frame(Vec::new());
        harness.frame(Vec::new());
        harness.click(color_close_id(harness.field));
        assert!(!is_color_picker_open(&harness.ctx, harness.field));

        harness.open();
        let output = harness.frame(Vec::new());
        let state = harness
            .ctx
            .data(|data| data.get_temp::<PickerState>(harness.field.with("picker-state")))
            .unwrap();
        assert!(
            !state.group_menu_open,
            "a closed menu must not return with the next opening"
        );
        assert!(
            !painted_texts(&output)
                .iter()
                .any(|text| text == "New Group..."),
            "the old menu should not be painted after reopening"
        );
    }

    #[test]
    fn escape_closes_the_group_menu_before_the_picker() {
        let mut harness = Harness::new(ColorProperty::ldr(Color::from_srgb8([20, 140, 220, 255])));
        harness.open();
        for _ in 0..3 {
            harness.frame(Vec::new());
        }
        harness.click(color_swatch_group_id(harness.field));
        harness.frame(Vec::new());
        harness.frame(Vec::new());
        harness.frame(vec![key(Key::Escape)]);

        let state = harness
            .ctx
            .data(|data| data.get_temp::<PickerState>(harness.field.with("picker-state")))
            .unwrap();
        assert!(!state.group_menu_open, "Escape should dismiss the menu");
        assert!(
            is_color_picker_open(&harness.ctx, harness.field),
            "the first Escape belongs to the menu, not the picker"
        );
    }

    #[test]
    fn the_group_editor_floats_without_moving_the_swatches() {
        let mut harness = Harness::new(ColorProperty::ldr(Color::from_srgb8([20, 140, 220, 255])));
        harness.open();
        for _ in 0..3 {
            harness.frame(Vec::new());
        }
        let before = harness
            .ctx
            .read_response(color_new_swatch_id(harness.field))
            .expect("new swatch tile")
            .rect;
        harness.click(color_swatch_group_id(harness.field));
        harness.frame(Vec::new());
        harness.frame(Vec::new());
        harness.click(color_swatch_group_rename_id(harness.field));
        harness.frame(Vec::new());

        let after = harness
            .ctx
            .read_response(color_new_swatch_id(harness.field))
            .expect("new swatch tile")
            .rect;
        assert_eq!(
            before, after,
            "the rename dialog must float over the swatches, not push them"
        );
    }

    #[test]
    fn the_delete_action_removes_the_active_group() {
        let mut harness = Harness::new(ColorProperty::ldr(Color::from_srgb8([20, 140, 220, 255])));
        harness.swatches.create_group("Warm");
        harness.open();
        for _ in 0..3 {
            harness.frame(Vec::new());
        }
        harness.click(color_swatch_group_id(harness.field));
        harness.frame(Vec::new());
        harness.frame(Vec::new());
        let before = harness.swatches.groups.len();
        harness.click(color_swatch_group_delete_id(harness.field));

        assert_eq!(harness.swatches.groups.len(), before - 1);
        assert!(
            harness
                .swatches
                .groups
                .iter()
                .all(|group| group.name != "Warm"),
            "the active group should be the one removed: {:?}",
            harness.swatches.groups
        );
        assert!(
            is_color_picker_open(&harness.ctx, harness.field),
            "Delete Group must not dismiss the picker"
        );
    }

    #[test]
    fn the_new_swatch_tile_saves_the_current_color() {
        let mut harness = Harness::new(ColorProperty::ldr(Color::from_srgb8([200, 100, 50, 255])));
        harness.open();
        harness.frame(Vec::new());
        let before = harness.swatches.active_group().colors.len();
        harness.click(color_new_swatch_id(harness.field));
        assert_eq!(harness.swatches.active_group().colors.len(), before + 1);
        assert_eq!(
            harness.swatches.active_group().colors.last(),
            Some(&harness.value.color)
        );
    }

    #[test]
    fn swatch_groups_round_trip_through_library() {
        let mut book = ColorSwatches::default();
        assert!(book.create_group("Warm"));
        assert!(book.add_color(Color::from_srgb8([255, 80, 20, 255])));
        assert!(book.rename_group(book.active_index(), "Warm & Bright"));
        assert!(book.delete_group(0));
        assert_eq!(book.groups.len(), 1);
        assert_eq!(book.active_group().name, "Warm & Bright");

        let path = std::env::temp_dir().join(format!(
            "kairos-color-swatches-{}-{}.ron",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        book.set_path(&path);
        book.save_if_dirty().expect("save swatches");
        let loaded = ColorSwatches::load(&path).expect("load swatches");
        assert_eq!(loaded.groups, book.groups);
        assert_eq!(loaded.active, book.active);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn the_eyedropper_icon_is_a_runtime_asset() {
        let icon = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../")
            .join(ColorPickerStyle::dark().eyedropper_icon);
        assert!(
            icon.exists(),
            "the picker icon should live under Preferences/Textures: {}",
            icon.display()
        );
    }
}
