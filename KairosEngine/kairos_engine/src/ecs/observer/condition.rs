//! Run conditions for observers.
//!
//! This module provides the types needed to add run conditions to observers,
//! allowing them to conditionally execute based on world state.

use crate::ecs::world::{World, unsafe_world_cell::UnsafeWorldCell};

/// Stores a boxed condition system for an observer.
pub(crate) struct ObserverCondition {}

impl ObserverCondition {
    pub(crate) fn initialize(&mut self, world: &mut World) {
        todo!()
    }

    /// # Safety
    /// - The condition must be initialized.
    /// - The world cell must have valid access for the condition's read-only parameters.
    pub(crate) unsafe fn check(&mut self, world: UnsafeWorldCell) -> bool {
        todo!()
    }
}

// TODO!
