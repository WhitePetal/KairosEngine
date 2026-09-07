use std::any::TypeId;

use fixedbitset::FixedBitSet;
use kairos_ecs_macros::Resource;
use log::{debug, error, info, warn};
use thiserror::Error;

use crate::{
    collections::{FixedHashMap, TypeIdMap},
    ecs::{
        change_detection::ResMut,
        schedule::{InternedScheduleLabel, NodeId, Schedule, ScheduleLabel, SystemKey},
        system::IntoSystem,
    },
};

#[cfg(all(test, feature = "debug_stepping"))]
#[expect(clippy::print_stdout, reason = "Allowed in tests.")]
mod tests;

#[derive(Debug, Default, PartialEq, Eq, Copy, Clone)]
enum Action {
    /// Stepping is disabled; run all systems
    #[default]
    RunAll,

    /// Stepping is enabled, but we're only running required systems this frame
    Waiting,

    /// Stepping is enabled; run all systems until the end of the frame, or
    /// until we encounter a system marked with [`SystemBehavior::Break`] or all
    /// systems in the frame have run.
    Continue,

    /// stepping is enabled; only run the next system in our step list
    Step,
}

#[derive(Debug, Copy, Clone)]
enum SystemBehavior {
    /// System will always run regardless of stepping action
    AlwaysRun,

    /// System will never run while stepping is enabled
    NeverRun,

    /// When [`Action::Waiting`] this system will not be run
    /// When [`Action::Step`] this system will be stepped
    /// When [`Action::Continue`] system execution will stop before executing
    /// this system unless its the first system run when continuing
    Break,

    /// When [`Action::Waiting`] this system will not be run
    /// When [`Action::Step`] this system will be stepped
    /// When [`Action::Continue`] this system will be run
    Continue,
}

// schedule_order index, and schedule start point
#[derive(Debug, Default, Clone, Copy)]
struct Cursor {
    /// index within `Stepping::schedule_order`
    pub schedule: usize,
    /// index within the schedule's system list
    pub system: usize,
}

// Two methods of referring to Systems, via TypeId, or per-Schedule NodeId
enum SystemIdentifier {
    Type(TypeId),
    Node(NodeId),
}

/// Updates to [`Stepping::schedule_states`] that will be applied at the start
/// of the next render frame
enum Update {
    /// Set the action stepping will perform for this render frame
    SetAction(Action),
    /// Enable stepping for this schedule
    AddSchedule(InternedScheduleLabel),
    /// Disable stepping for this schedule
    RemoveSchedule(InternedScheduleLabel),
    /// Clear any system-specific behaviors for this schedule
    ClearSchedule(InternedScheduleLabel),
    /// Set a system-specific behavior for this schedule & system
    SetBehavior(InternedScheduleLabel, SystemIdentifier, SystemBehavior),
    /// Clear any system-specific behavior for this schedule & system
    ClearBehavior(InternedScheduleLabel, SystemIdentifier),
}

#[derive(Error, Debug)]
#[error("not available until all configured schedules have been run; try again next frame")]
pub struct NotReady;

#[derive(Resource, Default)]
/// Resource for controlling system stepping behavior
pub struct Stepping {
    // [`ScheduleState`] for each [`Schedule`] with stepping enabled
    schedule_states: FixedHashMap<InternedScheduleLabel, ScheduleState>,

    // dynamically generated [`Schedule`] order
    schedule_order: Vec<InternedScheduleLabel>,

    // current position in the stepping frame
    cursor: Cursor,

    // index in [`schedule_order`] of the last schedule to call `skipped_systems()`
    previous_schedule: Option<usize>,

    // Action to perform during this render frame
    action: Action,

    // Updates apply at the start of the next render frame
    updates: Vec<Update>,
}

impl core::fmt::Debug for Stepping {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Stepping {{ action: {:?}, schedules: {:?}, order: {:?}",
            self.action,
            self.schedule_states.keys(),
            self.schedule_order
        )?;
        if self.action != Action::RunAll {
            let Cursor { schedule, system } = self.cursor;
            match self.schedule_order.get(schedule) {
                Some(label) => write!(f, "cursor: {label:?}[{system}], ")?,
                None => write!(f, "cursor: None, ")?,
            };
        }
        write!(f, "}}")
    }
}

impl Stepping {
    /// Create a new instance of the `Stepping` resource.
    pub fn new() -> Self {
        Stepping::default()
    }

