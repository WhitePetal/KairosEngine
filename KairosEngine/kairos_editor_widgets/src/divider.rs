//! Chrome rules: the 1 px lines that separate toolbar segments and sit under a
//! toolbar row.
//!
//! egui's [`egui::Separator`] takes its colour and thickness from the widget
//! style and always spans the available space; Unity-style chrome needs an
//! explicit colour, an exact thickness and — for vertical rules inside a
//! toolbar — an explicit length that matches the row (there "available height"
//! is the rest of the panel, not the row). [`Divider`] is that primitive. It
//! only paints; the returned [`Response`] exists so callers can anchor a
//! tooltip on it or assert its rect in tests.

use egui::{Color32, CornerRadius, Rect, Response, Sense, Ui, Widget, pos2, vec2};

/// Direction of a [`Divider`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DividerOrientation {
    /// Runs left to right; separates rows stacked top to bottom.
    #[default]
    Horizontal,
    /// Runs top to bottom; separates items laid out left to right.
    Vertical,
}

/// A themed rule between two pieces of chrome (Unity calls it a separator).
///
/// Horizontal rules default to the available width, vertical ones to the
/// available height; both can be given an explicit [`length`](Self::length).
#[derive(Clone, Debug)]
pub struct Divider {
    orientation: DividerOrientation,
    thickness: f32,
    length: Option<f32>,
    inset: f32,
    color: Option<Color32>,
}

impl Divider {
    /// A horizontal rule spanning the available width (the line under a toolbar).
    pub fn horizontal() -> Self {
        Self::new(DividerOrientation::Horizontal)
    }

    /// A vertical rule.
    ///
    /// Inside a horizontal row give it the row's height with
    /// [`length`](Self::length): egui lays a row out against its initial
    /// height (`interact_size.y`), not the height its widgets end up needing.
    pub fn vertical() -> Self {
        Self::new(DividerOrientation::Vertical)
    }

    fn new(orientation: DividerOrientation) -> Self {
        Self {
            orientation,
            thickness: 1.0,
            length: None,
            inset: 0.0,
            color: None,
        }
    }

    /// Line thickness in points (default 1 px, Unity's chrome line).
    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = thickness;
        self
    }

    /// Length along the rule; the default covers the available space in that
    /// direction.
    pub fn length(mut self, length: f32) -> Self {
        self.length = Some(length);
        self
    }

    /// Trim both ends of the rule by this many points, e.g. to keep it inside a
    /// row that pads its content. Clamped to half the length.
    pub fn inset(mut self, inset: f32) -> Self {
        self.inset = inset;
        self
    }

    /// Line colour; defaults to the theme's non-interactive stroke colour (the
    /// same one `egui::Separator` uses). Components pass their own style token.
    pub fn color(mut self, color: Color32) -> Self {
        self.color = Some(color);
        self
    }
}

impl Divider {
    /// Paint the rule into an existing strip, without allocating space.
    ///
    /// The [`Widget`] impl allocates a strip and calls this; chrome that lays
    /// itself out (Unity's measured module and timeline columns) calls it
    /// directly with the rect it already computed.
    pub fn paint_in(&self, ui: &Ui, strip: Rect) {
        let color = self
            .color
            .unwrap_or(ui.visuals().widgets.noninteractive.bg_stroke.color);
        let line = self.line_rect(strip);
        if ui.is_rect_visible(line) {
            ui.painter().rect_filled(line, CornerRadius::ZERO, color);
        }
    }

    /// The painted line inside `strip`: `inset` trims both ends.
    fn line_rect(&self, strip: Rect) -> Rect {
        let length = match self.orientation {
            DividerOrientation::Horizontal => strip.width(),
            DividerOrientation::Vertical => strip.height(),
        };
        let inset = self.inset.clamp(0.0, length * 0.5);
        match self.orientation {
            DividerOrientation::Horizontal => Rect::from_min_max(
                pos2(strip.left() + inset, strip.top()),
                pos2(strip.right() - inset, strip.bottom()),
            ),
            DividerOrientation::Vertical => Rect::from_min_max(
                pos2(strip.left(), strip.top() + inset),
                pos2(strip.right(), strip.bottom() - inset),
            ),
        }
    }
}

