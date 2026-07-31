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

/// The type used for [`Component`] lifecycle hooks such as `on_add`, `on_insert` or `on_remove`.
pub type ComponentHook = for<'w> fn(DeferredWorld<'w>, HookContext);

// TODO!