    /// System to call denoting that a new render frame has begun
    ///
    /// Note: This system is automatically added to the default `MainSchedule`.
    pub fn begin_frame(stepping: Option<ResMut<Self>>) {
        if let Some(mut stepping) = stepping {
            stepping.next_frame();
        }
    }

    /// Return the list of schedules with stepping enabled in the order
    /// they are executed in.
    pub fn schedules(&self) -> Result<&Vec<InternedScheduleLabel>, NotReady> {
        if self.schedule_order.len() == self.schedule_states.len() {
            Ok(&self.schedule_order)
        } else {
            Err(NotReady)
        }
    }

    /// Return our current position within the stepping frame
    ///
    /// NOTE: This function **will** return `None` during normal execution with
    /// stepping enabled.  This can happen at the end of the stepping frame
    /// after the last system has been run, but before the start of the next
    /// render frame.
    pub fn cursor(&self) -> Option<(InternedScheduleLabel, NodeId)> {
        if self.action == Action::RunAll {
            return None;
        }
        let label = self.schedule_order.get(self.cursor.schedule)?;
        let state = self.schedule_states.get(label)?;
        state
            .node_ids
            .get(self.cursor.system)
            .map(|node_id| (*label, NodeId::System(*node_id)))
    }

    /// Enable stepping for the provided schedule
    pub fn add_schedule(&mut self, schedule: impl ScheduleLabel) -> &mut Self {
        self.updates.push(Update::AddSchedule(schedule.intern()));
        self
    }

    /// Disable stepping for the provided schedule
    ///
    /// NOTE: This function will also clear any system-specific behaviors that
    /// may have been configured.
    pub fn remove_schedule(&mut self, schedule: impl ScheduleLabel) -> &mut Self {
        self.updates.push(Update::RemoveSchedule(schedule.intern()));
        self
    }

    /// Clear behavior set for all systems in the provided [`Schedule`]
    pub fn clear_schedule(&mut self, schedule: impl ScheduleLabel) -> &mut Self {
        self.updates.push(Update::ClearSchedule(schedule.intern()));
        self
    }

    /// Begin stepping at the start of the next frame
    pub fn enable(&mut self) -> &mut Self {
        #[cfg(feature = "debug_stepping")]
        self.updates.push(Update::SetAction(Action::Waiting));
        #[cfg(not(feature = "debug_stepping"))]
        error!(
            "Stepping cannot be enabled; \
            bevy was compiled without the bevy_debug_stepping feature"
        );
        self
    }

    /// Disable stepping, resume normal systems execution
    pub fn disable(&mut self) -> &mut Self {
        self.updates.push(Update::SetAction(Action::RunAll));
        self
    }

    /// Check if stepping is enabled
    pub fn is_enabled(&self) -> bool {
        self.action != Action::RunAll
    }

    /// Run the next system during the next render frame
    ///
    /// NOTE: This will have no impact unless stepping has been enabled
    pub fn step_frame(&mut self) -> &mut Self {
        self.updates.push(Update::SetAction(Action::Step));
        self
    }

    /// Run all remaining systems in the stepping frame during the next render
    /// frame
    ///
    /// NOTE: This will have no impact unless stepping has been enabled
    pub fn continue_frame(&mut self) -> &mut Self {
        self.updates.push(Update::SetAction(Action::Continue));
        self
    }

    /// Ensure this system always runs when stepping is enabled
    ///
    /// Note: if the system is run multiple times in the [`Schedule`], this
    /// will apply for all instances of the system.
    pub fn always_run<Marker>(
        &mut self,
        schedule: impl ScheduleLabel,
        system: impl IntoSystem<(), (), Marker>,
    ) -> &mut Self {
        let type_id = system.system_type_id();
        self.updates.push(Update::SetBehavior(
            schedule.intern(),
            SystemIdentifier::Type(type_id),
            SystemBehavior::AlwaysRun,
        ));

        self
    }

    /// Ensure this system instance always runs when stepping is enabled
    pub fn always_run_node(&mut self, schedule: impl ScheduleLabel, node: NodeId) -> &mut Self {
        self.updates.push(Update::SetBehavior(
            schedule.intern(),
            SystemIdentifier::Node(node),
            SystemBehavior::AlwaysRun,
        ));
        self
    }

