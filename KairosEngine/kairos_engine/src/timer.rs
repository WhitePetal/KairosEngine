use kairos_ecs::resource::Resource;
use std::time::{Duration, Instant};

/// Default ceiling for the raw (unscaled) wall-clock delta of a single frame.
///
/// Mirrors bevy's `Time<Virtual>` default `max_delta` (250 ms). It stops a
/// single over-long frame (a blocked main thread, a breakpoint, a slow first
/// frame) from jumping the virtual clock forward by the whole stall — which
/// would also indirectly bound how many fixed steps a future fixed-step driver
/// has to catch up in one frame ("spiral of death" protection lives on the
/// virtual side).
const DEFAULT_MAX_DELTA: Duration = Duration::from_millis(250);

/// The engine's per-frame virtual clock: raw wall-clock measurement unified
/// with `time_scale` and `paused` into one clock.
///
/// This is a ECS `World` resource: it is registered during `install` (see
/// `kairos_editor::schedule`) and advanced exactly once per frame by
/// `time_system` at the `First` stage. Engine-side (non-system) code only ever
/// reads it — via the `World` resource or an `Engine` read accessor.
#[derive(Resource, Debug)]
pub struct Time {
    start_time: Instant,
    pre_time: Instant,

    total_time: Duration,
    delta_time: Duration,

    total_frame: u64,

    time_scale: f32,
    paused: bool,

    /// Ceiling for the raw frame delta before `time_scale` scaling (250 ms).
    max_delta: Duration,
}

impl Time {
    pub fn new() -> Self {
        let now = Instant::now();

        Self {
            start_time: now,
            pre_time: now,
            total_time: Duration::ZERO,
            delta_time: Duration::ZERO,
            total_frame: 0,
            time_scale: 1.0,
            paused: false,
            max_delta: DEFAULT_MAX_DELTA,
        }
    }

    /// Advances the clock by the wall-clock time elapsed since the previous
    /// call.
    ///
    /// Invoked exactly once per frame by `time_system` at the `First` stage.
    pub fn update(&mut self) {
        let now = Instant::now();
        let raw_delta = now.duration_since(self.pre_time);
        self.pre_time = now;

        self.update_with_raw_delta(raw_delta);
    }

    /// Advances the clock by an externally supplied raw (unscaled, unclamped)
    /// delta — the shared core of [`update`](Self::update).
    ///
    /// `raw_delta` is clamped to `max_delta` before `time_scale` scaling;
    /// `paused` freezes `delta_time` at zero without accumulating a catch-up
    /// debt (the next frame measures from scratch). Kept private: only the
    /// wall-clock [`update`](Self::update) drives it in production; unit tests
    /// use it for deterministic deltas.
    fn update_with_raw_delta(&mut self, raw_delta: Duration) {
        self.total_frame = self.total_frame + 1;

        if self.paused {
            self.delta_time = Duration::ZERO;
            return;
        }

        let raw_delta = if raw_delta > self.max_delta {
            self.max_delta
        } else {
            raw_delta
        };

        self.delta_time = raw_delta.mul_f32(self.time_scale);
        self.total_time += self.delta_time;
    }

    #[inline(always)]
    pub fn total_time(&self) -> Duration {
        self.total_time
    }

    #[inline(always)]
    pub fn delta_time(&self) -> Duration {
        self.delta_time
    }

    #[inline(always)]
    pub fn delta_time_secs(&self) -> f32 {
        self.delta_time().as_secs_f32()
    }

    #[inline(always)]
    pub fn total_frame(&self) -> u64 {
        self.total_frame
    }

    #[inline(always)]
    pub fn total_time_ignore_scale(&self) -> Duration {
        Instant::now().duration_since(self.start_time)
    }

    #[inline(always)]
    pub fn set_time_scale(&mut self, scale: f32) {
        self.time_scale = scale
    }

    #[inline(always)]
    pub fn pause(&mut self) {
        self.paused = true;
    }

