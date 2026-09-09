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