    /// Ensure this system never runs when stepping is enabled
    pub fn never_run<Marker>(
        &mut self,
        schedule: impl ScheduleLabel,
        system: impl IntoSystem<(), (), Marker>,
    ) -> &mut Self {
        let type_id = system.system_type_id();
        self.updates.push(Update::SetBehavior(
            schedule.intern(),
            SystemIdentifier::Type(type_id),
            SystemBehavior::NeverRun,
        ));

        self
    }

    /// Ensure this system instance never runs when stepping is enabled
    pub fn never_run_node(&mut self, schedule: impl ScheduleLabel, node: NodeId) -> &mut Self {
        self.updates.push(Update::SetBehavior(
            schedule.intern(),
            SystemIdentifier::Node(node),
            SystemBehavior::NeverRun,
        ));
        self
    }

    /// Add a breakpoint for system
    pub fn set_breakpoint<Marker>(
        &mut self,
        schedule: impl ScheduleLabel,
        system: impl IntoSystem<(), (), Marker>,
    ) -> &mut Self {
        let type_id = system.system_type_id();
        self.updates.push(Update::SetBehavior(
            schedule.intern(),
            SystemIdentifier::Type(type_id),
            SystemBehavior::Break,
        ));

        self
    }

    /// Add a breakpoint for system instance
    pub fn set_breakpoint_node(&mut self, schedule: impl ScheduleLabel, node: NodeId) -> &mut Self {
        self.updates.push(Update::SetBehavior(
            schedule.intern(),
            SystemIdentifier::Node(node),
            SystemBehavior::Break,
        ));
        self
    }

    /// Clear a breakpoint for the system
    pub fn clear_breakpoint<Marker>(
        &mut self,
        schedule: impl ScheduleLabel,
        system: impl IntoSystem<(), (), Marker>,
    ) -> &mut Self {
        self.clear_system(schedule, system);

        self
    }

    /// clear a breakpoint for system instance
    pub fn clear_breakpoint_node(
        &mut self,
        schedule: impl ScheduleLabel,
        node: NodeId,
    ) -> &mut Self {
        self.clear_node(schedule, node);
        self
    }

    /// Clear any behavior set for the system
    pub fn clear_system<Marker>(
        &mut self,
        schedule: impl ScheduleLabel,
        system: impl IntoSystem<(), (), Marker>,
    ) -> &mut Self {
        let type_id = system.system_type_id();
        self.updates.push(Update::ClearBehavior(
            schedule.intern(),
            SystemIdentifier::Type(type_id),
        ));

        self
    }

    /// clear a breakpoint for system instance
    pub fn clear_node(&mut self, schedule: impl ScheduleLabel, node: NodeId) -> &mut Self {
        self.updates.push(Update::ClearBehavior(
            schedule.intern(),
            SystemIdentifier::Node(node),
        ));
        self
    }

    /// lookup the first system for the supplied schedule index
    fn first_system_index_for_schedule(&self, index: usize) -> usize {
        let Some(label) = self.schedule_order.get(index) else {
            return 0;
        };
        let Some(state) = self.schedule_states.get(label) else {
            return 0;
        };
        state.first.unwrap_or(0)
    }

    /// Move the cursor to the start of the first schedule
    fn reset_cursor(&mut self) {
        self.cursor = Cursor {
            schedule: 0,
            system: self.first_system_index_for_schedule(0),
        };
    }

