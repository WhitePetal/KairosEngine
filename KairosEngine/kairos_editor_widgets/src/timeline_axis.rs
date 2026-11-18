//! A compressed time axis and its ruler.
//!
//! A timeline is rarely one continuous stretch of time: a profiler capture can
//! be stopped and started, and an animation can leave gaps. [TimelineAxis] lays
//! such segments end to end and compresses every gap between them into a
//! fixed-width break band. The mapping is affine inside a segment and jumps by
//! one band at each boundary:
//!
//!     content(time) = time * zoom + breaks_before(time) * break_px
//!     screen(time)  = content(time) - content(view_left)
//!
//! Everything here is pure layout: the caller owns the view state in the axis
//! and paints what it reports ([VisibleSegment], [AxisBreak], [AxisTick]).
//! [TimelineRuler] paints the ruler itself: the ticks, the break bands and the
//! labels printed on a band's two sides.
//!
//! The axis knows no clock. Its unit is whatever the caller feeds it —
//! nanoseconds for the Profiler panel, seconds for an animation timeline — and
//! [RulerScale] decides where labelled ticks fall while [TimeLabels] formats
//! them. A continuous timeline is simply one segment covering its whole range.

use egui::{Color32, CornerRadius, FontId, Rect, Ui, pos2, vec2};

use crate::min_max_scroll::ScrollWindow;

/// Height of a major tick, as a fraction of the ruler's height.
const MAJOR_TICK_FRACTION: f32 = 0.4;
/// Height of a minor tick, as a fraction of the ruler's height.
const MINOR_TICK_FRACTION: f32 = 0.25;
/// Gap between a tick or break band and its label, in points.
const LABEL_PADDING: f32 = 3.0;

/// Sizing rules for a [TimelineAxis], in the caller's time units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AxisConfig {
    /// The narrowest visible time span the axis normally zooms to.
    pub min_span: f64,
    /// The smallest time unit the axis will show. A timeline shorter than eight
    /// times [min_span](Self::min_span) zooms down to this instead, so a brief
    /// timeline still zooms instead of being stuck fitting end to end.
    pub resolution: f64,
    /// Width of one break band, in points.
    pub break_width: f32,
    /// The most of the chart columns all break bands may take together, as a
    /// fraction, so a timeline with very many segments cannot squeeze the data
    /// out of view.
    pub break_budget: f32,
}

impl Default for AxisConfig {
    fn default() -> Self {
        Self {
            min_span: 1.0,
            resolution: 1e-6,
            break_width: 14.0,
            break_budget: 0.4,
        }
    }
}

/// One continuous stretch on the caller's clock. The gaps between segments are
/// the compressed pauses the axis draws as break bands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimeSegment {
    /// Start on the caller's clock.
    pub start: f64,
    /// End on the caller's clock.
    pub end: f64,
}

/// The slice of one segment the view shows, and where it lands on screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VisibleSegment {
    /// Index into the axis' segments.
    pub index: usize,
    /// The visible slice on the axis' own clock: the segments laid end to end,
    /// with every pause removed.
    pub time: (f64, f64),
    /// The same slice on the caller's clock.
    pub source: (f64, f64),
    /// Screen columns: x.0 is the left edge, x.1 the right edge.
    pub x: (f32, f32),
}

/// A break band between two segments, where a pause is compressed away.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AxisBreak {
    /// Screen x of the band's left edge.
    pub x: f32,
    /// Band width in points; the data columns stop at x and resume at
    /// x + width.
    pub width: f32,
    /// The caller's clock time at the stop.
    pub stop: f64,
    /// The caller's clock time at the restart.
    pub restart: f64,
}

/// One tick on a ruler.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AxisTick {
    /// Screen x in the ruler's columns.
    pub x: f32,
    /// The caller-clock time under the tick, before the ruler's origin is added
    /// for the label.
    pub source: f64,
    /// Interval between two labelled ticks, in label units.
    pub step: f64,
    /// Whether this is a labelled tick (false for the four minors between).
    pub major: bool,
}

/// Where a ruler's labelled ticks fall and what its values mean.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RulerScale<'a> {
    /// Candidate intervals between labelled ticks, in label units, finest
    /// first. The ruler picks the finest one whose labels still fit.
    pub steps: &'a [f64],
    /// Smallest distance between two labelled ticks, in points.
    pub min_spacing: f32,
    /// Added to a caller-clock time to get the value a label reads (for
    /// example the capture's wall clock at its origin, or zero for elapsed
    /// time).
    pub origin: f64,
}

