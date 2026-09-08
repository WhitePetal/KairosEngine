//! Contains APIs for ordering systems and executing them on a [`World`](crate::world::World)

mod auto_insert_apply_deferred;
mod condition;
mod config;
mod error;
mod executor;
mod node;
mod pass;
mod schedule;
mod set;
#[cfg(feature = "debug_stepping")]
mod stepping;

/// An implementation of a graph data structure.
pub mod graph;

pub use graph::GraphInfo;

pub mod passes {
    pub use crate::schedule::auto_insert_apply_deferred::*;
}

pub use condition::*;
pub use config::*;
pub use error::*;
pub use executor::*;
pub use node::*;
pub use pass::{FlattenedDependencies, ScheduleBuildPass};
pub use schedule::*;
pub use set::*;

#[cfg(test)]
mod tests;
