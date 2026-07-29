use crate::ecs::change_detection::MAX_CHANGE_AGE;

/// A value that tracks when a system ran relative to other systems.
/// This is used to power change detection.
///
/// *Note* that a system that hasn't been run yet has a `Tick` of 0.
#[derive(Copy, Clone, Default, Debug, Eq, Hash, PartialEq)]
pub struct Tick {
    tick: u32,
}

impl Tick {
    /// The maximum relative age for a change tick.
    /// The value of this is equal to [`MAX_CHANGE_AGE`].
    ///
    /// Since change detection will not work for any ticks older than this,
    /// ticks are periodically scanned to ensure their relative values are below this.
    pub const MAX: Self = Self::new(MAX_CHANGE_AGE);

    /// Creates a new [`Tick`] wrapping the given value.
    #[inline]
    pub const fn new(tick: u32) -> Self {
        Self { tick }
    }

    /// Gets the value of this change tick.
    #[inline]
    pub const fn get(self) -> u32 {
        self.tick
    }

    /// Sets the value of this change tick.
    #[inline]
    pub fn set(&mut self, tick: u32) {
        self.tick = tick;
    }

    /// Returns `true` if this `Tick` occurred since the system's `last_run`.
    ///
    /// `this_run` is the current tick of the system, used as a reference to help deal with wraparound.
    #[inline]
    pub fn is_newer_than(self, last_run: Tick, this_run: Tick) -> bool {
        // This works even with wraparound because the world tick (`this_run`) is always "newer" than
        // `last_run` and `self.tick`, and we scan periodically to clamp `ComponentTicks` values
        // so they never get older than `u32::MAX` (the difference would overflow).
        //
        // The clamp here ensures determinism (since scans could differ between app runs).
        let ticks_since_insert = this_run.relative_to(self).tick.min(MAX_CHANGE_AGE);
        let ticks_since_system = this_run.relative_to(last_run).tick.min(MAX_CHANGE_AGE);

        ticks_since_system > ticks_since_insert
    }

    /// Returns a change tick representing the relationship between `self` and `other`.
    #[inline]
    pub(crate) fn relative_to(self, other: Self) -> Self {
        let tick = self.tick.wrapping_sub(other.tick);
        Self { tick }
    }

    /// Wraps this change tick's value if it exceeds [`Tick::MAX`].
    ///
    /// Returns `true` if wrapping was performed. Otherwise, returns `false`.
    #[inline]
    pub fn check_tick(&mut self, check: CheckChangeTicks) -> bool {
        let age = check.present_tick().relative_to(*self);
        // This comparison assumes that `age` has not overflowed `u32::MAX` before, which will be true
        // so long as this check always runs before that can happen.
        if age.get() > Self::MAX.get() {
            *self = check.present_tick().relative_to(Self::MAX);
            true
        } else {
            false
        }
    }
}

/// An [`Event`] that can be used to maintain [`Tick`]s in custom data structures, enabling to make
/// use of bevy's periodic checks that clamps ticks to a certain range, preventing overflows and thus
/// keeping methods like [`Tick::is_newer_than`] reliably return `false` for ticks that got too old.
///
/// # Example
///
/// Here a schedule is stored in a custom resource. This way the systems in it would not have their change
/// ticks automatically updated via [`World::check_change_ticks`](crate::world::World::check_change_ticks),
/// possibly causing `Tick`-related bugs on long-running apps.
///
/// To fix that, add an observer for this event that calls the schedule's
/// [`Schedule::check_change_ticks`](bevy_ecs::schedule::Schedule::check_change_ticks).
///
/// ```
/// use bevy_ecs::prelude::*;
/// use bevy_ecs::change_detection::CheckChangeTicks;
///
/// #[derive(Resource)]
/// struct CustomSchedule(Schedule);
///
/// # let mut world = World::new();
/// world.add_observer(|check: On<CheckChangeTicks>, mut schedule: ResMut<CustomSchedule>| {
///     schedule.0.check_change_ticks(*check);
/// });
/// ```
#[derive(Debug, Clone, Copy, Event)]
pub struct CheckChangeTicks(pub(crate) Tick);

impl CheckChangeTicks {
    /// Get the present `Tick` that other ticks get compared to.
    pub fn present_tick(self) -> Tick {
        self.0
    }
}

// TODO!