impl<'a> RulerScale<'a> {
    /// A scale over steps with labels at least min_spacing apart.
    pub fn new(steps: &'a [f64], min_spacing: f32, origin: f64) -> Self {
        Self {
            steps,
            min_spacing,
            origin,
        }
    }
}

/// How a [TimelineRuler] prints its values.
pub trait TimeLabels {
    /// Label of a major tick at time (label units), with step between two
    /// labelled ticks.
    fn tick(&self, time: f64, step: f64) -> String;

    /// The two sides of a break band: the time of the stop on the left and of
    /// the restart on the right, in label units.
    fn break_band(&self, stop: f64, restart: f64) -> (String, String);
}

/// A timeline's compressed time axis: the segments, the view window and the
/// mapping between the caller's clock, the axis' own clock and screen columns.
#[derive(Clone, Debug, Default)]
pub struct TimelineAxis {
    config: AxisConfig,
    segments: Vec<TimeSegment>,
    /// prefix[i] is the axis time at which segment i starts.
    prefix: Vec<f64>,
    /// Total axis time across every segment.
    duration: f64,
    /// Width of the chart columns, in points.
    width: f32,
    /// Width of one break band, in points (0 with a single segment).
    break_px: f32,
    /// Content points per axis time unit.
    zoom: f64,
    /// Axis time at the view's left edge.
    left: f64,
}

impl TimelineAxis {
    /// An empty axis with config.
    pub fn new(config: AxisConfig) -> Self {
        Self {
            config,
            ..Self::default()
        }
    }

    /// Replace the segments, keeping the view (which layout re-clamps).
    pub fn set_segments(&mut self, segments: Vec<TimeSegment>) {
        self.segments = segments;
        self.prefix.clear();
        let mut prefix = 0.0;
        for segment in &self.segments {
            self.prefix.push(prefix);
            prefix += (segment.end - segment.start).max(0.0);
        }
        self.duration = prefix;
        if self.duration <= 0.0 {
            self.zoom = 0.0;
            self.left = 0.0;
        } else if self.zoom > 0.0 {
            self.clamp_view();
        }
    }

    /// Size the axis to the chart columns, then re-clamp the view.
    pub fn layout(&mut self, width: f32) {
        self.width = width.max(1.0);
        self.break_px = self.break_width(self.segments.len().saturating_sub(1));
        if self.zoom > 0.0 {
            self.clamp_view();
        }
    }

    /// Whether the axis has data to map.
    pub fn is_empty(&self) -> bool {
        self.duration <= 0.0 || self.zoom <= 0.0 || self.segments.is_empty()
    }

    /// Content points per axis time unit.
    pub fn zoom(&self) -> f64 {
        self.zoom
    }

    /// Axis time at the view's left edge.
    pub fn left(&self) -> f64 {
        self.left
    }

    /// Total axis time across every segment (the pauses removed).
    pub fn duration(&self) -> f64 {
        self.duration
    }

    /// The visible axis span: the window width, less the break bands that fall
    /// inside it.
    pub fn view_span(&self) -> f64 {
        if self.zoom <= 0.0 {
            return 0.0;
        }
        let mut span = f64::from(self.width) / self.zoom;
        for _ in 0..4 {
            let breaks = self.breaks_between(self.left, self.left + span);
            let next = ((f64::from(self.width) - breaks as f64 * f64::from(self.break_px))
                / self.zoom)
                .max(0.0);
            if (next - span).abs() < 1e-9 {
                break;
            }
            span = next;
        }
        span
    }

    /// Screen x of an axis time.
    pub fn screen(&self, time: f64) -> f32 {
        (self.content(time) - self.content(self.left)) as f32
    }

    /// Screen x of a time on the caller's clock, if it falls inside a segment.
    pub fn screen_of_source(&self, source: f64) -> Option<f32> {
        let (index, time) = self.time_of_source(source)?;
        Some((self.content_in(index, time) - self.content(self.left)) as f32)
    }

