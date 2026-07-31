use std::{
    any::TypeId,
    marker::PhantomData,
    ops::Deref,
    sync::{PoisonError, atomic::AtomicUsize},
};

use crate::{
    collections::TypeIdMap,
    ecs::{
        component::{Component, ComponentDescriptor, ComponentId, Components},
        lifecycle::ComponentHooks,
        world::World,
    },
};

/// Generates [`ComponentId`]s.
#[derive(Debug, Default)]
pub struct ComponentIds {
    next: AtomicUsize,
}

impl ComponentIds {
    /// Peeks the next [`ComponentId`] to be generated without generating it.
    pub fn peek(&self) -> ComponentId {
        ComponentId(self.next.load(std::sync::atomic::Ordering::Relaxed))
    }

    /// Generates and returns the next [`ComponentId`].
    pub fn next(&self) -> ComponentId {
        ComponentId(self.next.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }

    /// Peeks the next [`ComponentId`] to be generated without generating it.
    pub fn peek_mut(&mut self) -> ComponentId {
        ComponentId(*self.next.get_mut())
    }

    /// Generates and returns the next [`ComponentId`].
    pub fn next_mut(&mut self) -> ComponentId {
        let id = self.next.get_mut();
        let result = ComponentId(*id);
        *id += 1;
        result
    }

    /// Returns the number of [`ComponentId`]s generated.
    pub fn len(&self) -> usize {
        self.peek().0
    }

    /// Returns true if and only if no ids have been generated.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A [`Components`] wrapper that enables additional features, like registration.
pub struct ComponentsRegistrator<'w> {
    pub(super) components: &'w mut Components,
    pub(super) ids: &'w mut ComponentIds,
    pub(super) recursion_check_stack: Vec<ComponentId>,
}

impl Deref for ComponentsRegistrator<'_> {
    type Target = Components;

    fn deref(&self) -> &Self::Target {
        self.components
    }
}

impl<'w> ComponentsRegistrator<'w> {
    /// Constructs a new [`ComponentsRegistrator`].
    ///
    /// # Safety
    ///
    /// The [`Components`] and [`ComponentIds`] must match.
    /// For example, they must be from the same world.
    pub unsafe fn new(components: &'w mut Components, ids: &'w mut ComponentIds) -> Self {
        Self {
            components,
            ids,
            recursion_check_stack: Vec::new(),
        }
    }

    /// Converts this [`ComponentsRegistrator`] into a [`ComponentsQueuedRegistrator`].
    /// This is intended for use to pass this value to a function that requires [`ComponentsQueuedRegistrator`].
    /// It is generally not a good idea to queue a registration when you can instead register directly on this type.
    pub fn as_queued(&self) -> ComponentsQueuedRegistrator<'_> {
        // SAFETY: ensured by the caller that created self.
        unsafe { ComponentsQueuedRegistrator::new(self.components, self.ids) }
    }

    /// Applies every queued registration.
    /// This ensures that every valid [`ComponentId`] is registered,
    /// enabling retrieving [`ComponentInfo`](super::ComponentInfo), etc.
    pub fn apply_queued_registrations(&mut self) {
        if !self.any_queued_mut() {
            return;
        }

        // Note:
        //
        // This is not just draining the queue. We need to empty the queue without removing the information from `Components`.
        // If we drained directly, we could break invariance.
        //
        // For example, say `ComponentA` and `ComponentB` are queued, and `ComponentA` requires `ComponentB`.
        // If we drain directly, and `ComponentA` was the first to be registered, then, when `ComponentA`
        // registers `ComponentB` in `Component::register_required_components`,
        // `Components` will not know that `ComponentB` was queued
        // (since it will have been drained from the queue.)
        // If that happened, `Components` would assign a new `ComponentId` to `ComponentB`
        // which would be *different* than the id it was assigned in the queue.
        // Then, when the drain iterator gets to `ComponentB`,
        // it would be unsafely registering `ComponentB`, which is already registered.
        //
        // As a result, we need to pop from each queue one by one instead of draining.

        // components
        while let Some(registrator) = {
            let queued = self
                .components
                .queued
                .get_mut()
                .unwrap_or_else(PoisonError::into_inner);
            queued.components.keys().next().copied().map(|type_id| {
                // SAFETY: the id just came from a valid iterator.
                unsafe {
                    queued
                        .components
                        .shift_remove(&type_id)
                        .debug_checked_unwrap()
                }
            })
        } {
            registrator.register(self);
        }

        // dynamic
        let queued = &mut self
            .components
            .queued
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner);
        if !queued.dynamic_registrations.is_empty() {
            for registrator in std::mem::take(&mut queued.dynamic_registrations) {
                registrator.register(self);
            }
        }
    }

    pub fn register_component<T: Component>(&mut self) -> ComponentId {}

    // This exists to cut down on monomorphized code in register_component, which reduces compile times and binary sizes.
    fn register_component_checked(
        &mut self,
        type_id: TypeId,
        descriptor: fn() -> ComponentDescriptor,
        register_required_components: fn(ComponentId, &mut RequiredComponentsRegistrator),
        update_from_component: fn(&mut ComponentHooks) -> &mut ComponentHooks,
    ) -> ComponentId {
        if let Some(&id) = self.indices.get(&type_id) {}

        todo!()
    }
}

