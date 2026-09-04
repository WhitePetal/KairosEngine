use std::any::TypeId;

use fixedbitset::FixedBitSet;

use crate::{
    debug::DebugName,
    ecs::{
        change_detection::{CheckChangeTicks, Tick},
        error::{ErrorContext, KairosError},
        query::FilteredAccessSet,
        schedule::{
            ConditionWithAccess, InternedSystemSet, IntoSystemSet, SystemKey, SystemSet,
            SystemSetKey, SystemTypeSet, SystemWithAccess,
        },
        system::{RunSystemError, System, SystemIn, SystemStateFlags},
        world::{DeferredWorld, World, unsafe_world_cell::UnsafeWorldCell},
    },
};

mod multi_threaded;

pub use multi_threaded::MultiThreadedExecutor;

#[cfg(test)]
mod tests;

/// Types that can run a [`SystemSchedule`] on a [`World`].
pub trait SystemExecutor: Send + Sync {
    /// Called once after the schedule is built or rebuilt.
    fn init(&mut self, schedule: &SystemSchedule);

    /// Runs the systems in the schedule.
    fn run(
        &mut self,
        schedule: &mut SystemSchedule,
        world: &mut World,
        skip_systems: Option<&FixedBitSet>,
        error_handler: fn(KairosError, ErrorContext),
    );

    /// Sets whether deferred system buffers should be applied after all systems have run.
    fn set_apply_final_deferred(&mut self, value: bool);
}

/// Returns the default executor for the current platform.
///
/// On Wasm or when the `multi_threaded` feature is disabled, this returns a
/// [`SingleThreadedExecutor`]. Otherwise it returns a [`MultiThreadedExecutor`].
pub fn default_executor() -> Box<dyn SystemExecutor> {
    Box::new(MultiThreadedExecutor::new())
}

/// Holds systems and conditions of a [`Schedule`](super::Schedule) sorted in topological order
/// (along with dependency information for `multi_threaded` execution).
///
/// Since the arrays are sorted in the same order, elements are referenced by their index.
/// [`FixedBitSet`] is used as a smaller, more efficient substitute of `HashSet<usize>`.
#[derive(Default)]
pub struct SystemSchedule {
    /// List of system node ids.
    pub(super) system_ids: Vec<SystemKey>,
    /// Indexed by system node id.
    pub(super) systems: Vec<SystemWithAccess>,
    /// Indexed by system node id.
    pub(super) system_conditions: Vec<Vec<ConditionWithAccess>>,
    /// Indexed by system node id.
    /// Number of systems that the system immediately depends on.
    pub(super) system_dependencies: Vec<usize>,
    /// Indexed by system node id.
    /// List of systems that immediately depend on the system.
    pub(super) system_dependents: Vec<Vec<usize>>,
    /// Indexed by system node id.
    /// List of sets containing the system that have conditions
    pub(super) sets_with_conditions_of_systems: Vec<FixedBitSet>,
    /// List of system set node ids.
    pub(super) set_ids: Vec<SystemSetKey>,
    /// Indexed by system set node id.
    pub(super) set_conditions: Vec<Vec<ConditionWithAccess>>,
    /// Indexed by system set node id.
    /// List of systems that are in sets that have conditions.
    ///
    /// If a set doesn't run because of its conditions, this is used to skip all systems in it.
    pub(super) systems_in_sets_with_conditions: Vec<FixedBitSet>,
}

impl SystemSchedule {
    /// Creates an empty [`SystemSchedule`].
    pub const fn new() -> Self {
        Self {
            systems: Vec::new(),
            system_conditions: Vec::new(),
            set_conditions: Vec::new(),
            system_ids: Vec::new(),
            set_ids: Vec::new(),
            system_dependencies: Vec::new(),
            system_dependents: Vec::new(),
            sets_with_conditions_of_systems: Vec::new(),
            systems_in_sets_with_conditions: Vec::new(),
        }
    }
}

/// A special [`System`] that instructs the executor to call
/// [`System::apply_deferred`] on the systems that have run but not applied
/// their [`Deferred`] system parameters (like [`Commands`]) or other system buffers.
///
/// ## Scheduling
///
/// `ApplyDeferred` systems are scheduled *by default*
/// - later in the same schedule run (for example, if a system with `Commands` param
///   is scheduled in `Update`, all the changes will be visible in `PostUpdate`)
/// - between systems with dependencies if the dependency [has deferred buffers]
///   (if system `bar` directly or indirectly depends on `foo`, and `foo` uses
///   `Commands` param, changes to the world in `foo` will be visible in `bar`)
///
/// ## Notes
/// - This system (currently) does nothing if it's called manually or wrapped
///   inside a [`PipeSystem`].
/// - Modifying a [`Schedule`] may change the order buffers are applied.
///
/// [`System::apply_deferred`]: crate::system::System::apply_deferred
/// [`Deferred`]: crate::system::Deferred
/// [`Commands`]: crate::prelude::Commands
/// [has deferred buffers]: crate::system::System::has_deferred
/// [`PipeSystem`]: crate::system::PipeSystem
/// [`Schedule`]: super::Schedule
#[doc(alias = "apply_system_buffers")]
pub struct ApplyDeferred;

/// Returns `true` if the [`System`] is an instance of [`ApplyDeferred`].
pub(super) fn is_apply_deferred(system: &dyn System<In = (), Out = ()>) -> bool {
    system.system_type() == TypeId::of::<ApplyDeferred>()
}

impl System for ApplyDeferred {
    type In = ();
    type Out = ();

    fn name(&self) -> DebugName {
        DebugName::borrowed("kairos_ecs::apply_deferred")
    }

    fn flags(&self) -> SystemStateFlags {
        // non-send , exclusive , no deferred
        SystemStateFlags::NON_SEND | SystemStateFlags::EXCLUSIVE
    }

    unsafe fn run_unsafe(
        &mut self,
        _input: SystemIn<'_, Self>,
        _world: UnsafeWorldCell,
    ) -> Result<Self::Out, RunSystemError> {
        // This system does nothing on its own. The executor will apply deferred
        // commands from other systems instead of running this system.
        Ok(())
    }

    #[cfg(feature = "hotpatching")]
    #[inline]
    fn refresh_hotpatch(&mut self) {}

    fn run(
        &mut self,
        _input: SystemIn<'_, Self>,
        _world: &mut World,
    ) -> Result<Self::Out, RunSystemError> {
        // This system does nothing on its own. The executor will apply deferred
        // commands from other systems instead of running this system.
        Ok(())
    }

    fn apply_deferred(&mut self, _world: &mut World) {}

    fn queue_deferred(&mut self, _world: DeferredWorld) {}

    fn initialize(&mut self, _world: &mut World) -> FilteredAccessSet {
        FilteredAccessSet::new()
    }

    fn check_change_tick(&mut self, _check: CheckChangeTicks) {}

    fn default_system_sets(&self) -> Vec<InternedSystemSet> {
        vec![SystemTypeSet::<Self>::new().intern()]
    }

    fn get_last_run(&self) -> Tick {
        // This system is never run, so it has no last run tick.
        Tick::MAX
    }

    fn set_last_run(&mut self, _last_run: Tick) {}
}

impl IntoSystemSet<()> for ApplyDeferred {
    type Set = SystemTypeSet<Self>;

    fn into_system_set(self) -> Self::Set {
        SystemTypeSet::<Self>::new()
    }
}

// TODO!
