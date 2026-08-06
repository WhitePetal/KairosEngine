//! [`Event`] functionality.

mod trigger;

use std::marker::PhantomData;

pub use trigger::*;

use crate::ecs::{component::ComponentId, entity::Entity, world::World};

#[cfg(test)]
mod tests;

/// An [`Event`] is something that "happens" at a given moment.
///
/// To make an [`Event`] "happen", you "trigger" it on a [`World`] using [`World::trigger`] or via a [`Command`](crate::system::Command)
/// using [`Commands::trigger`](crate::system::Commands::trigger). This causes any [`Observer`](crate::observer::Observer) watching for that
/// [`Event`] to run _immediately_, as part of the [`World::trigger`] call.
///
/// First, we create an [`Event`] type, typically by deriving the trait.
///
/// ```
/// # use bevy_ecs::prelude::*;
/// #
/// #[derive(Event)]
/// struct Speak {
///     message: String,
/// }
/// ```
///
/// Then, we add an [`Observer`](crate::observer::Observer) to watch for this event type:
///
/// ```
/// # use bevy_ecs::prelude::*;
/// #
/// # #[derive(Event)]
/// # struct Speak {
/// #     message: String,
/// # }
/// #
/// # let mut world = World::new();
/// #
/// world.add_observer(|speak: On<Speak>| {
///     println!("{}", speak.message);
/// });
/// ```
///
/// Finally, we trigger the event by calling [`World::trigger`](World::trigger):
///
/// ```
/// # use bevy_ecs::prelude::*;
/// #
/// # #[derive(Event)]
/// # struct Speak {
/// #     message: String,
/// # }
/// #
/// # let mut world = World::new();
/// #
/// # world.add_observer(|speak: On<Speak>| {
/// #     println!("{}", speak.message);
/// # });
/// #
/// # world.flush();
/// #
/// world.trigger(Speak {
///     message: "Hello!".to_string(),
/// });
/// ```
///
/// # Triggers
///
/// Every [`Event`] has an associated [`Trigger`] implementation (set via [`Event::Trigger`]), which defines which observers will run,
/// what data will be passed to them, and the order they will be run in. Unless you are an internals developer or you have very specific
/// needs, you don't need to worry too much about [`Trigger`]. When you derive [`Event`] (or a more specific event trait like [`EntityEvent`]),
/// a [`Trigger`] will be provided for you.
///
/// The [`Event`] derive defaults [`Event::Trigger`] to [`GlobalTrigger`], which will run all observers that watch for the [`Event`].
///
/// # Entity Events
///
/// For events that "target" a specific [`Entity`], see [`EntityEvent`].
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not an `Event`",
    label = "invalid `Event`",
    note = "consider annotating `{Self}` with `#[derive(Event)]`"
)]
pub trait Event: Send + Sync + Sized + 'static {
    type Trigger<'a>: Trigger<Self>;
}

