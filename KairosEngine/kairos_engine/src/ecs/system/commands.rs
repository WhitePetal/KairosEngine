use std::marker::PhantomData;

use crate::ecs::entity::{Entities, Entity, EntityAllocator};

pub mod command;
pub mod entity_command;

pub use command::Command;
pub use entity_command::EntityCommand;

/// A [`Command`] queue to perform structural changes to the [`World`].
///
/// Since each command requires exclusive access to the `World`,
/// all queued commands are automatically applied in sequence
/// when the `ApplyDeferred` system runs (see [`ApplyDeferred`] documentation for more details).
///
/// Each command can be used to modify the [`World`] in arbitrary ways:
/// * spawning or despawning entities
/// * inserting components on new or existing entities
/// * inserting resources
/// * etc.
///
/// For a version of [`Commands`] that works in parallel contexts (such as
/// within [`Query::par_iter`](crate::system::Query::par_iter)) see
/// [`ParallelCommands`]
///
/// # Usage
///
/// Add `mut commands: Commands` as a function argument to your system to get a
/// copy of this struct that will be applied the next time a copy of [`ApplyDeferred`] runs.
/// Commands are almost always used as a [`SystemParam`](crate::system::SystemParam).
///
/// ```
/// # use bevy_ecs::prelude::*;
/// fn my_system(mut commands: Commands) {
///    // ...
/// }
/// # bevy_ecs::system::assert_is_system(my_system);
/// ```
///
/// # Implementing
///
/// Each built-in command is implemented as a separate method, e.g. [`Commands::spawn`].
/// In addition to the pre-defined command methods, you can add commands with any arbitrary
/// behavior using [`Commands::queue`], which accepts any type implementing [`Command`].
///
/// Since closures and other functions implement this trait automatically, this allows one-shot,
/// anonymous custom commands.
///
/// ```
/// # use bevy_ecs::prelude::*;
/// # fn foo(mut commands: Commands) {
/// // NOTE: type inference fails here, so annotations are required on the closure.
/// commands.queue(|w: &mut World| {
///     // Mutate the world however you want...
/// });
/// # }
/// ```
///
/// # Error handling
///
/// A [`Command`] can return a [`Result`](crate::error::Result),
/// which will be passed to an [error handler](crate::error) if the `Result` is an error.
///
/// The fallback error handler panics. It can be configured via
/// the [`FallbackErrorHandler`](crate::error::FallbackErrorHandler) resource.
///
/// Alternatively, you can customize the error handler for a specific command
/// by calling [`Commands::queue_handled`].
///
/// The [`error`](crate::error) module provides some simple error handlers for convenience.
///
/// [`ApplyDeferred`]: crate::schedule::ApplyDeferred
pub struct Commands<'w, 's> {
    queue: InternalQueue<'s>,
    entities: &'w Entities,
    allocator: &'w EntityAllocator,
}

enum InternalQueue<'s> {
    TODO(PhantomData<&'s u32>),
}

impl<'w, 's> Commands<'w, 's> {
    /// Pushes a generic [`Command`] to the command queue.
    ///
    /// If the [`Command`] returns a [`Result`],
    /// it will be handled using the [fallback error handler](crate::error::FallbackErrorHandler).
    ///
    /// To use a custom error handler, see [`Commands::queue_handled`].
    ///
    /// The command can be:
    /// - A custom struct that implements [`Command`].
    /// - A closure or function that matches one of the following signatures:
    ///   - [`(&mut World)`](World)
    /// - A built-in command from the [`command`] module.
    ///
    /// # Example
    ///
    /// ```
    /// # use bevy_ecs::prelude::*;
    /// #[derive(Resource, Default)]
    /// struct Counter(u64);
    ///
    /// struct AddToCounter(String);
    ///
    /// impl Command for AddToCounter {
    ///     type Out = Result;
    ///
    ///     fn apply(self, world: &mut World) -> Result {
    ///         let mut counter = world.get_resource_or_insert_with(Counter::default);
    ///         let amount: u64 = self.0.parse()?;
    ///         counter.0 += amount;
    ///         Ok(())
    ///     }
    /// }
    ///
    /// fn add_three_to_counter_system(mut commands: Commands) {
    ///     commands.queue(AddToCounter("3".to_string()));
    /// }
    ///
    /// fn add_twenty_five_to_counter_system(mut commands: Commands) {
    ///     commands.queue(|world: &mut World| {
    ///         let mut counter = world.get_resource_or_insert_with(Counter::default);
    ///         counter.0 += 25;
    ///     });
    /// }
    /// # bevy_ecs::system::assert_is_system(add_three_to_counter_system);
    /// # bevy_ecs::system::assert_is_system(add_twenty_five_to_counter_system);
    /// ```
    pub fn queue(&mut self, command: impl Command) {
        todo!()
    }

