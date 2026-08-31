//! Defines the [`World`] and APIs for accessing it directly.

use std::{
    fmt,
    mem::ManuallyDrop,
    sync::atomic::{AtomicU32, Ordering},
};

use crate::{
    debug::{DebugCheckedUnwrap, DebugName, MaybeLocation},
    ecs::{
        archetype::{ArchetypeId, Archetypes},
        bundle::{
            self, Bundle, BundleId, BundleInfo, BundleInserter, BundleSpawner, Bundles,
            DynamicBundle, InsertMode, NoBundleEffect,
        },
        change_detection::{ComponentTicksMut, Mut, MutUntyped, Tick},
        component::{
            Component, ComponentId, ComponentIds, Components, ComponentsRegistrator, Mutable,
        },
        entity::{Entities, Entity, EntityAllocator, EntityNotSpawnedError, SpawnError},
        error::{ErrorHandler, FallbackErrorHandler},
        lifecycle::RemovedComponentMessages,
        observer::Observers,
        query::{QueryData, QueryFilter, QueryState},
        relationship::RelationshipHookMode,
        resource::{Resource, ResourceEntities},
        storage::Storages,
        world::{
            command_queue::RawCommandQueue,
            error::{EntityMutableFetchError, TryInsertBatchError},
            unsafe_world_cell::UnsafeWorldCell,
        },
    },
    move_as_ptr,
    ptr::{MovingPtr, OwningPtr},
};

pub(crate) mod command_queue;

pub mod unsafe_world_cell;

mod deferred_world;
mod entity_access;
mod entity_fetch;
mod filtered_resource;
mod identifier;
mod spawn_batch;

pub mod error;

pub use deferred_world::DeferredWorld;
pub use entity_access::{
    EntityMut, EntityMutExcept, EntityRef, EntityRefExcept, EntityWorldMut, FilteredEntityMut,
    FilteredEntityRef,
};
pub use entity_fetch::{EntityFetcher, WorldEntityFetch};
pub use filtered_resource::*;
pub use identifier::WorldId;
pub use spawn_batch::*;

/// Stores and exposes operations on [entities](Entity), [components](Component), resources,
/// and their associated metadata.
///
/// Each [`Entity`] has a set of unique components, based on their type.
/// Entity components can be created, updated, removed, and queried using a given [`World`].
///
/// For complex access patterns involving [`SystemParam`](crate::system::SystemParam),
/// consider using [`SystemState`](crate::system::SystemState).
///
/// To mutate different parts of the world simultaneously,
/// use [`World::resource_scope`] or [`SystemState`](crate::system::SystemState).
///
/// ## Resources
///
/// Worlds can also store [`Resource`]s,
/// which are unique instances of a given type that belong to a specific unique Entity.
/// There are also *non send resources*, which can only be accessed on the main thread.
/// These are stored outside of the ECS.
/// See [`Resource`] for usage.
pub struct World {
    id: WorldId,
    pub(crate) entities: Entities,
    pub(crate) entity_allocator: EntityAllocator,
    pub(crate) components: Components,
    pub(crate) component_ids: ComponentIds,
    pub(crate) resource_entities: ResourceEntities,
    pub(crate) archetypes: Archetypes,
    pub(crate) storages: Storages,
    pub(crate) bundles: Bundles,
    pub(crate) observers: Observers,
    pub(crate) removed_components: RemovedComponentMessages,
    pub(crate) change_tick: AtomicU32,
    pub(crate) last_change_tick: Tick,
    pub(crate) last_check_tick: Tick,
    pub(crate) last_trigger_id: u32,
    pub(crate) command_queue: RawCommandQueue,
}

/// Creates an instance of the type this trait is implemented for
/// using data from the supplied [`World`].
///
/// This can be helpful for complex initialization or context-aware defaults.
///
/// [`FromWorld`] may be derived for:
/// - any struct whose fields all implement `FromWorld`
/// - any enum where one variant has the attribute `#[from_world]`
///
/// ```rs
///
/// struct C;
///
/// impl FromWorld for C {
///     fn from_world(_world: &mut World) -> Self {
///         Self
///     }
/// }
///
/// #[derive(FromWorld)]
/// struct D(A, B, C);
///
/// #[derive(FromWorld)]
/// enum E {
///     #[from_world]
///     F,
///     G
/// }
/// ```
pub trait FromWorld {
    fn from_world(world: &mut World) -> Self;
}

impl<T: Default> FromWorld for T {
    fn from_world(world: &mut World) -> Self {
        T::default()
    }
}

impl World {
    pub fn new() -> Self {
        todo!()
    }

    /// Retrieves this [`World`]'s unique ID
    #[inline]
    pub fn id(&self) -> WorldId {
        self.id
    }

