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
    component::ComponentId,
    entity::Entity,
    event::{
        Event,
        EventKey,
        trigger_entity_internal,
    },
    world::{
        DeferredWorld,
        EntityWorldMut,
        World,
    },
    ptr::PtrMut,
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
    /// # use kairos_ecs::prelude::*;
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

    /// Triggers the given [`Event`] using the given [`Trigger`](crate::event::Trigger), which will run any [`Observer`]s watching for it.
    ///
    /// For a variant that borrows the `event` rather than consuming it, use [`World::trigger_ref`] instead.
    #[track_caller]
    pub fn trigger_with<'a, E: Event>(&mut self, mut event: E, mut trigger: E::Trigger<'a>) {
        self.trigger_ref_with_caller(&mut event, &mut trigger, MaybeLocation::caller());
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

    /// Triggers the given mutable [`Event`] reference using the given mutable [`Trigger`](crate::event::Trigger) reference, which
    /// will run any [`Observer`]s watching for it.
    ///
    /// Compared to [`World::trigger`], this method is most useful when it's necessary to check
    /// or use the event after it has been modified by observers.
    pub fn trigger_ref_with<'a, E: Event>(&mut self, event: &mut E, trigger: &mut E::Trigger<'a>) {
        self.trigger_ref_with_caller(event, trigger, MaybeLocation::caller());
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

    /// Splits `&mut self` into a [`DeferredWorld`] and the [`CachedObservers`]
    /// registered for `event_key`, or returns `None` if no observers exist.
    ///
    /// # Safety
    ///
    /// Caller must not use the returned [`DeferredWorld`] to access observer
    /// storage, as it aliases with the returned [`CachedObservers`] reference.
    unsafe fn split_for_event(
        &mut self,
        event_key: EventKey,
    ) -> Option<(DeferredWorld<'_>, &CachedObservers)> {
        let world_cell = self.as_unsafe_world_cell();
        let observers = world_cell.observers();
        let observers = observers.try_get_observers(event_key)?;
        // SAFETY: The caller guarantees the returned `DeferredWorld` will not
        // be used to access observer storage (which `observers` borrows).
        Some((unsafe { world_cell.into_deferred() }, observers))
    }

    /// Triggers global [`Observer`]s for `event_key` with untyped event and
    /// trigger data.
    ///
    /// Dynamic equivalent of [`World::trigger`]. Only fires global observers,
    /// not entity- or component-scoped ones.
    ///
    /// Use [`World::trigger_dynamic_targets`] to also fire entity-scoped
    /// observers.
    ///
    /// # Safety
    ///
    /// - `event_data` must point to a valid, aligned value whose layout matches
    ///   what observers registered for this `event_key` expect.
    /// - `trigger_data` must point to a valid, aligned value whose layout
    ///   matches what observers registered for this `event_key` expect.
    #[track_caller]
    pub unsafe fn trigger_dynamic(
        &mut self,
        event_key: EventKey,
        mut event_data: PtrMut,
        mut trigger_data: PtrMut,
    ) {
        // SAFETY: We have exclusive access via `&mut self` and will not
        // access observer storage through the returned `DeferredWorld`.
        let Some((mut world, observers)) = (unsafe { self.split_for_event(event_key) }) else {
            return;
        };

        let context = TriggerContext {
            event_key,
            caller: MaybeLocation::caller(),
        };

        // SAFETY: no outstanding world references besides `observers`
        unsafe {
            world.as_unsafe_world_cell().increment_trigger_id();
        }

        for (observer, runner) in observers.global_observers() {
            // SAFETY:
            // - `observers` come from `world` and correspond to `event_key`
            // - caller guarantees `event_data` and `trigger_data` are valid
            unsafe {
                (runner)(
                    world.reborrow(),
                    *observer,
                    &context,
                    event_data.reborrow(),
                    trigger_data.reborrow(),
                );
            }
        }
    }

    /// Triggers [`Observer`]s for `event_key` targeting `entity`, with untyped
    /// event and trigger data.
    ///
    /// Fires global and entity-scoped observers. Dynamic equivalent of
    /// [`EntityWorldMut::trigger`].
    ///
    /// # Safety
    ///
    /// - `event_data` must point to a valid, aligned value whose layout matches
    ///   what observers registered for this `event_key` expect.
    /// - `trigger_data` must point to a valid, aligned value whose layout
    ///   matches what observers registered for this `event_key` expect.
    #[track_caller]
    pub unsafe fn trigger_dynamic_targets(
        &mut self,
        event_key: EventKey,
        entity: Entity,
        event_data: PtrMut,
        trigger_data: PtrMut,
    ) {
        // SAFETY: We have exclusive access via `&mut self` and will not
        // access observer storage through the returned `DeferredWorld`.
        let Some((world, observers)) = (unsafe { self.split_for_event(event_key) }) else {
            return;
        };

        let context = TriggerContext {
            event_key,
            caller: MaybeLocation::caller(),
        };

        // SAFETY:
        // - `observers` come from `world` and correspond to `event_key`
        // - caller guarantees `event_data` and `trigger_data` are valid
        // - `trigger_entity_internal` increments the trigger id
        unsafe {
            trigger_entity_internal(world, observers, event_data, trigger_data, entity, &context);
        }
    }

    /// Triggers [`Observer`]s for `event_key` targeting `entity` and
    /// `components`, with untyped event and trigger data.
    ///
    /// Fires global, entity-scoped, and component-scoped observers.
    /// Dynamic equivalent of [`EntityComponentsTrigger`].
    ///
    /// [`EntityComponentsTrigger`]: crate::event::EntityComponentsTrigger
    ///
    /// # Safety
    ///
    /// - `event_data` must point to a valid, aligned value whose layout matches
    ///   what observers registered for this `event_key` expect.
    /// - `trigger_data` must point to a valid, aligned value whose layout
    ///   matches what observers registered for this `event_key` expect.
    #[track_caller]
    pub unsafe fn trigger_dynamic_targets_components(
        &mut self,
        event_key: EventKey,
        entity: Entity,
        components: &[ComponentId],
        mut event_data: PtrMut,
        mut trigger_data: PtrMut,
    ) {
        // SAFETY: We have exclusive access via `&mut self` and will not
        // access observer storage through the returned `DeferredWorld`.
        let Some((mut world, observers)) = (unsafe { self.split_for_event(event_key) }) else {
            return;
        };

        let context = TriggerContext {
            event_key,
            caller: MaybeLocation::caller(),
        };

        // SAFETY:
        // - `observers` come from `world` and correspond to `event_key`
        // - caller guarantees `event_data` and `trigger_data` are valid
        // - `trigger_entity_internal` increments the trigger id
        unsafe {
            trigger_entity_internal(
                world.reborrow(),
                observers,
                event_data.reborrow(),
                trigger_data.reborrow(),
                entity,
                &context,
            );
        }

        // Trigger observers watching for specific components.
        for id in components {
            if let Some(component_observers) = observers.component_observers().get(id) {
                for (observer, runner) in component_observers.global_observers() {
                    // SAFETY: same as above, caller guarantees data validity
                    unsafe {
                        (runner)(
                            world.reborrow(),
                            *observer,
                            &context,
                            event_data.reborrow(),
                            trigger_data.reborrow(),
                        );
                    }
                }

                if let Some(map) = component_observers
                    .entity_component_observers()
                    .get(&entity)
                {
                    for (observer, runner) in map {
                        // SAFETY: same as above, caller guarantees data validity
                        unsafe {
                            (runner)(
                                world.reborrow(),
                                *observer,
                                &context,
                                event_data.reborrow(),
                                trigger_data.reborrow(),
                            );
                        }
                    }
                }
            }
        }
    }

    /// Register an observer to the cache, called when an observer is created
    pub(crate) fn register_observer(&mut self, observer_entity: Entity) {
        // SAFETY: References do not alias.
        let (observer_state, archetypes, observers) = unsafe {
            let observer_state: *const Observer = self.get::<Observer>(observer_entity).unwrap();
            // Populate ObservedBy for each observed entity.
            for watched_entity in (*observer_state).descriptor.entities.iter().copied() {
                let mut entity_mut = self.entity_mut(watched_entity);
                let mut observed_by = entity_mut.entry::<ObservedBy>().or_default().into_mut();
                observed_by.0.push(observer_entity);
            }
            (&*observer_state, &mut self.archetypes, &mut self.observers)
        };
        let descriptor = &observer_state.descriptor;

        for &event_key in &descriptor.event_keys {
            let cache = observers.get_observers_mut(event_key);

            if descriptor.components.is_empty() && descriptor.entities.is_empty() {
                cache
                    .global_observers
                    .insert(observer_entity, observer_state.runner);
            } else if descriptor.components.is_empty() {
                // Observer is not targeting any components so register it as an entity observer
                for &watched_entity in &observer_state.descriptor.entities {
                    let map = cache.entity_observers.entry(watched_entity).or_default();
                    map.insert(observer_entity, observer_state.runner);
                }
            } else {
                // Register observer for each watched component
                for &component in &descriptor.components {
                    let observers =
                        cache
                            .component_observers
                            .entry(component)
                            .or_insert_with(|| {
                                if let Some(flag) = Observers::is_archetype_cached(event_key) {
                                    archetypes.update_flags(component, flag, true);
                                }
                                CachedComponentObservers::default()
                            });
                    if descriptor.entities.is_empty() {
                        // Register for all triggers targeting the component
                        observers
                            .global_observers
                            .insert(observer_entity, observer_state.runner);
                    } else {
                        // Register for each watched entity
                        for &watched_entity in &descriptor.entities {
                            let map = observers
                                .entity_component_observers
                                .entry(watched_entity)
                                .or_default();
                            map.insert(observer_entity, observer_state.runner);
                        }
                    }
                }
            }
        }
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
}
