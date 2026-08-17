//! Types that detect when their internal data mutate.

mod params;
mod tick;
mod traits;

pub use params::*;
pub use tick::*;

pub use traits::{DetectChanges, DetectChangesMut};

#[cfg(test)]
mod tests;

/// The (arbitrarily chosen) minimum number of world tick increments between `check_tick` scans.
///
/// Change ticks can only be scanned when systems aren't running. Thus, if the threshold is `N`,
/// the maximum is `2 * N - 1` (i.e. the world ticks `N - 1` times, then `N` times).
///
/// If no change is older than `u32::MAX - (2 * N - 1)` following a scan, none of their ages can
/// overflow and cause false positives.
// (518,400,000 = 1000 ticks per frame * 144 frames per second * 3600 seconds per hour)
pub const CHECK_TICK_THRESHOLD: u32 = 518_400_000;

/// The maximum change tick difference that won't overflow before the next `check_tick` scan.
///
/// Changes stop being detected once they become this old.
pub const MAX_CHANGE_AGE: u32 = u32::MAX - (2 * CHECK_TICK_THRESHOLD - 1);
