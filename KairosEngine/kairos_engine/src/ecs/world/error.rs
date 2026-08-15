use crate::ecs::{
    component::ComponentId,
    entity::{Entity, EntityNotSpawnedError},
};

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
