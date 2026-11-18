//! The module area's compressed clock axis.
//!
//! The generic mapping and ruler live in
//! [kairos_editor_widgets::timeline_axis]; this module is the Profiler's
//! adapter between a capture and that axis.
//!
//! A capture can be stopped and started again: the stored frames stay, and the
//! next take continues on the same clock, so the capture has pauses in it. The
//! module area lays the takes end to end and compresses every pause into a
//! fixed-width break band, with the stop and restart clock times printed on the
//! band's two sides. The x axis is therefore the *recorded* time (the takes
//! concatenated), not the wall time: a break band is a discontinuity, not empty
//! chart space.
//!
//! [ModuleAxis] keeps the capture's takes and wall clock next to the generic
//! [TimelineAxis]: [sync](ModuleAxis::sync) translates a [ProfileSession] into
//! the axis' segments, and the labels read the capture's clock readings (a
//! capture can run past midnight, so the clock wraps at 24 h).

use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use kairos_editor_widgets::timeline_axis::{
    AxisConfig, RulerScale, TimeLabels, TimeSegment, TimelineAxis,
};
use kairos_profile::{FrameSnapshot, ProfileSession};

use super::tick_label;

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
/// Smallest distance between two labelled clock ticks, in points. Wider than the
/// timeline ruler's because a clock label reads 10:48:57.123.
const MIN_TICK_SPACING: f32 = 92.0;
/// Narrowest recorded span the axis zooms to (10 ms).
const MIN_VISIBLE_NS: f64 = 10_000_000.0;
/// Smallest meaningful time on the axis (1 us): a capture shorter than the
/// normal minimum still zooms in to this.
const RESOLUTION_NS: f64 = 1_000.0;
/// Nanoseconds in a day: the clock labels wrap here.
const DAY_NS: f64 = 86_400.0 * 1_000_000_000.0;

/// The module area's axis: the generic [TimelineAxis] plus the capture clock
/// its labels read.
///
/// The deref impls make the wrapper behave like the axis it owns, so the panel
/// calls the generic view operations directly and this module only adds the
/// capture translation ([sync](Self::sync), [show_last_frames](Self::show_last_frames))
/// and the label policy ([ruler_scale](Self::ruler_scale), [labels](Self::labels)).
#[derive(Debug)]
pub(crate) struct ModuleAxis {
    axis: TimelineAxis,
    /// Clock time of the capture origin, ns since local midnight.
    wall_origin_ns: Option<f64>,
}

impl Default for ModuleAxis {
    fn default() -> Self {
        Self {
            axis: TimelineAxis::new(AxisConfig {
                min_span: MIN_VISIBLE_NS,
                resolution: RESOLUTION_NS,
                ..AxisConfig::default()
            }),
            wall_origin_ns: None,
        }
    }
}

impl Deref for ModuleAxis {
    type Target = TimelineAxis;

    fn deref(&self) -> &Self::Target {
        &self.axis
    }
}

impl DerefMut for ModuleAxis {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.axis
    }
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
        let mut segments: Vec<TimeSegment> = session
            .segments
            .iter()
            .map(|segment| {
                let start = segment.start_ns as f64;
                let end = segment.end_ns.map_or(last_end, |end| end as f64);
                TimeSegment {
                    start,
                    end: end.max(start),
                }
            })
            .collect();
        if segments.is_empty() {
            if let (Some(first), Some(last)) = (frames.first(), frames.last()) {
                let start = first.start_ns as f64;
                segments.push(TimeSegment {
                    start,
                    end: (last.end_ns as f64).max(start),
                });
            }
        }
        self.wall_origin_ns = session.wall_clock.map(|clock| clock.time_of_day_ns as f64);
        self.axis.set_segments(segments);
    }

    /// Show the most recent count frames, the module area's default window.
    pub(crate) fn show_last_frames(&mut self, frames: &[Arc<FrameSnapshot>], count: usize) {
        if frames.len() <= count {
            self.axis.fit();
            return;
        }
        let first = &frames[frames.len() - count];
        let last = &frames[frames.len() - 1];
        let Some((_, from)) = self.axis.time_of_source(first.start_ns as f64) else {
            self.axis.fit();
            return;
        };
        let Some((_, to)) = self.axis.time_of_source(last.end_ns as f64) else {
            self.axis.fit();
            return;
        };
        self.axis.show_window(from, to);
    }

    /// Where the module ruler's labelled ticks fall: clock-aligned multiples of
    /// the capture's wall clock, or of the capture origin for a synthetic one.
    pub(crate) fn ruler_scale(&self) -> RulerScale<'static> {
        RulerScale::new(
            &CLOCK_STEPS_NS,
            MIN_TICK_SPACING,
            self.wall_origin_ns.unwrap_or(0.0),
        )
    }

    /// The labels the module ruler prints.
    pub(crate) fn labels(&self) -> ModuleLabels {
        ModuleLabels {
            wall_origin_ns: self.wall_origin_ns,
        }
    }
}

/// The module ruler's labels: real clock readings for a live capture, elapsed
/// time for a synthetic one.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ModuleLabels {
    wall_origin_ns: Option<f64>,
}

impl TimeLabels for ModuleLabels {
    fn tick(&self, time: f64, step: f64) -> String {
        match self.wall_origin_ns {
            Some(_) => clock_label(time, step < 1e9),
            None => tick_label(time / 1e6, step / 1e6),
        }
    }

    fn break_band(&self, stop: f64, restart: f64) -> (String, String) {
        match self.wall_origin_ns {
            Some(_) => (clock_label(stop, true), clock_label(restart, true)),
            None => (elapsed_label(stop), elapsed_label(restart)),
        }
    }
}