    /// Advance schedule states for the next render frame
    fn next_frame(&mut self) {
        // if stepping is enabled; reset our internal state for the start of
        // the next frame
        if self.action != Action::RunAll {
            self.action = Action::Waiting;
            self.previous_schedule = None;

            // if the cursor passed the last schedule, reset it
            if self.cursor.schedule >= self.schedule_order.len() {
                self.reset_cursor();
            }
        }

        if self.updates.is_empty() {
            return;
        }

        let mut reset_cursor = false;
        for update in self.updates.drain(..) {
            match update {
                Update::SetAction(Action::RunAll) => {
                    self.action = Action::RunAll;
                    reset_cursor = true;
                }
                Update::SetAction(action) => {
                    // This match block is really just to filter out invalid
                    // transitions, and add debugging messages for permitted
                    // transitions.  Any action transition that falls through
                    // this match block will be performed.
                    #[expect(
                        clippy::match_same_arms,
                        reason = "Readability would be negatively impacted by combining the `(Waiting, RunAll)` and `(Continue, RunAll)` match arms."
                    )]
                    match (self.action, action) {
                        // ignore non-transition updates, and prevent a call to
                        // enable() from overwriting a step or continue call
                        (Action::RunAll, Action::RunAll)
                        | (Action::Waiting, Action::Waiting)
                        | (Action::Continue, Action::Continue)
                        | (Action::Step, Action::Step)
                        | (Action::Continue, Action::Waiting)
                        | (Action::Step, Action::Waiting) => continue,

                        // when stepping is disabled
                        (Action::RunAll, Action::Waiting) => info!("enabled stepping"),
                        (Action::RunAll, _) => {
                            warn!(
                                "stepping not enabled; call Stepping::enable() \
                                before step_frame() or continue_frame()"
                            );
                            continue;
                        }

                        // stepping enabled; waiting
                        (Action::Waiting, Action::RunAll) => info!("disabled stepping"),
                        (Action::Waiting, Action::Continue) => info!("continue frame"),
                        (Action::Waiting, Action::Step) => info!("step frame"),

                        // stepping enabled; continue frame
                        (Action::Continue, Action::RunAll) => info!("disabled stepping"),
                        (Action::Continue, Action::Step) => {
                            warn!("ignoring step_frame(); already continuing next frame");
                            continue;
                        }

                        // stepping enabled; step frame
                        (Action::Step, Action::RunAll) => info!("disabled stepping"),
                        (Action::Step, Action::Continue) => {
                            warn!("ignoring continue_frame(); already stepping next frame");
                            continue;
                        }
                    }

                    // permitted action transition; make the change
                    self.action = action;
                }
                Update::AddSchedule(l) => {
                    self.schedule_states.insert(l, ScheduleState::default());
                }
                Update::RemoveSchedule(label) => {
                    self.schedule_states.remove(&label);
                    if let Some(index) = self.schedule_order.iter().position(|l| l == &label) {
                        self.schedule_order.remove(index);
                    }
                    reset_cursor = true;
                }
                Update::ClearSchedule(label) => match self.schedule_states.get_mut(&label) {
                    Some(state) => state.clear_behaviors(),
                    None => {
                        warn!(
                            "stepping is not enabled for schedule {label:?}; \
                            use `.add_stepping({label:?})` to enable stepping"
                        );
                    }
                },
                Update::SetBehavior(label, system, behavior) => {
                    match self.schedule_states.get_mut(&label) {
                        Some(state) => state.set_behavior(system, behavior),
                        None => {
                            warn!(
                                "stepping is not enabled for schedule {label:?}; \
                                use `.add_stepping({label:?})` to enable stepping"
                            );
                        }
                    }
                }
                Update::ClearBehavior(label, system) => {
                    match self.schedule_states.get_mut(&label) {
                        Some(state) => state.clear_behavior(system),
                        None => {
                            warn!(
                                "stepping is not enabled for schedule {label:?}; \
                                use `.add_stepping({label:?})` to enable stepping"
                            );
                        }
                    }
                }
            }
        }

        if reset_cursor {
            self.reset_cursor();
        }
    }

    /// get the list of systems this schedule should skip for this render
    /// frame
    pub fn skipped_systems(&mut self, schedule: &Schedule) -> Option<FixedBitSet> {
        if self.action == Action::RunAll {
            return None;
        }

        // grab the label and state for this schedule
        let label = schedule.label();
        let state = self.schedule_states.get_mut(&label)?;

        // Stepping is enabled, and this schedule is supposed to be stepped.
        //
        // We need to maintain a list of schedules in the order that they call
        // this function. We'll check the ordered list now to see if this
        // schedule is present. If not, we'll add it after the last schedule
        // that called this function. Finally we want to save off the index of
        // this schedule in the ordered schedule list. This is used to
        // determine if this is the schedule the cursor is pointed at.
        let index = self.schedule_order.iter().position(|l| *l == label);
        let index = match (index, self.previous_schedule) {
            (Some(index), _) => index,
            (None, None) => {
                self.schedule_order.insert(0, label);
                0
            }
            (None, Some(last)) => {
                self.schedule_order.insert(last + 1, label);
                last + 1
            }
        };
        // Update the index of the previous schedule to be the index of this
        // schedule for the next call
        self.previous_schedule = Some(index);

        #[cfg(test)]
        debug!(
            "cursor {:?}, index {}, label {:?}",
            self.cursor, index, label
        );

        // if the stepping frame cursor is pointing at this schedule, we'll run
        // the schedule with the current stepping action.  If this is not the
        // cursor schedule, we'll run the schedule with the waiting action.
        let cursor = self.cursor;
        let (skip_list, next_system) = if index == cursor.schedule {
            let (skip_list, next_system) =
                state.skipped_systems(schedule, cursor.system, self.action);

            // if we just stepped this schedule, then we'll switch the action
            // to be waiting
            if self.action == Action::Step {
                self.action = Action::Waiting;
            }
            (skip_list, next_system)
        } else {
            // we're not supposed to run any systems in this schedule, so pull
            // the skip list, but ignore any changes it makes to the cursor.
            let (skip_list, _) = state.skipped_systems(schedule, 0, Action::Waiting);
            (skip_list, Some(cursor.system))
        };

        // update the stepping frame cursor based on if there are any systems
        // remaining to be run in the schedule
        // Note: Don't try to detect the end of the render frame here using the
        // schedule index.  We don't know all schedules have been added to the
        // schedule_order, so only next_frame() knows its safe to reset the
        // cursor.
        match next_system {
            Some(i) => self.cursor.system = i,
            None => {
                let index = cursor.schedule + 1;
                self.cursor = Cursor {
                    schedule: index,
                    system: self.first_system_index_for_schedule(index),
                };

                #[cfg(test)]
                debug!("advanced schedule index: {} -> {}", cursor.schedule, index);
            }
        }

        Some(skip_list)
    }
}

