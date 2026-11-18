//! The module area's compressed time axis.
//!
//! A capture can be stopped and started again: the stored frames stay, and the
//! next take continues on the same clock, so the capture has pauses in it. The
//! module area lays the takes end to end and compresses every pause into a
//! fixed-width break band, with the stop and restart clock times printed on the
//! band's two sides. The x axis is therefore the *recorded* time (the takes
//! concatenated), not the wall time: a break band is a discontinuity, not empty
//! chart space.
//!
//! The mapping is affine inside a take and jumps by one break band at each
//! boundary:
//!
//! `text
//! content(recorded) = recorded * zoom + breaks_before(recorded) * break_px
//! screen(recorded)  = content(recorded) - content(view_left)
//! `
//!
//! Everything here is pure layout: the panel owns the view state and paints what
//! this module reports (`VisibleSegment`, `AxisBreak`, `AxisTick`). The ruler's
//! labels are real clock times, anchored to the wall clock the capture recorded
//! at its origin (`kairos_profile::WallClock`); a capture can run past midnight,
//! so the clock wraps at 24 h.

use std::sync::Arc;

use kairos_editor_widgets::min_max_scroll::ScrollWindow;
use kairos_profile::{FrameSnapshot, ProfileSession};

use super::tick_label;

/// Width of one break band, in points.
const BREAK_WIDTH: f32 = 14.0;
/// Break bands never take more than this fraction of the chart columns, so a
/// capture with very many takes cannot squeeze the data out of view.
const BREAK_BUDGET: f32 = 0.4;
/// Narrowest recorded span the axis zooms to (10 ms).
const MIN_VISIBLE_NS: f64 = 10_000_000.0;
/// Smallest distance between two labelled clock ticks, in points. Wider than the
/// timeline ruler's because a clock label reads 10:48:57.123.
const MIN_TICK_SPACING: f32 = 92.0;
/// Clock steps, finest first: 1 ms to 15 min. The axis picks the finest step
/// whose labels still fit, exactly like the timeline's ruler does with ms steps.
const CLOCK_STEPS_NS: [f64; 20] = [
    1_000_000.0,
    2_000_000.0,
    5_000_000.0,
    10_000_000.0,
    20_000_000.0,
    50_000_000.0,
    100_000_000.0,
    200_000_000.0,
    500_000_000.0,
    1_000_000_000.0,
    2_000_000_000.0,
    5_000_000_000.0,
    10_000_000_000.0,
    15_000_000_000.0,
    30_000_000_000.0,
    60_000_000_000.0,
    120_000_000_000.0,
    300_000_000_000.0,
    600_000_000_000.0,
    900_000_000_000.0,
];
/// Nanoseconds in a day: the clock labels wrap here.
const DAY_NS: f64 = 86_400.0 * 1_000_000_000.0;

/// One take on the axis: its capture-relative time range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AxisSegment {
    /// Start in capture time, relative to the capture origin (ns).
    pub start_ns: f64,
    /// End in capture time (ns); a take still recording ends at its last frame.
    pub end_ns: f64,
}

/// The slice of one take the view shows, and where it lands on screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct VisibleSegment {
    /// Index into the axis' segments.
    pub index: usize,
    /// The visible slice in recorded time: the takes concatenated, pauses removed.
    pub recorded: (f64, f64),
    /// The same slice in capture time (the timestamps frames carry).
    pub capture: (f64, f64),
    /// Screen columns: `x.0` is the left edge, `x.1` the right edge.
    pub x: (f32, f32),
}

/// A break band between two takes, where the pause is compressed away.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AxisBreak {
    /// Screen x of the band's left edge.
    pub x: f32,
    /// Band width in points; the data columns stop at `x` and resume at
    /// `x + width`.
    pub width: f32,
    /// Clock time of the stop (printed on the band's left side).
    pub stop: String,
    /// Clock time of the restart (printed on the band's right side).
    pub start: String,
}

/// One tick on the module area's ruler.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AxisTick {
    /// Screen x in the chart columns.
    pub x: f32,
    /// Clock label at a major tick; `None` for the four minor ticks between.
    pub label: Option<String>,
}