/// A clock reading as HH:MM:SS, with .mmm appended when millis.
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
    use kairos_profile::{Category, RecordingSegment};

    const MS: f64 = 1_000_000.0;
    const TEN_AM_NS: f64 = 10.0 * 3_600.0 * 1e9;

    fn frame(index: u64, start_ns: u64, end_ns: u64) -> Arc<FrameSnapshot> {
        Arc::new(FrameSnapshot {
            index,
            source_index: index,
            start_ns,
            end_ns,
            threads: Vec::new(),
            categories: [0; Category::COUNT],
        })
    }

    fn sample_frames(count: usize, step_ns: u64) -> Vec<Arc<FrameSnapshot>> {
        (0..count)
            .map(|index| {
                frame(
                    index as u64,
                    index as u64 * step_ns,
                    (index as u64 + 1) * step_ns,
                )
            })
            .collect()
    }

    fn axis_with(segments: &[(f64, f64)], wall_origin_ns: Option<f64>, width: f32) -> ModuleAxis {
        let mut axis = ModuleAxis::default();
        axis.set_segments(
            segments
                .iter()
                .map(|(start, end)| TimeSegment {
                    start: *start,
                    end: *end,
                })
                .collect(),
        );
        axis.wall_origin_ns = wall_origin_ns;
        axis.layout(width);
        axis.fit();
        axis
    }

    #[test]
    fn an_open_take_ends_at_the_newest_frame() {
        let session = ProfileSession {
            frames: vec![
                frame(0, 0, 10 * MS as u64),
                frame(1, 30 * MS as u64, 40 * MS as u64),
            ],
            segments: vec![
                RecordingSegment {
                    start_ns: 0,
                    end_ns: Some(10 * MS as u64),
                },
                RecordingSegment {
                    start_ns: 30 * MS as u64,
                    end_ns: None,
                },
            ],
            ..Default::default()
        };
        let mut axis = ModuleAxis::default();
        axis.sync(&session);
        axis.layout(800.0);
        axis.fit();

        let visible = axis.visible_segments();
        assert_eq!(visible.len(), 2, "the capture has two takes");
        assert_eq!(
            visible[1].source,
            (30.0 * MS, 40.0 * MS),
            "the open take ends at the newest frame"
        );
    }

    #[test]
    fn a_synthetic_capture_is_one_continuous_take() {
        let session = ProfileSession {
            frames: sample_frames(3, 10 * MS as u64),
            ..Default::default()
        };
        let mut axis = ModuleAxis::default();
        axis.sync(&session);
        axis.layout(800.0);
        axis.fit();

        let visible = axis.visible_segments();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].source, (0.0, 30.0 * MS));
        assert!(axis.breaks().is_empty());
    }

    #[test]
    fn the_default_window_keeps_the_last_frames() {
        let history = sample_frames(350, 20_000_000);
        let mut axis = ModuleAxis::default();
        axis.set_segments(vec![TimeSegment {
            start: 0.0,
            end: 7_000.0 * MS,
        }]);
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
        assert!((visible[0].source.0 - 1_000.0 * MS).abs() < 1.0);
        let few = sample_frames(10, 20_000_000);
        axis.show_last_frames(&few, 300);
        assert!(axis.left().abs() < 1e-9);
    }

    #[test]
    fn clock_ticks_land_on_whole_clock_times() {
        let axis = axis_with(&[(0.0, 2_000.0 * MS)], Some(TEN_AM_NS), 800.0);
        let scale = axis.ruler_scale();
        let labels = axis.labels();
        let ticks = axis.ticks(&scale);
        let labeled: Vec<String> = ticks
            .iter()
            .filter(|tick| tick.major)
            .map(|tick| labels.tick(scale.origin + tick.source, tick.step))
            .collect();
        assert!(
            labeled.contains(&"10:00:00.000".to_owned())
                && labeled.contains(&"10:00:00.500".to_owned()),
            "the labels are real clock readings: {labeled:?}"
        );
        assert!(
            ticks.iter().any(|tick| !tick.major),
            "minor ticks sit between the labelled ones"
        );
        let x_of = |label: &str| {
            ticks
                .iter()
                .find(|tick| {
                    tick.major && labels.tick(scale.origin + tick.source, tick.step) == label
                })
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
        let axis = axis_with(
            &[(0.0, 1_000.0 * MS), (2_000.0 * MS, 3_000.0 * MS)],
            Some(TEN_AM_NS),
            800.0,
        );
        let breaks = axis.breaks();
        assert_eq!(breaks.len(), 1);
        let labels = axis.labels();
        let origin = axis.ruler_scale().origin;
        let (stop, restart) =
            labels.break_band(origin + breaks[0].stop, origin + breaks[0].restart);
        assert_eq!(stop, "10:00:01.000");
        assert_eq!(restart, "10:00:02.000");
        let visible = axis.visible_segments();
        assert!(
            (breaks[0].x - visible[0].x.1).abs() < 0.01,
            "the band starts where the first take ends"
        );
    }

    #[test]
    fn a_synthetic_capture_reads_elapsed_time() {
        let axis = axis_with(&[(0.0, 3_000.0 * MS)], None, 800.0);
        let labels = axis.labels();
        assert_eq!(labels.tick(500.0 * MS, 100.0 * MS), "500ms");
        assert_eq!(
            labels.break_band(1_000.0 * MS, 2_000.0 * MS),
            ("1.00s".to_owned(), "2.00s".to_owned())
        );
    }

    #[test]
    fn a_capture_past_midnight_wraps_its_clock_labels() {
        assert_eq!(clock_label(DAY_NS - 0.5e9, true), "23:59:59.500");
        assert_eq!(clock_label(DAY_NS + 0.5e9, true), "00:00:00.500");
        assert_eq!(clock_label(0.0, false), "00:00:00");
    }
}