    /// Returns a [`Commands`] with a smaller lifetime.
    ///
    /// This is useful if you have `&mut Commands` but need `Commands`.
    ///
    /// # Example
    ///
    /// ```
    /// # use bevy_ecs::prelude::*;
    /// fn my_system(mut commands: Commands) {
    ///     // We do our initialization in a separate function,
    ///     // which expects an owned `Commands`.
    ///     do_initialization(commands.reborrow());
    ///
    ///     // Since we only reborrowed the commands instead of moving them, we can still use them.
    ///     commands.spawn_empty();
    /// }
    /// #
    /// # fn do_initialization(_: Commands) {}
    /// ```
    pub fn reborrow(&mut self) -> Commands<'w, '_> {
        todo!()
    }

    ///
    /// This method does not guarantee that commands queued by the returned `EntityCommands`
    /// will be successful, since the entity could be despawned before they are executed.
    ///
    /// # Example
    ///
    /// ```
    /// # use bevy_ecs::prelude::*;
    /// #[derive(Resource)]
    /// struct PlayerEntity {
    ///     entity: Entity
    /// }
    ///
    /// #[derive(Component)]
    /// struct Label(&'static str);
    ///
    /// fn example_system(mut commands: Commands, player: Res<PlayerEntity>) {
    ///     // Get the entity and add a component.
    ///     commands.entity(player.entity).insert(Label("hello world"));
    /// }
    /// # bevy_ecs::system::assert_is_system(example_system);
    /// ```
    ///
    /// # See also
    ///
    /// - [`get_entity`](Self::get_entity) for the fallible version.
    #[inline]
    #[track_caller]
    pub fn entity(&mut self, entity: Entity) -> EntityCommands<'_> {
        EntityCommands {
            entity,
            commands: self.reborrow(),
        }
    }
}

/// A list of commands that will be run to modify an [`Entity`].
///
/// # Note
///
/// Most [`Commands`] (and thereby [`EntityCommands`]) are deferred:
/// when you call the command, if it requires mutable access to the [`World`]
/// (that is, if it removes, adds, or changes something), it's not executed immediately.
///
/// Instead, the command is added to a "command queue."
/// The command queue is applied later
/// when the [`ApplyDeferred`](crate::schedule::ApplyDeferred) system runs.
/// Commands are executed one-by-one so that
/// each command can have exclusive access to the `World`.
///
/// # Fallible
///
/// Due to their deferred nature, an entity you're trying to change with an [`EntityCommand`]
/// can be despawned by the time the command is executed.
///
/// All deferred entity commands will check whether the entity exists at the time of execution
/// and will return an error if it doesn't.
///
/// # Error handling
///
/// An [`EntityCommand`] can return a [`Result`](crate::error::Result),
/// which will be passed to an [error handler](crate::error) if the `Result` is an error.
///
/// The fallback error handler panics. It can be configured via
/// the [`FallbackErrorHandler`](crate::error::FallbackErrorHandler) resource.
///
/// Alternatively, you can customize the error handler for a specific command
/// by calling [`EntityCommands::queue_handled`].
///
/// The [`error`](crate::error) module provides some simple error handlers for convenience.
pub struct EntityCommands<'a> {
    pub(crate) entity: Entity,
    pub(crate) commands: Commands<'a, 'a>,
}

impl<'a> EntityCommands<'a> {
    /// Despawns the entity.
    ///
    /// This will emit a warning if the entity does not exist.
    ///
    /// # Note
    ///
    /// This will also despawn the entities in any [`RelationshipTarget`](crate::relationship::RelationshipTarget)
    /// that is configured to despawn descendants.
    ///
    /// For example, this will recursively despawn [`Children`](crate::hierarchy::Children).
    ///
    /// # Example
    ///
    /// ```
    /// # use bevy_ecs::prelude::*;
    /// # #[derive(Resource)]
    /// # struct CharacterToRemove { entity: Entity }
    /// #
    /// fn remove_character_system(
    ///     mut commands: Commands,
    ///     character_to_remove: Res<CharacterToRemove>
    /// ) {
    ///     commands.entity(character_to_remove.entity).despawn();
    /// }
    /// # bevy_ecs::system::assert_is_system(remove_character_system);
    /// ```
    #[track_caller]
    pub fn despawn(&mut self) {
        todo!()
    }
}

// TODO
