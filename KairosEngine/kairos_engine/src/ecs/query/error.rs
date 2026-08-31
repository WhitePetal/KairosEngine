use crate::{
    debug::DebugName,
    ecs::{
        archetype::ArchetypeId,
        entity::{Entity, EntityNotSpawnedError},
    },
};

#[cfg(test)]
mod tests;

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

/// An error that occurs when evaluating a [`Query`](crate::system::Query) or [`QueryState`](crate::query::QueryState) as a single expected result via
/// [`single`](crate::system::Query::single) or [`single_mut`](crate::system::Query::single_mut).
#[derive(Debug, thiserror::Error)]
pub enum QuerySingleError {
    /// No entity fits the query.
    #[error("No entities fit the query {0}")]
    NonEntities(DebugName),
    /// Multiple entities fit the query.
    #[error("Multiple entities fit the query {0}")]
    MultipleEntities(DebugName),
}

/// An error that occurs when creating a contiguous iterator from a non-dense [`Query`](crate::system::Query) or [`QueryState`](crate::query::QueryState) via
/// [`contiguous_iter`](crate::system::Query::contiguous_iter) or [`contiguous_iter_mut`](crate::system::Query::contiguous_iter_mut).
#[derive(Debug, thiserror::Error)]
#[error("Cannot contiguously iterate non-dense query {0}")]
pub struct QueryNotDenseError(pub DebugName);
