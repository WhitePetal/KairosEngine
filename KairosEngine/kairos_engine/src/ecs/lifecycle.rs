//! This module contains various tools to allow you to react to component insertion or removal,
//! as well as entity spawning and despawning.
//!
//! There are four main ways to react to these lifecycle events:
//!
//! 1. Using component hooks, which act as inherent constructors and destructors for components.
//! 2. Using [observers], which are a user-extensible way to respond to events, including component lifecycle events.
//! 3. Using the [`RemovedComponents`] system parameter, which offers an event-style interface.
//! 4. Using the [`Added`] query filter, which checks each component to see if it has been added since the last time a system ran.
//!
//! [observers]: crate::observer
//! [`Added`]: crate::query::Added
//!
//! # Types of lifecycle events
//!
//! There are five types of lifecycle events, split into two categories. First, we have lifecycle events that are triggered
//! when a component is added to an entity:
//!
//! - [`Add`]: Triggered when a component is added to an entity that did not already have it.
//! - [`Insert`]: Triggered when a component is added to an entity, regardless of whether it already had it.
//!
//! When both events occur, [`Add`] hooks are evaluated before [`Insert`].
//!
//! Next, we have lifecycle events that are triggered when a component is removed from an entity:
//!
//! - [`Discard`]: Triggered when a component is removed from an entity, regardless if it is then replaced with a new value.
//! - [`Remove`]: Triggered when a component is removed from an entity and not replaced, before the component is removed.
//! - [`Despawn`]: Triggered for each component on an entity when it is despawned.
//!
//! [`Discard`] hooks are evaluated before [`Remove`], then finally [`Despawn`] hooks are evaluated.
//!
//! [`Add`] and [`Remove`] are counterparts: they are only triggered when a component is added or removed
//! from an entity in such a way as to cause a change in the component's presence on that entity.
//! Similarly, [`Insert`] and [`Discard`] are counterparts: they are triggered when a component is added or overwritten
//! on an entity, regardless of whether this results in a change in the component's presence on that entity.
//!
//! To reliably synchronize data structures using with component lifecycle events,
//! you can combine [`Insert`] and [`Discard`] to fully capture any changes to the data.
//! This is particularly useful in combination with immutable components,
//! to avoid any lifecycle-bypassing mutations.
//!
//! ## Lifecycle events and component types
//!
//! Despite the absence of generics, each lifecycle event is associated with a specific component.
//! When defining a component hook for a [`Component`] type, that component is used.
//! When observers watch lifecycle events, the `B: Bundle` generic is used.
//!
//! Each of these lifecycle events also corresponds to a fixed [`ComponentId`],
//! which are assigned during [`World`] initialization.
//! For example, [`Add`] corresponds to [`ADD`].
//! This is used to skip [`TypeId`](core::any::TypeId) lookups in hot paths.
//!

use crate::ecs::world::DeferredWorld;

/// The type used for [`Component`] lifecycle hooks such as `on_add`, `on_insert` or `on_remove`.
pub type ComponentHook = for<'w> fn(DeferredWorld<'w>);

/// [`World`]-mutating functions that run as part of lifecycle events of a [`Component`].
///
/// Hooks are functions that run when a component is added, overwritten, or removed from an entity.
/// These are intended to be used for structural side effects that need to happen when a component is added or removed,
/// and are not intended for general-purpose logic.
///
/// For example, you might use a hook to update a cached index when a component is added,
/// to clean up resources when a component is removed,
/// or to keep hierarchical data structures across entities in sync.
///
/// This information is stored in the [`ComponentInfo`](crate::component::ComponentInfo) of the associated component.
///
/// There are two ways of configuring hooks for a component:
/// 1. Defining the relevant hooks on the [`Component`] implementation
/// 2. Using the [`World::register_component_hooks`] method
///
/// # Example
///
/// ```
/// use bevy_ecs::prelude::*;
/// use bevy_ecs::entity::EntityHashSet;
///
/// #[derive(Component)]
/// struct MyTrackedComponent;
///
/// #[derive(Resource, Default)]
/// struct TrackedEntities(EntityHashSet);
///
/// let mut world = World::new();
/// world.init_resource::<TrackedEntities>();
///
/// // No entities with `MyTrackedComponent` have been added yet, so we can safely add component hooks
/// let mut tracked_component_query = world.query::<&MyTrackedComponent>();
/// assert!(tracked_component_query.iter(&world).next().is_none());
///
/// world.register_component_hooks::<MyTrackedComponent>().on_add(|mut world, context| {
///    let mut tracked_entities = world.resource_mut::<TrackedEntities>();
///   tracked_entities.0.insert(context.entity);
/// });
///
/// world.register_component_hooks::<MyTrackedComponent>().on_remove(|mut world, context| {
///   let mut tracked_entities = world.resource_mut::<TrackedEntities>();
///   tracked_entities.0.remove(&context.entity);
/// });
///
/// let entity = world.spawn(MyTrackedComponent).id();
/// let tracked_entities = world.resource::<TrackedEntities>();
/// assert!(tracked_entities.0.contains(&entity));
///
/// world.despawn(entity);
/// let tracked_entities = world.resource::<TrackedEntities>();
/// assert!(!tracked_entities.0.contains(&entity));
/// ```
#[derive(Debug, Clone, Default)]
pub struct ComponentHooks {
    pub(crate) on_add: Option<ComponentHook>,
    pub(crate) on_insert: Option<ComponentHook>,
    pub(crate) on_discard: Option<ComponentHook>,
    pub(crate) on_remove: Option<ComponentHook>,
    pub(crate) on_despawn: Option<ComponentHook>,
}

// TODO!
