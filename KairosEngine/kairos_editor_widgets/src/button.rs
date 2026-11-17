//! A themed, immediately-mode text button.
//!
//! The editor chrome has a few button shapes that egui's default `Button` cannot
//! express without per-call style patching: Unity-style menu bar items (square,
//! bar-height hover fill), Console toolbar segments (fixed width, optional caret,
//! toggle state), and image-only buttons (the engine logo, the Profiler's record
//! toggle). [`Button`] takes one reusable [`ButtonStyle`] and paints all of them;
//! [`Button::icon_only`] is the constructor for a button that is nothing but an
//! icon.
//!
//! The widget is pure appearance: it returns an `egui::Response` and never emits
//! a domain action. Components map `clicked()` to their own `Action`.

use egui::{
    Color32, CornerRadius, Id, Response, Sense, Ui, Vec2, Widget, WidgetInfo, WidgetType, vec2,
};

use crate::text::TextStyle;

/// Horizontal alignment of the button's content.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonAlign {
    /// Text starts after the left padding (toolbar labels, Console "Clear").
    #[default]
    Left,
    /// Text is centred in the button (Console "Collapse" / "Error Pause").
    Center,
}

/// Visual tokens for a button. Components build one from their serializable
/// `XxxStyle` and reuse it for every button of the same kind.
#[derive(Clone, Debug)]
pub struct ButtonStyle {
    pub text: TextStyle,
    /// Background when idle. Usually transparent; the reveal comes from fills.
    pub fill: Color32,
    /// Background under the pointer.
    pub hover_fill: Color32,
    /// Background while pressed, open, or selected (toggle on).
    pub active_fill: Color32,
    pub corner_radius: CornerRadius,
    /// Horizontal text padding, vertical content padding.
    pub padding: Vec2,
    /// Default height (a floor: the button also grows to fit its content).
    pub height: f32,
    /// Distance from the right edge to the centre of the optional caret.
    pub caret_inset: f32,
}

impl ButtonStyle {
    /// A transparent, square button with default padding.
    pub fn new(text: TextStyle) -> Self {
        Self {
            text,
            fill: Color32::TRANSPARENT,
            hover_fill: Color32::TRANSPARENT,
            active_fill: Color32::TRANSPARENT,
            corner_radius: CornerRadius::ZERO,
            padding: vec2(8.0, 2.0),
            height: 20.0,
            caret_inset: 10.0,
        }
    }

    pub fn fills(mut self, fill: Color32, hover_fill: Color32, active_fill: Color32) -> Self {
        self.fill = fill;
        self.hover_fill = hover_fill;
        self.active_fill = active_fill;
        self
    }

    pub fn corner_radius(mut self, corner_radius: impl Into<CornerRadius>) -> Self {
        self.corner_radius = corner_radius.into();
        self
    }

    pub fn padding(mut self, padding: Vec2) -> Self {
        self.padding = padding;
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    pub fn caret_inset(mut self, caret_inset: f32) -> Self {
        self.caret_inset = caret_inset;
        self
    }
}

struct Icon<'a> {
    path: &'a str,
    size: f32,
}

/// One text (and/or image) button.
pub struct Button<'a> {
    text: &'a str,
    style: &'a ButtonStyle,
    width: Option<f32>,
    height: Option<f32>,
    padding: Option<Vec2>,
    align: ButtonAlign,
    icon: Option<Icon<'a>>,
    /// `true` for [`Button::icon_only`]: a label-less button sized to its icon.
    icon_only: bool,
    caret: bool,
    caret_inset: Option<f32>,
    /// `Some(true)` paints the active fill and marks the widget as selected,
    /// which is how toggles and open menu buttons report their state.
    selected: Option<bool>,
    enabled: bool,
    id: Option<Id>,
    tooltip: Option<&'a str>,
}

impl<'a> Button<'a> {
    pub fn new(text: &'a str, style: &'a ButtonStyle) -> Self {
        Self {
            text,
            style,
            width: None,
            height: None,
            padding: None,
            align: ButtonAlign::Left,
            icon: None,
            icon_only: false,
            caret: false,
            caret_inset: None,
            selected: None,
            enabled: true,
            id: None,
            tooltip: None,
        }
    }

    /// An image-only button: no label, centred icon, square by default.
    ///
    /// The rect is sized by the icon, not by the style padding: its height is
    /// at least `size` plus the style's vertical padding, and without an
    /// explicit [`Button::width`] it is a square of that height — Unity's
    /// toolbar turns a 16 px icon into a 20 × 20 toggle exactly this way. A
    /// larger icon makes the button larger instead of overhanging the fill.
    ///
    /// Give it a [`Button::tooltip`]: the tooltip doubles as the accessible
    /// label, which is the only name an icon-only button has.
    pub fn icon_only(icon: &'a str, size: f32, style: &'a ButtonStyle) -> Self {
        let mut button = Self::new("", style)
            .icon(icon, size)
            .align(ButtonAlign::Center);
        button.icon_only = true;
        button
    }

