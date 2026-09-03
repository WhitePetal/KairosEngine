use kairos_ecs_macros::Resource;

use crate::{collections::FixedHashMap, ecs::schedule::InternedScheduleLabel};

/// Resource that stores [`Schedule`]s mapped to [`ScheduleLabel`]s excluding the current running [`Schedule`].
#[derive(Default, Resource)]
pub struct Schedules {
    inner: FixedHashMap<InternedScheduleLabel, Schedule>,
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
}

/// Metadata for a [`Schedule`].
///
/// The order isn't optimized; calling `ScheduleGraph::build_schedule` will return a
/// `SystemSchedule` where the order is optimized for execution.
#[derive(Default)]
pub struct ScheduleGraph {}

//TODO!
