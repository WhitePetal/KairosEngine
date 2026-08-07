use crate::ecs::{archetype::Archetype, component::ComponentId, event::{EntityEvent, Event}};

pub unsafe trait Trigger<E: Event> {}

/// A [`Trigger`] that runs _every_ "global" [`Observer`](crate::observer::Observer) (ex: registered via [`World::add_observer`](crate::world::World::add_observer))
/// that matches the given [`Event`].
///
/// The [`Event`] derive defaults to using this [`Trigger`], and it is usable for any [`Event`] type.
#[derive(Default, Debug)]
pub struct GlobalTrigger;

unsafe impl<E: for<'a> Event<Trigger<'a> = Self>> Trigger<E> for GlobalTrigger {}


/// An [`EntityEvent`] [`Trigger`] that, in addition to behaving like a normal [`EntityTrigger`], _also_ runs observers
/// that watch for components that match the slice of [`ComponentId`]s referenced in [`EntityComponentsTrigger`]. This includes
/// both _global_ observers of those components and "entity scoped" observers that watch the [`EntityEvent::event_target`].
///
/// This is used by Bevy's built-in [lifecycle events](crate::lifecycle).
#[derive(Default)]
pub struct EntityComponentsTrigger<'a> {
    /// All of the components whose observers were triggered together for the target entity. For example,
    /// if components `A` and `B` are added together, producing the [`Add`](crate::lifecycle::Add) event, this will
    /// contain the [`ComponentId`] for both `A` and `B`.
    pub components: &'a [ComponentId],

    /// The [`Archetype`] of the target entity before this change, or `None` if the entity was just spawned.
    /// For observers that run before the change, like [`Discard`](crate::lifecycle::Discard) and [`Remove`](crate::lifecycle::Remove), this will be the current archetype.
    ///
    /// This can be useful in [`Insert`](crate::lifecycle::Insert) and [`Add`](crate::lifecycle::Add) observers,
    /// since the old archetype will not include any other components added at the same time.
    ///
    /// Note that `None` should usually be treated the same as an archetype with no components,
    /// since spawning an entity should be equivalent to spawning an empty entity and then inserting all components.
    ///
    /// # Example
    /// ```
    /// # use bevy_ecs::{
    /// #     component::ComponentIdFor, entity::EntityHashSet, entity_disabling::Disabled,
    /// #     prelude::*,
    /// # };
    /// # #[derive(Component)]
    /// # struct A;
    /// # #[derive(Resource)]
    /// # struct EntitiesWithA(EntityHashSet);
    /// #
    /// # let mut world = World::new();
    /// #
    /// fn on_add_disable(
    ///     on: On<Add, Disabled>,
    ///     mut cache: ResMut<EntitiesWithA>,
    ///     a_component: ComponentIdFor<A>,
    /// ) {
    ///     // The `A` component may have been added at the same time as `Disabled`,
    ///     // either due to an insert or spawn.  Only try to remove this entity from
    ///     // our cache if the `A` component was in the old archetype.
    ///     if on.trigger().old_archetype.is_some_and(|a| a.contains(*a_component)) {
    ///         cache.0.remove(&on.entity);
    ///     }
    /// }
    /// #
    /// # world.add_observer(on_add_disable);
    /// ```
    pub old_archetype: Option<&'a Archetype>,

    /// The [`Archetype`] of the target entity after this change, or `None` if the entity will be despawned.
    /// For observers that run after the change, like [`Insert`](crate::lifecycle::Insert) and [`Add`](crate::lifecycle::Add), this will be the current archetype.
    ///
    /// This can be useful in [`Discard`](crate::lifecycle::Discard) and [`Remove`](crate::lifecycle::Remove) observers,
    /// since the new archetype will not include any other components removed at the same time.
    ///
    /// Note that `None` should usually be treated the same as an archetype with no components,
    /// since despawning an entity should be equivalent to removing all its components and then despawning the empty entity.
    ///
    /// # Example
    /// ```
    /// # use bevy_ecs::{
    /// #     component::ComponentIdFor, entity::EntityHashSet, entity_disabling::Disabled,
    /// #     prelude::*,
    /// # };
    /// # #[derive(Component)]
    /// # struct A;
    /// # #[derive(Resource)]
    /// # struct EntitiesWithA(EntityHashSet);
    /// #
    /// # let mut world = World::new();
    /// #
    /// fn on_remove_disable(
    ///     on: On<Remove, Disabled>,
    ///     mut cache: ResMut<EntitiesWithA>,
    ///     a_component: ComponentIdFor<A>,
    /// ) {
    ///     // The `A` component may have been removed at the same time as `Disabled`,
    ///     // either due to a remove or despawn.  Only try to add this entity to our
    ///     // cache if the `A` component is still in the new archetype.
    ///     if on.trigger().new_archetype.is_some_and(|a| a.contains(*a_component)) {
    ///         cache.0.insert(on.entity);
    ///     }
    /// }
    /// #
    /// # world.add_observer(on_remove_disable);
    /// ```
    pub new_archetype: Option<&'a Archetype>,
}

unsafe impl<'a, E: EntityEvent + Event<Trigger<'a> = EntityComponentsTrigger<'a>>> Trigger<E> for EntityComponentsTrigger<'a> {

}