    /// Creates a new [`UnsafeWorldCell`] view with complete read+write access.
    #[inline]
    pub fn as_unsafe_world_cell(&mut self) -> UnsafeWorldCell<'_> {
        UnsafeWorldCell::new_mutable(self)
    }

    /// Creates a new [`UnsafeWorldCell`] view with only read access to everything.
    #[inline]
    pub fn as_unsafe_world_cell_readonly(&self) -> UnsafeWorldCell<'_> {
        UnsafeWorldCell::new_readonly(self)
    }

    /// Retrieves this world's [`Entities`] collection.
    #[inline]
    pub fn entities(&self) -> &Entities {
        &self.entities
    }

    /// Retrieves this world's [`Archetypes`] collection.
    #[inline]
    pub fn archetypes(&self) -> &Archetypes {
        &self.archetypes
    }

    /// Retrieves this world's [`Components`] collection.
    #[inline]
    pub fn components(&self) -> &Components {
        &self.components
    }

    /// Retrieves this world's [`Storages`] collection.
    #[inline]
    pub fn storages(&self) -> &Storages {
        &self.storages
    }

    /// Retrieves this world's [`Bundles`] collection.
    #[inline]
    pub fn bundles(&self) -> &Bundles {
        &self.bundles
    }

    /// Prepares a [`ComponentsRegistrator`] for the world.
    #[inline]
    pub fn components_registrator(&mut self) -> ComponentsRegistrator<'_> {
        // SAFETY: These are from the same world.
        unsafe { ComponentsRegistrator::new(&mut self.components, &mut self.component_ids) }
    }

    /// Registers a new [`Component`] type and returns the [`ComponentId`] created for it.
    ///
    /// # Usage Notes
    /// In most cases, you don't need to call this method directly since component registration
    /// happens automatically during system initialization.
    #[doc(alias = "register_resource")]
    pub fn register_component<T: Component>(&mut self) -> ComponentId {
        self.components_registrator().register_component::<T>()
    }

    /// Returns the [`ComponentId`] of the given [`Component`] type `T`.
    ///
    /// The returned `ComponentId` is specific to the `World` instance
    /// it was retrieved from and should not be used with another `World` instance.
    ///
    /// Returns [`None`] if the `Component` type has not yet been initialized within
    /// the `World` using [`World::register_component`].
    ///
    /// ```
    /// use bevy_ecs::prelude::*;
    ///
    /// let mut world = World::new();
    ///
    /// #[derive(Component)]
    /// struct ComponentA;
    ///
    /// let component_a_id = world.register_component::<ComponentA>();
    ///
    /// assert_eq!(component_a_id, world.component_id::<ComponentA>().unwrap())
    /// ```
    ///
    /// # See also
    ///
    /// * [`ComponentIdFor`](crate::component::ComponentIdFor)
    /// * [`Components::component_id()`]
    /// * [`Components::get_id()`]
    #[inline]
    pub fn component_id<T: Component>(&self) -> Option<ComponentId> {
        self.components.component_id::<T>()
    }

    /// Reads the current change tick of this world.
    ///
    /// If you have exclusive (`&mut`) access to the world, consider using [`change_tick()`](Self::change_tick),
    /// which is more efficient since it does not require atomic synchronization.
    #[inline]
    pub fn read_change_tick(&self) -> Tick {
        let tick = self.change_tick.load(Ordering::Acquire);
        Tick::new(tick)
    }

    /// Reads the current change tick of this world.
    ///
    /// This does the same thing as [`read_change_tick()`](Self::read_change_tick), only this method
    /// is more efficient since it does not require atomic synchronization.
    #[inline]
    pub fn change_tick(&mut self) -> Tick {
        let tick = *self.change_tick.get_mut();
        Tick::new(tick)
    }

    /// When called from within an exclusive system (a [`System`] that takes `&mut World` as its first
    /// parameter), this method returns the [`Tick`] indicating the last time the exclusive system was run.
    ///
    /// Otherwise, this returns the `Tick` indicating the last time that [`World::clear_trackers`] was called.
    ///
    /// [`System`]: crate::system::System
    #[inline]
    pub fn last_change_tick(&self) -> Tick {
        self.last_change_tick
    }

    /// Returns the id of the last ECS event that was fired.
    /// Used internally to ensure observers don't trigger multiple times for the same event.
    #[inline]
    pub(crate) fn last_trigger_id(&self) -> u32 {
        self.last_trigger_id
    }

    /// Registers all of the components in the given [`Bundle`] and returns both the component
    /// ids and the bundle id.
    ///
    /// This is largely equivalent to calling [`register_component`](Self::register_component) on each
    /// component in the bundle.
    #[inline]
    pub fn register_bundle<B: Bundle>(&mut self) -> &BundleInfo {
        let id = self.register_bundle_info::<B>();

        // SAFETY: We just initialized the bundle so its id should definitely be valid.
        unsafe { self.bundles.get(id).debug_checked_unwrap() }
    }

    pub(crate) fn register_bundle_info<B: Bundle>(&mut self) -> BundleId {
        let mut registrator =
            unsafe { ComponentsRegistrator::new(&mut self.components, &mut self.component_ids) };

        // SAFETY: `registrator`, `self.storages` and `self.bundles` all come from this world.
        unsafe {
            self.bundles
                .register_info::<B>(&mut registrator, &mut self.storages)
        }
    }

    /// Applies any commands in the world's internal [`CommandQueue`].
    /// This does not apply commands from any systems, only those stored in the world.
    ///
    /// # Panics
    /// This will panic if any of the queued commands are [`spawn`](Commands::spawn).
    /// If this is possible, you should instead use [`flush`](Self::flush).
    pub(crate) fn flush_commands(&mut self) {
        todo!()
    }

    /// Applies any queued component registration.
    /// For spawning vanilla rust component types and resources, this is not strictly necessary.
    /// However, flushing components can make information available more quickly, and can have performance benefits.
    /// Additionally, for components and resources registered dynamically through a raw descriptor or similar,
    /// this is the only way to complete their registration.
    pub(crate) fn flush_components(&mut self) {
        self.components_registrator().apply_queued_registrations();
    }

    /// Flushes queued entities and commands.
    ///
    /// Queued entities will be spawned, and then commands will be applied.
    #[inline]
    #[track_caller]
    pub fn flush(&mut self) {
        self.flush_components();
        self.flush_commands();
    }

    /// Spawns a new [`Entity`] and returns a corresponding [`EntityWorldMut`], which can be used
    /// to add components to the entity or retrieve its id.
    ///
    /// ```
    /// use bevy_ecs::{component::Component, world::World};
    ///
    /// #[derive(Component)]
    /// struct Position {
    ///   x: f32,
    ///   y: f32,
    /// }
    /// #[derive(Component)]
    /// struct Label(&'static str);
    /// #[derive(Component)]
    /// struct Num(u32);
    ///
    /// let mut world = World::new();
    /// let entity = world.spawn_empty()
    ///     .insert(Position { x: 0.0, y: 0.0 }) // add a single component
    ///     .insert((Num(1), Label("hello"))) // add a bundle of components
    ///     .id();
    ///
    /// let position = world.entity(entity).get::<Position>().unwrap();
    /// assert_eq!(position.x, 0.0);
    /// ```
    #[track_caller]
    pub fn spawn_empty(&mut self) -> EntityWorldMut<'_> {
        todo!()
    }

    /// Retrieves a reference to the given `entity`'s [`Component`] of the given type.
    /// Returns `None` if the `entity` does not have a [`Component`] of the given type.
    /// ```
    /// use bevy_ecs::{component::Component, world::World};
    ///
    /// #[derive(Component)]
    /// struct Position {
    ///   x: f32,
    ///   y: f32,
    /// }
    ///
    /// let mut world = World::new();
    /// let entity = world.spawn(Position { x: 0.0, y: 0.0 }).id();
    /// let position = world.get::<Position>(entity).unwrap();
    /// assert_eq!(position.x, 0.0);
    /// ```
    #[inline]
    pub fn get<T: Component>(&self, entity: Entity) -> Option<&T> {
        todo!()
    }

    /// Retrieves a mutable reference to the given `entity`'s [`Component`] of the given type.
    /// Returns `None` if the `entity` does not have a [`Component`] of the given type.
    /// ```
    /// use bevy_ecs::{component::Component, world::World};
    ///
    /// #[derive(Component)]
    /// struct Position {
    ///   x: f32,
    ///   y: f32,
    /// }
    ///
    /// let mut world = World::new();
    /// let entity = world.spawn(Position { x: 0.0, y: 0.0 }).id();
    /// let mut position = world.get_mut::<Position>(entity).unwrap();
    /// position.x = 1.0;
    /// ```
    #[inline]
    pub fn get_mut<T: Component<Mutability = Mutable>>(
        &mut self,
        entity: Entity,
    ) -> Option<Mut<'_, T>> {
        todo!()
    }

    /// Gets a reference to the resource of the given type if it exists
    #[inline]
    pub fn get_resource<R: Resource>(&self) -> Option<&R> {
        // SAFETY:
        // - `as_unsafe_world_cell_readonly` gives permission to access everything immutably
        // - `&self` ensures nothing in world is borrowed mutably
        unsafe { self.as_unsafe_world_cell_readonly().get_resource() }
    }

    /// Gets a pointer to the resource with the id [`ComponentId`] if it exists and is mutable.
    /// The returned pointer may be used to modify the resource, as long as the mutable borrow
    /// of the [`World`] is still valid.
    ///
    /// **You should prefer to use the typed API [`World::get_resource_mut`] where possible and only
    /// use this in cases where the actual types are not known at compile time.**
    #[inline]
    pub fn get_resource_mut_by_id(&mut self, component_id: ComponentId) -> Option<MutUntyped<'_>> {
        // SAFETY:
        // - `&mut self` ensures that all accessed data is unaliased
        // - `as_unsafe_world_cell` provides mutable permission to the whole world
        unsafe {
            self.as_unsafe_world_cell()
                .get_resource_mut_by_id(component_id)
        }
    }

    /// Convenience method for accessing the world's fallback error handler,
    /// which can be overwritten with [`FallbackErrorHandler`].
    #[inline]
    pub fn fallback_error_handler(&self) -> ErrorHandler {
        self.get_resource::<FallbackErrorHandler>()
            .copied()
            .unwrap_or_default()
            .0
    }

    /// Returns [`EntityRef`]s that expose read-only operations for the given
    /// `entities`, returning [`Err`] if any of the given entities do not exist.
    /// Instead of immediately unwrapping the value returned from this function,
    /// prefer [`World::entity`].
    ///
    /// This function supports fetching a single entity or multiple entities:
    /// - Pass an [`Entity`] to receive a single [`EntityRef`].
    /// - Pass a slice of [`Entity`]s to receive a [`Vec<EntityRef>`].
    /// - Pass an array of [`Entity`]s to receive an equally-sized array of [`EntityRef`]s.
    /// - Pass a reference to a [`EntityHashSet`](crate::entity::EntityHashMap) to receive an
    ///   [`EntityHashMap<EntityRef>`](crate::entity::EntityHashMap).
    ///
    /// # Errors
    ///
    /// If any of the given `entities` do not exist in the world, the first
    /// [`Entity`] found to be missing will return an [`EntityNotSpawnedError`].
    ///
    /// # Examples
    ///
    /// For examples, see [`World::entity`].
    ///
    /// [`EntityHashSet`]: crate::entity::EntityHashSet
    #[inline]
    pub fn get_entity<F: WorldEntityFetch>(
        &self,
        entities: F,
    ) -> Result<F::Ref<'_>, EntityNotSpawnedError> {
        let cell = self.as_unsafe_world_cell_readonly();
        // SAFETY: `&self` gives read access to the entire world, and prevents mutable access.
        unsafe { entities.fetch_ref(cell) }
    }

    /// Returns [`EntityMut`]s that expose read and write operations for the
    /// given `entities`, returning [`Err`] if any of the given entities do not
    /// exist. Instead of immediately unwrapping the value returned from this
    /// function, prefer [`World::entity_mut`].
    ///
    /// This function supports fetching a single entity or multiple entities:
    /// - Pass an [`Entity`] to receive a single [`EntityWorldMut`].
    ///    - This reference type allows for structural changes to the entity,
    ///      such as adding or removing components, or despawning the entity.
    /// - Pass a slice of [`Entity`]s to receive a [`Vec<EntityMut>`].
    /// - Pass an array of [`Entity`]s to receive an equally-sized array of [`EntityMut`]s.
    /// - Pass a reference to a [`EntityHashSet`](crate::entity::EntityHashMap) to receive an
    ///   [`EntityHashMap<EntityMut>`](crate::entity::EntityHashMap).
    ///
    /// In order to perform structural changes on the returned entity reference,
    /// such as adding or removing components, or despawning the entity, only a
    /// single [`Entity`] can be passed to this function. Allowing multiple
    /// entities at the same time with structural access would lead to undefined
    /// behavior, so [`EntityMut`] is returned when requesting multiple entities.
    ///
    /// # Errors
    ///
    /// - Returns [`EntityMutableFetchError::NotSpawned`] if any of the given `entities` do not exist in the world.
    ///     - Only the first entity found to be missing will be returned.
    /// - Returns [`EntityMutableFetchError::AliasedMutability`] if the same entity is requested multiple times.
    ///
    /// # Examples
    ///
    /// For examples, see [`World::entity_mut`].
    ///
    /// [`EntityHashSet`]: crate::entity::EntityHashSet
    #[inline]
    pub fn get_entity_mut<F: WorldEntityFetch>(
        &mut self,
        entities: F,
    ) -> Result<F::Mut<'_>, EntityMutableFetchError> {
        let cell = self.as_unsafe_world_cell();
        // SAFETY: `&mut self` gives mutable access to the entire world,
        // and prevents any other access to the world.
        unsafe { entities.fetch_mut(cell) }
    }

    /// Returns [`EntityMut`]s that expose read and write operations for the
    /// given `entities`. This will panic if any of the given entities do not
    /// exist. Use [`World::get_entity_mut`] if you want to check for entity
    /// existence instead of implicitly panicking.
    ///
    /// This function supports fetching a single entity or multiple entities:
    /// - Pass an [`Entity`] to receive a single [`EntityWorldMut`].
    ///    - This reference type allows for structural changes to the entity,
    ///      such as adding or removing components, or despawning the entity.
    /// - Pass a slice of [`Entity`]s to receive a [`Vec<EntityMut>`].
    /// - Pass an array of [`Entity`]s to receive an equally-sized array of [`EntityMut`]s.
    /// - Pass a reference to a [`EntityHashSet`](crate::entity::EntityHashMap) to receive an
    ///   [`EntityHashMap<EntityMut>`](crate::entity::EntityHashMap).
    ///
    /// In order to perform structural changes on the returned entity reference,
    /// such as adding or removing components, or despawning the entity, only a
    /// single [`Entity`] can be passed to this function. Allowing multiple
    /// entities at the same time with structural access would lead to undefined
    /// behavior, so [`EntityMut`] is returned when requesting multiple entities.
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
    /// # use bevy_ecs::prelude::*;
    /// #[derive(Component)]
    /// struct Position {
    ///   x: f32,
    ///   y: f32,
    /// }
    ///
    /// let mut world = World::new();
    /// let entity = world.spawn(Position { x: 0.0, y: 0.0 }).id();
    ///
    /// let mut entity_mut = world.entity_mut(entity);
    /// let mut position = entity_mut.get_mut::<Position>().unwrap();
    /// position.y = 1.0;
    /// assert_eq!(position.x, 0.0);
    /// entity_mut.despawn();
    /// # assert!(world.get_entity_mut(entity).is_err());
    /// ```
    ///
    /// ## Array of [`Entity`]s
    ///
    /// ```
    /// # use bevy_ecs::prelude::*;
    /// #[derive(Component)]
    /// struct Position {
    ///   x: f32,
    ///   y: f32,
    /// }
    ///
    /// let mut world = World::new();
    /// let e1 = world.spawn(Position { x: 0.0, y: 0.0 }).id();
    /// let e2 = world.spawn(Position { x: 1.0, y: 1.0 }).id();
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
    /// # use bevy_ecs::prelude::*;
    /// #[derive(Component)]
    /// struct Position {
    ///   x: f32,
    ///   y: f32,
    /// }
    ///
    /// let mut world = World::new();
    /// let e1 = world.spawn(Position { x: 0.0, y: 1.0 }).id();
    /// let e2 = world.spawn(Position { x: 0.0, y: 1.0 }).id();
    /// let e3 = world.spawn(Position { x: 0.0, y: 1.0 }).id();
    ///
    /// let ids = vec![e1, e2, e3];
    /// for mut eref in world.entity_mut(&ids[..]) {
    ///     let mut pos = eref.get_mut::<Position>().unwrap();
    ///     pos.y = 2.0;
    ///     assert_eq!(pos.y, 2.0);
    /// }
    /// ```
    ///
    /// ## [`EntityHashSet`](crate::entity::EntityHashSet)
    ///
    /// ```
    /// # use bevy_ecs::{prelude::*, entity::EntityHashSet};
    /// #[derive(Component)]
    /// struct Position {
    ///   x: f32,
    ///   y: f32,
    /// }
    ///
    /// let mut world = World::new();
    /// let e1 = world.spawn(Position { x: 0.0, y: 1.0 }).id();
    /// let e2 = world.spawn(Position { x: 0.0, y: 1.0 }).id();
    /// let e3 = world.spawn(Position { x: 0.0, y: 1.0 }).id();
    ///
    /// let ids = EntityHashSet::from_iter([e1, e2, e3]);
    /// for (_id, mut eref) in world.entity_mut(&ids) {
    ///     let mut pos = eref.get_mut::<Position>().unwrap();
    ///     pos.y = 2.0;
    ///     assert_eq!(pos.y, 2.0);
    /// }
    /// ```
    ///
    /// [`EntityHashSet`]: crate::entity::EntityHashSet
    #[inline]
    #[track_caller]
    pub fn entity_mut<F: WorldEntityFetch>(&mut self, entities: F) -> F::Mut<'_> {
        #[inline(never)]
        #[cold]
        #[track_caller]
        fn panic_on_err(e: EntityMutableFetchError) -> ! {
            panic!("{e}")
        }

        match self.get_entity_mut(entities) {
            Ok(fetched) => fetched,
            Err(e) => panic_on_err(e),
        }
    }

    /// Spawns `bundle` on `entity`.
    ///
    /// # Panics
    ///
    /// Panics if the entity index is already constructed
    pub(crate) fn spawn_at_unchecked<B: Bundle>(
        &mut self,
        entity: Entity,
        bundle: MovingPtr<'_, B>,
        caller: MaybeLocation,
    ) -> EntityWorldMut<'_> {
        let change_tick = self.change_tick();
        let mut bundle_spawner = BundleSpawner::new::<B>(self, change_tick);
        let (bundle, entity_location) = bundle.partial_move(|bundle| {
            // SAFETY:
            // - `B` matches `bundle_spawner`'s type
            // -  `entity` is allocated but non-existent
            // - `B::Effect` is unconstrained, and `B::apply_effect` is called exactly once on the bundle after this call.
            // - This function ensures that the value pointed to by `bundle` must not be accessed for anything afterwards by consuming
            //   the `MovingPtr`. The value is otherwise only used to call `apply_effect` within this function, and the safety invariants
            //   of `DynamicBundle` ensure that only the elements that have not been moved out of by this call are accessed.
            unsafe { bundle_spawner.spawn_at::<B>(entity, bundle, caller) }
        });

        let mut entity_location = Some(entity_location);

        todo!();

        // SAFETY: The entity and location started as valid.
        // If they were changed by commands, the location was updated to match.
        let mut entity = unsafe { EntityWorldMut::new(self, entity, entity_location) };

        // SAFETY:
        // - This is called exactly once after `get_components` has been called in `spawn_non_existent`.
        // - `bundle` had it's `get_components` function called exactly once inside `spawn_non_existent`.
        unsafe { B::apply_effect(bundle, &mut entity) };
        entity
    }

    /// A faster version of [`spawn_at`](Self::spawn_at) for the empty bundle.
    #[track_caller]
    pub fn spawn_empty_at(&mut self, entity: Entity) -> Result<EntityWorldMut<'_>, SpawnError> {
        self.spawn_empty_at_with_caller(entity, MaybeLocation::caller())
    }

    pub(crate) fn spawn_empty_at_with_caller(
        &mut self,
        entity: Entity,
        caller: MaybeLocation,
    ) -> Result<EntityWorldMut<'_>, SpawnError> {
        self.entities.check_can_spawn_at(entity)?;
        Ok(self.spawn_empty_at_unchecked(entity, caller))
    }

    /// A faster version of [`spawn_at_unchecked`](Self::spawn_at_unchecked) for the empty bundle.
    ///
    /// # Panics
    ///
    /// Panics if the entity index is already spawned
    pub(crate) fn spawn_empty_at_unchecked(
        &mut self,
        entity: Entity,
        caller: MaybeLocation,
    ) -> EntityWorldMut<'_> {
        // SAFETY: Locations are immediately made valid
        unsafe {
            let archetype = self.archetypes.empty_mut();

            let table_row = self.storages.tables[archetype.table_id()].allocate(entity);

            let location = archetype.allocate(entity, table_row);
            let change_tick = self.change_tick();
            let was_at = self.entities.set_location(entity.index(), Some(location));
            assert!(
                was_at.is_none(),
                "Attempting to construct an empty entity, but it was already constructed."
            );
            self.entities
                .mark_spawned_or_despawned(entity.index(), caller, change_tick);

            EntityWorldMut::new(self, entity, Some(location))
        }
    }

    pub(crate) fn spawn_with_caller<B: Bundle>(
        &mut self,
        bundle: MovingPtr<'_, B>,
        caller: MaybeLocation,
    ) -> EntityWorldMut<'_> {
        let entity = self.entity_allocator.alloc();
        // This was just spawned from null, so it shouldn't panic.
        self.spawn_at_unchecked(entity, bundle, caller)
    }

    fn insert_resource_if_not_exists_with_caller<R: Resource>(
        &mut self,
        func: impl FnOnce(&mut World) -> R,
        caller: MaybeLocation,
    ) -> (ComponentId, EntityWorldMut<'_>) {
        let resource_id = self.register_component::<R>();

        if let Some(entity) = self.resource_entities.get(resource_id) {
            let entity_ref = self.get_entity(entity).expect("ResourceCache is in sync");
            if !entity_ref.contains_id(resource_id) {
                let resource = func(self);
                move_as_ptr!(resource);
                self.entity_mut(entity).insert_with_caller(
                    resource,
                    InsertMode::Replace,
                    caller,
                    RelationshipHookMode::Run,
                );
            }
            return (resource_id, self.entity_mut(entity));
        }

        let resource = func(self);
        move_as_ptr!(resource);
        let entity_mut = self.spawn_with_caller(resource, caller); // ResourceCache is updated automatically
        (resource_id, entity_mut)
    }

    /// Initializes a new resource and returns the [`ComponentId`] created for it.
    ///
    /// If the resource already exists, nothing happens.
    ///
    /// The value given by the [`FromWorld::from_world`] method will be used.
    /// Note that any resource with the [`Default`] trait automatically implements [`FromWorld`],
    /// and those default values will be here instead.
    #[inline]
    #[track_caller]
    pub fn init_resource<R: Resource + FromWorld>(&mut self) -> ComponentId {
        let caller = MaybeLocation::caller();
        self.insert_resource_if_not_exists_with_caller(R::from_world, caller)
            .0
    }

    /// Gets a mutable reference to the resource of type `T` if it exists,
    /// otherwise initializes the resource by calling its [`FromWorld`]
    /// implementation.
    ///
    /// # Example
    ///
    /// ```
    /// # use bevy_ecs::prelude::*;
    /// #
    /// #[derive(Resource)]
    /// struct Foo(i32);
    ///
    /// impl Default for Foo {
    ///     fn default() -> Self {
    ///         Self(15)
    ///     }
    /// }
    ///
    /// #[derive(Resource)]
    /// struct MyResource(i32);
    ///
    /// impl FromWorld for MyResource {
    ///     fn from_world(world: &mut World) -> Self {
    ///         let foo = world.get_resource_or_init::<Foo>();
    ///         Self(foo.0 * 2)
    ///     }
    /// }
    ///
    /// # let mut world = World::new();
    /// let my_res = world.get_resource_or_init::<MyResource>();
    /// assert_eq!(my_res.0, 30);
    /// ```
    #[track_caller]
    pub fn get_resource_or_init<R: Resource<Mutability = Mutable> + FromWorld>(
        &mut self,
    ) -> Mut<'_, R> {
        let caller = MaybeLocation::caller();
        let (resource_id, entity) =
            self.insert_resource_if_not_exists_with_caller(R::from_world, caller);
        let untyped = entity
            .into_mut_by_id(resource_id)
            .expect("Resource must exist");
        // SAFETY: resource is of type R
        unsafe { untyped.with_type() }
    }

    /// Removes the resource of a given type and returns it, if it exists. Otherwise returns `None`.
    #[inline]
    pub fn remove_resource<R: Resource>(&mut self) -> Option<R> {
        let resource_id = self.component_id::<R>()?;
        let entity = self.resource_entities.get(resource_id)?;
        let value = self
            .get_entity_mut(entity)
            .expect("ResourceCache is in sync")
            .take::<R>()?;
        Some(value)
    }

    /// Temporarily removes the requested resource from this [`World`], runs custom user code,
    /// then re-adds the resource before returning.
    ///
    /// This enables safe simultaneous mutable access to both a resource and the rest of the [`World`].
    /// For more complex access patterns, consider using [`SystemState`](crate::system::SystemState).
    ///
    /// # Panics
    ///
    /// Panics if the resource does not exist.
    /// Use [`try_resource_scope`](Self::try_resource_scope) instead if you want to handle this case.
    ///
    /// # Example
    /// ```
    /// use bevy_ecs::prelude::*;
    /// #[derive(Resource)]
    /// struct A(u32);
    /// #[derive(Component)]
    /// struct B(u32);
    /// let mut world = World::new();
    /// world.insert_resource(A(1));
    /// let entity = world.spawn(B(1)).id();
    ///
    /// world.resource_scope(|world, mut a: Mut<A>| {
    ///     let b = world.get_mut::<B>(entity).unwrap();
    ///     a.0 += b.0;
    /// });
    /// assert_eq!(world.get_resource::<A>().unwrap().0, 2);
    /// ```
    ///
    /// # Note
    ///
    /// If the world's resource metadata is cleared within the scope, such as by calling
    /// [`World::clear_resources`] or [`World::clear_all`], the resource will *not* be re-inserted
    /// at the end of the scope.
    #[track_caller]
    pub fn resource_scope<R: Resource, U>(&mut self, f: impl FnOnce(&mut World, Mut<R>) -> U) -> U {
        self.try_resource_scope(f)
            .unwrap_or_else(|| panic!("resource does not exist: {}", DebugName::type_name::<R>()))
    }

    /// Temporarily removes the requested resource from this [`World`] if it exists, runs custom user code,
    /// then re-adds the resource before returning. Returns `None` if the resource does not exist in this [`World`].
    ///
    /// This enables safe simultaneous mutable access to both a resource and the rest of the [`World`].
    /// For more complex access patterns, consider using [`SystemState`](crate::system::SystemState).
    ///
    /// See also [`resource_scope`](Self::resource_scope).
    ///
    /// # Note
    ///
    /// If the world's resource metadata is cleared within the scope, such as by calling
    /// [`World::clear_resources`] or [`World::clear_all`], the resource will *not* be re-inserted
    /// at the end of the scope.
    pub fn try_resource_scope<R: Resource, U>(
        &mut self,
        f: impl FnOnce(&mut World, Mut<R>) -> U,
    ) -> Option<U> {
        let last_change_tick = self.last_change_tick();
        let change_tick = self.change_tick();

        let component_id = self.components.valid_component_id::<R>()?;
        let entity = self.resource_entities.get(component_id)?;
        let mut entity_mut = self.get_entity_mut(entity).ok()?;

        let mut ticks = entity_mut.get_change_ticks::<R>()?;
        let changed_by = entity_mut.get_changed_by::<R>()?;
        let value = entity_mut.take::<R>()?;

        struct ReinserGuard<'a, R: Resource> {
            world: &'a mut World,
            entity: Entity,
            component_id: ComponentId,
            value: ManuallyDrop<R>,
            caller: MaybeLocation,
        }
        impl<R: Resource> Drop for ReinserGuard<'_, R> {
            fn drop(&mut self) {
                // take ownership of the value first so it'll get dropped if we return early
                // SAFETY: drop semantics ensure that `self.value` will never be accessed again after this call
                let value = unsafe { ManuallyDrop::take(&mut self.value) };

                let Ok(mut entity_mut) = self.world.get_entity_mut(self.entity) else {
                    return;
                };

                // in debug mode, raise a panic if user code re-inserted a resource of this type within the scope.
                // resource insertion usually indicates a logic error in user code, which is useful to catch at dev time,
                // however it does not inherently lead to corrupted state, so we avoid introducing an unnecessary crash
                // for production builds.
                if entity_mut.contains_id(self.component_id) {
                    #[cfg(debug_assertions)]
                    {
                        use crate::debug::DebugName;

                        if std::thread::panicking() {
                            use crate::debug::DebugName;

                            log::error!(
                                "Resource `{}` was inserted during a call to World::resource_scope, which may result in unexpected behacior.\n\
                                In release builds, the value inserted will be overwritten at the end of the scope.",
                                DebugName::type_name::<R>()
                            );
                            // return early to maintain consistent behavior with non-panicking calls in debug builds
                            return;
                        }

                        panic!(
                            "Resource `{}` was inserted during a call to World::resource_scope, which may result in unexpected behacior.\n\
                            In release builds, the value inserted will be overwritten at the end of the scope.",
                            DebugName::type_name::<R>()
                        );
                    }
                    #[cfg(not(debug_assertions))]
                    {
                        #[cold]
                        #[inline(never)]
                        fn warn_reinsert(resource_name: &str) {
                            warn!(
                                "Resource `{resource_name}` was inserted during a call to World::resource_scope: the inserted value will be overwritten.",
                            );
                        }

                        warn_reinsert(&DebugName::type_name::<R>());
                    }
                }

                move_as_ptr!(value);

                // See EntityWorldMut::insert_with_caller for the original code.
                // This is copied here to update the change ticks. This way we can ensure that the commands
                // ran during self.flush(), interact with the correct ticks on the resource component.
                {
                    let location = entity_mut.location();
                    // SAFETY:
                    // - We update the entity location like in `EntityWorldMut::insert_with_caller`.
                    let world = unsafe { entity_mut.world_mut() };
                    let tick = world.change_tick();
                    // SAFETY:
                    // - `location.archetype_id` is part of a valid `EntityLocation`.
                    let mut bundle_inserter =
                        unsafe { BundleInserter::new::<R>(world, location.archetype_id, tick) };
                    // SAFETY:
                    // - `location` matches current entity and thus must currently exist in the source
                    //   archetype for this inserter and its location within the archetype.
                    // - `T` matches the type used to create the `BundleInserter`.
                    // - `apply_effect` is called exactly once after this function.
                    // - The value pointed at by `bundle` is not accessed for anything other than `apply_effect`
                    //   and the caller ensures that the value is not accessed or dropped after this function
                    //   returns.
                    let (bundle, _) = value.partial_move(|bundle| unsafe {
                        bundle_inserter.insert(
                            self.entity,
                            location,
                            bundle,
                            InsertMode::Replace,
                            self.caller,
                            RelationshipHookMode::Run,
                        )
                    });
                    entity_mut.update_location();

                    // SAFETY: We update the entity location afterwards.
                    unsafe { entity_mut.world_mut() }.flush();

                    entity_mut.update_location();
                    // SAFETY:
                    // - This is called exactly once after the `BundleInsert::insert` call before returning to safe code.
                    // - `bundle` points to the same `B` that `BundleInsert::insert` was called on.
                    unsafe { R::apply_effect(bundle, &mut entity_mut) };
                }
            }
        }

        let mut guard = ReinserGuard {
            world: self,
            entity,
            component_id,
            value: ManuallyDrop::new(value),
            caller: changed_by,
        };

        let value_mut = Mut {
            value: &mut *guard.value,
            ticks: ComponentTicksMut {
                added: &mut ticks.added,
                changed: &mut ticks.changed,
                changed_by: guard.caller.as_mut(),
                last_run: last_change_tick,
                this_run: change_tick,
            },
        };

        let result = f(guard.world, value_mut);

        Some(result)
    }

    /// Returns [`QueryState`] for the given filtered [`QueryData`], which is used to efficiently
    /// run queries on the [`World`] by storing and reusing the [`QueryState`].
    /// ```
    /// use bevy_ecs::{component::Component, entity::Entity, world::World, query::With};
    ///
    /// #[derive(Component)]
    /// struct A;
    /// #[derive(Component)]
    /// struct B;
    ///
    /// let mut world = World::new();
    /// let e1 = world.spawn(A).id();
    /// let e2 = world.spawn((A, B)).id();
    ///
    /// let mut query = world.query_filtered::<Entity, With<B>>();
    /// let matching_entities = query.iter(&world).collect::<Vec<Entity>>();
    ///
    /// assert_eq!(matching_entities, vec![e2]);
    /// ```
    #[inline]
    pub fn query_filtered<D: QueryData, F: QueryFilter>(&mut self) -> QueryState<D, F> {
        QueryState::new(self)
    }

    /// Returns [`QueryState`] for the given [`QueryData`], which is used to efficiently
    /// run queries on the [`World`] by storing and reusing the [`QueryState`].
    /// ```
    /// use bevy_ecs::{component::Component, entity::Entity, world::World};
    ///
    /// #[derive(Component, Debug, PartialEq)]
    /// struct Position {
    ///   x: f32,
    ///   y: f32,
    /// }
    ///
    /// #[derive(Component)]
    /// struct Velocity {
    ///   x: f32,
    ///   y: f32,
    /// }
    ///
    /// let mut world = World::new();
    /// let entities = world.spawn_batch(vec![
    ///     (Position { x: 0.0, y: 0.0}, Velocity { x: 1.0, y: 0.0 }),
    ///     (Position { x: 0.0, y: 0.0}, Velocity { x: 0.0, y: 1.0 }),
    /// ]).collect::<Vec<Entity>>();
    ///
    /// let mut query = world.query::<(&mut Position, &Velocity)>();
    /// for (mut position, velocity) in query.iter_mut(&mut world) {
    ///    position.x += velocity.x;
    ///    position.y += velocity.y;
    /// }
    ///
    /// assert_eq!(world.get::<Position>(entities[0]).unwrap(), &Position { x: 1.0, y: 0.0 });
    /// assert_eq!(world.get::<Position>(entities[1]).unwrap(), &Position { x: 0.0, y: 1.0 });
    /// ```
    ///
    /// To iterate over entities in a deterministic order,
    /// sort the results of the query using the desired component as a key.
    /// Note that this requires fetching the whole result set from the query
    /// and allocation of a [`Vec`] to store it.
    ///
    /// ```
    /// use bevy_ecs::{component::Component, entity::Entity, world::World};
    ///
    /// #[derive(Component, PartialEq, Eq, PartialOrd, Ord, Debug)]
    /// struct Order(i32);
    /// #[derive(Component, PartialEq, Debug)]
    /// struct Label(&'static str);
    ///
    /// let mut world = World::new();
    /// let a = world.spawn((Order(2), Label("second"))).id();
    /// let b = world.spawn((Order(3), Label("third"))).id();
    /// let c = world.spawn((Order(1), Label("first"))).id();
    /// let mut entities = world.query::<(Entity, &Order, &Label)>()
    ///     .iter(&world)
    ///     .collect::<Vec<_>>();
    /// // Sort the query results by their `Order` component before comparing
    /// // to expected results. Query iteration order should not be relied on.
    /// entities.sort_by_key(|e| e.1);
    /// assert_eq!(entities, vec![
    ///     (c, &Order(1), &Label("first")),
    ///     (a, &Order(2), &Label("second")),
    ///     (b, &Order(3), &Label("third")),
    /// ]);
    /// ```
    #[inline]
    pub fn query<D: QueryData>(&mut self) -> QueryState<D, ()> {
        self.query_filtered::<D, ()>()
    }

    /// Spawns a new [`Entity`] with a given [`Bundle`] of [components](`Component`) and returns
    /// a corresponding [`EntityWorldMut`], which can be used to add components to the entity or
    /// retrieve its id. In case large batches of entities need to be spawned, consider using
    /// [`World::spawn_batch`] instead.
    ///
    /// ```
    /// use bevy_ecs::{bundle::Bundle, component::Component, world::World};
    ///
    /// #[derive(Component)]
    /// struct Position {
    ///   x: f32,
    ///   y: f32,
    /// }
    ///
    /// #[derive(Component)]
    /// struct Velocity {
    ///     x: f32,
    ///     y: f32,
    /// };
    ///
    /// #[derive(Component)]
    /// struct Name(&'static str);
    ///
    /// #[derive(Bundle)]
    /// struct PhysicsBundle {
    ///     position: Position,
    ///     velocity: Velocity,
    /// }
    ///
    /// let mut world = World::new();
    ///
    /// // `spawn` can accept a single component:
    /// world.spawn(Position { x: 0.0, y: 0.0 });
    ///
    /// // It can also accept a tuple of components:
    /// world.spawn((
    ///     Position { x: 0.0, y: 0.0 },
    ///     Velocity { x: 1.0, y: 1.0 },
    /// ));
    ///
    /// // Or it can accept a pre-defined Bundle of components:
    /// world.spawn(PhysicsBundle {
    ///     position: Position { x: 2.0, y: 2.0 },
    ///     velocity: Velocity { x: 0.0, y: 4.0 },
    /// });
    ///
    /// let entity = world
    ///     // Tuples can also mix Bundles and Components
    ///     .spawn((
    ///         PhysicsBundle {
    ///             position: Position { x: 2.0, y: 2.0 },
    ///             velocity: Velocity { x: 0.0, y: 4.0 },
    ///         },
    ///         Name("Elaina Proctor"),
    ///     ))
    ///     // Calling id() will return the unique identifier for the spawned entity
    ///     .id();
    /// let position = world.entity(entity).get::<Position>().unwrap();
    /// assert_eq!(position.x, 2.0);
    /// ```
    #[track_caller]
    pub fn spawn<B: Bundle>(&mut self, bundle: B) -> EntityWorldMut<'_> {
        move_as_ptr!(bundle);
        self.spawn_with_caller(bundle, MaybeLocation::caller())
    }

    /// Split into a new function so we can differentiate the calling location.
    ///
    /// This can be called by:
    /// - [`World::try_insert_batch`]
    /// - [`World::try_insert_batch_if_new`]
    /// - [`Commands::insert_batch`]
    /// - [`Commands::insert_batch_if_new`]
    /// - [`Commands::try_insert_batch`]
    /// - [`Commands::try_insert_batch_if_new`]
    #[inline]
    pub(crate) fn try_insert_batch_with_caller<I, B>(
        &mut self,
        batch: I,
        insert_mode: InsertMode,
        caller: MaybeLocation,
    ) -> Result<(), TryInsertBatchError>
    where
        I: IntoIterator,
        I::IntoIter: Iterator<Item = (Entity, B)>,
        B: Bundle<Effect: NoBundleEffect>,
    {
        struct InserterArchetypeCache<'w> {
            inserter: BundleInserter<'w>,
            archetype_id: ArchetypeId,
        }

        let change_tick = self.change_tick();
        let bundle_id = self.register_bundle_info::<B>();

        let mut invalid_entities = Vec::<Entity>::new();
        let mut batch_iter = batch.into_iter();

        // We need to find the first valid entity so we can initialize the bundle inserter.
        // This differs from `insert_batch_with_caller` because that method can just panic
        // if the first entity is invalid, whereas this method needs to keep going.
        let cache = loop {
            if let Some((first_entity, first_bundle)) = batch_iter.next() {
                if let Ok(first_location) = self.entities().get_spawned(first_entity) {
                    let mut cache = InserterArchetypeCache {
                        // SAFETY: we initialized this bundle_id in `register_bundle_info`
                        inserter: unsafe {
                            BundleInserter::new_with_id(
                                self,
                                first_location.archetype_id,
                                bundle_id,
                                change_tick,
                            )
                        },
                        archetype_id: first_location.archetype_id,
                    };

                    move_as_ptr!(first_bundle);
                    // SAFETY:
                    // - `entity` is valid, `location` matches entity, bundle matches inserter
                    // - `apply_effect` is never called on this bundle.
                    // - `first_bundle` is not be accessed or dropped after this.
                    unsafe {
                        cache.inserter.insert(
                            first_entity,
                            first_location,
                            first_bundle,
                            insert_mode,
                            caller,
                            RelationshipHookMode::Run,
                        )
                    };
                    break Some(cache);
                }
                invalid_entities.push(first_entity);
            } else {
                // We reached the end of the entities the caller provided and none were valid.
                break None;
            }
        };

        if let Some(mut cache) = cache {
            for (entity, bundle) in batch_iter {
                if let Ok(location) = cache.inserter.entities().get_spawned(entity) {
                    if location.archetype_id != cache.archetype_id {
                        cache = InserterArchetypeCache {
                            // SAFETY: we initialized this bundle_id in `register_info`
                            inserter: unsafe {
                                BundleInserter::new_with_id(
                                    self,
                                    location.archetype_id,
                                    bundle_id,
                                    change_tick,
                                )
                            },
                            archetype_id: location.archetype_id,
                        }
                    }

                    move_as_ptr!(bundle);
                    // SAFETY:
                    // - `entity` is valid, `location` matches entity, bundle matches inserter
                    // - `apply_effect` is never called on this bundle.
                    // - `bundle` is not be accessed or dropped after this.
                    unsafe {
                        cache.inserter.insert(
                            entity,
                            location,
                            bundle,
                            insert_mode,
                            caller,
                            RelationshipHookMode::Run,
                        )
                    };
                } else {
                    invalid_entities.push(entity);
                }
            }
        }

        if invalid_entities.is_empty() {
            Ok(())
        } else {
            Err(TryInsertBatchError {
                bundle_type: DebugName::type_name::<B>(),
                entities: invalid_entities,
            })
        }
    }

    /// Split into a new function so we can pass the calling location into the function when using
    /// as a command.
    #[inline]
    pub(crate) fn insert_resource_with_caller<R: Resource>(
        &mut self,
        value: R,
        caller: MaybeLocation,
    ) {
        let component_id = self.components_registrator().register_component::<R>();
        OwningPtr::make(value, |ptr| {
            // SAFETY: component_id was just initialized and corresponds to resource of type R.
            unsafe {
                self.insert_resouce_by_id(component_id, ptr, caller);
            }
        });
    }

    /// Inserts a new resource with the given `value`. Will replace the value if it already existed.
    ///
    /// **You should prefer to use the typed API [`World::insert_resource`] where possible and only
    /// use this in cases where the actual types are not known at compile time.**
    ///
    /// # Safety
    /// The value referenced by `value` must be valid for the given [`ComponentId`] of this world.
    #[inline]
    #[track_caller]
    pub unsafe fn insert_resource_by_id(
        &mut self,
        component_id: ComponentId,
        value: OwningPtr<'_>,
        caller: MaybeLocation,
    ) {
        // if the resource already exists, we replace it on the same entity
        let mut entity_mut = if let Some(entity) = self.resource_entities.get(component_id) {
            self.get_entity_mut(entity)
                .expect("ResourceCache is in sync")
        } else {
            self.spawn_empty()
        };
        entity_mut.insert_by_id_with_caller(
            component_id,
            value,
            InsertMode::Replace,
            caller,
            RelationshipHookMode::Run,
        );
    }
}

impl fmt::Debug for World {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // SAFETY: `UnsafeWorldCell` requires that this must only access metadata.
        // Accessing any data stored in the world would be unsound.
        f.debug_struct("World")
            .field("id", &self.id)
            .field("entity_count", &self.entities.count_spawned())
            .field("archetype_count", &self.archetypes.len())
            .field("component_count", &self.components.len())
            .finish()
    }
}