    /// The segment and axis time of a time on the caller's clock.
    pub fn time_of_source(&self, source: f64) -> Option<(usize, f64)> {
        if self.segments.is_empty() {
            return None;
        }
        let index = self
            .segments
            .partition_point(|segment| segment.start <= source)
            .saturating_sub(1);
        let segment = self.segments.get(index)?;
        if source < segment.start || source > segment.end {
            return None;
        }
        Some((index, self.prefix[index] + (source - segment.start)))
    }

    /// The segment and caller-clock time behind an axis time.
    pub fn source_of_time(&self, time: f64) -> Option<(usize, f64)> {
        if self.segments.is_empty() {
            return None;
        }
        let index = self.seg_index(time);
        let start = self.prefix[index];
        Some((index, self.segments[index].start + (time - start)))
    }

    /// The caller's clock under a screen column, or None over a break band.
    pub fn source_at_screen(&self, x: f32) -> Option<(usize, f64)> {
        if self.zoom <= 0.0 || !(0.0..=self.width).contains(&x) {
            return None;
        }
        for band in self.breaks() {
            if x >= band.x && x <= band.x + band.width {
                return None;
            }
        }
        let content = self.content(self.left) + f64::from(x);
        let time = self.time_at_content(content);
        self.source_of_time(time)
    }

    /// The segments the view shows, with their screen columns.
    pub fn visible_segments(&self) -> Vec<VisibleSegment> {
        let mut visible = Vec::new();
        if self.zoom <= 0.0 || self.segments.is_empty() {
            return visible;
        }
        let span = self.view_span();
        let view = (self.left, self.left + span);
        for (index, segment) in self.segments.iter().enumerate() {
            let start = self.prefix[index];
            let end = start + (segment.end - segment.start).max(0.0);
            let lo = start.max(view.0);
            let hi = end.min(view.1);
            if hi <= lo {
                continue;
            }
            let left_content = self.content(self.left);
            visible.push(VisibleSegment {
                index,
                time: (lo, hi),
                source: (segment.start + (lo - start), segment.start + (hi - start)),
                x: (
                    (self.content_in(index, lo) - left_content) as f32,
                    (self.content_in(index, hi) - left_content) as f32,
                ),
            });
        }
        visible
    }

    /// The break bands the view shows, with their stop/restart times.
    pub fn breaks(&self) -> Vec<AxisBreak> {
        let mut breaks = Vec::new();
        if self.zoom <= 0.0 || self.segments.len() < 2 {
            return breaks;
        }
        let left_content = self.content(self.left);
        for index in 1..self.segments.len() {
            let band_left =
                self.prefix[index] * self.zoom + (index - 1) as f64 * f64::from(self.break_px);
            let x = (band_left - left_content) as f32;
            if x > self.width || x + self.break_px < 0.0 {
                continue;
            }
            breaks.push(AxisBreak {
                x,
                width: self.break_px,
                stop: self.segments[index - 1].end,
                restart: self.segments[index].start,
            });
        }
        breaks
    }

    /// Ruler ticks: labelled majors with four minors between them.
    pub fn ticks(&self, scale: &RulerScale<'_>) -> Vec<AxisTick> {
        let mut ticks = Vec::new();
        if self.zoom <= 0.0 || self.segments.is_empty() || scale.steps.is_empty() {
            return ticks;
        }
        let step = scale
            .steps
            .iter()
            .copied()
            .find(|step| step * self.zoom >= f64::from(scale.min_spacing))
            .unwrap_or(scale.steps[scale.steps.len() - 1]);
        let minor = step / 5.0;
        let origin = scale.origin;
        for visible in self.visible_segments() {
            let (source_start, source_end) = visible.source;
            let first = ((origin + source_start) / minor).ceil() as i64;
            let last = ((origin + source_end) / minor).floor() as i64;
            if last < first || last - first > 4_096 {
                continue;
            }
            for index in first..=last {
                let source = index as f64 * minor - origin;
                let time = visible.time.0 + (source - source_start);
                let x = (self.content_in(visible.index, time) - self.content(self.left)) as f32;
                if x < -1.0 || x > self.width + 1.0 {
                    continue;
                }
                ticks.push(AxisTick {
                    x,
                    source,
                    step,
                    major: index.rem_euclid(5) == 0,
                });
            }
        }
        ticks
    }