/// An [`EntityEvent`] is an [`Event`] that is triggered for a specific [`EntityEvent::event_target`] entity:
///
/// ```
/// # use bevy_ecs::prelude::*;
/// # let mut world = World::default();
/// # let entity = world.spawn_empty().id();
/// #[derive(EntityEvent)]
/// struct Explode {
///     entity: Entity,
/// }
///
/// world.add_observer(|event: On<Explode>, mut commands: Commands| {
///     println!("Entity {} goes BOOM!", event.entity);
///     commands.entity(event.entity).despawn();
/// });
///
/// world.trigger(Explode { entity });
/// ```
///
/// [`EntityEvent`] will set [`EntityEvent::event_target`] automatically for named structs with an `entity` field name (as seen above). It also works for tuple structs
/// whose only field is [`Entity`]:
///
/// ```
/// # use bevy_ecs::prelude::*;
/// #[derive(EntityEvent)]
/// struct Explode(Entity);
/// ```
///
/// The [`EntityEvent::event_target`] can also be manually set using the `#[event_target]` field attribute:
///
/// ```
/// # use bevy_ecs::prelude::*;
/// #[derive(EntityEvent)]
/// struct Explode {
///     #[event_target]
///     exploded_entity: Entity,
/// }
/// ```
///
/// ```
/// # use bevy_ecs::prelude::*;
/// #[derive(EntityEvent)]
/// struct Explode(#[event_target] Entity);
/// ```
///
/// You may also use any type which implements [`ContainsEntity`](crate::entity::ContainsEntity) as the event target:
///
/// ```
/// # use bevy_ecs::prelude::*;
/// struct Bomb(Entity);
///
/// impl ContainsEntity for Bomb {
///     fn entity(&self) -> Entity {
///         self.0
///     }
/// }
///
/// #[derive(EntityEvent)]
/// struct Explode(Bomb);
/// ```
///
/// By default, an [`EntityEvent`] is immutable. This means the event data, including the target, does not change while the event
/// is triggered. However, to support event propagation, your event must also implement the [`SetEntityEventTarget`] trait.
///
/// This trait is automatically implemented for you if you enable event propagation:
/// ```
/// # use bevy_ecs::prelude::*;
/// #[derive(EntityEvent)]
/// #[entity_event(propagate)]
/// struct Explode(Entity);
/// ```
///
/// ## Trigger Behavior
///
/// When derived, [`EntityEvent`] defaults to setting [`Event::Trigger`] to [`EntityTrigger`], which will run all normal "untargeted"
/// observers added via [`World::add_observer`], just like a default [`Event`] would (see the example above).
///
/// However it will _also_ run all observers that watch _specific_ entities, which enables you to assign entity-specific logic:
///
/// ```
/// # use bevy_ecs::prelude::*;
/// # #[derive(Component, Debug)]
/// # struct Name(String);
/// # let mut world = World::default();
/// # let e1 = world.spawn_empty().id();
/// # let e2 = world.spawn_empty().id();
/// # #[derive(EntityEvent)]
/// # struct Explode {
/// #    entity: Entity,
/// # }
/// world.entity_mut(e1).observe(|event: On<Explode>, mut commands: Commands| {
///     println!("Boom!");
///     commands.entity(event.entity).despawn();
/// });
///
/// world.entity_mut(e2).observe(|event: On<Explode>, mut commands: Commands| {
///     println!("The explosion fizzles! This entity is immune!");
/// });
/// ```
///
/// ## [`EntityEvent`] Propagation
///
/// When deriving [`EntityEvent`], you can enable "event propagation" (also known as "event bubbling") by
/// specifying the `#[entity_event(propagate)]` attribute:
///
/// ```
/// # use bevy_ecs::prelude::*;
/// #[derive(EntityEvent)]
/// #[entity_event(propagate)]
/// struct Click {
///     entity: Entity,
/// }
/// ```
///
/// This will default to using the [`ChildOf`](crate::hierarchy::ChildOf) component to propagate the [`Event`] "up"
/// the hierarchy (from child to parent).
///
/// You can also specify your own [`Traversal`](crate::traversal::Traversal) implementation. A common pattern is to use
/// [`Relationship`](crate::relationship::Relationship) components, which will follow the relationships to their root
/// (just be sure to avoid cycles ... these aren't detected for performance reasons):
///
/// ```
/// # use bevy_ecs::prelude::*;
/// #[derive(Component)]
/// #[relationship(relationship_target = ClickableBy)]
/// struct Clickable(Entity);
///
/// #[derive(Component)]
/// #[relationship_target(relationship = Clickable)]
/// struct ClickableBy(Vec<Entity>);
///
/// #[derive(EntityEvent)]
/// #[entity_event(propagate = &'static Clickable)]
/// struct Click {
///     entity: Entity,
/// }
/// ```
///
/// By default, propagation requires observers to opt-in:
///
/// ```
/// # use bevy_ecs::prelude::*;
/// #[derive(EntityEvent)]
/// #[entity_event(propagate)]
/// struct Click {
///     entity: Entity,
/// }
///
/// # let mut world = World::default();
/// world.add_observer(|mut click: On<Click>| {
///   // this will propagate the event up to the parent, using `ChildOf`
///   click.propagate(true);
/// });
/// ```
///
/// But you can enable auto propagation using the `#[entity_event(auto_propagate)]` attribute:
/// ```
/// # use bevy_ecs::prelude::*;
/// #[derive(EntityEvent)]
/// #[entity_event(propagate, auto_propagate)]
/// struct Click {
///     entity: Entity,
/// }
/// ```
///
/// You can also _stop_ propagation like this:
/// ```
/// # use bevy_ecs::prelude::*;
/// # #[derive(EntityEvent)]
/// # #[entity_event(propagate)]
/// # struct Click {
/// #    entity: Entity,
/// # }
/// # fn is_finished_propagating() -> bool { true }
/// # let mut world = World::default();
/// world.add_observer(|mut click: On<Click>| {
///   if is_finished_propagating() {
///     click.propagate(false);
///   }
/// });
/// ```
///
/// ## Best practices for event propagation
///
/// Propagation is useful for events that should be handled by multiple entities in a hierarchy, such as UI events.
/// In these cases, it is common for the event to be triggered on a "leaf" entity, and then propagate up to "root" entities.
/// In this pattern, it is generally recommended to trigger the event on the most specific entity possible (the leaf), and then use propagation to have it handled by more general entities (the roots).
///
/// Once an event is handled by a given entity, you should stop propagation.
/// This ensures that only a single "behavior" resolves per event sent,
/// avoiding unexpected behavior from entities higher up the hierarchy.
///
/// This advice has one notable wrinkle:
/// if an entity is "disabled" (e.g. if a UI node is grayed out),
/// the event should still be considered "handled" by that entity,
/// even though the observer logic should not be run.
/// This ensures consistent behavior regardless of the enabled/disabled state of entities.
///
/// ## Naming and Usage Conventions
///
/// In most cases, it is recommended to use a named struct field for the "event target" entity, and to use
/// a name that is descriptive as possible, as this makes events easier to understand and read.
///
/// For events with only one [`Entity`] field, `entity` is often a reasonable name. But if there are multiple
/// [`Entity`] fields, it is often a good idea to use a more descriptive name.
///
/// It is also generally recommended to _consume_ "event target" entities directly via their named field, as this
/// can make the context clearer, allows for more specific documentation hints in IDEs, and it generally reads better.
///
/// ## Manually spawning [`EntityEvent`] observers
///
/// The examples above that call [`EntityWorldMut::observe`] to add entity-specific observer logic are
/// just shorthand for spawning an [`Observer`] directly and manually watching the entity:
///
/// ```
/// # use bevy_ecs::prelude::*;
/// # let mut world = World::default();
/// # let entity = world.spawn_empty().id();
/// # #[derive(EntityEvent)]
/// # struct Explode(Entity);
/// let mut observer = Observer::new(|event: On<Explode>| {});
/// observer.watch_entity(entity);
/// world.spawn(observer);
/// ```
///
/// Note that the [`Observer`] component is not added to the entity it is observing. Observers should always be their own entities, as there
/// can be multiple observers of the same entity!
///
/// You can call [`Observer::watch_entity`] more than once or [`Observer::watch_entities`] to watch multiple entities with the same [`Observer`].
///
/// [`EntityWorldMut::observe`]: crate::world::EntityWorldMut::observe
/// [`Observer`]: crate::observer::Observer
/// [`Observer::watch_entity`]: crate::observer::Observer::watch_entity
/// [`Observer::watch_entities`]: crate::observer::Observer::watch_entities
pub trait EntityEvent: Event {
    /// The [`Entity`] "target" of this [`EntityEvent`]. When triggered, this will run observers that watch for this specific entity.
    fn event_target(&self) -> Entity;
}

