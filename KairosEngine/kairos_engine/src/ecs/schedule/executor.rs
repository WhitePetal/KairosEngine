use fixedbitset::FixedBitSet;
use sonic_rs::error::ErrorCode;

use crate::ecs::{
    error::KairosError,
    schedule::{ConditionWithAccess, SystemKey, SystemSetKey, SystemWithAccess},
    world::World,
};

mod multi_threaded;

pub use multi_threaded::MultiThreadedExecutor;

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
        error_handler: fn(KairosError, ErrorCode),
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
