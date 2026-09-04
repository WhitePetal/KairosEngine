use std::{any::TypeId, collections::BTreeSet};

use indexmap::IndexMap;
use kairos_ecs_macros::{Resource, ScheduleLabel};

use crate::{
    collections::{FixedHashMap, FixedHashSet},
    ecs::{
        component::ComponentId,
        schedule::{
            ConflictingSystems, InternedScheduleLabel, NodeId, ScheduleLabel, SystemExecutor,
            SystemSchedule, SystemSetKey, SystemSets, Systems, default_executor,
            graph::{
                Dag, DagGroups,
                Direction::{Incoming, Outgoing},
                UnGraph,
            },
            pass::ScheduleBuildPassObj,
        },
        system::System,
    },
    hash::FixedHasher,
};

/// Resource that stores [`Schedule`]s mapped to [`ScheduleLabel`]s excluding the current running [`Schedule`].
#[derive(Default, Resource)]
pub struct Schedules {
    inner: FixedHashMap<InternedScheduleLabel, Schedule>,
    /// List of [`ComponentId`]s to ignore when reporting system order ambiguity conflicts
    pub ingnored_scheduling_ambiguities: BTreeSet<ComponentId>,
    /// Set of schedule labels that have been removed to execute in [`World::try_schedule_scope`].
    temporarily_removed: FixedHashSet<InternedScheduleLabel>,
    /// Set of schedule labels that have attempted to be read in [`World::try_schedule_scope`],
    /// but have no associated [`Schedule`] in `inner`
    empty_labels: FixedHashSet<InternedScheduleLabel>,
}

impl Schedules {
    /// Constructs an empty `Schedules` with zero initial capacity.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a labeled schedule into the map.
    ///
    /// If the map already had an entry for `label`, `schedule` is inserted,
    /// and the old schedule is returned. Otherwise, `None` is returned.
    pub fn insert(&mut self, schedule: Schedule) -> Option<Schedule> {
        self.temporarily_removed.remove(&schedule.label);
        // error if above is true
        self.inner.insert(schedule.label, schedule)
    }

    /// Inserts a labeled schedule into the map.
    ///
    /// If the map already had an entry for `label`, `schedule` is inserted,
    /// and the old schedule is returned. Otherwise, `None` is returned.
    pub fn reinsert(&mut self, schedule: Schedule) -> Option<Schedule> {
        self.temporarily_removed.remove(&schedule.label);
        // error if above false
        self.inner.insert(schedule.label, schedule)
    }

    /// Removes the schedule corresponding to the `label` from the map, returning it if it existed.
    pub fn remove(&mut self, label: impl ScheduleLabel) -> Option<Schedule> {
        self.inner.remove(&label.intern())
    }

    /// Removes the schedule corresponding to the `label` from the map, returning it if it existed, tracks.
    pub fn remove_temporarily(&mut self, label: impl ScheduleLabel) -> Option<Schedule> {
        let label = label.intern();
        let k = self.inner.remove(&label);
        if k.is_some() {
            self.temporarily_removed.insert(label);
            // error if above false
            self.empty_labels.remove(&label);
        } else {
            self.empty_labels.insert(label);
        }
        k
    }

    /// Removes the (schedule, label) pair corresponding to the `label` from the map, returning it if it existed.
    pub fn remove_entry(
        &mut self,
        label: impl ScheduleLabel,
    ) -> Option<(InternedScheduleLabel, Schedule)> {
        self.inner.remove_entry(&label.intern())
    }

    /// Gets a set of temporarily removed schedules
    pub fn get_temporarily_removed(&self) -> FixedHashSet<InternedScheduleLabel> {
        self.temporarily_removed.clone()
    }

    /// Gets a set of empty schedule labels
    pub fn get_empty_labels(&self) -> FixedHashSet<InternedScheduleLabel> {
        self.empty_labels.clone()
    }

    /// Does a schedule with the provided label already exist?
    pub fn contains(&self, lable: impl ScheduleLabel) -> bool {
        self.inner.contains_key(&lable.intern())
    }

    /// Returns a reference to the schedule associated with `label`, if it exists.
    pub fn get(&self, label: impl ScheduleLabel) -> Option<&Schedule> {
        self.inner.get(&label.intern())
    }

    /// Returns a mutable reference to the schedule associated with `label`, if it exists.
    pub fn get_mut(&mut self, label: impl ScheduleLabel) -> Option<&mut Schedule> {
        self.inner.get_mut(&label.intern())
    }

    /// Returns a mutable reference to the schedules associated with `label`, creating one if it doesn't already exist.
    pub fn entry(&mut self, label: impl ScheduleLabel) -> &mut Schedule {
        self.inner
            .entry(label.intern())
            .or_insert_with(|| Schedule::new(label))
    }
}