/// A trait which is used to set the target of an [`EntityEvent`].
///
/// By default, entity events are immutable; meaning their target does not change during the lifetime of the event. However, some events
/// may require mutable access to provide features such as event propagation.
///
/// You should never need to implement this trait manually if you use `#[derive(EntityEvent)]`. It is automatically implemented for you if you
/// use `#[entity_event(propagate)]`.
pub trait SetEntityEventTarget: EntityEvent {
    /// Sets the [`Entity`] "target" of this [`EntityEvent`]. When triggered, this will run observers that watch for this specific entity.
    ///
    /// Note: In general, this should not be called from within an [`Observer`](crate::observer::Observer), as this will not "retarget"
    /// the event in any of Bevy's built-in [`Trigger`] implementations.
    fn set_event_target(&mut self, entity: Entity);
}

/// An internal type that implements [`Component`] for a given [`Event`] type.
///
/// This exists so we can easily get access to a unique [`ComponentId`] for each [`Event`] type,
/// without requiring that [`Event`] types implement [`Component`] directly.
/// [`ComponentId`] is used internally as a unique identifier for events because they are:
///
/// - Unique to each event type.
/// - Can be quickly generated and looked up.
/// - Are compatible with dynamic event types, which aren't backed by a Rust type.
///
/// This type is an implementation detail and should never be made public.
// TODO: refactor events to store their metadata on distinct entities, rather than using `ComponentId`
#[derive(Component)]
struct EventWrapperComponent<E: Event>(PhantomData<E>);