/// The compressed time axis: takes, the view window, and the mapping between
/// recorded time, capture time and screen columns.
#[derive(Clone, Debug, Default)]
pub(crate) struct ModuleAxis {
    segments: Vec<AxisSegment>,
    /// `prefix[i]` is the recorded time at which segment `i` starts.
    prefix: Vec<f64>,
    /// Total recorded time across every take.
    duration: f64,
    /// Clock time of the capture origin, ns since local midnight.
    wall_origin_ns: Option<f64>,
    /// Width of the chart columns, in points.
    width: f32,
    /// Width of one break band, in points (0 with a single take).
    break_px: f32,
    /// Content points per recorded nanosecond.
    zoom: f64,
    /// Recorded time at the view's left edge.
    left: f64,
}

impl ModuleAxis {
    /// Refresh the takes from a capture, keeping the current view.
    ///
    /// An open take ends at the newest frame; a capture with no takes at all
    /// (a synthetic session) is drawn as one continuous take so tests and
    /// imports still chart.
    pub(crate) fn sync(&mut self, session: &ProfileSession) {
        let frames = &session.frames;
        let last_end = frames.last().map_or(0.0, |frame| frame.end_ns as f64);
        let mut segments: Vec<AxisSegment> = session
            .segments
            .iter()
            .map(|segment| {
                let start = segment.start_ns as f64;
                let end = segment.end_ns.map_or(last_end, |end| end as f64);
                AxisSegment {
                    start_ns: start,
                    end_ns: end.max(start),
                }
            })
            .collect();
        if segments.is_empty() {
            if let (Some(first), Some(last)) = (frames.first(), frames.last()) {
                let start = first.start_ns as f64;
                segments.push(AxisSegment {
                    start_ns: start,
                    end_ns: (last.end_ns as f64).max(start),
                });
            }
        }
        self.install(
            segments,
            session.wall_clock.map(|clock| clock.time_of_day_ns as f64),
        );
    }

