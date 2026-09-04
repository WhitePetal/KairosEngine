use std::{any::Any, marker::PhantomData, sync::Mutex};

use concurrent_queue::ConcurrentQueue;
use fixedbitset::FixedBitSet;
use kairos_tasks::Scope;
#[cfg(feature = "trace")]
use tracing::{Span, info_span};

use crate::{
    cell::SyncUnsafeCell, ecs::{
        error::{ErrorContext, ErrorHandler, KairosError}, schedule::{ConditionWithAccess, SystemExecutor, SystemSchedule, SystemWithAccess}, world::{World, unsafe_world_cell::UnsafeWorldCell},
    },
};

/// Borrowed data used by the [`MultiThreadedExecutor`].
struct Environment<'env, 'sys> {
    executor: &'env MultiThreadedExecutor,
    systems: &'sys [SyncUnsafeCell<SystemWithAccess>],
    conditions: SyncUnsafeCell<Conditions<'sys>>,
    world_cell: UnsafeWorldCell<'env>,
}

struct Conditions<'a> {
    system_conditions: &'a mut [Vec<ConditionWithAccess>],
    set_conditions: &'a mut [Vec<ConditionWithAccess>],
    sets_with_conditions_of_systems: &'a [FixedBitSet],
    systems_in_sets_with_conditions: &'a [FixedBitSet],
}

impl<'env, 'sys> Environment<'env, 'sys> {
    fn new(
        executor: &'env MultiThreadedExecutor,
        schedule: &'sys mut SystemSchedule,
        world: &'env mut World,
    ) -> Self {
        Environment {
            executor,
            systems: SyncUnsafeCell::from_mut(schedule.systems.as_mut_slice()).as_slice_of_cells(),
            conditions: SyncUnsafeCell::new(Conditions {
                system_conditions: &mut schedule.system_conditions,
                set_conditions: &mut schedule.set_conditions,
                sets_with_conditions_of_systems: &schedule.sets_with_conditions_of_systems,
                systems_in_sets_with_conditions: &schedule.systems_in_sets_with_conditions,
            }),
            world_cell: world.as_unsafe_world_cell(),
        }
    }
}

/// Per-system data used by the [`MultiThreadedExecutor`].
// Copied here because it can't be read from the system when it's running.
struct SystemTaskMetadata {
    /// The set of systems whose `component_access_set()` conflicts with this one.
    conflicting_systems: FixedBitSet,
    /// The set of systems whose `component_access_set()` conflicts with this system's conditions.
    /// Note that this is separate from `conflicting_systems` to handle the case where
    /// a system is skipped by an earlier system set condition or system stepping,
    /// and needs access to run its conditions but not for itself.
    condition_conflicting_systems: FixedBitSet,
    /// Indices of the systems that directly depend on the system.
    dependents: Vec<usize>,
    /// Is `true` if the system does not access `!Send` data.
    is_send: bool,
    /// Is `true` if the system is exclusive.
    is_exclusive: bool,
}

/// The result of running a system that is sent across a channel.
struct SystemResult {
    system_index: usize,
}

/// Runs the schedule using a thread pool. Non-conflicting systems can run in parallel.
pub struct MultiThreadedExecutor {
    /// The running state, protected by a mutex so that a reference to the executor can be shared across tasks.
    state: Mutex<ExecutorState>,
    /// Queue of system completion events.
    system_completion: ConcurrentQueue<SystemResult>,
    /// Setting when true applies deferred system buffers after all systems have run
    apply_final_deferred: bool,
    /// When set, tells the executor that a thread has panicked.
    painc_payload: Mutex<Option<Box<dyn Any + Send>>>,
    starting_systems: FixedBitSet,
    #[cfg(feature = "trace")]
    executor_span: Span,
}

/// The state of the executor while running.
pub struct ExecutorState {
    /// Metadata for scheduling and running system tasks.
    system_task_metadata: Vec<SystemTaskMetadata>,
    /// The set of systems whose `component_access_set()` conflicts with this system set's conditions.
    set_condition_conflicting_systems: Vec<FixedBitSet>,
    /// Returns `true` if a system with non-`Send` access is running.
    local_thread_running: bool,
    /// Returns `true` if an exclusive system is running.
    exclusive_running: bool,
    /// The number of systems that are running.
    num_running_systems: usize,
    /// The number of dependencies each system has that have not completed.
    num_dependencies_remaining: Vec<usize>,
    /// System sets whose conditions have been evaluated.
    evaluated_sets: FixedBitSet,
    /// Systems that have no remaining dependencies and are waiting to run.
    ready_systems: FixedBitSet,
    /// copy of `ready_systems`
    ready_systems_copy: FixedBitSet,
    /// Systems that are running.
    running_systems: FixedBitSet,
    /// Systems that got skipped.
    skipped_systems: FixedBitSet,
    /// Systems whose conditions have been evaluated and were run or skipped.
    completed_systems: FixedBitSet,
    /// Systems that have run but have not had their buffers applied.
    unapplied_systems: FixedBitSet,
}

struct Context<'scope, 'env, 'sys> {
    environment: &'env Environment<'env, 'sys>,
    scope: &'scope Scope<'scope, 'env, ()>,
    error_handler: ErrorHandler,
}

impl Default for MultiThreadedExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemExecutor for MultiThreadedExecutor {
    fn init(&mut self, schedule: &SystemSchedule) {
        todo!()
    }

    fn run(
        &mut self,
        schedule: &mut SystemSchedule,
        world: &mut World,
        skip_systems: Option<&FixedBitSet>,
        error_handler: fn(KairosError, ErrorContext),
    ) {
        todo!()
    }

    fn set_apply_final_deferred(&mut self, value: bool) {
        todo!()
    }
}

impl MultiThreadedExecutor {
    /// Creates a new `multi_threaded` executor for use with a [`Schedule`].
    ///
    /// [`Schedule`]: crate::schedule::Schedule
    pub fn new() -> Self {
        Self {
            state: Mutex::new(ExecutorState::new()),
            system_completion: ConcurrentQueue::unbounded(),
            starting_systems: FixedBitSet::new(),
            apply_final_deferred: true,
            painc_payload: Mutex::new(None),
            #[cfg(feature = "trace")]
            executor_span: info_span!("multithreaded executor"),
        }
    }
}

impl ExecutorState {
    fn new() -> Self {
        Self {
            system_task_metadata: Vec::new(),
            set_condition_conflicting_systems: Vec::new(),
            num_running_systems: 0,
            num_dependencies_remaining: Vec::new(),
            local_thread_running: false,
            exclusive_running: false,
            evaluated_sets: FixedBitSet::new(),
            ready_systems: FixedBitSet::new(),
            ready_systems_copy: FixedBitSet::new(),
            running_systems: FixedBitSet::new(),
            skipped_systems: FixedBitSet::new(),
            completed_systems: FixedBitSet::new(),
            unapplied_systems: FixedBitSet::new(),
        }
    }
}
