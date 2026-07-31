use std::{marker::PhantomData, sync::atomic::AtomicUsize};

use crate::{
    collections::TypeIdMap,
    ecs::{
        component::{ComponentDescriptor, ComponentId, Components},
        world::World,
    },
};

/// This is a safe handle around `ComponentsRegistrator` and `RequiredComponents` to register required components.
pub struct RequiredComponentsRegistrator<'a, 'w> {
    _todo: PhantomData<(&'a ComponentDescriptor, &'w World)>,
}

/// Generates [`ComponentId`]s.
#[derive(Debug, Default)]
pub struct ComponentIds {
    next: AtomicUsize,
}

/// A [`Components`] wrapper that enables additional features, like registration.
pub struct ComponentsRegistrator<'w> {
    pub(super) components: &'w mut Components,
    pub(super) ids: &'w mut ComponentIds,
    pub(super) recursion_check_stack: Vec<ComponentId>,
}

/// A queued component registration.
pub(super) struct QueuedRegistration {
    pub(super) registrator: fn(&mut ComponentsRegistrator, ComponentId, ComponentDescriptor),
    pub(super) id: ComponentId,
    pub(super) descriptor: ComponentDescriptor,
}

/// Allows queuing components to be registered.
#[derive(Default)]
pub struct QueuedComponents {
    pub(super) components: TypeIdMap<QueuedRegistration>,
    pub(super) dynamic_registrations: Vec<QueuedRegistration>,
}

// TODO!
