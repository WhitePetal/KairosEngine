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

// ---------------------------------------------------------------------------
// FixedTime — the fixed-timestep clock resource (issue #136)
// ---------------------------------------------------------------------------

/// The default clock runs at 64 Hz: a 15 625 µs timestep, nothing accrued,
/// nothing reported.
#[test]
fn fixed_time_defaults_to_64hz_zeroed() {
    let fixed = FixedTime::new();

    assert_eq!(fixed.timestep(), Duration::from_micros(15_625));
    assert_eq!(fixed.overstep(), Duration::ZERO);
    assert_eq!(fixed.delta(), Duration::ZERO);
    assert_eq!(fixed.elapsed(), Duration::ZERO);
}

/// `from_duration` / `from_seconds` / `from_hz` set the timestep the config
/// entry promises; 64 Hz and 128 Hz are exact in every representation.
#[test]
fn fixed_time_config_constructors_set_the_timestep() {
    let from_duration = FixedTime::from_duration(Duration::from_millis(10));
    assert_eq!(from_duration.timestep(), Duration::from_millis(10));

    // 1/64 s = 15 625 µs and 1/128 s = 7 812.5 µs are exact in f64, so the
    // Hz/second constructors must land on the same nanosecond value.
    let from_hz = FixedTime::from_hz(64.0);
    assert_eq!(from_hz.timestep(), Duration::from_micros(15_625));

    let from_seconds = FixedTime::from_seconds(1.0 / 128.0);
    assert_eq!(from_seconds.timestep(), Duration::from_nanos(7_812_500));
}

/// A sub-step frame only adds overstep: no step runs, nothing is deducted,
/// and delta/elapsed stay untouched.
#[test]
fn fixed_time_accumulate_below_a_step_does_not_step() {
    let mut fixed = FixedTime::new();

    fixed.accumulate(Duration::from_micros(10_000));

    assert_eq!(fixed.overstep(), Duration::from_micros(10_000));
    assert!(
        !fixed.expend(),
        "a frame shorter than one timestep must not run a step"
    );
    assert_eq!(
        fixed.overstep(),
        Duration::from_micros(10_000),
        "a failed expend must not deduct from the accumulator"
    );
    assert_eq!(fixed.delta(), Duration::ZERO);
    assert_eq!(fixed.elapsed(), Duration::ZERO);
}

/// One successful `expend` consumes exactly one timestep and reports it as
/// `delta`, advancing `elapsed` by one timestep.
#[test]
fn fixed_time_expend_steps_exactly_one_timestep() {
    let mut fixed = FixedTime::new();

    fixed.accumulate(Duration::from_micros(20_000));

    assert!(fixed.expend());
    assert_eq!(fixed.delta(), Duration::from_micros(15_625));
    assert_eq!(fixed.elapsed(), Duration::from_micros(15_625));
    assert_eq!(fixed.overstep(), Duration::from_micros(4_375));

    assert!(
        !fixed.expend(),
        "a 4 375 µs remainder must not run a second step"
    );
    assert_eq!(fixed.overstep(), Duration::from_micros(4_375));
    assert_eq!(fixed.elapsed(), Duration::from_micros(15_625));
}

/// A single long frame can chain several steps; each step reports one
/// timestep of delta and advances elapsed by exactly that much.
#[test]
fn fixed_time_a_frame_can_chain_multiple_steps() {
    let mut fixed = FixedTime::new();

    fixed.accumulate(Duration::from_millis(50)); // 3 whole 15 625 µs steps + 3 125 µs

    for step in 1..=3u64 {
        assert!(fixed.expend(), "step {step} must succeed");
        assert_eq!(fixed.delta(), Duration::from_micros(15_625));
        assert_eq!(fixed.elapsed(), Duration::from_micros(15_625 * step));
    }
    assert!(!fixed.expend());
    assert_eq!(fixed.overstep(), Duration::from_micros(3_125));
}

/// An overstep remainder below one step is carried across frames — it is
/// never reset, so partial frames keep accruing toward the next step.
#[test]
fn fixed_time_remainder_is_carried_across_frames() {
    let mut fixed = FixedTime::new();

    // Frame 1: 20 ms → 1 step, 4 375 µs carried into the next frame.
    fixed.accumulate(Duration::from_millis(20));
    assert!(fixed.expend());
    assert_eq!(fixed.overstep(), Duration::from_micros(4_375));
    assert_eq!(fixed.elapsed(), Duration::from_micros(15_625));

    // Frame 2: 12 ms → 4 375 + 12 000 = 16 375 µs → 1 step, 750 µs carried.
    fixed.accumulate(Duration::from_millis(12));
    assert!(fixed.expend());
    assert_eq!(fixed.overstep(), Duration::from_micros(750));
    assert_eq!(fixed.elapsed(), Duration::from_micros(31_250));
    assert_eq!(fixed.delta(), Duration::from_micros(15_625));

    // Frame 3: 500 µs → 750 + 500 = 1 250 µs < one step → 0 steps, and the
    // clock does not move: delta/elapsed keep their last-step values.
    fixed.accumulate(Duration::from_micros(500));
    assert!(!fixed.expend());
    assert_eq!(fixed.overstep(), Duration::from_micros(1_250));
    assert_eq!(fixed.elapsed(), Duration::from_micros(31_250));
    assert_eq!(fixed.delta(), Duration::from_micros(15_625));
}

/// `set_timestep` takes effect immediately: the leftover overstep is then
/// processed under the new timestep (bevy parity).
#[test]
fn fixed_time_set_timestep_applies_to_the_leftover_overstep() {
    let mut fixed = FixedTime::new();

    fixed.accumulate(Duration::from_millis(20));
    assert!(fixed.expend()); // 4 375 µs left under the 64 Hz default
    fixed.set_timestep(Duration::from_micros(4_000));

    assert!(
        fixed.expend(),
        "leftover 4 375 µs covers the new 4 000 µs step"
    );
    assert_eq!(fixed.delta(), Duration::from_micros(4_000));
    assert_eq!(fixed.elapsed(), Duration::from_micros(19_625)); // 15 625 + 4 000
    assert_eq!(fixed.overstep(), Duration::from_micros(375));
}

/// A zero timestep is rejected: the fixed clock must always advance.
#[test]
#[should_panic(expected = "attempted to set fixed timestep to zero")]
fn fixed_time_zero_timestep_is_rejected() {
    let _ = FixedTime::from_duration(Duration::ZERO);
}

/// Negative seconds are not a valid timestep.
#[test]
#[should_panic]
fn fixed_time_negative_seconds_are_rejected() {
    let _ = FixedTime::from_seconds(-1.0);
}

/// A zero frequency (infinite step) is not a valid timestep.
#[test]
#[should_panic]
fn fixed_time_zero_hz_is_rejected() {
    let _ = FixedTime::from_hz(0.0);
}

/// Non-finite input (NaN/infinity) is not a valid timestep.
#[test]
#[should_panic]
fn fixed_time_non_finite_input_is_rejected() {
    let mut fixed = FixedTime::new();
    fixed.set_timestep_seconds(f64::NAN);
}
