use std::ops::Deref;

use crate::{
    debug::MaybeLocation,
    ecs::{
        archetype::Archetype,
        change_detection::Mut,
        component::{Component, ComponentId, Mutable},
        entity::Entity,
        event::{Event, EventKey},
        lifecycle::HookContext,
        relationship::RelationshipHookMode,
        system::Commands,
        world::{
            World, WorldEntityFetch, error::EntityMutableFetchError,
            unsafe_world_cell::UnsafeWorldCell,
        },
    },
};

/// A [`World`] reference that disallows structural ECS changes.
/// This includes initializing resources, registering components or spawning entities.
///
/// This means that in order to add entities, for example, you will need to use commands instead of the world directly.
pub struct DeferredWorld<'w> {
    // SAFETY: Implementers must not use this reference to make structural changes
    world: UnsafeWorldCell<'w>,
}

impl<'w> Deref for DeferredWorld<'w> {
    type Target = World;

    fn deref(&self) -> &Self::Target {
        // SAFETY: Structural changes cannot be made through &World
        unsafe { self.world.world() }
    }
}

impl<'w> DeferredWorld<'w> {
    /// Reborrow self as a new instance of [`DeferredWorld`]
    #[inline]
    pub fn reborrow(&mut self) -> DeferredWorld<'_> {
        DeferredWorld { world: self.world }
    }

    /// Triggers all `on_add` hooks for [`ComponentId`] in target.
    ///
    /// # Safety
    /// Caller must ensure [`ComponentId`] in target exist in self.
    #[inline]
    pub(crate) unsafe fn trigger_on_add(
        &mut self,
        archetype: &Archetype,
        entity: Entity,
        targets: impl Iterator<Item = ComponentId>,
        caller: MaybeLocation,
    ) {
        todo!()
    }

    /// Triggers all `on_insert` hooks for [`ComponentId`] in target.
    ///
    /// # Safety
    /// Caller must ensure [`ComponentId`] in target exist in self.
    #[inline]
    pub(crate) unsafe fn trigger_on_insert(
        &mut self,
        archetype: &Archetype,
        entity: Entity,
        targets: impl Iterator<Item = ComponentId>,
        callder: MaybeLocation,
        relationship_hook_mode: RelationshipHookMode,
    ) {
        todo!()
    }

    /// Triggers all `on_discard` hooks for [`ComponentId`] in target.
    ///
    /// # Safety
    /// Caller must ensure [`ComponentId`] in target exist in self.
    pub(crate) unsafe fn trigger_on_discard(
        &mut self,
        archetype: &Archetype,
        entity: Entity,
        targets: impl Iterator<Item = ComponentId>,
        callder: MaybeLocation,
        relationship_hook_mode: RelationshipHookMode,
    ) {
        todo!()
    }

    /// Triggers all `on_remove` hooks for [`ComponentId`] in target.
    ///
    /// # Safety
    /// Caller must ensure [`ComponentId`] in target exist in self.
    #[inline]
    pub(crate) unsafe fn trigger_on_remove(
        &mut self,
        archetype: &Archetype,
        entity: Entity,
        targets: impl Iterator<Item = ComponentId>,
        caller: MaybeLocation,
    ) {
        todo!()
    }

    /// Triggers all `on_despawn` hooks for [`ComponentId`] in target.
    ///
    /// # Safety
    /// Caller must ensure [`ComponentId`] in target exist in self.
    #[inline]
    pub(crate) unsafe fn trigger_on_despawn(
        &mut self,
        arhcetype: &Archetype,
        entity: Entity,
        targets: impl Iterator<Item = ComponentId>,
        caller: MaybeLocation,
    ) {
        if arhcetype.has_despawn_hook() {
            for component_id in targets {
                // SAFETY: Caller ensures that these components exist
                let hooks = unsafe { self.components().get_info_unchecked(component_id) }.hooks();
                if let Some(hook) = hooks.on_despawn {
                    hook(
                        DeferredWorld { world: self.world },
                        HookContext {
                            entity,
                            component_id,
                            caller,
                            relationship_hook_mode: RelationshipHookMode::Run,
                        },
                    )
                }
            }
        }
    }

    /// Triggers all `event` observers for the given `targets`
    ///
    /// # Safety
    /// - Caller must ensure `E` is accessible as the type represented by `event_key`
    #[inline]
    pub unsafe fn trigger_raw<'a, E: Event>(
        &mut self,
        event_key: EventKey,
        event: &mut E,
        trigger: &mut E::Trigger<'a>,
        caller: MaybeLocation,
    ) {
        todo!()
    }

    /// Sends a global [`Event`] without any targets.
    ///
    /// This will run any [`Observer`] of the given [`Event`] that isn't scoped to specific targets.
    ///
    /// [`Observer`]: crate::observer::Observer
    pub fn trigger<'a>(&mut self, event: impl Event<Trigger<'a>: Default>) {
        todo!()
    }

    /// Creates a [`Commands`] instance that pushes to the world's command queue
    #[inline]
    pub fn commands(&mut self) -> Commands<'_, '_> {
        todo!()
    }

    /// Returns [`EntityMut`]s that expose read and write operations for the
    /// given `entities`, returning [`Err`] if any of the given entities do not
    /// exist. Instead of immediately unwrapping the value returned from this
    /// function, prefer [`World::entity_mut`].
    ///
    /// This function supports fetching a single entity or multiple entities:
    /// - Pass an [`Entity`] to receive a single [`EntityMut`].
    /// - Pass a slice of [`Entity`]s to receive a [`Vec<EntityMut>`].
    /// - Pass an array of [`Entity`]s to receive an equally-sized array of [`EntityMut`]s.
    /// - Pass an [`&EntityHashSet`] to receive an [`EntityHashMap<EntityMut>`].
    ///
    /// **As [`DeferredWorld`] does not allow structural changes, all returned
    /// references are [`EntityMut`]s, which do not allow structural changes
    /// (i.e. adding/removing components or despawning the entity).**
    ///
    /// # Errors
    ///
    /// - Returns [`EntityMutableFetchError::NotSpawned`] if any of the given `entities` do not exist in the world.
    ///     - Only the first entity found to be missing will be returned.
    /// - Returns [`EntityMutableFetchError::AliasedMutability`] if the same entity is requested multiple times.
    ///
    /// # Examples
    ///
    /// For examples, see [`DeferredWorld::entity_mut`].
    ///
    /// [`EntityMut`]: crate::world::EntityMut
    /// [`&EntityHashSet`]: crate::entity::EntityHashSet
    /// [`EntityHashMap<EntityMut>`]: crate::entity::EntityHashMap
    /// [`Vec<EntityMut>`]: alloc::vec::Vec
    #[inline]
    pub fn get_entity_mut<F: WorldEntityFetch>(
        &mut self,
        entities: F,
    ) -> Result<F::DeferredMut<'_>, EntityMutableFetchError> {
        let cell = self.as_unsafe_world_cell();
        // SAFETY: `&mut self` gives mutable access to the entire world,
        // and prevents any other access to the world.
        unsafe { entities.fetch_deferred_mut(cell) }
    }

    /// Retrieves a mutable reference to the given `entity`'s [`Component`] of the given type.
    /// Returns `None` if the `entity` does not have a [`Component`] of the given type.
    #[inline]
    pub fn get_mut<T: Component<Mutability = Mutable>>(
        &mut self,
        entity: Entity,
    ) -> Option<Mut<'_, T>> {
        self.get_entity_mut(entity).ok()?.into_mut()
    }

    /// Returns [`EntityMut`]s that expose read and write operations for the
    /// given `entities`. This will panic if any of the given entities do not
    /// exist. Use [`DeferredWorld::get_entity_mut`] if you want to check for
    /// entity existence instead of implicitly panicking.
    ///
    /// This function supports fetching a single entity or multiple entities:
    /// - Pass an [`Entity`] to receive a single [`EntityMut`].
    /// - Pass a slice of [`Entity`]s to receive a [`Vec<EntityMut>`].
    /// - Pass an array of [`Entity`]s to receive an equally-sized array of [`EntityMut`]s.
    /// - Pass an [`&EntityHashSet`] to receive an [`EntityHashMap<EntityMut>`].
    ///
    /// **As [`DeferredWorld`] does not allow structural changes, all returned
    /// references are [`EntityMut`]s, which do not allow structural changes
    /// (i.e. adding/removing components or despawning the entity).**
    ///
    /// # Panics
    ///
    /// If any of the given `entities` do not exist in the world.
    ///
    /// # Examples
    ///
    /// ## Single [`Entity`]
    ///
    /// ```
    /// # use bevy_ecs::{prelude::*, world::DeferredWorld};
    /// #[derive(Component)]
    /// struct Position {
    ///   x: f32,
    ///   y: f32,
    /// }
    ///
    /// # let mut world = World::new();
    /// # let entity = world.spawn(Position { x: 0.0, y: 0.0 }).id();
    /// let mut world: DeferredWorld = // ...
    /// #   DeferredWorld::from(&mut world);
    ///
    /// let mut entity_mut = world.entity_mut(entity);
    /// let mut position = entity_mut.get_mut::<Position>().unwrap();
    /// position.y = 1.0;
    /// assert_eq!(position.x, 0.0);
    /// ```
    ///
    /// ## Array of [`Entity`]s
    ///
    /// ```
    /// # use bevy_ecs::{prelude::*, world::DeferredWorld};
    /// #[derive(Component)]
    /// struct Position {
    ///   x: f32,
    ///   y: f32,
    /// }
    ///
    /// # let mut world = World::new();
    /// # let e1 = world.spawn(Position { x: 0.0, y: 0.0 }).id();
    /// # let e2 = world.spawn(Position { x: 1.0, y: 1.0 }).id();
    /// let mut world: DeferredWorld = // ...
    /// #   DeferredWorld::from(&mut world);
    ///
    /// let [mut e1_ref, mut e2_ref] = world.entity_mut([e1, e2]);
    /// let mut e1_position = e1_ref.get_mut::<Position>().unwrap();
    /// e1_position.x = 1.0;
    /// assert_eq!(e1_position.x, 1.0);
    /// let mut e2_position = e2_ref.get_mut::<Position>().unwrap();
    /// e2_position.x = 2.0;
    /// assert_eq!(e2_position.x, 2.0);
    /// ```
    ///
    /// ## Slice of [`Entity`]s
    ///
    /// ```
    /// # use bevy_ecs::{prelude::*, world::DeferredWorld};
    /// #[derive(Component)]
    /// struct Position {
    ///   x: f32,
    ///   y: f32,
    /// }
    ///
    /// # let mut world = World::new();
    /// # let e1 = world.spawn(Position { x: 0.0, y: 1.0 }).id();
    /// # let e2 = world.spawn(Position { x: 0.0, y: 1.0 }).id();
    /// # let e3 = world.spawn(Position { x: 0.0, y: 1.0 }).id();
    /// let mut world: DeferredWorld = // ...
    /// #   DeferredWorld::from(&mut world);
    ///
    /// let ids = vec![e1, e2, e3];
    /// for mut eref in world.entity_mut(&ids[..]) {
    ///     let mut pos = eref.get_mut::<Position>().unwrap();
    ///     pos.y = 2.0;
    ///     assert_eq!(pos.y, 2.0);
    /// }
    /// ```
    ///
    /// ## [`&EntityHashSet`]
    ///
    /// ```
    /// # use bevy_ecs::{prelude::*, entity::EntityHashSet, world::DeferredWorld};
    /// #[derive(Component)]
    /// struct Position {
    ///   x: f32,
    ///   y: f32,
    /// }
    ///
    /// # let mut world = World::new();
    /// # let e1 = world.spawn(Position { x: 0.0, y: 1.0 }).id();
    /// # let e2 = world.spawn(Position { x: 0.0, y: 1.0 }).id();
    /// # let e3 = world.spawn(Position { x: 0.0, y: 1.0 }).id();
    /// let mut world: DeferredWorld = // ...
    /// #   DeferredWorld::from(&mut world);
    ///
    /// let ids = EntityHashSet::from_iter([e1, e2, e3]);
    /// for (_id, mut eref) in world.entity_mut(&ids) {
    ///     let mut pos = eref.get_mut::<Position>().unwrap();
    ///     pos.y = 2.0;
    ///     assert_eq!(pos.y, 2.0);
    /// }
    /// ```
    ///
    /// [`EntityMut`]: crate::world::EntityMut
    /// [`&EntityHashSet`]: crate::entity::EntityHashSet
    /// [`EntityHashMap<EntityMut>`]: crate::entity::EntityHashMap
    /// [`Vec<EntityMut>`]: alloc::vec::Vec
    #[inline]
    pub fn entity_mut<F: WorldEntityFetch>(&mut self, entities: F) -> F::DeferredMut<'_> {
        todo!()
    }

    /// Gets an [`UnsafeWorldCell`] containing the underlying world.
    ///
    /// # Safety
    /// - must only be used to make non-structural ECS changes
    #[inline]
    pub fn as_unsafe_world_cell(&mut self) -> UnsafeWorldCell<'_> {
        self.world
    }
}

impl<'w> UnsafeWorldCell<'w> {
    /// Turn self into a [`DeferredWorld`]
    ///
    /// # Safety
    /// Caller must ensure there are no outstanding mutable references to world and no
    /// outstanding references to the world's command queue, resource or component data
    #[inline]
    pub unsafe fn into_deferred(self) -> DeferredWorld<'w> {
        DeferredWorld { world: self }
    }
}

impl<'w> From<&'w mut World> for DeferredWorld<'w> {
    fn from(world: &'w mut World) -> DeferredWorld<'w> {
        DeferredWorld {
            world: world.as_unsafe_world_cell(),
        }
    }
}

// TODO!
