use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct Time {
    start_time: Instant,
    pre_time: Instant,

    total_time: Duration,
    delta_time: Duration,

    total_frame: u64,

    time_scale: f32,
    paused: bool,
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
        }
    }

    pub fn update(&mut self) {
        let now = Instant::now();
        let raw_delta = now.duration_since(self.pre_time);
        self.pre_time = now;

        self.total_frame = self.total_frame + 1;

        if self.paused {
            self.delta_time = Duration::ZERO;
            return;
        }

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