    /// The min-max scroll bar's window over the axis time.
    pub fn window(&self) -> ScrollWindow {
        let span = self.view_span().clamp(0.0, self.duration);
        ScrollWindow::new(self.left, span, (0.0, self.duration))
            .with_min_span(self.min_visible_span())
    }

    /// Apply the scroll bar's answer: its value pans, its span zooms.
    pub fn apply_window(&mut self, value: f64, span: f64) {
        if self.duration <= 0.0 {
            return;
        }
        let span = span.clamp(self.min_span(), self.duration);
        let value = value.clamp(0.0, (self.duration - span).max(0.0));
        let breaks = self.breaks_between(value, value + span);
        let data_width =
            (f64::from(self.width) - breaks as f64 * f64::from(self.break_px)).max(1.0);
        self.zoom = data_width / span;
        self.left = value;
        self.clamp_view();
    }

    /// Show every segment, edge to edge.
    pub fn fit(&mut self) {
        if self.segments.is_empty() || self.duration <= 0.0 {
            return;
        }
        self.zoom = self.fit_zoom();
        self.left = 0.0;
        self.clamp_view();
    }

    /// Show the axis range from..to across the chart columns.
    pub fn show_window(&mut self, from: f64, to: f64) {
        if self.segments.is_empty() || self.duration <= 0.0 {
            return;
        }
        let from = from.clamp(0.0, self.duration);
        let to = to.clamp(from, self.duration);
        let span = (to - from).max(self.min_span());
        let breaks = self.breaks_between(from, to);
        let data_width =
            (f64::from(self.width) - breaks as f64 * f64::from(self.break_px)).max(1.0);
        self.zoom = data_width / span;
        self.left = from;
        self.clamp_view();
    }

    /// Keep the right edge on the newest data.
    pub fn follow_end(&mut self) {
        if self.zoom <= 0.0 || self.duration <= 0.0 {
            return;
        }
        let total = self.content(self.duration);
        let target = (total - f64::from(self.width)).max(0.0);
        self.left = self.time_at_content(target);
        self.clamp_view();
    }

    /// Zoom by factor around screen column x, as a wheel does.
    pub fn zoom_around(&mut self, x: f32, factor: f64) {
        if self.zoom <= 0.0 || self.duration <= 0.0 {
            return;
        }
        let x = x.clamp(0.0, self.width);
        let anchor = self.time_at_content(self.content(self.left) + f64::from(x));
        let fit = self.fit_zoom();
        let max = self.max_zoom().max(fit);
        self.zoom = (self.zoom * factor).clamp(fit, max);
        self.left = self.time_at_content(self.content(anchor) - f64::from(x));
        self.clamp_view();
    }

    /// Pan by a drag of dx points: dragging the chart right shows earlier time.
    pub fn pan_screen(&mut self, dx: f32) {
        if self.zoom <= 0.0 || self.duration <= 0.0 {
            return;
        }
        let target = self.content(self.left) - f64::from(dx);
        self.left = self.time_at_content(target.max(0.0));
        self.clamp_view();
    }

    /// Pan the view so an axis time is visible again.
    pub fn reveal(&mut self, time: f64) {
        if self.zoom <= 0.0 || self.duration <= 0.0 {
            return;
        }
        let x = self.screen(time);
        if (0.0..=self.width).contains(&x) {
            return;
        }
        let target = self.content(time) - f64::from(self.width) * 0.5;
        self.left = self.time_at_content(target.max(0.0));
        self.clamp_view();
    }

    /// Content x of an axis time: linearly inside a segment, plus one band per
    /// segment already passed.
    fn content(&self, time: f64) -> f64 {
        time * self.zoom + self.seg_index(time) as f64 * f64::from(self.break_px)
    }

    /// Content x of an axis time known to be inside segment index.
    ///
    /// The axis time at a segment's end is ambiguous with the next segment's
    /// start; painting a segment's own columns must count the bands before that
    /// segment, not after it, or the segment would end a band too far right.
    fn content_in(&self, index: usize, time: f64) -> f64 {
        time * self.zoom + index as f64 * f64::from(self.break_px)
    }

    /// Index of the segment an axis time falls in.
    fn seg_index(&self, time: f64) -> usize {
        let index = self.prefix.partition_point(|prefix| *prefix <= time);
        index
            .saturating_sub(1)
            .min(self.segments.len().saturating_sub(1))
    }

