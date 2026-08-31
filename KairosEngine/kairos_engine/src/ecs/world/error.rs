use crate::{
    debug::DebugName,
    ecs::{
        component::ComponentId,
        entity::{Entity, EntityNotSpawnedError},
    },
};

/// The error type returned by [`World::try_insert_batch`] and [`World::try_insert_batch_if_new`]
/// if any of the provided entities do not exist.
///
/// [`World::try_insert_batch`]: crate::world::World::try_insert_batch
/// [`World::try_insert_batch_if_new`]: crate::world::World::try_insert_batch_if_new
#[derive(thiserror::Error, Debug, Clone)]
#[error(
    "Could not insert bundles of type {bundle_type} into the entities with the following IDs because they do not exist: {entities:?}"
)]
pub struct TryInsertBatchError {
    /// The bundles' type name.
    pub bundle_type: DebugName,
    /// The IDs of the provided entities that do not exist.
    pub entities: Vec<Entity>,
}

/// An error that occurs when fetching entities mutably from a world.
#[derive(thiserror::Error, Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityMutableFetchError {
    /// The entity is not spawned.
    #[error(
        "{0}\n
    If you were attempting to apply a command to this entity,
    and want to handle this error gracefully, consider using `EntityCommands::queue_handled` or `queue_silenced`."
    )]
    NotSpawned(#[from] EntityNotSpawnedError),
    /// The entity with the given ID was requested mutably more than once.
    #[error("The entity with ID {0} was requested mutably more than once")]
    AliasedMutability(Entity),
}

/// An error that occurs when dynamically retrieving components from an entity.
#[derive(thiserror::Error, Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityComponentError {
    /// The component with the given [`ComponentId`] does not exist on the entity.
    #[error("The component with ID {0:?} does not exist on the entity.")]
    MissingComponent(ComponentId),
    /// The component with the given [`ComponentId`] was requested mutably more than once.
    #[error("The component with ID {0:?} was requested mutably more than once.")]
    AliasedMutability(ComponentId),
}

// TODO!