    /// Pin the width (Console segments are measured; default is text width).
    ///
    /// For an icon-only button the pinned width wins over the square default,
    /// so a wide segment with a centred icon is just `width(..)`.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Set a minimum height (defaults to the style height).
    ///
    /// The final height is this value or the content height, whichever is
    /// larger: an icon never overhangs the button it was added to.
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    pub fn align(mut self, align: ButtonAlign) -> Self {
        self.align = align;
        self
    }

    /// Override the style's padding for this button (the Console's split
    /// buttons have different left insets).
    pub fn padding(mut self, padding: Vec2) -> Self {
        self.padding = Some(padding);
        self
    }

    /// Paint a leading runtime image (a file under `Preferences/Textures`).
    pub fn icon(mut self, path: &'a str, size: f32) -> Self {
        self.icon = Some(Icon { path, size });
        self
    }

    /// Paint the dropdown caret at the right edge.
    pub fn caret(mut self) -> Self {
        self.caret = true;
        self
    }

    pub fn caret_inset(mut self, inset: f32) -> Self {
        self.caret_inset = Some(inset);
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = Some(selected);
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Stable id, for controls whose position is fixed and whose state must
    /// survive egui's multi-pass layout (the Console toolbar).
    pub fn id(mut self, id: Id) -> Self {
        self.id = Some(id);
        self
    }

    pub fn tooltip(mut self, tooltip: &'a str) -> Self {
        self.tooltip = Some(tooltip);
        self
    }
}

impl Widget for Button<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let enabled = self.enabled && ui.is_enabled();
        let padding = self.padding.unwrap_or(self.style.padding);
        let color = if enabled {
            self.style.text.text_color()
        } else {
            self.style.text.text_color().gamma_multiply(0.5)
        };

        // An empty label lays out no text at all: an icon-only button must not
        // reserve a text line, and must not depend on the text style's size.
        let has_text = !self.text.is_empty();
        let galley = has_text.then(|| {
            ui.painter().layout_no_wrap(
                self.text.to_owned(),
                self.style.text.font_id(ui.style()),
                color,
            )
        });
        let text_size = galley.as_ref().map_or(Vec2::ZERO, |galley| galley.size());
        let icon_size = self.icon.as_ref().map_or(0.0, |icon| icon.size);
        let icon_gap = if has_text && self.icon.is_some() {
            ICON_GAP
        } else {
            0.0
        };
        let content_width = icon_size + icon_gap + text_size.x;
        let content_height = icon_size.max(text_size.y);
        // The rect always covers the content: a bigger icon grows the button
        // instead of overhanging its fill. The style height (or an explicit
        // one) is a floor, not a cap.
        let height = self
            .height
            .unwrap_or(self.style.height)
            .max(content_height + 2.0 * padding.y);
        let width = self.width.unwrap_or(if self.icon_only {
            // Unity's toolbar toggles are square: the icon plus the vertical
            // padding is both the width and the height (16 + 2 + 2 = 20 px).
            height
        } else {
            content_width + 2.0 * padding.x
        });

        let (auto_id, rect) = ui.allocate_space(vec2(width, height));
        let id = self.id.unwrap_or(auto_id);
        let sense = if enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let response = ui.interact(rect, id, sense);

        let fill = if !enabled {
            Color32::TRANSPARENT
        } else if self.selected == Some(true) || response.is_pointer_button_down_on() {
            self.style.active_fill
        } else if response.hovered() {
            self.style.hover_fill
        } else {
            self.style.fill
        };
        if fill.a() > 0 {
            ui.painter()
                .rect_filled(rect, self.style.corner_radius, fill);
        }

        let content_left = match self.align {
            ButtonAlign::Left => rect.left() + padding.x,
            ButtonAlign::Center => rect.center().x - content_width * 0.5,
        };
        if let Some(icon) = &self.icon {
            let icon_rect = egui::Rect::from_center_size(
                egui::pos2(content_left + icon.size * 0.5, rect.center().y),
                Vec2::splat(icon.size),
            );
            // The artwork is pre-coloured for its skin, so only the tint can
            // express "disabled" for an icon-only button.
            let tint = if enabled {
                Color32::WHITE
            } else {
                Color32::WHITE.gamma_multiply(0.5)
            };
            crate::ui_image::paint(ui, icon_rect, icon.path, tint);
        }
        if let Some(galley) = galley {
            let text_pos = egui::pos2(
                content_left + icon_size + icon_gap,
                rect.center().y - galley.size().y * 0.5,
            );
            ui.painter().galley(text_pos, galley, color);
        }