/// This is a safe handle around `ComponentsRegistrator` and `RequiredComponents` to register required components.
pub struct RequiredComponentsRegistrator<'a, 'w> {
    _todo: PhantomData<(&'a ComponentDescriptor, &'w World)>,
}

/// A queued component registration.
pub(super) struct QueuedRegistration {
    pub(super) registrator: fn(&mut ComponentsRegistrator, ComponentId, ComponentDescriptor),
    pub(super) id: ComponentId,
    pub(super) descriptor: ComponentDescriptor,
}

impl QueuedRegistration {
    /// Creates the [`QueuedRegistration`].
    ///
    /// # Safety
    ///
    /// [`ComponentId`] must be unique.
    unsafe fn new(
        id: ComponentId,
        descriptor: ComponentDescriptor,
        func: fn(&mut ComponentsRegistrator, ComponentId, ComponentDescriptor),
    ) -> Self {
        Self {
            registrator: func,
            id,
            descriptor,
        }
    }

    /// Performs the registration, returning the now valid [`ComponentId`].
    pub(super) fn register(self, registrator: &mut ComponentsRegistrator) -> ComponentId {
        (self.registrator)(registrator, self.id, self.descriptor);
        self.id
    }
}

/// Allows queuing components to be registered.
#[derive(Default)]
pub struct QueuedComponents {
    pub(super) components: TypeIdMap<QueuedRegistration>,
    pub(super) dynamic_registrations: Vec<QueuedRegistration>,
}

/// A type that enables queuing registration in [`Components`].
///
/// # Note
///
/// These queued registrations return [`ComponentId`]s.
/// These ids are not yet valid, but they will become valid
/// when either [`ComponentsRegistrator::apply_queued_registrations`] is called or the same registration is made directly.
/// In either case, the returned [`ComponentId`]s will be correct, but they are not correct yet.
///
/// Generally, that means these [`ComponentId`]s can be safely used for read-only purposes.
/// Modifying the contents of the world through these [`ComponentId`]s directly without waiting for them to be fully registered
/// and without then confirming that they have been fully registered is not supported.
/// Hence, extra care is needed with these [`ComponentId`]s to ensure all safety rules are followed.
///
/// As a rule of thumb, if you have mutable access to [`ComponentsRegistrator`], prefer to use that instead.
/// Use this only if you need to know the id of a component but do not need to modify the contents of the world based on that id.
#[derive(Clone, Copy)]
pub struct ComponentsQueuedRegistrator<'w> {
    components: &'w Components,
    ids: &'w ComponentIds,
}

impl Deref for ComponentsQueuedRegistrator {
    type Target = Components;

    fn deref(&self) -> &Self::Target {
        self.components
    }
}

impl<'w> ComponentsQueuedRegistrator<'w> {
    /// Constructs a new [`ComponentsQueuedRegistrator`].
    ///
    /// # Safety
    ///
    /// The [`Components`] and [`ComponentIds`] must match.
    /// For example, they must be from the same world.
    pub unsafe fn new(components: &'w Components, ids: &'w ComponentIds) -> Self {
        Self { components, ids }
    }
}
