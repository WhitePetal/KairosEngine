//! Contains the definition of the [`EntityCommand`] trait,
//! as well as the blanket implementation of the trait for closures.
//!
//! It also contains functions that return closures for use with
//! [`EntityCommands`](crate::system::EntityCommands).

use crate::ecs::world::error::EntityMutableFetchError;

/// A command which gets executed for a given [`Entity`].
///
/// Should be used with [`EntityCommands::queue`](crate::system::EntityCommands::queue).
///
/// The `Out` generic parameter is the returned "output" of the command.
///
/// # Examples
///
/// ```
/// # use std::collections::HashSet;
/// # use bevy_ecs::prelude::*;
/// use bevy_ecs::system::EntityCommand;
/// #
/// # #[derive(Component, PartialEq)]
/// # struct Name(String);
/// # impl Name {
/// #   fn new(s: String) -> Self { Name(s) }
/// #   fn as_str(&self) -> &str { &self.0 }
/// # }
///
/// #[derive(Resource, Default)]
/// struct Counter(i64);
///
/// /// A `Command` which names an entity based on a global counter.
/// fn count_name(mut entity: EntityWorldMut) {
///     // Get the current value of the counter, and increment it for next time.
///     let i = {
///         let mut counter = entity.resource_mut::<Counter>();
///         let i = counter.0;
///         counter.0 += 1;
///         i
///     };
///     // Name the entity after the value of the counter.
///     entity.insert(Name::new(format!("Entity #{i}")));
/// }
///
/// // App creation boilerplate omitted...
/// # let mut world = World::new();
/// # world.init_resource::<Counter>();
/// #
/// # let mut setup_schedule = Schedule::default();
/// # setup_schedule.add_systems(setup);
/// # let mut assert_schedule = Schedule::default();
/// # assert_schedule.add_systems(assert_names);
/// #
/// # setup_schedule.run(&mut world);
/// # assert_schedule.run(&mut world);
///
/// fn setup(mut commands: Commands) {
///     commands.spawn_empty().queue(count_name);
///     commands.spawn_empty().queue(count_name);
/// }
///
/// fn assert_names(named: Query<&Name>) {
///     // We use a HashSet because we do not care about the order.
///     let names: HashSet<_> = named.iter().map(Name::as_str).collect();
///     assert_eq!(names, HashSet::from_iter(["Entity #0", "Entity #1"]));
/// }
/// ```
pub trait EntityCommand: Send + 'static {}

/// An error that occurs when running an [`EntityCommand`] on a specific entity.
#[derive(thiserror::Error, Debug)]
pub enum EntityCommandError<E> {
    /// The entity this [`EntityCommand`] tried to run on could not be fetched.
    #[error(transparent)]
    EntityFetchError(#[from] EntityMutableFetchError),
    /// An error that occurred while running the [`EntityCommand`].
    #[error("{0}")]
    CommandFailed(E),
}

// TODO!