    /// Replace the takes, keeping the view (which layout re-clamps).
    fn install(&mut self, segments: Vec<AxisSegment>, wall_origin_ns: Option<f64>) {
        self.segments = segments;
        self.wall_origin_ns = wall_origin_ns;
        self.prefix.clear();
        let mut prefix = 0.0;
        for segment in &self.segments {
            self.prefix.push(prefix);
            prefix += (segment.end_ns - segment.start_ns).max(0.0);
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
    pub(crate) fn layout(&mut self, width: f32) {
        self.width = width.max(1.0);
        self.break_px = break_width(self.width, self.segments.len().saturating_sub(1));
        if self.zoom > 0.0 {
            self.clamp_view();
        }
    }

    /// Whether the axis has data to map.
    pub(crate) fn is_empty(&self) -> bool {
        self.duration <= 0.0 || self.zoom <= 0.0 || self.segments.is_empty()
    }

    /// Content points per recorded nanosecond.
    pub(crate) fn zoom(&self) -> f64 {
        self.zoom
    }

    /// Recorded time at the view's left edge.
    pub(crate) fn left(&self) -> f64 {
        self.left
    }

    /// The visible recorded span: the window width, less the break bands that
    /// fall inside it.
    pub(crate) fn view_span(&self) -> f64 {
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

    /// Screen x of a recorded time.
    pub(crate) fn screen(&self, recorded: f64) -> f32 {
        (self.content(recorded) - self.content(self.left)) as f32
    }

    /// Screen x of a capture time, if it falls inside a take.
    pub(crate) fn screen_of_capture(&self, capture_ns: f64) -> Option<f32> {
        let (index, recorded) = self.recorded_of_capture(capture_ns)?;
        Some((self.content_in(index, recorded) - self.content(self.left)) as f32)
    }

    /// The take (index into the segments) and recorded time of a capture time.
    pub(crate) fn recorded_of_capture(&self, capture_ns: f64) -> Option<(usize, f64)> {
        if self.segments.is_empty() {
            return None;
        }
        let index = self
            .segments
            .partition_point(|segment| segment.start_ns <= capture_ns)
            .saturating_sub(1);
        let segment = self.segments.get(index)?;
        if capture_ns < segment.start_ns || capture_ns > segment.end_ns {
            return None;
        }
        Some((index, self.prefix[index] + (capture_ns - segment.start_ns)))
    }

    /// The take and capture time behind a recorded time.
    pub(crate) fn capture_of_recorded(&self, recorded: f64) -> (usize, f64) {
        let index = self.seg_index(recorded);
        let start = self.prefix[index];
        (index, self.segments[index].start_ns + (recorded - start))
    }

    /// The capture time under a screen column, or `None` over a break band.
    pub(crate) fn capture_at_screen(&self, x: f32) -> Option<(usize, f64)> {
        if self.zoom <= 0.0 || !(0.0..=self.width).contains(&x) {
            return None;
        }
        for band in self.breaks() {
            if x >= band.x && x <= band.x + band.width {
                return None;
            }
        }
        let content = self.content(self.left) + f64::from(x);
        let recorded = self.time_at_content(content);
        Some(self.capture_of_recorded(recorded))
    }

    /// The takes the view shows, with their screen columns.
    pub(crate) fn visible_segments(&self) -> Vec<VisibleSegment> {
        let mut visible = Vec::new();
        if self.zoom <= 0.0 || self.segments.is_empty() {
            return visible;
        }
        let span = self.view_span();
        let view = (self.left, self.left + span);
        for (index, segment) in self.segments.iter().enumerate() {
            let start = self.prefix[index];
            let end = start + (segment.end_ns - segment.start_ns).max(0.0);
            let lo = start.max(view.0);
            let hi = end.min(view.1);
            if hi <= lo {
                continue;
            }
            let left_content = self.content(self.left);
            visible.push(VisibleSegment {
                index,
                recorded: (lo, hi),
                capture: (
                    segment.start_ns + (lo - start),
                    segment.start_ns + (hi - start),
                ),
                x: (
                    (self.content_in(index, lo) - left_content) as f32,
                    (self.content_in(index, hi) - left_content) as f32,
                ),
            });
        }
        visible
    }

    /// The break bands the view shows, with their stop/restart clock labels.
    pub(crate) fn breaks(&self) -> Vec<AxisBreak> {
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
            let stop = self.segments[index - 1].end_ns;
            let start = self.segments[index].start_ns;
            let (stop, start) = match self.wall_origin_ns {
                Some(origin) => (
                    clock_label(origin + stop, true),
                    clock_label(origin + start, true),
                ),
                None => (elapsed_label(stop), elapsed_label(start)),
            };
            breaks.push(AxisBreak {
                x,
                width: self.break_px,
                stop,
                start,
            });
        }
        breaks
    }

    /// Ruler ticks: clock-aligned majors with four minors between them.
    pub(crate) fn ticks(&self) -> Vec<AxisTick> {
        let mut ticks = Vec::new();
        if self.zoom <= 0.0 || self.segments.is_empty() {
            return ticks;
        }
        let step = CLOCK_STEPS_NS
            .iter()
            .copied()
            .find(|step| step * self.zoom >= f64::from(MIN_TICK_SPACING))
            .unwrap_or(CLOCK_STEPS_NS[0]);
        let minor = step / 5.0;
        let origin = self.wall_origin_ns.unwrap_or(0.0);
        for visible in self.visible_segments() {
            let (capture_start, capture_end) = visible.capture;
            let first = ((origin + capture_start) / minor).ceil() as i64;
            let last = ((origin + capture_end) / minor).floor() as i64;
            if last < first || last - first > 4_096 {
                continue;
            }
            for index in first..=last {
                let capture = index as f64 * minor - origin;
                let recorded = visible.recorded.0 + (capture - capture_start);
                let x = (self.content_in(visible.index, recorded) - self.content(self.left)) as f32;
                if x < -1.0 || x > self.width + 1.0 {
                    continue;
                }
                let label = (index.rem_euclid(5) == 0).then(|| match self.wall_origin_ns {
                    Some(origin) => clock_label(origin + capture, step < 1e9),
                    None => tick_label(capture / 1e6, step / 1e6),
                });
                ticks.push(AxisTick { x, label });
            }
        }
        ticks
    }

    /// The min-max scroll bar's window over the recorded time.
    pub(crate) fn window(&self) -> ScrollWindow {
        let span = self.view_span().clamp(0.0, self.duration);
        ScrollWindow::new(self.left, span, (0.0, self.duration))
            .with_min_span(self.min_visible_span())
    }

    /// The narrowest recorded window the axis shows: ten milliseconds normally,
    /// shrinking for captures shorter than that (a brief capture still zooms).
    fn min_visible_span(&self) -> f64 {
        let by_capture = (self.duration / 8.0).max(1_000.0);
        MIN_VISIBLE_NS.min(by_capture).min(self.duration.max(1.0))
    }

    /// Apply the scroll bar's answer: its value pans, its span zooms.
    pub(crate) fn apply_window(&mut self, value: f64, span: f64) {
        if self.duration <= 0.0 {
            return;
        }
        let span = span.clamp(1.0, self.duration);
        let value = value.clamp(0.0, (self.duration - span).max(0.0));
        let breaks = self.breaks_between(value, value + span);
        let data_width =
            (f64::from(self.width) - breaks as f64 * f64::from(self.break_px)).max(1.0);
        self.zoom = data_width / span;
        self.left = value;
        self.clamp_view();
    }

    /// Show every take, edge to edge.
    pub(crate) fn fit(&mut self) {
        if self.segments.is_empty() || self.duration <= 0.0 {
            return;
        }
        self.zoom = self.fit_zoom();
        self.left = 0.0;
        self.clamp_view();
    }

    /// Show the recorded range `from..to` across the chart columns.
    pub(crate) fn show_recorded_window(&mut self, from: f64, to: f64) {
        if self.segments.is_empty() || self.duration <= 0.0 {
            return;
        }
        let from = from.clamp(0.0, self.duration);
        let to = to.clamp(from, self.duration);
        let span = (to - from).max(1.0);
        let breaks = self.breaks_between(from, to);
        let data_width =
            (f64::from(self.width) - breaks as f64 * f64::from(self.break_px)).max(1.0);
        self.zoom = data_width / span;
        self.left = from;
        self.clamp_view();
    }

    /// Show the most recent `count` frames, the module area's default window.
    pub(crate) fn show_last_frames(&mut self, frames: &[Arc<FrameSnapshot>], count: usize) {
        if frames.len() <= count {
            self.fit();
            return;
        }
        let first = &frames[frames.len() - count];
        let last = &frames[frames.len() - 1];
        let Some((_, from)) = self.recorded_of_capture(first.start_ns as f64) else {
            self.fit();
            return;
        };
        let Some((_, to)) = self.recorded_of_capture(last.end_ns as f64) else {
            self.fit();
            return;
        };
        self.show_recorded_window(from, to);
    }

    /// Keep the right edge on the newest data (the header's Live toggle).
    pub(crate) fn follow_end(&mut self) {
        if self.zoom <= 0.0 || self.duration <= 0.0 {
            return;
        }
        let total = self.content(self.duration);
        let target = (total - f64::from(self.width)).max(0.0);
        self.left = self.time_at_content(target);
        self.clamp_view();
    }

    /// Zoom by `factor` around screen column `x`, as the wheel does.
    pub(crate) fn zoom_around(&mut self, x: f32, factor: f64) {
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

    /// Pan by a drag of `dx` points: dragging the chart right shows earlier time.
    pub(crate) fn pan_screen(&mut self, dx: f32) {
        if self.zoom <= 0.0 || self.duration <= 0.0 {
            return;
        }
        let target = self.content(self.left) - f64::from(dx);
        self.left = self.time_at_content(target.max(0.0));
        self.clamp_view();
    }

    /// Pan the view so a frame is visible again (the transport buttons).
    pub(crate) fn reveal(&mut self, recorded: f64) {
        if self.zoom <= 0.0 || self.duration <= 0.0 {
            return;
        }
        let x = self.screen(recorded);
        if (0.0..=self.width).contains(&x) {
            return;
        }
        let target = self.content(recorded) - f64::from(self.width) * 0.5;
        self.left = self.time_at_content(target.max(0.0));
        self.clamp_view();
    }

    /// Content x of a recorded time: linearly inside a take, plus one band per
    /// take already passed.
    fn content(&self, recorded: f64) -> f64 {
        recorded * self.zoom + self.seg_index(recorded) as f64 * f64::from(self.break_px)
    }

    /// Content x of a recorded time known to be inside take `index`.
    ///
    /// The recorded time at a take's end is ambiguous with the next take's start;
    /// painting a take's own columns must count the bands before that take, not
    /// after it, or the take would end a band too far right.
    fn content_in(&self, index: usize, recorded: f64) -> f64 {
        recorded * self.zoom + index as f64 * f64::from(self.break_px)
    }

    /// Index of the take a recorded time falls in.
    fn seg_index(&self, recorded: f64) -> usize {
        let index = self.prefix.partition_point(|prefix| *prefix <= recorded);
        index
            .saturating_sub(1)
            .min(self.segments.len().saturating_sub(1))
    }

    /// Recorded time at a content x; the fixed point walks past band jumps.
    fn time_at_content(&self, content: f64) -> f64 {
        if self.zoom <= 0.0 || self.duration <= 0.0 {
            return 0.0;
        }
        let mut index = self.seg_index(self.left) as f64;
        let mut recorded = 0.0;
        for _ in 0..8 {
            recorded = ((content - index * f64::from(self.break_px)) / self.zoom)
                .clamp(0.0, self.duration);
            let next = self.seg_index(recorded) as f64;
            if next == index {
                break;
            }
            index = next;
        }
        recorded
    }

    /// Number of take boundaries strictly inside `from..to`.
    fn breaks_between(&self, from: f64, to: f64) -> usize {
        if to <= from {
            return 0;
        }
        let lo = self.prefix.partition_point(|prefix| *prefix <= from);
        let hi = self.prefix.partition_point(|prefix| *prefix < to);
        hi.saturating_sub(lo)
    }

    /// Content points per recorded ns at the fully zoomed-out view.
    fn fit_zoom(&self) -> f64 {
        if self.duration <= 0.0 {
            return 1.0;
        }
        let data_width = (f64::from(self.width) - self.breaks_total()).max(1.0);
        data_width / self.duration
    }

    /// Widest zoom: a window at the narrowest visible span.
    fn max_zoom(&self) -> f64 {
        f64::from(self.width) / self.min_visible_span()
    }

    /// Content points taken by all break bands.
    fn breaks_total(&self) -> f64 {
        self.segments.len().saturating_sub(1) as f64 * f64::from(self.break_px)
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

/// Width of one break band for `breaks` pauses across `width` points: a small
/// fixed band, squeezed when there are so many takes that the bands would eat
/// more than BREAK_BUDGET of the chart.
fn break_width(width: f32, breaks: usize) -> f32 {
    if breaks == 0 {
        return 0.0;
    }
    let budget = width * BREAK_BUDGET / breaks as f32;
    BREAK_WIDTH.clamp(2.0, width).min(budget).max(1.0)
}

/// A clock reading as HH:MM:SS, with .mmm appended when `millis`.
///
/// The value is nanoseconds since local midnight and wraps at 24 h, so a capture
/// that runs over midnight keeps reading like a clock.
pub(crate) fn clock_label(time_of_day_ns: f64, millis: bool) -> String {
    let day = time_of_day_ns.rem_euclid(DAY_NS);
    let mut seconds = (day / 1e9).floor() as i64;
    let mut ms = ((day - seconds as f64 * 1e9) / 1e6).round() as i64;
    if ms >= 1000 {
        ms -= 1000;
        seconds += 1;
    }
    let seconds = seconds.rem_euclid(86_400);
    let (hour, minute, second) = (seconds / 3_600, (seconds / 60) % 60, seconds % 60);
    if millis {
        format!("{hour:02}:{minute:02}:{second:02}.{ms:03}")
    } else {
        format!("{hour:02}:{minute:02}:{second:02}")
    }
}

/// Elapsed-time label, for a synthetic capture with no wall clock.
fn elapsed_label(capture_ns: f64) -> String {
    let ms = capture_ns / 1e6;
    if ms < 1_000.0 {
        format!("{ms:.1}ms")
    } else {
        format!("{:.2}s", ms / 1_000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: f64 = 1_000_000.0;

    fn axis(segments: &[(f64, f64)], width: f32) -> ModuleAxis {
        let mut axis = ModuleAxis::default();
        axis.install(
            segments
                .iter()
                .map(|(start, end)| AxisSegment {
                    start_ns: *start,
                    end_ns: *end,
                })
                .collect(),
            Some(10.0 * 3_600.0 * 1e9),
        );
        axis.layout(width);
        axis.fit();
        axis
    }

    fn sample_frames(count: usize, step_ns: u64) -> Vec<Arc<FrameSnapshot>> {
        (0..count)
            .map(|index| {
                Arc::new(FrameSnapshot {
                    index: index as u64,
                    source_index: index as u64,
                    start_ns: index as u64 * step_ns,
                    end_ns: (index as u64 + 1) * step_ns,
                    threads: Vec::new(),
                    categories: [0; ::kairos_profile::Category::COUNT],
                })
            })
            .collect()
    }

    #[test]
    fn one_take_fills_the_columns() {
        let axis = axis(&[(0.0, 1_000.0 * MS)], 1_000.0);
        let visible = axis.visible_segments();
        assert_eq!(visible.len(), 1);
        assert!((visible[0].x.0 - 0.0).abs() < 0.01);
        assert!((visible[0].x.1 - 1_000.0).abs() < 0.01);
        assert!((axis.screen(500.0 * MS) - 500.0).abs() < 0.01);
        assert!(axis.breaks().is_empty(), "a single take has no boundary");
    }

    #[test]
    fn a_pause_is_one_fixed_break_band_between_the_takes() {
        // Two 500 ms takes separated by a 200 ms pause: the pause must not take
        // any time columns, only one band's worth.
        let axis = axis(&[(0.0, 500.0 * MS), (700.0 * MS, 1_200.0 * MS)], 1_000.0);
        let visible = axis.visible_segments();
        assert_eq!(visible.len(), 2);
        let breaks = axis.breaks();
        assert_eq!(breaks.len(), 1);
        assert!(
            (breaks[0].width - BREAK_WIDTH).abs() < 0.01,
            "a normal break band keeps its fixed width: {}",
            breaks[0].width
        );
        assert!(
            (visible[1].x.0 - (visible[0].x.1 + breaks[0].width)).abs() < 0.01,
            "the second take starts right after the band: {visible:?}"
        );
        let data: f32 = visible
            .iter()
            .map(|segment| segment.x.1 - segment.x.0)
            .sum();
        assert!((data + breaks[0].width - 1_000.0).abs() < 0.01);
    }

    #[test]
    fn zoom_around_keeps_the_time_under_the_pointer() {
        let mut axis = axis(&[(0.0, 1_000.0 * MS), (1_200.0 * MS, 2_200.0 * MS)], 800.0);
        let x = 300.0;
        let before = {
            let content = axis.content(axis.left) + f64::from(x);
            axis.time_at_content(content)
        };
        axis.zoom_around(x, 2.0);
        let content = axis.content(axis.left) + f64::from(x);
        let after = axis.time_at_content(content);
        assert!(
            (after - before).abs() < 1.0,
            "the time under the pointer stays: {before} -> {after}"
        );
    }

    #[test]
    fn zooming_out_stops_at_the_whole_capture() {
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
    fn the_default_window_keeps_the_last_frames() {
        let history = sample_frames(350, 20_000_000);
        let mut axis = ModuleAxis::default();
        axis.install(
            vec![AxisSegment {
                start_ns: 0.0,
                end_ns: 7_000.0 * MS,
            }],
            None,
        );
        axis.layout(800.0);
        axis.show_last_frames(&history, 300);
        // The last 300 frames start at 50 * 20 ms = 1 s and end at 7 s.
        assert!((axis.left() - 1_000.0 * MS).abs() < 1.0, "{}", axis.left());
        assert!(
            (axis.view_span() - 6_000.0 * MS).abs() < 1.0,
            "{}",
            axis.view_span()
        );
        let visible = axis.visible_segments();
        assert_eq!(visible.len(), 1);
        assert!((visible[0].capture.0 - 1_000.0 * MS).abs() < 1.0);
        let few = sample_frames(10, 20_000_000);
        axis.show_last_frames(&few, 300);
        assert!(axis.left().abs() < 1e-9);
    }

    #[test]
    fn clock_ticks_land_on_whole_clock_times() {
        let mut axis = ModuleAxis::default();
        axis.install(
            vec![AxisSegment {
                start_ns: 0.0,
                end_ns: 2_000.0 * MS,
            }],
            Some(10.0 * 3_600.0 * 1e9),
        );
        axis.layout(800.0);
        axis.fit();
        let ticks = axis.ticks();
        let labels: Vec<&str> = ticks
            .iter()
            .filter_map(|tick| tick.label.as_deref())
            .collect();
        assert!(
            labels.contains(&"10:00:00.000") && labels.contains(&"10:00:00.500"),
            "the labels are real clock readings: {labels:?}"
        );
        assert!(
            ticks.iter().any(|tick| tick.label.is_none()),
            "minor ticks sit between the labelled ones"
        );
        let x_of = |label: &str| {
            ticks
                .iter()
                .find(|tick| tick.label.as_deref() == Some(label))
                .map(|tick| tick.x)
                .unwrap()
        };
        let step = x_of("10:00:00.500") - x_of("10:00:00.000");
        assert!(
            (step - 200.0).abs() < 0.5,
            "half a second is a quarter of 800: {step}"
        );
    }

    #[test]
    fn break_bands_print_the_stop_and_restart_times() {
        let mut axis = ModuleAxis::default();
        axis.install(
            vec![
                AxisSegment {
                    start_ns: 0.0,
                    end_ns: 1_000.0 * MS,
                },
                AxisSegment {
                    start_ns: 2_000.0 * MS,
                    end_ns: 3_000.0 * MS,
                },
            ],
            Some(10.0 * 3_600.0 * 1e9),
        );
        axis.layout(800.0);
        axis.fit();
        let breaks = axis.breaks();
        assert_eq!(breaks.len(), 1);
        assert_eq!(breaks[0].stop, "10:00:01.000");
        assert_eq!(breaks[0].start, "10:00:02.000");
        let visible = axis.visible_segments();
        assert!(
            (breaks[0].x - visible[0].x.1).abs() < 0.01,
            "the band starts where the first take ends"
        );
    }

    #[test]
    fn clicking_a_break_band_selects_no_time() {
        let axis = axis(&[(0.0, 500.0 * MS), (700.0 * MS, 1_200.0 * MS)], 1_000.0);
        let visible = axis.visible_segments();
        let band = axis.breaks()[0].x;
        assert!(axis.capture_at_screen(visible[0].x.1 - 1.0).is_some());
        assert!(
            axis.capture_at_screen(band + 1.0).is_none(),
            "the pause is not a frame"
        );
        let (index, capture) = axis.capture_at_screen(band + 30.0).unwrap();
        assert_eq!(index, 1, "right of the band is the second take");
        assert!(capture >= 700.0 * MS);
    }

    #[test]
    fn a_capture_past_midnight_wraps_its_clock_labels() {
        assert_eq!(clock_label(DAY_NS - 0.5e9, true), "23:59:59.500");
        assert_eq!(clock_label(DAY_NS + 0.5e9, true), "00:00:00.500");
        assert_eq!(clock_label(0.0, false), "00:00:00");
    }

    #[test]
    fn many_takes_squeeze_the_bands_instead_of_the_data() {
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
            band_total <= 400.0 * BREAK_BUDGET + 0.01,
            "the bands keep their budget: {band_total}"
        );
        assert!(breaks.iter().all(|band| band.width >= 1.0));
    }

    #[test]
    fn reveals_a_frame_by_centring_it() {
        let mut axis = axis(&[(0.0, 10_000.0 * MS)], 800.0);
        axis.zoom_around(400.0, 10.0);
        axis.reveal(1_000.0 * MS);
        assert!(
            (axis.screen(1_000.0 * MS) - 400.0).abs() < 1.0,
            "the frame comes to the middle: {}",
            axis.screen(1_000.0 * MS)
        );
    }
}