    /// Axis time at a content x; the fixed point walks past band jumps.
    fn time_at_content(&self, content: f64) -> f64 {
        if self.zoom <= 0.0 || self.duration <= 0.0 {
            return 0.0;
        }
        let mut index = self.seg_index(self.left) as f64;
        let mut time = 0.0;
        for _ in 0..8 {
            time = ((content - index * f64::from(self.break_px)) / self.zoom)
                .clamp(0.0, self.duration);
            let next = self.seg_index(time) as f64;
            if next == index {
                break;
            }
            index = next;
        }
        time
    }

    /// Number of segment boundaries strictly inside from..to.
    fn breaks_between(&self, from: f64, to: f64) -> usize {
        if to <= from {
            return 0;
        }
        let lo = self.prefix.partition_point(|prefix| *prefix <= from);
        let hi = self.prefix.partition_point(|prefix| *prefix < to);
        hi.saturating_sub(lo)
    }

    /// Content points per axis time unit at the fully zoomed-out view.
    fn fit_zoom(&self) -> f64 {
        if self.duration <= 0.0 {
            return 1.0;
        }
        let data_width = (f64::from(self.width) - self.breaks_total()).max(1.0);
        data_width / self.duration
    }

    /// Widest zoom: a window at the narrowest visible span.
    fn max_zoom(&self) -> f64 {
        f64::from(self.width) / self.min_visible_span().max(self.config.resolution)
    }

    /// Content points taken by all break bands.
    fn breaks_total(&self) -> f64 {
        self.segments.len().saturating_sub(1) as f64 * f64::from(self.break_px)
    }

    /// The narrowest axis window the axis shows: min_span normally, shrinking
    /// for a timeline shorter than eight times that.
    fn min_visible_span(&self) -> f64 {
        let by_duration = (self.duration / 8.0).max(self.config.resolution);
        self.config
            .min_span
            .min(by_duration)
            .min(self.duration.max(self.config.resolution))
    }

    /// The narrowest span a window operation may produce.
    fn min_span(&self) -> f64 {
        self.config.resolution.max(f64::MIN_POSITIVE)
    }

    /// Width of one break band for breaks pauses across the chart columns.
    fn break_width(&self, breaks: usize) -> f32 {
        if breaks == 0 {
            return 0.0;
        }
        let budget = self.width * self.config.break_budget / breaks as f32;
        self.config
            .break_width
            .clamp(2.0, self.width)
            .min(budget)
            .max(1.0)
    }

    fn clamp_view(&mut self) {
        if self.segments.is_empty() || self.duration <= 0.0 {
            self.left = 0.0;
            return;
        }
        if self.zoom <= 0.0 {
            self.zoom = self.fit_zoom();
        }
        let fit = self.fit_zoom();
        let max = self.max_zoom().max(fit);
        self.zoom = self.zoom.clamp(fit, max);
        let span = self.view_span().min(self.duration);
        let max_left = (self.duration - span).max(0.0);
        self.left = self.left.clamp(0.0, max_left);
    }
}

/// Appearance of a [TimelineRuler].
#[derive(Clone, Debug, PartialEq)]
pub struct TimelineRulerStyle {
    /// Fill behind the ruler.
    pub background: Color32,
    /// Tick marks and label colour.
    pub text: Color32,
    /// Fill of a compressed-gap band.
    pub break_background: Color32,
    /// Font of the tick and band labels.
    pub font: FontId,
}

/// The ruler over a [TimelineAxis]: ticks, compressed-gap bands and labels.
///
/// The caller lays the row out and paints its own chrome (the name column and
/// its rules); the widget fills only the row it is given and clips its labels
/// to it.
#[derive(Clone, Copy)]
pub struct TimelineRuler<'a> {
    axis: &'a TimelineAxis,
    scale: RulerScale<'a>,
    labels: &'a dyn TimeLabels,
    style: &'a TimelineRulerStyle,
}

impl<'a> TimelineRuler<'a> {
    /// A ruler for axis, with scale placing the ticks and labels printing them.
    pub fn new(
        axis: &'a TimelineAxis,
        scale: RulerScale<'a>,
        labels: &'a dyn TimeLabels,
        style: &'a TimelineRulerStyle,
    ) -> Self {
        Self {
            axis,
            scale,
            labels,
            style,
        }
    }

