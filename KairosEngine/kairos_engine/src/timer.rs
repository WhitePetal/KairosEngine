use kairos_ecs::resource::Resource;
use std::time::{Duration, Instant};

#[cfg(test)]
mod tests;

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

/// Default fixed timestep: 64 Hz (15 625 µs).
///
/// Mirrors bevy's `Time<Fixed>` default. 64 Hz is preferred over 60 Hz: at
/// common monitor refresh rates 60 Hz risks a pathological alternation between
/// frames that run two fixed steps and frames that run none, and a 64 Hz step
/// is 1/64 s — an exact power of two in seconds that converts losslessly to
/// f32/f64 and to the integer 15 625 µs.
const DEFAULT_FIXED_TIMESTEP: Duration = Duration::from_micros(15_625);

/// The engine's fixed-timestep clock: the kairos shape of bevy's `Time<Fixed>`
/// as a single concrete (non-generic) `World` resource.
///
/// Registered during `install` (see `kairos_editor::schedule`), but nothing
/// advances it yet — the fixed-step driver (accumulate the virtual frame delta
/// each frame, then `expend` until it returns false) lands with the
/// fixed-step scheduling ticket, so engine behavior is unchanged. Unit tests
/// drive it directly.
///
/// Semantics mirror bevy `Time<Fixed>`:
///
/// - [`accumulate`](Self::accumulate) is pure addition onto the `overstep`
///   accumulator — one frame's worth of virtual time, however long;
/// - [`expend`](Self::expend) prepays exactly one step with a `checked_sub`:
///   if less than a full timestep remains it deducts nothing and returns
///   `false`, and the remainder is kept for the next frame;
/// - a successful step reports one timestep as [`delta`](Self::delta) and
///   moves [`elapsed`](Self::elapsed) forward by one timestep. On a frame
///   where no step runs, both keep their last-step values.
#[derive(Resource, Debug)]
pub struct FixedTime {
    /// Virtual time that must accrue before one fixed step runs.
    timestep: Duration,
    /// Accumulated-but-unspent virtual time; the sub-step remainder is never
    /// reset, it carries across frames.
    overstep: Duration,
    /// Time reported for the most recent successful step — exactly one
    /// `timestep`.
    delta: Duration,
    /// Total fixed time elapsed, advanced by one `timestep` per step.
    elapsed: Duration,
}

impl FixedTime {
    /// Returns a new fixed clock at the default 64 Hz timestep with nothing
    /// accumulated and zero delta/elapsed.
    pub fn new() -> Self {
        Self {
            timestep: DEFAULT_FIXED_TIMESTEP,
            overstep: Duration::ZERO,
            delta: Duration::ZERO,
            elapsed: Duration::ZERO,
        }
    }

    /// Returns a new fixed clock with the given [`Duration`] timestep.
    ///
    /// # Panics
    ///
    /// Panics if `timestep` is zero.
    pub fn from_duration(timestep: Duration) -> Self {
        let mut clock = Self::new();
        clock.set_timestep(timestep);
        clock
    }

    /// Returns a new fixed clock with the given timestep in seconds.
    ///
    /// # Panics
    ///
    /// Panics if `seconds` is zero, negative or not finite.
    pub fn from_seconds(seconds: f64) -> Self {
        let mut clock = Self::new();
        clock.set_timestep_seconds(seconds);
        clock
    }

    /// Returns a new fixed clock with the given timestep frequency in Hertz
    /// (steps per second).
    ///
    /// # Panics
    ///
    /// Panics if `hz` is zero, negative or not finite.
    pub fn from_hz(hz: f64) -> Self {
        let mut clock = Self::new();
        clock.set_timestep_hz(hz);
        clock
    }

    /// Returns the amount of virtual time that must pass before the fixed
    /// schedule runs again.
    #[inline(always)]
    pub fn timestep(&self) -> Duration {
        self.timestep
    }

    /// Sets the amount of virtual time that must pass before the fixed
    /// schedule runs again, as a [`Duration`].
    ///
    /// Takes effect for the next run; any leftover `overstep` is processed
    /// under the new timestep.
    ///
    /// # Panics
    ///
    /// Panics if `timestep` is zero.
    pub fn set_timestep(&mut self, timestep: Duration) {
        assert!(
            timestep > Duration::ZERO,
            "attempted to set fixed timestep to zero"
        );
        self.timestep = timestep;
    }

    /// Sets the timestep in seconds.
    ///
    /// # Panics
    ///
    /// Panics if `seconds` is zero, negative or not finite.
    pub fn set_timestep_seconds(&mut self, seconds: f64) {
        assert!(
            seconds.is_finite() && seconds > 0.0,
            "timestep seconds must be positive and finite, got {seconds}"
        );
        self.set_timestep(Duration::from_secs_f64(seconds));
    }

    /// Sets the timestep frequency in Hertz (steps per second).
    ///
    /// The timestep is set to `1 / hz`, converted to a [`Duration`].
    ///
    /// # Panics
    ///
    /// Panics if `hz` is zero, negative or not finite.
    pub fn set_timestep_hz(&mut self, hz: f64) {
        assert!(
            hz.is_finite() && hz > 0.0,
            "timestep hz must be positive and finite, got {hz}"
        );
        self.set_timestep_seconds(1.0 / hz);
    }

    /// Returns the amount of virtual time accumulated toward the next step.
    #[inline(always)]
    pub fn overstep(&self) -> Duration {
        self.overstep
    }

    /// Adds a frame's worth of virtual time to the accumulator — pure
    /// addition, no stepping.
    ///
    /// Mirrors bevy's `Time<Fixed>::accumulate_overstep`; in production the
    /// fixed-step driver calls this once per frame with the virtual clock's
    /// delta.
    pub fn accumulate(&mut self, delta: Duration) {
        self.overstep += delta;
    }

    /// Attempts to consume exactly one timestep from the accumulator.
    ///
    /// Prepaid deduction via `checked_sub`: only a full timestep of overstep
    /// is ever deducted, and never more. On success the remainder (if any) is
    /// kept, [`delta`](Self::delta) reports one timestep, and
    /// [`elapsed`](Self::elapsed) advances by one timestep. Returns `false`
    /// when less than a full timestep remains, deducting nothing.
    pub fn expend(&mut self) -> bool {
        let Some(remaining) = self.overstep.checked_sub(self.timestep) else {
            return false;
        };

        self.overstep = remaining;
        self.delta = self.timestep;
        self.elapsed += self.timestep;
        true
    }

    /// Returns the time reported for the most recent successful step — always
    /// exactly one timestep while steps have run, zero before the first.
    #[inline(always)]
    pub fn delta(&self) -> Duration {
        self.delta
    }

    /// Returns the total fixed time elapsed, advanced by one timestep per
    /// successful step.
    #[inline(always)]
    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }
}

impl Default for FixedTime {
    fn default() -> Self {
        Self::new()
    }
}