/// A collection of systems, and the metadata and executor needed to run them
/// in a certain order under certain conditions.
///
/// # Schedule labels
///
/// Each schedule has a [`ScheduleLabel`] value. This value is used to uniquely identify the
/// schedule when added to a [`World`]’s [`Schedules`], and may be used to specify which schedule
/// a system should be added to.
///
/// # Example
///
/// Here is an example of a `Schedule` running a "Hello world" system:
///
/// ```
/// # use bevy_ecs::prelude::*;
/// fn hello_world() { println!("Hello world!") }
///
/// fn main() {
///     let mut world = World::new();
///     let mut schedule = Schedule::default();
///     schedule.add_systems(hello_world);
///
///     schedule.run(&mut world);
/// }
/// ```
///
/// A schedule can also run several systems in an ordered way:
///
/// ```
/// # use bevy_ecs::prelude::*;
/// fn system_one() { println!("System 1 works!") }
/// fn system_two() { println!("System 2 works!") }
/// fn system_three() { println!("System 3 works!") }
///
/// fn main() {
///     let mut world = World::new();
///     let mut schedule = Schedule::default();
///     schedule.add_systems((
///         system_two,
///         system_one.before(system_two),
///         system_three.after(system_two),
///     ));
///
///     schedule.run(&mut world);
/// }
/// ```
///
/// Schedules are often inserted into a [`World`] and identified by their [`ScheduleLabel`] only:
///
/// ```
/// # use bevy_ecs::prelude::*;
/// use bevy_ecs::schedule::ScheduleLabel;
///
/// // Declare a new schedule label.
/// #[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash, Default)]
/// struct Update;
///
/// // This system shall be part of the schedule.
/// fn an_update_system() {
///     println!("Hello world!");
/// }
///
/// fn main() {
///     let mut world = World::new();
///
///     // Add a system to the schedule with that label (creating it automatically).
///     world.get_resource_or_init::<Schedules>().add_systems(Update, an_update_system);
///
///     // Run the schedule, and therefore run the system.
///     world.run_schedule(Update);
/// }
/// ```
pub struct Schedule {
    label: InternedScheduleLabel,
    graph: ScheduleGraph,
    executable: SystemSchedule,
    executor: Box<dyn SystemExecutor>,
    executor_initialized: bool,
}

#[derive(ScheduleLabel, Hash, PartialEq, Eq, Debug, Clone)]
struct DefaultSchedule;

impl Schedule {
    pub fn new(label: impl ScheduleLabel) -> Self {
        /// Constructs an empty `Schedule`.
        let mut this = Self {
            label: label.intern(),
            graph: ScheduleGraph::new(),
            executable: SystemSchedule::new(),
            executor: default_executor(),
            executor_initialized: false,
        };
        // Call `set_build_settings` to add any default build passes
        this.set_build_settings(Default::default());
        this
    }

    pub fn set_build_settings(&mut self, settings: ScheduleBuildSettings) -> &mut Self {
        todo!()
    }
}

/// Metadata for a [`Schedule`].
///
/// The order isn't optimized; calling `ScheduleGraph::build_schedule` will return a
/// `SystemSchedule` where the order is optimized for execution.
#[derive(Default)]
pub struct ScheduleGraph {
    /// Container of systems in the schedule.
    pub systems: Systems,
    /// Container of system sets in the schedule.
    pub system_sets: SystemSets,
    /// Container of system sets in the schedule.
    hierarchy: Dag<NodeId>,
    /// Directed acyclic graph of the hierarchy (which systems/sets are children of which sets)
    dependency: Dag<NodeId>,
    /// Map of systems in each set
    set_systems: DagGroups<SystemSetKey, SystemSetKey>,
    ambiguous_with: UnGraph<NodeId>,
    /// Nodes that are allowed to have ambiguous ordering relationship with any other systems.0
    pub ambiguous_with_all: FixedHashSet<NodeId>,
    conflicting_systems: ConflictingSystems,
    anonymous_sets: usize,
    changed: bool,
    settings: ScheduleBuildSettings,
    passes: IndexMap<TypeId, Box<dyn ScheduleBuildPassObj>, FixedHasher>,
}

impl ScheduleGraph {
    /// Creates an empty [`ScheduleGraph`] with default settings.
    pub fn new() -> Self {
        Self {
            systems: Systems::default(),
            system_sets: SystemSets::default(),
            hierarchy: Dag::new(),
            dependency: Dag::new(),
            set_systems: DagGroups::default(),
            ambiguous_with: UnGraph::default(),
            ambiguous_with_all: FixedHashSet::default(),
            conflicting_systems: ConflictingSystems::default(),
            anonymous_sets: 0,
            changed: false,
            settings: Default::default(),
            passes: Default::default(),
        }
    }
}

/// Specifies how schedule construction should respond to detecting a certain kind of issue.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogLevel {
    /// Occurrences are completely ignored.
    Ignore,
    /// Occurrences are logged only.
    Warn,
    /// Occurrences are logged and result in errors.
    Error,
}

