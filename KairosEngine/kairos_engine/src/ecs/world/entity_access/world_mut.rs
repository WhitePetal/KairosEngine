use crate::{
    debug::{DebugCheckedUnwrap, MaybeLocation},
    ecs::{
        bundle::{Bundle, BundleFromComponents, BundleInserter, BundleRemover, InsertMode},
        component::{Component, ComponentId, StorageType},
        entity::{Entity, EntityLocation},
        relationship::RelationshipHookMode,
        world::{
            World,
            entity_access::{DynamicComponentFetch, EntityMut, EntityRef},
            error::EntityComponentError,
            unsafe_world_cell::UnsafeEntityCell,
        },
    },
    ptr::{MovingPtr, OwningPtr},
};

/// A mutable reference to a particular [`Entity`], and the entire world.
///
/// This is essentially a performance-optimized `(Entity, &mut World)` tuple,
/// which caches the [`EntityLocation`] to reduce duplicate lookups.
///
/// Since this type provides mutable access to the entire world, only one
/// [`EntityWorldMut`] can exist at a time for a given world.
///
/// See also [`EntityMut`], which allows disjoint mutable access to multiple
/// entities at once.  Unlike `EntityMut`, this type allows adding and
/// removing components, and despawning the entity.
///
/// # Invariants and Risk
///
/// An [`EntityWorldMut`] may point to a despawned entity.
/// You can check this via [`is_despawned`](Self::is_despawned).
/// Using an [`EntityWorldMut`] of a despawned entity may panic in some contexts, so read method documentation carefully.
///
/// Unless you have strong reason to assume these invariants, you should generally avoid keeping an [`EntityWorldMut`] to an entity that is potentially not spawned.
/// For example, when inserting a component, that component insert may trigger an observer that despawns the entity.
/// So, when you don't have full knowledge of what commands may interact with this entity,
/// do not further use this value without first checking [`is_despawned`](Self::is_despawned).
pub struct EntityWorldMut<'w> {
    world: &'w mut World,
    entity: Entity,
    location: Option<EntityLocation>,
}

impl<'w> EntityWorldMut<'w> {
    /// # Safety
    ///
    ///  The `location` must be sourced from `world`'s `Entities` and must exactly match the location for `entity`.
    ///  If the `entity` is not spawned for any reason (See [`EntityNotSpawnedError`](crate::entity::EntityNotSpawnedError)), the location should be `None`.
    ///
    ///  The above is trivially satisfied if `location` was sourced from `world.entities().get_spawned(entity).ok()`.
    #[inline]
    pub(crate) unsafe fn new(
        world: &'w mut World,
        entity: Entity,
        location: Option<EntityLocation>,
    ) -> Self {
        debug_assert_eq!(world.entities.get_spawned(entity).ok(), location);

        EntityWorldMut {
            world,
            entity,
            location,
        }
    }

    #[track_caller]
    #[inline(never)]
    #[cold]
    fn panic_despawned(&self) -> ! {
        panic!(
            "Entity {} {}",
            self.entity,
            self.world.entities.get_spawned(self.entity).unwrap_err()
        )
    }

    /// Gets metadata indicating the location where the current entity is stored.
    #[inline]
    pub fn try_location(&self) -> Option<EntityLocation> {
        self.location
    }

    /// Gets metadata indicating the location where the current entity is stored.
    ///
    /// # Panics
    ///
    /// If the entity has been despawned while this `EntityWorldMut` is still alive.
    #[inline]
    pub fn location(&self) -> EntityLocation {
        match self.try_location() {
            Some(a) => a,
            None => self.panic_despawned(),
        }
    }

