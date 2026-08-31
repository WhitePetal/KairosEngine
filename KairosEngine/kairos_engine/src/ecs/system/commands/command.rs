//! Contains the definition of the [`Command`] trait,
//! as well as the blanket implementation of the trait for closures.
//!
//! It also contains functions that return closures for use with
//! [`Commands`](crate::system::Commands).

use crate::ecs::{error::CommandOutput, world::World};

/// A [`World`] mutation.
///
/// Should be used with [`Commands::queue`](crate::system::Commands::queue).
///
/// The `Out` generic parameter is the returned "output" of the command.
///
/// # Usage
///
/// ```
/// # use bevy_ecs::prelude::*;
/// // Our world resource
/// #[derive(Resource, Default)]
/// struct Counter(u64);
///
/// // Our custom command
/// struct AddToCounter(u64);
///
/// impl Command for AddToCounter {
///     type Out = ();
///
///     fn apply(self, world: &mut World) {
///         let mut counter = world.get_resource_or_insert_with(Counter::default);
///         counter.0 += self.0;
///     }
/// }
///
/// fn some_system(mut commands: Commands) {
///     commands.queue(AddToCounter(42));
/// }
/// ```
pub trait Command: Send + 'static {
    /// The return type of [`apply`](Command::apply).
    type Out: CommandOutput;

    /// Applies this command, causing it to mutate the provided `world`.
    ///
    /// This method is used to define what a command "does" when it is ultimately applied.
    /// Because this method takes `self`, you can store data or settings on the type that implements this trait.
    /// This data is set by the system or other source of the command, and then ultimately read in this method.
    fn apply(self, world: &mut World) -> Self::Out;
}

impl<F, Out> Command for F
where
    F: FnOnce(&mut World) -> Out + Send + 'static,
    Out: CommandOutput,
{
    type Out = Out;

    fn apply(self, world: &mut World) -> Self::Out {
        todo!()
    }
}

// TODO!