    /// Paint the ruler into row, which must be the chart columns the axis was
    /// laid out for.
    pub fn show(&self, ui: &Ui, row: Rect) {
        let painter = ui.painter();
        painter.rect_filled(row, CornerRadius::ZERO, self.style.background);
        if row.width() <= 0.0 || self.axis.is_empty() {
            return;
        }
        let labels = painter.with_clip_rect(row);
        let text = self.style.text;

        // Break bands print the pause's stop time on their left and its restart
        // time on their right; labels that would collide step aside.
        let mut reserved: Vec<(f32, f32)> = Vec::new();
        for band in self.axis.breaks() {
            let x = row.left() + band.x;
            let band_rect = Rect::from_min_size(pos2(x, row.top()), vec2(band.width, row.height()));
            painter.rect_filled(band_rect, CornerRadius::ZERO, self.style.break_background);
            let (stop, restart) = self.labels.break_band(
                self.scale.origin + band.stop,
                self.scale.origin + band.restart,
            );
            let stop = labels.layout_no_wrap(stop, self.style.font.clone(), text);
            let stop_x = x - LABEL_PADDING - stop.size().x;
            labels.galley(pos2(stop_x, row.top() + LABEL_PADDING), stop, text);
            reserved.push((stop_x - 1.0, x - 1.0));
            let restart = labels.layout_no_wrap(restart, self.style.font.clone(), text);
            let restart_x = x + band.width + LABEL_PADDING;
            reserved.push((restart_x, restart_x + restart.size().x + 1.0));
            labels.galley(pos2(restart_x, row.top() + LABEL_PADDING), restart, text);
        }

        for tick in self.axis.ticks(&self.scale) {
            let x = row.left() + tick.x;
            if x < row.left() - 0.5 || x > row.right() + 0.5 {
                continue;
            }
            let height = row.height()
                * if tick.major {
                    MAJOR_TICK_FRACTION
                } else {
                    MINOR_TICK_FRACTION
                };
            painter.rect_filled(
                Rect::from_min_size(pos2(x, row.bottom() - height), vec2(1.0, height)),
                CornerRadius::ZERO,
                text,
            );
            if !tick.major {
                continue;
            }
            let label = self.labels.tick(self.scale.origin + tick.source, tick.step);
            let galley = labels.layout_no_wrap(label, self.style.font.clone(), text);
            let left = x + LABEL_PADDING;
            let right = left + galley.size().x;
            if reserved.iter().any(|(a, b)| left < *b && right > *a) {
                continue;
            }
            labels.galley(pos2(left, row.top() + LABEL_PADDING), galley, text);
            reserved.push((left - 1.0, right + 1.0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: f64 = 1_000_000.0;

    fn config() -> AxisConfig {
        AxisConfig {
            min_span: 10.0 * MS,
            resolution: 1_000.0,
            ..AxisConfig::default()
        }
    }

    fn axis(segments: &[(f64, f64)], width: f32) -> TimelineAxis {
        let mut axis = TimelineAxis::new(config());
        axis.set_segments(
            segments
                .iter()
                .map(|(start, end)| TimeSegment {
                    start: *start,
                    end: *end,
                })
                .collect(),
        );
        axis.layout(width);
        axis.fit();
        axis
    }

    #[test]
    fn one_segment_fills_the_columns() {
        let axis = axis(&[(0.0, 1_000.0 * MS)], 1_000.0);
        let visible = axis.visible_segments();
        assert_eq!(visible.len(), 1);
        assert!((visible[0].x.0 - 0.0).abs() < 0.01);
        assert!((visible[0].x.1 - 1_000.0).abs() < 0.01);
        assert!((axis.screen(500.0 * MS) - 500.0).abs() < 0.01);
        assert!(axis.breaks().is_empty(), "a single segment has no boundary");
    }

    #[test]
    fn a_pause_is_one_fixed_break_band_between_the_segments() {
        // Two 500 ms segments separated by a 200 ms pause: the pause must not
        // take any time columns, only one band's worth.
        let axis = axis(&[(0.0, 500.0 * MS), (700.0 * MS, 1_200.0 * MS)], 1_000.0);
        let visible = axis.visible_segments();
        assert_eq!(visible.len(), 2);
        let breaks = axis.breaks();
        assert_eq!(breaks.len(), 1);
        assert!(
            (breaks[0].width - 14.0).abs() < 0.01,
            "a normal break band keeps its default width: {}",
            breaks[0].width
        );
        assert!(
            (visible[1].x.0 - (visible[0].x.1 + breaks[0].width)).abs() < 0.01,
            "the second segment starts right after the band: {visible:?}"
        );
        let data: f32 = visible
            .iter()
            .map(|segment| segment.x.1 - segment.x.0)
            .sum();
        assert!((data + breaks[0].width - 1_000.0).abs() < 0.01);
        assert_eq!(breaks[0].stop, 500.0 * MS);
        assert_eq!(breaks[0].restart, 700.0 * MS);
    }

    #[test]
    fn zoom_around_keeps_the_time_under_the_pointer() {
        let mut axis = axis(&[(0.0, 1_000.0 * MS), (1_200.0 * MS, 2_200.0 * MS)], 800.0);
        let x = 300.0;
        let before = axis.time_at_content(axis.content(axis.left) + f64::from(x));
        axis.zoom_around(x, 2.0);
        let after = axis.time_at_content(axis.content(axis.left) + f64::from(x));
        assert!(
            (after - before).abs() < 1.0,
            "the time under the pointer stays: {before} -> {after}"
        );
    }

    #[test]
    fn zooming_out_stops_at_the_whole_timeline() {
        let mut axis = axis(&[(0.0, 1_000.0 * MS)], 800.0);
        let fit = axis.zoom();
        axis.zoom_around(400.0, 0.01);
        assert!((axis.zoom() - fit).abs() < 1e-12, "no zoom past fit-all");
        axis.pan_screen(1_000.0);
        assert!(
            axis.left().abs() < 1e-9,
            "the view passes no earlier than 0"
        );
        axis.zoom_around(400.0, 100.0);
        axis.pan_screen(-1_000_000.0);
        let span = axis.view_span();
        assert!(
            (axis.left() + span - axis.window().bounds.1).abs() < 1.0,
            "the view stops at the newest data: {} + {span}",
            axis.left()
        );
    }

    #[test]
    fn the_scroll_window_matches_what_the_axis_shows() {
        let mut axis = axis(&[(0.0, 2_000.0 * MS)], 900.0);
        let window = axis.window();
        assert!((window.value - 0.0).abs() < 1e-9);
        assert!((window.span - window.bounds.1).abs() < 1.0);
        assert!((window.value - window.bounds.0).abs() < 1.0);

        let value = 500.0 * MS;
        let span = 400.0 * MS;
        axis.apply_window(value, span);
        assert!((axis.left() - value).abs() < 1.0);
        assert!(
            (axis.view_span() - span).abs() < 1.0,
            "the bar's span is the visible span: {} vs {span}",
            axis.view_span()
        );
    }

    #[test]
    fn ticks_land_on_multiples_of_the_step() {
        let mut axis = TimelineAxis::new(config());
        axis.set_segments(vec![TimeSegment {
            start: 0.0,
            end: 2_000.0 * MS,
        }]);
        axis.layout(800.0);
        axis.fit();

        // A 2 s timeline in 800 pt at the smallest step whose labels keep 80 pt
        // apart: 200 ms, drawn as 40 ms minors.
        let steps = [
            5.0 * MS,
            10.0 * MS,
            20.0 * MS,
            50.0 * MS,
            100.0 * MS,
            200.0 * MS,
            500.0 * MS,
        ];
        let scale = RulerScale::new(&steps, 80.0, 10.0 * 3_600.0 * 1e9);
        let ticks = axis.ticks(&scale);
        let major: Vec<AxisTick> = ticks.iter().copied().filter(|tick| tick.major).collect();
        assert!(
            major.len() >= 10,
            "the 2 s window labels several 200 ms ticks: {major:?}"
        );
        assert!(
            ticks.iter().any(|tick| !tick.major),
            "minor ticks sit between the labelled ones"
        );
        let x_of = |time: f64| {
            major
                .iter()
                .find(|tick| (tick.source - time).abs() < 1.0)
                .map(|tick| tick.x)
                .unwrap()
        };
        let step = x_of(400.0 * MS) - x_of(0.0);
        assert!(
            (step - 160.0).abs() < 0.5,
            "a 200 ms step is a fifth of the 800 pt window: {step}"
        );
        assert!(
            major
                .iter()
                .all(|tick| (tick.step - 200.0 * MS).abs() < 1.0)
        );
    }

    #[test]
    fn clicking_a_break_band_selects_no_time() {
        let axis = axis(&[(0.0, 500.0 * MS), (700.0 * MS, 1_200.0 * MS)], 1_000.0);
        let visible = axis.visible_segments();
        let band = axis.breaks()[0].x;
        assert!(axis.source_at_screen(visible[0].x.1 - 1.0).is_some());
        assert!(
            axis.source_at_screen(band + 1.0).is_none(),
            "the pause is not a segment"
        );
        let (index, source) = axis.source_at_screen(band + 30.0).unwrap();
        assert_eq!(index, 1, "right of the band is the second segment");
        assert!(source >= 700.0 * MS);
    }

    #[test]
    fn many_segments_squeeze_the_bands_instead_of_the_data() {
        let segments: Vec<(f64, f64)> = (0..12)
            .map(|index| {
                let start = index as f64 * 300.0 * MS;
                (start, start + 100.0 * MS)
            })
            .collect();
        let axis = axis(&segments, 400.0);
        let breaks = axis.breaks();
        assert_eq!(breaks.len(), 11);
        let band_total: f32 = breaks.iter().map(|band| band.width).sum();
        assert!(
            band_total <= 400.0 * 0.4 + 0.01,
            "the bands keep their budget: {band_total}"
        );
        assert!(breaks.iter().all(|band| band.width >= 1.0));
    }

    #[test]
    fn reveals_a_time_by_centring_it() {
        let mut axis = axis(&[(0.0, 10_000.0 * MS)], 800.0);
        axis.zoom_around(400.0, 10.0);
        axis.reveal(1_000.0 * MS);
        assert!(
            (axis.screen(1_000.0 * MS) - 400.0).abs() < 1.0,
            "the time comes to the middle: {}",
            axis.screen(1_000.0 * MS)
        );
    }

    struct TestLabels;

    impl TimeLabels for TestLabels {
        fn tick(&self, time: f64, _step: f64) -> String {
            format!("{:.1}s", time / 1e9)
        }

        fn break_band(&self, stop: f64, restart: f64) -> (String, String) {
            (
                format!("stop {:.1}s", stop / 1e9),
                format!("restart {:.1}s", restart / 1e9),
            )
        }
    }

    fn painted_text(output: &egui::FullOutput) -> Vec<String> {
        fn collect(shape: &egui::Shape, texts: &mut Vec<String>) {
            match shape {
                egui::Shape::Text(text) => texts.push(text.galley.text().to_owned()),
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        collect(shape, texts);
                    }
                }
                _ => {}
            }
        }
        let mut texts = Vec::new();
        for clipped in &output.shapes {
            collect(&clipped.shape, &mut texts);
        }
        texts
    }

    #[test]
    fn the_ruler_paints_ticks_and_the_band_sides() {
        let ctx = egui::Context::default();
        let mut axis = TimelineAxis::new(config());
        axis.set_segments(vec![
            TimeSegment {
                start: 0.0,
                end: 1_000.0 * MS,
            },
            TimeSegment {
                start: 2_000.0 * MS,
                end: 3_000.0 * MS,
            },
        ]);
        axis.layout(400.0);
        axis.fit();

        let steps = [200.0 * MS, 500.0 * MS, 1_000.0 * MS];
        let scale = RulerScale::new(&steps, 80.0, 0.0);
        let style = TimelineRulerStyle {
            background: Color32::BLACK,
            text: Color32::WHITE,
            break_background: Color32::from_gray(0x40),
            font: FontId::proportional(10.0),
        };
        let row = Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 20.0));
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(400.0, 20.0))),
                ..Default::default()
            },
            |ui| {
                TimelineRuler::new(&axis, scale, &TestLabels, &style).show(ui, row);
            },
        );

        let text = painted_text(&output);
        assert!(
            text.iter().any(|line| line.ends_with('s')
                && !line.starts_with("stop")
                && !line.starts_with("restart")),
            "a tick label is painted: {text:?}"
        );
        assert!(
            text.iter().any(|line| line == "stop 1.0s")
                && text.iter().any(|line| line == "restart 2.0s"),
            "both sides of the break band are painted: {text:?}"
        );
    }
}
