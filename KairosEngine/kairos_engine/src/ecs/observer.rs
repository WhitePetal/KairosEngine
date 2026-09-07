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
        world::{DeferredWorld, EntityWorldMut, World},
    },
};

#[cfg(test)]
mod tests;

impl World {
    /// Spawns a "global" [`Observer`] which will watch for the given event.
    /// Returns its [`Entity`] as a [`EntityWorldMut`].
    ///
    /// `system` can be any system whose first parameter is [`On`].
    ///
    /// # Example
    ///
    /// ```
    /// # use bevy_ecs::prelude::*;
    /// #[derive(Component)]
    /// struct A;
    ///
    /// # let mut world = World::new();
    /// world.add_observer(|_: On<Add, A>| {
    ///     // ...
    /// });
    /// world.add_observer(|_: On<Remove, A>| {
    ///     // ...
    /// });
    /// ```
    ///
    /// **Calling [`observe`](EntityWorldMut::observe) on the returned
    /// [`EntityWorldMut`] will observe the observer itself, which you very
    /// likely do not want.**
    ///
    /// # Panics
    ///
    /// Panics if the given system is an exclusive system.
    pub fn add_observer<M>(&mut self, observer: impl IntoObserver<M>) -> EntityWorldMut<'_> {
        self.spawn(observer.into_observer())
    }

    /// Register an observer to the cache, called when an observer is created
    pub(crate) fn register_observer(&mut self, observer_entity: Entity) {
        todo!()
    }

    /// Remove the observer from the cache, called when an observer gets despawned
    pub(crate) fn unregister_observer(&mut self, entity: Entity, descriptor: ObserverDescriptor) {
        // Remove this observer from all the corresponding ObservedBy components.
        for &observing in descriptor.entities.iter() {
            let Ok(mut observing) = self.get_entity_mut(observing) else {
                // This can happen when ObservedBy is despawning and is despawning the related
                // observers.
                continue;
            };
            let Some(mut observed_by) = observing.get_mut::<ObservedBy>() else {
                // In "normal" usage, this should be impossible, but there's nothing stopping a user
                // from just removing the ObservedBy component themselves. While that's odd usage,
                // there's no reason to panic if a user does so.
                continue;
            };

            observed_by.0.retain(|e| *e != entity);
            if observed_by.0.is_empty() {
                observing.remove::<ObservedBy>();
            }
        }

        let archetypes = &mut self.archetypes;
        let observers = &mut self.observers;

        for &event_key in &descriptor.event_keys {
            let cache = observers.get_observers_mut(event_key);
            if descriptor.components.is_empty() && descriptor.entities.is_empty() {
                cache.global_observers.remove(&entity);
            } else if descriptor.components.is_empty() {
                for watched_entity in &descriptor.entities {
                    // This check should be unnecessary since this observer hasn't been unregistered yet
                    let Some(observers) = cache.entity_observers.get_mut(watched_entity) else {
                        continue;
                    };
                    observers.remove(&entity);
                    if observers.is_empty() {
                        cache.entity_observers.remove(watched_entity);
                    }
                }
            } else {
                for component in &descriptor.components {
                    let Some(observers) = cache.component_observers.get_mut(component) else {
                        continue;
                    };
                    if descriptor.entities.is_empty() {
                        observers.global_observers.remove(&entity);
                    } else {
                        for watched_entity in &descriptor.entities {
                            let Some(map) =
                                observers.entity_component_observers.get_mut(watched_entity)
                            else {
                                continue;
                            };
                            map.remove(&entity);
                            if map.is_empty() {
                                observers.entity_component_observers.remove(watched_entity);
                            }
                        }
                    }

                    if observers.global_observers.is_empty()
                        && observers.entity_component_observers.is_empty()
                    {
                        cache.component_observers.remove(component);
                        if let Some(flag) = Observers::is_archetype_cached(event_key)
                            && let Some(by_component) = archetypes.by_component.get(component)
                        {
                            for archetype in by_component.keys() {
                                let archetype = &mut archetypes.archetypes[archetype.index()];
                                if archetype.contains(*component) {
                                    let no_longer_observed = archetype
                                        .iter_components()
                                        .all(|id| !cache.component_observers.contains_key(&id));

                                    if no_longer_observed {
                                        archetype.flags.set(flag, false);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
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

    /// Triggers the given [`Event`], which will run any [`Observer`]s watching for it.
    ///
    /// For a variant that borrows the `event` rather than consuming it, use [`World::trigger_ref`] instead.
    #[track_caller]
    pub fn trigger<'a, E: Event<Trigger<'a>: Default>>(&mut self, mut event: E) {
        self.trigger_ref_with_caller(
            &mut event,
            &mut <E::Trigger<'a> as Default>::default(),
            MaybeLocation::caller(),
        );
    }

    /// Triggers the given mutable [`Event`] reference, which will run any [`Observer`]s watching for it.
    ///
    /// Compared to [`World::trigger`], this method is most useful when it's necessary to check
    /// or use the event after it has been modified by observers.
    #[track_caller]
    pub fn trigger_ref<'a, E: Event<Trigger<'a>: Default>>(&mut self, event: &mut E) {
        self.trigger_ref_with_caller(
            event,
            &mut <E::Trigger<'a> as Default>::default(),
            MaybeLocation::caller(),
        );
    }
}

// TODO!