/// A unique identifier for an [`Event`], used by [observers].
///
/// You can look up the key for your event by calling the [`World::event_key`] method.
///
/// For dynamic events not backed by a Rust type, create an `EventKey` from
/// a [`ComponentId`] using [`EventKey::new`]. Obtain a [`ComponentId`] via
/// [`World::register_component_with_descriptor`].
///
/// [observers]: crate::observer
#[derive(Debug, Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct EventKey(pub(crate) ComponentId);

impl EventKey {
    /// Creates a new [`EventKey`] from a [`ComponentId`].
    ///
    /// Useful for dynamic events not backed by a Rust type. Obtain a
    /// [`ComponentId`] via [`World::register_component_with_descriptor`].
    ///
    /// # Safety
    ///
    /// The caller must ensure that `component_id` was registered for use as
    /// an event (e.g. via [`World::register_component_with_descriptor`]).
    /// Using an unrelated [`ComponentId`] may cause observers to receive
    /// data with an unexpected layout.
    ///
    /// [`World::register_component_with_descriptor`]: crate::world::World::register_component_with_descriptor
    #[inline]
    pub const unsafe fn new(component_id: ComponentId) -> Self {
        Self(component_id)
    }

    /// Returns the underlying [`ComponentId`] for this event key.
    #[inline]
    pub const fn component_id(self) -> ComponentId {
        self.0
    }
}

impl World {
    /// Generates the [`EventKey`] for this event type.
    ///
    /// If this type has already been registered,
    /// this will return the existing [`EventKey`].
    ///
    /// This is used by various dynamically typed observer APIs,
    /// such as [`DeferredWorld::trigger_raw`](crate::world::DeferredWorld::trigger_raw).
    pub fn register_event_key<E: Event>(&mut self) -> EventKey {
        EventKey(self.register_component::<EventWrapperComponent<E>>())
    }

    /// Fetches the [`EventKey`] for this event type,
    /// if it has already been generated.
    ///
    /// This is used by various dynamically typed observer APIs,
    /// such as [`DeferredWorld::trigger_raw`](crate::world::DeferredWorld::trigger_raw).
    pub fn event_key<E: Event>(&self) -> Option<EventKey> {
        self.component_id::<EventWrapperComponent<E>>()
            .map(EventKey)
    }
}