    /// Inserts a dynamic [`Bundle`] into the entity.
    ///
    /// This will overwrite any previous value(s) of the same component type.
    ///
    /// You should prefer to use the typed API [`EntityWorldMut::insert`] where possible.
    /// If your [`Bundle`] only has one component, use the cached API [`EntityWorldMut::insert_by_id`].
    ///
    /// If possible, pass a sorted slice of `ComponentId` to maximize caching potential.
    ///
    /// # Safety
    /// - Each [`ComponentId`] must be from the same world as [`EntityWorldMut`]
    /// - Each [`OwningPtr`] must be a valid reference to the type represented by [`ComponentId`]
    ///
    /// # Panics
    ///
    /// If the entity has been despawned while this `EntityWorldMut` is still alive.
    #[track_caller]
    pub unsafe fn insert_by_ids<'a, I: Iterator<Item = OwningPtr<'a>>>(
        &mut self,
        component_ids: &[ComponentId],
        iter_components: I,
    ) -> &mut Self {
        self.insert_by_ids_internal(component_ids, iter_components, RelationshipHookMode::Run)
    }

    #[track_caller]
    pub(crate) unsafe fn insert_by_ids_internal<'a, I: Iterator<Item = OwningPtr<'a>>>(
        &mut self,
        component_ids: &[ComponentId],
        iter_components: I,
        relationship_hook_inter_mode: RelationshipHookMode,
    ) -> &mut Self {
        todo!()
    }