    #[inline(always)]
    pub fn resume(&mut self) {
        self.paused = false;
        self.pre_time = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_time_starts_from_zero() {
        let time = Time::new();

        assert_eq!(time.total_time(), Duration::ZERO);
        assert_eq!(time.delta_time(), Duration::ZERO);
        assert_eq!(time.total_frame(), 0);
        assert_eq!(time.max_delta, DEFAULT_MAX_DELTA);
    }

    #[test]
    fn regular_frames_accumulate_the_raw_delta() {
        let mut time = Time::new();

        time.update_with_raw_delta(Duration::from_millis(16));
        time.update_with_raw_delta(Duration::from_millis(17));

        assert_eq!(time.delta_time(), Duration::from_millis(17));
        assert_eq!(time.total_time(), Duration::from_millis(33));
        assert_eq!(time.total_frame(), 2);
    }

    #[test]
    fn overlong_frame_delta_is_clamped_to_default_max_delta() {
        let mut time = Time::new();

        // A 10 s stall must not jump the virtual clock past the 250 ms ceiling.
        time.update_with_raw_delta(Duration::from_secs(10));

        assert_eq!(time.delta_time(), DEFAULT_MAX_DELTA);
        assert_eq!(time.total_time(), DEFAULT_MAX_DELTA);
        assert_eq!(time.total_frame(), 1);
    }

    #[test]
    fn custom_max_delta_clamps_before_time_scale_scaling() {
        let mut time = Time::new();
        time.max_delta = Duration::from_millis(100);
        time.set_time_scale(2.0);

        time.update_with_raw_delta(Duration::from_secs(5));

        // Clamp to 100 ms first, then scale by 2.0 → 200 ms.
        assert_eq!(time.delta_time(), Duration::from_millis(200));
        assert_eq!(time.total_time(), Duration::from_millis(200));
    }

    #[test]
    fn frames_under_max_delta_are_unaffected_by_clamp() {
        let mut time = Time::new();
        time.max_delta = Duration::from_millis(100);

        time.update_with_raw_delta(Duration::from_millis(30));

        assert_eq!(time.delta_time(), Duration::from_millis(30));
        assert_eq!(time.total_time(), Duration::from_millis(30));
    }

    #[test]
    fn paused_advance_freezes_delta_without_catch_up() {
        let mut time = Time::new();
        time.update_with_raw_delta(Duration::from_millis(16));

        time.pause();
        // While paused the wall clock still elapses; delta must stay zero and
        // `total_time` must not accumulate a catch-up debt.
        time.update_with_raw_delta(Duration::from_secs(1));

        assert_eq!(time.delta_time(), Duration::ZERO);
        assert_eq!(time.total_time(), Duration::from_millis(16));
        // The frame counter still advances: a frame happened, it just ran with
        // a frozen clock.
        assert_eq!(time.total_frame(), 2);
    }

    #[test]
    fn resumed_clock_measures_from_resume_instant() {
        let mut time = Time::new();
        time.pause();
        // Simulates frames passing while paused.
        time.update_with_raw_delta(Duration::from_secs(1));
        time.resume();

        // A short frame right after resume must not inherit the paused gap.
        time.update_with_raw_delta(Duration::from_millis(16));

        assert_eq!(time.delta_time(), Duration::from_millis(16));
        assert_eq!(time.total_time(), Duration::from_millis(16));
    }

    #[test]
    fn wall_clock_update_measures_and_clamps() {
        let mut time = Time::new();
        time.max_delta = Duration::from_millis(5);

        // Sleep well past the ceiling: whatever the scheduler does, the raw
        // delta is ≥ 5 ms, so the clamp is guaranteed to be exercised.
        std::thread::sleep(Duration::from_millis(30));
        time.update();

        assert_eq!(time.delta_time(), Duration::from_millis(5));
        assert_eq!(time.total_time(), Duration::from_millis(5));
        assert_eq!(time.total_frame(), 1);
    }
}
