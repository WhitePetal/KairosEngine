use crate::ecs::{archetype::ArchetypeId, entity::{Entity, EntityNotSpawnedError}};



/// An error that occurs when retrieving a specific [`Entity`]'s query result from [`Query`](crate::system::Query) or [`QueryState`](crate::query::QueryState).
// TODO: return the type_name as part of this error
#[derive(thiserror::Error, Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryEntityError {
    /// The given [`Entity`]'s components do not match the query.
    ///
    /// Either it does not have a requested component, or it has a component which the query filters out.
    #[error("The query does not match entity {0}")]
    QueryDoesNotMatch(Entity, ArchetypeId),
    /// The given [`Entity`] is not spawned.
    #[error("{0}")]
    NotSpawned(#[from] EntityNotSpawnedError),
    /// The [`Entity`] was requested mutably more than once.
    ///
    /// See [`Query::get_many_mut`](crate::system::Query::get_many_mut) for an example.
    #[error("The entity with ID {0} was requested mutably more than once")]
    AliasedMutability(Entity),
}