impl Widget for Divider {
    fn ui(self, ui: &mut Ui) -> Response {
        let length = self.length.unwrap_or_else(|| match self.orientation {
            DividerOrientation::Horizontal => ui.available_width(),
            DividerOrientation::Vertical => ui.available_height(),
        });
        let size = match self.orientation {
            DividerOrientation::Horizontal => vec2(length, self.thickness),
            DividerOrientation::Vertical => vec2(self.thickness, length),
        };

        let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
        self.paint_in(ui, rect);
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Context, Pos2, RawInput};

    fn input() -> RawInput {
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(240.0, 80.0))),
            ..Default::default()
        }
    }

    /// Every filled rect the pass painted, as `(rect, fill)`.
    fn painted(output: &egui::FullOutput) -> Vec<(Rect, Color32)> {
        output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Rect(shape) => Some((shape.rect, shape.fill)),
                _ => None,
            })
            .collect()
    }

    const LINE: Color32 = Color32::from_rgb(0x25, 0x25, 0x25);

    #[test]
    fn a_horizontal_divider_spans_the_available_width() {
        let ctx = Context::default();
        let mut rect = Rect::NOTHING;
        let mut available = 0.0;
        let output = ctx.run_ui(input(), |ui| {
            available = ui.available_width();
            rect = ui
                .add(Divider::horizontal().thickness(2.0).color(LINE))
                .rect;
        });

        assert_eq!(rect.width(), available);
        assert_eq!(rect.height(), 2.0);
        assert_eq!(painted(&output), vec![(rect, LINE)]);
    }

    #[test]
    fn a_vertical_divider_uses_its_explicit_length() {
        let ctx = Context::default();
        let mut rect = Rect::NOTHING;
        let output = ctx.run_ui(input(), |ui| {
            ui.horizontal(|ui| {
                rect = ui.add(Divider::vertical().length(12.0).color(LINE)).rect;
            });
        });

        assert_eq!(rect.size(), vec2(1.0, 12.0));
        assert_eq!(painted(&output), vec![(rect, LINE)]);
    }

    #[test]
    fn inset_trims_both_ends_without_moving_the_strip() {
        let ctx = Context::default();
        let mut strip = Rect::NOTHING;
        let output = ctx.run_ui(input(), |ui| {
            strip = ui
                .add(Divider::horizontal().length(100.0).inset(4.0).color(LINE))
                .rect;
        });

        let expected = Rect::from_min_max(
            pos2(strip.left() + 4.0, strip.top()),
            pos2(strip.right() - 4.0, strip.bottom()),
        );
        assert_eq!(strip.width(), 100.0);
        assert_eq!(painted(&output), vec![(expected, LINE)]);
    }

    #[test]
    fn paint_in_draws_into_a_measured_strip() {
        let ctx = Context::default();
        let strip = Rect::from_min_size(egui::pos2(10.0, 12.0), vec2(1.0, 20.0));
        let output = ctx.run_ui(input(), |ui| {
            Divider::vertical()
                .thickness(1.0)
                .color(LINE)
                .paint_in(ui, strip);
        });

        assert_eq!(painted(&output), vec![(strip, LINE)]);
    }

    #[test]
    fn the_default_colour_comes_from_the_theme() {
        let ctx = Context::default();
        let mut theme_line = Color32::BLACK;
        let mut rect = Rect::NOTHING;
        let output = ctx.run_ui(input(), |ui| {
            theme_line = ui.visuals().widgets.noninteractive.bg_stroke.color;
            rect = ui.add(Divider::horizontal().length(32.0)).rect;
        });

        assert_eq!(painted(&output), vec![(rect, theme_line)]);
    }
}
