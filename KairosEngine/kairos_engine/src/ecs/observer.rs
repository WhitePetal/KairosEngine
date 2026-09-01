//! Observers are a push-based tool for responding to [`Event`]s. The [`Observer`] component holds a [`System`] that runs whenever a matching [`Event`]
//! is triggered.
//!
//! See [`Event`] and [`Observer`] for in-depth documentation and usage examples.

mod centralized_storage;
mod condition;
mod distributed_storage;
mod runner;
mod system_param;

pub use centralized_storage::*;
pub use condition::*;
pub use distributed_storage::*;
pub use runner::*;
pub use system_param::*;

use crate::{
    debug::MaybeLocation,
    ecs::{
        entity::Entity,
        event::Event,
        world::{DeferredWorld, World},
    },
};

impl World {
    /// Register an observer to the cache, called when an observer is created
    pub(crate) fn register_observer(&mut self, observer_entity: Entity) {
        todo!()
    }

    /// Remove the observer from the cache, called when an observer gets despawned
    pub(crate) fn unregister_observer(&mut self, entity: Entity, descriptor: ObserverDescriptor) {
        todo!()
    }

    pub(crate) fn trigger_ref_with_caller<'a, E: Event>(
        &mut self,
        event: &mut E,
        trigger: &mut E::Trigger<'a>,
        caller: MaybeLocation,
    ) {
        let event_key = self.register_event_key::<E>();
        // SAFETY: event_key was just registered and matches `event`
        unsafe {
            DeferredWorld::from(self).trigger_raw(event_key, event, trigger, caller);
        }
    }
}