    /// Gets read-only access to all of the entity's components.
    #[inline]
    pub fn as_readonly(&self) -> EntityRef<'_> {
        todo!()
    }

    /// Gets access to the component of type `T` for the current entity.
    /// Returns `None` if the entity does not have a component of type `T`.
    ///
    /// # Panics
    ///
    /// If the entity has been despawned while this `EntityWorldMut` is still alive.
    #[inline]
    pub fn get<T: Component>(&self) -> Option<&'_ T> {
        todo!()
    }

    /// Returns `true` if the current entity has a component of type `T`.
    /// Otherwise, this returns `false`.
    ///
    /// ## Notes
    ///
    /// If you do not know the concrete type of a component, consider using
    /// [`Self::contains_id`] or [`Self::contains_type_id`].
    ///
    /// # Panics
    ///
    /// If the entity has been despawned while this `EntityWorldMut` is still alive.
    #[inline]
    pub fn contains<T: Component>(&self) -> bool {
        todo!()
    }

    #[inline(always)]
    fn into_unsafe_entity_cell(self) -> UnsafeEntityCell<'w> {
        let location = self.location();
        let last_change_tick = self.world.last_change_tick;
        let change_tick = self.world.change_tick();
        UnsafeEntityCell::new(
            self.world.as_unsafe_world_cell(),
            self.entity,
            location,
            last_change_tick,
            change_tick,
        )
    }

    /// Consumes `self` and returns non-structural mutable access to all of the
    /// entity's components, with the world `'w` lifetime.
    pub fn into_mutable(self) -> EntityMut<'w> {
        // SAFETY:
        // - We have exclusive access to the entire world.
        // - Consuming `self` ensures there are no other accesses.
        unsafe { EntityMut::new(self.into_unsafe_entity_cell()) }
    }

    pub(crate) fn insert_with_caller<T: Bundle>(
        &mut self,
        bundle: MovingPtr<'_, T>,
        mode: InsertMode,
        caller: MaybeLocation,
        relationship_hook_mode: RelationshipHookMode,
    ) -> &mut Self {
        let location = self.location();
        let change_tick = self.world.change_tick();

        // SAFETY:
        // - `location.archetype_id` is part of a valid `EntityLocation`.
        let mut bundle_inserter =
            unsafe { BundleInserter::new::<T>(self.world, location.archetype_id, change_tick) };

        // SAFETY:
        // - `location` matches current entity and thus must currently exist in the source
        //   archetype for this inserter and its location within the archetype.
        // - `T` matches the type used to create the `BundleInserter`.
        // - `apply_effect` is called exactly once after this function.
        // - The value pointed at by `bundle` is not accessed for anything other than `apply_effect`
        //   and the caller ensures that the value is not accessed or dropped after this function
        //   returns.
        let (bundle, location) = bundle.partial_move(|bundle| unsafe {
            bundle_inserter.insert(
                self.entity,
                location,
                bundle,
                mode,
                caller,
                relationship_hook_mode,
            )
        });
        self.location = Some(location);
        self.world.flush();
        self.update_location();
        // SAFETY:
        // - This is called exactly once after the `BundleInsert::insert` call before returning to safe code.
        // - `bundle` points to the same `B` that `BundleInsert::insert` was called on.
        unsafe { T::apply_effect(bundle, self) };
        self
    }

    /// Updates the internal entity location to match the current location in the internal
    /// [`World`].
    ///
    /// This is *only* required when using the unsafe function [`EntityWorldMut::world_mut`],
    /// which enables the location to change.
    ///
    /// Note that if the entity is not spawned for any reason,
    /// this will have a location of `None`, leading some methods to panic.
    pub fn update_location(&mut self) {
        self.location = self.world.entities().get_spawned(self.entity).ok();
    }

    /// Consumes `self` and returns [untyped mutable reference(s)](MutUntyped)
    /// to component(s) with lifetime `'w` for the current entity, based on the
    /// given [`ComponentId`]s.
    ///
    /// **You should prefer to use the typed API [`EntityWorldMut::into_mut`] where
    /// possible and only use this in cases where the actual component types
    /// are not known at compile time.**
    ///
    /// Unlike [`EntityWorldMut::into_mut`], this returns untyped reference(s) to
    /// component(s), and it's the job of the caller to ensure the correct
    /// type(s) are dereferenced (if necessary).
    ///
    /// # Errors
    ///
    /// - Returns [`EntityComponentError::MissingComponent`] if the entity does
    ///   not have a component.
    /// - Returns [`EntityComponentError::AliasedMutability`] if a component
    ///   is requested multiple times.
    ///
    /// # Examples
    ///
    /// For examples on how to use this method, see [`EntityMut::get_mut_by_id`].
    ///
    /// # Panics
    ///
    /// If the entity has been despawned while this `EntityWorldMut` is still alive.
    #[inline]
    pub fn into_mut_by_id<F: DynamicComponentFetch>(
        self,
        component_ids: F,
    ) -> Result<F::Mut<'w>, EntityComponentError> {
        self.into_mutable().into_mut_by_id(component_ids)
    }

    /// Removes all components in the [`Bundle`] from the entity and returns their previous values.
    ///
    /// **Note:** If the entity does not have every component in the bundle, this method will not
    /// remove any of them.
    ///
    /// # Panics
    ///
    /// If the entity has been despawned while this `EntityWorldMut` is still alive.
    #[must_use]
    #[track_caller]
    pub fn take<T: Bundle + BundleFromComponents>(&mut self) -> Option<T> {
        let location = self.location();
        let entity = self.entity;

        let mut remover = unsafe {
            // SAFETY: The archetype id must be valid since this entity is in it.
            BundleRemover::new::<T>(self.world, location.archetype_id, true)
        }?;

        let (new_location, result) = unsafe {
            remover.remove(
                entity,
                location,
                MaybeLocation::caller(),
                |sets, table, components, bundle_components| {
                    let mut bundle_components = bundle_components.iter().copied();
                    (
                        false,
                        T::from_components(&mut (sets, table), &mut |(sets, table)| {
                            let component_id = bundle_components.next().unwrap();
                            // SAFETY: the component existed to be removed, so its id must be valid.
                            let component_info = components.get_info_unchecked(component_id);
                            match component_info.storage_type() {
                                StorageType::Table => {
                                    table
                                        .as_mut()
                                        // SAFETY: The table must be valid if the component is in it.
                                        .debug_checked_unwrap()
                                        // SAFETY: The remover is cleaning this up.
                                        .take_component(component_id, location.table_row)
                                }
                                StorageType::SparseSet => sets
                                    .get_mut(component_id)
                                    .unwrap()
                                    .remove_and_forget(entity)
                                    .unwrap(),
                            }
                        }),
                    )
                },
            )
        };
        self.location = Some(new_location);

        self.world.flush();
        self.update_location();
        Some(result)
    }
}

// TODO!