#[derive(Default)]
struct ScheduleState {
    /// per-system [`SystemBehavior`]
    behaviors: FixedHashMap<NodeId, SystemBehavior>,

    /// order of [`NodeId`]s in the schedule
    ///
    /// This is a cached copy of `SystemExecutable::system_ids`. We need it
    /// available here to be accessed by [`Stepping::cursor()`] so we can return
    /// [`NodeId`]s to the caller.
    node_ids: Vec<SystemKey>,

    /// changes to system behavior that should be applied the next time
    /// [`ScheduleState::skipped_systems()`] is called
    behavior_updates: TypeIdMap<Option<SystemBehavior>>,

    /// This field contains the first steppable system in the schedule.
    first: Option<usize>,
}

impl ScheduleState {
    // set the stepping behavior for a system in this schedule
    fn set_behavior(&mut self, system: SystemIdentifier, behavior: SystemBehavior) {
        self.first = None;
        match system {
            SystemIdentifier::Node(node_id) => {
                self.behaviors.insert(node_id, behavior);
            }
            // Behaviors are indexed by NodeId, but we cannot map a system
            // TypeId to a NodeId without the `Schedule`.  So queue this update
            // to be processed the next time `skipped_systems()` is called.
            SystemIdentifier::Type(type_id) => {
                self.behavior_updates.insert(type_id, Some(behavior));
            }
        }
    }

    // clear the stepping behavior for a system in this schedule
    fn clear_behavior(&mut self, system: SystemIdentifier) {
        self.first = None;
        match system {
            SystemIdentifier::Node(node_id) => {
                self.behaviors.remove(&node_id);
            }
            // queue TypeId updates to be processed later when we have Schedule
            SystemIdentifier::Type(type_id) => {
                self.behavior_updates.insert(type_id, None);
            }
        }
    }

    // clear all system behaviors
    fn clear_behaviors(&mut self) {
        self.behaviors.clear();
        self.behavior_updates.clear();
        self.first = None;
    }

    // apply system behavior updates by looking up the node id of the system in
    // the schedule, and updating `systems`
    fn apply_behavior_updates(&mut self, schedule: &Schedule) {
        // Systems may be present multiple times within a schedule, so we
        // iterate through all systems in the schedule, and check our behavior
        // updates for the system TypeId.
        // PERF: If we add a way to efficiently query schedule systems by their TypeId, we could remove the full
        // system scan here
        for (key, system) in schedule.systems().unwrap() {
            let behavior = self.behavior_updates.get(&system.system_type());
            match behavior {
                None => continue,
                Some(None) => {
                    self.behaviors.remove(&NodeId::System(key));
                }
                Some(Some(behavior)) => {
                    self.behaviors.insert(NodeId::System(key), *behavior);
                }
            }
        }
        self.behavior_updates.clear();

        #[cfg(test)]
        debug!("apply_updates(): {:?}", self.behaviors);
    }

