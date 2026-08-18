use std::fmt::Display;

use crate::ecs::{
    component::{ComponentId, Components},
    query::{Access, QueryData},
};

// found by benchmarking
// too low, and smaller queries do unnecessary work
// maintaining the bloom filter for a handful of checks
// too high, and the benefit of a simpler loop
// is outweighed by the n^2 check
const USE_FILTER_THRESHOLD: usize = 4;

pub fn has_conflicits<Q: QueryData>(components: &Components) -> Result<(), QueryAccessError> {
    todo!()
}

/// Error returned from [`has_conflicts`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum QueryAccessError {
    /// Component was not registered on world
    ComponentNotRegistered,
    /// Entity did not have the requested components
    EntityDoesNotMatch,
}

impl std::error::Error for QueryAccessError {}

impl Display for QueryAccessError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match *self {
            QueryAccessError::ComponentNotRegistered => {
                write!(
                    f,
                    "At least one component in Q was not registered in world.
                    Consider calling `World::register_component`"
                )
            }
            QueryAccessError::EntityDoesNotMatch => {
                write!(f, "Entity does not match Q")
            }
        }
    }
}

/// The data storage type that is being accessed.
#[derive(Copy, Clone, Debug, PartialEq, Hash)]
pub enum EcsAccessType<'a> {
    /// Accesses [`Component`](crate::prelude::Component) data
    Component(EcsAccessLevel),
    /// borrowed access from [`WorldQuery::State`](crate::query::WorldQuery)
    Access(&'a Access),
    /// Does not access any data that can conflict.
    Empty,
}

/// The way the data will be accessed and whether we take access on all the components on
/// an entity or just one component.
#[derive(Clone, Copy, Debug, PartialEq, Hash)]
pub enum EcsAccessLevel {
    /// Reads [`Component`](crate::prelude::Component) with [`ComponentId`]
    Read(ComponentId),
    /// Writes [`Component`](crate::prelude::Component) with [`ComponentId`]
    Write(ComponentId),
    /// Potentially reads all [`Component`](crate::prelude::Component)'s in the [`World`](crate::prelude::World)
    ReadAll,
    /// Potentially writes all [`Component`](crate::prelude::Component)'s in the [`World`](crate::prelude::World)
    WriteAll,
}

// TODO!