        if self.caret {
            let inset = self.caret_inset.unwrap_or(self.style.caret_inset);
            crate::paint::caret(
                ui.painter(),
                egui::pos2(rect.right() - inset, rect.center().y),
                color,
            );
        }

        // Image-only buttons have no visible text; their tooltip is the name
        // screen readers and label queries should report.
        let label = if self.text.is_empty() {
            self.tooltip.unwrap_or(self.text)
        } else {
            self.text
        };
        response.widget_info(|| match self.selected {
            Some(selected) => {
                WidgetInfo::selected(WidgetType::SelectableLabel, enabled, selected, label)
            }
            None => WidgetInfo::labeled(WidgetType::Button, enabled, label),
        });

        match self.tooltip {
            Some(tooltip) => response.on_hover_text(tooltip),
            None => response,
        }
    }
}

const ICON_GAP: f32 = 6.0;

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Context, Event, Pos2, RawInput, Rect};

    fn input() -> RawInput {
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(240.0, 80.0))),
            ..Default::default()
        }
    }

    fn style() -> ButtonStyle {
        ButtonStyle::new(TextStyle::sized(Color32::from_rgb(200, 200, 200), 12.0))
            .fills(
                Color32::TRANSPARENT,
                Color32::from_rgb(70, 70, 70),
                Color32::from_rgb(50, 50, 50),
            )
            .height(23.0)
    }

    fn rects(output: &egui::FullOutput) -> Vec<egui::Rect> {
        let mut rects = Vec::new();
        for clipped in &output.shapes {
            if let egui::Shape::Rect(rect) = &clipped.shape {
                rects.push(rect.rect);
            }
        }
        rects
    }

    #[test]
    fn hover_paints_the_style_hover_fill() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-button-hover");

        let _ = ctx.run_ui(input(), |ui| {
            ui.add(Button::new("Edit", &style).id(id));
        });
        let rect = ctx
            .read_response(id)
            .expect("button response from the first pass")
            .rect;

        let mut hover = input();
        hover.events = vec![Event::PointerMoved(rect.center())];
        let output = ctx.run_ui(hover, |ui| {
            ui.add(Button::new("Edit", &style).id(id));
        });

        let painted = rects(&output);
        assert!(
            painted.contains(&rect),
            "hover fill should cover the button rect {rect:?}, painted {painted:?}"
        );
    }

    #[test]
    fn selected_buttons_use_the_active_fill() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-button-selected");
        let output = ctx.run_ui(input(), |ui| {
            ui.add(Button::new("Collapse", &style).id(id).selected(true));
        });
        let rect = ctx.read_response(id).expect("button response").rect;
        assert!(
            rects(&output).contains(&rect),
            "a selected toggle paints its active fill over {rect:?}"
        );
    }

    /// Draw one icon-only button and read back its rect.
    fn measured(style: &ButtonStyle, icon_size: f32, height: Option<f32>) -> Rect {
        let ctx = Context::default();
        let id = Id::new("kairos-icon-button");
        let _ = ctx.run_ui(input(), |ui| {
            let button = Button::icon_only("icon.png", icon_size, style).id(id);
            ui.add(match height {
                Some(height) => button.height(height),
                None => button,
            });
        });
        ctx.read_response(id).expect("button response").rect
    }

    #[test]
    fn icon_only_buttons_are_square_and_follow_the_icon_size() {
        let style = style();

        // A small icon sits inside the style height: Unity's 16 px toolbar icon
        // in a 20 px row is a 20 × 20 button, independent of the text padding.
        let small = measured(&style, 16.0, None);
        assert_eq!(small.size(), vec2(23.0, 23.0));

        // A larger icon grows the button instead of overhanging the fill.
        let large = measured(&style, 30.0, None);
        assert_eq!(large.size(), vec2(34.0, 34.0));
    }

    #[test]
    fn a_pinned_height_still_covers_a_larger_icon() {
        let style = style();
        let rect = measured(&style, 30.0, Some(23.0));
        assert_eq!(rect.height(), 34.0);
        assert_eq!(rect.width(), rect.height());
    }

    #[test]
    fn fixed_width_and_centre_alignment_keep_the_requested_rect() {
        let ctx = Context::default();
        let style = style();
        let id = Id::new("kairos-button-sized");
        let _ = ctx.run_ui(input(), |ui| {
            ui.add(
                Button::new("Collapse", &style)
                    .width(58.0)
                    .height(20.0)
                    .align(ButtonAlign::Center)
                    .id(id),
            );
        });
        let rect = ctx.read_response(id).expect("button response").rect;
        assert_eq!(rect.width(), 58.0);
        assert_eq!(rect.height(), 20.0);
    }
}