    fn skipped_systems(
        &mut self,
        schedule: &Schedule,
        start: usize,
        mut action: Action,
    ) -> (FixedBitSet, Option<usize>) {
        use core::cmp::Ordering;

        // if our NodeId list hasn't been populated, copy it over from the
        // schedule
        if self.node_ids.len() != schedule.systems_len() {
            self.node_ids.clone_from(&schedule.executable().system_ids);
        }

        // Now that we have the schedule, apply any pending system behavior
        // updates.  The schedule is required to map from system `TypeId` to
        // `NodeId`.
        if !self.behavior_updates.is_empty() {
            self.apply_behavior_updates(schedule);
        }

        // if we don't have a first system set, set it now
        if self.first.is_none() {
            for (i, (key, _)) in schedule.systems().unwrap().enumerate() {
                match self.behaviors.get(&NodeId::System(key)) {
                    Some(SystemBehavior::AlwaysRun | SystemBehavior::NeverRun) => continue,
                    Some(_) | None => {
                        self.first = Some(i);
                        break;
                    }
                }
            }
        }

        let mut skip = FixedBitSet::with_capacity(schedule.systems_len());
        let mut pos = start;

        for (i, (key, _system)) in schedule.systems().unwrap().enumerate() {
            let behavior = self
                .behaviors
                .get(&NodeId::System(key))
                .unwrap_or(&SystemBehavior::Continue);

            #[cfg(test)]
            debug!(
                "skipped_systems(): systems[{}], pos {}, Action::{:?}, Behavior::{:?}, {}",
                i,
                pos,
                action,
                behavior,
                _system.name()
            );

            match (action, behavior) {
                // regardless of which action we're performing, if the system
                // is marked as NeverRun, add it to the skip list.
                // Also, advance the cursor past this system if it is our
                // current position
                (_, SystemBehavior::NeverRun) => {
                    skip.insert(i);
                    if i == pos {
                        pos += 1;
                    }
                }
                // similarly, ignore any system marked as AlwaysRun; they should
                // never be added to the skip list
                // Also, advance the cursor past this system if it is our
                // current position
                (_, SystemBehavior::AlwaysRun) => {
                    if i == pos {
                        pos += 1;
                    }
                }
                // if we're waiting, no other systems besides AlwaysRun should
                // be run, so add systems to the skip list
                (Action::Waiting, _) => skip.insert(i),

                // If we're stepping, the remaining behaviors don't matter,
                // we're only going to run the system at our cursor.  Any system
                // prior to the cursor is skipped.  Once we encounter the system
                // at the cursor, we'll advance the cursor, and set behavior to
                // Waiting to skip remaining systems.
                (Action::Step, _) => match i.cmp(&pos) {
                    Ordering::Less => skip.insert(i),
                    Ordering::Equal => {
                        pos += 1;
                        action = Action::Waiting;
                    }
                    Ordering::Greater => unreachable!(),
                },
                // If we're continuing, and the step behavior is continue, we
                // want to skip any systems prior to our start position.  That's
                // where the stepping frame left off last time we ran anything.
                (Action::Continue, SystemBehavior::Continue) => {
                    if i < start {
                        skip.insert(i);
                    }
                }
                // If we're continuing, and we encounter a breakpoint we may
                // want to stop before executing the system.  To do this we
                // skip this system and set the action to Waiting.
                //
                // Note: if the cursor is pointing at this system, we will run
                // it anyway.  This allows the user to continue, hit a
                // breakpoint, then continue again to run the breakpoint system
                // and any following systems.
                (Action::Continue, SystemBehavior::Break) => {
                    if i != start {
                        skip.insert(i);

                        // stop running systems if the breakpoint isn't the
                        // system under the cursor.
                        if i > start {
                            action = Action::Waiting;
                        }
                    }
                }
                // should have never gotten into this method if stepping is
                // disabled
                (Action::RunAll, _) => unreachable!(),
            }

            // If we're at the cursor position, and not waiting, advance the
            // cursor.
            if i == pos && action != Action::Waiting {
                pos += 1;
            }
        }

        // output is the skip list, and the index of the next system to run in
        // this schedule.
        if pos >= schedule.systems_len() {
            (skip, None)
        } else {
            (skip, Some(pos))
        }
    }
}