/// Specifies miscellaneous settings for schedule construction.
#[derive(Clone, Debug)]
pub struct ScheduleBuildSettings {
    /// Determines whether the presence of ambiguities (systems with conflicting access but indeterminate order)
    /// is only logged or also results in an [`Ambiguity`](ScheduleBuildWarning::Ambiguity)
    /// warning or error.
    ///
    /// Defaults to [`LogLevel::Ignore`].
    pub ambiguity_detection: LogLevel,
    /// Determines whether the presence of redundant edges in the hierarchy of system sets is only
    /// logged or also results in a [`HierarchyRedundancy`](ScheduleBuildWarning::HierarchyRedundancy)
    /// warning or error.
    ///
    /// Defaults to [`LogLevel::Warn`].
    pub hierarchy_detection: LogLevel,
    /// Auto insert [`ApplyDeferred`] systems into the schedule,
    /// when there are [`Deferred`](crate::prelude::Deferred)
    /// in one system and there are ordering dependencies on that system. [`Commands`](crate::system::Commands) is one
    /// such deferred buffer.
    ///
    /// You may want to disable this if you only want to sync deferred params at the end of the schedule,
    /// or want to manually insert all your sync points.
    ///
    /// Defaults to `true`
    pub auto_insert_apply_deferred: bool,
    /// If set to true, node names will be shortened instead of the fully qualified type path.
    ///
    /// Defaults to `true`.
    pub use_shortnames: bool,
    /// If set to true, report all system sets the conflicting systems are part of.
    ///
    /// Defaults to `true`.
    pub report_sets: bool,
}

impl Default for ScheduleBuildSettings {
    fn default() -> Self {
        Self::new()
    }
}

impl ScheduleBuildSettings {
    /// Default build settings.
    /// See the field-level documentation for the default value of each field.
    pub const fn new() -> Self {
        Self {
            ambiguity_detection: LogLevel::Ignore,
            hierarchy_detection: LogLevel::Warn,
            auto_insert_apply_deferred: true,
            use_shortnames: true,
            report_sets: true,
        }
    }
}

impl ScheduleGraph {
    /// Returns the name of the node with the given [`NodeId`]. Resolves
    /// anonymous sets to a string that describes their contents.
    ///
    /// Also displays the set(s) the node is contained in if
    /// [`ScheduleBuildSettings::report_sets`] is true, and shortens system names
    /// if [`ScheduleBuildSettings::use_shortnames`] is true.
    pub fn get_node_name(&self, id: &NodeId) -> String {
        self.get_node_name_inner(id, self.settings.report_sets)
    }

    #[inline]
    fn get_node_name_inner(&self, id: &NodeId, report_sets: bool) -> String {
        match *id {
            NodeId::System(key) => {
                let name = self.systems[key].name();
                let name = if self.settings.use_shortnames {
                    name.shortname().to_string()
                } else {
                    name.to_string()
                };
                if report_sets {
                    let sets = self.names_of_sets_containing_node(id);
                    if sets.is_empty() {
                        name
                    } else if sets.len() == 1 {
                        format!("{name} (in set {})", sets[0])
                    } else {
                        format!("{name} (in sets {}", sets.join(", "))
                    }
                } else {
                    name
                }
            }
            NodeId::Set(key) => {
                let set = &self.system_sets[key];
                if set.is_anonymous() {
                    self.anonymous_set_name(id)
                } else {
                    format!("{set:?}")
                }
            }
        }
    }

    fn anonymous_set_name(&self, id: &NodeId) -> String {
        format!(
            "({})",
            self.hierarchy
                .edges_directed(*id, Outgoing)
                // never get the sets of the members or this will infinite recurse when the report_sets setting is on.
                .map(|(_, member_id)| self.get_node_name_inner(&member_id, false))
                .reduce(|a, b| format!("{a}, {b}"))
                .unwrap_or_default()
        )
    }

    fn traverse_sets_containing_node(&self, id: NodeId, f: &mut impl FnMut(SystemSetKey) -> bool) {
        for (set_id, _) in self.hierarchy.edges_directed(id, Incoming) {
            let NodeId::Set(set_key) = set_id else {
                continue;
            };
            if f(set_key) {
                self.traverse_sets_containing_node(NodeId::Set(set_key), f);
            }
        }
    }

    fn names_of_sets_containing_node(&self, id: &NodeId) -> Vec<String> {
        let mut sets = <FixedHashSet<_>>::default();
        self.traverse_sets_containing_node(*id, &mut |key| {
            self.system_sets[key].system_type().is_none() && sets.insert(key)
        });
        let mut sets: Vec<_> = sets
            .into_iter()
            .map(|key| self.get_node_name(&NodeId::Set(key)))
            .collect();
        sets.sort();
        sets
    }
}

//TODO!
