use crate::ecs::{
    entity::{Entity, EntityNotSpawnedError},
    world::{
        EntityWorldMut,
        entity_access::{EntityMut, EntityRef},
        error::EntityMutableFetchError,
        unsafe_world_cell::UnsafeWorldCell,
    },
};

/// Types that can be used to fetch [`Entity`] references from a [`World`].
///
/// Provided implementations are:
/// - [`Entity`]: Fetch a single entity.
/// - `[Entity; N]`/`&[Entity; N]`: Fetch multiple entities, receiving a
///   same-sized array of references.
/// - `&[Entity]`: Fetch multiple entities, receiving a vector of references.
/// - [`&EntityHashSet`](EntityHashSet): Fetch multiple entities, receiving a
///   hash map of [`Entity`] IDs to references.
///
/// # Performance
///
/// - The slice and array implementations perform an aliased mutability check
///   in [`WorldEntityFetch::fetch_mut`] that is `O(N^2)`.
/// - The single [`Entity`] implementation performs no such check as only one
///   reference is returned.
///
/// # Safety
///
/// Implementor must ensure that:
/// - No aliased mutability is caused by the returned references.
/// - [`WorldEntityFetch::fetch_ref`] returns only read-only references.
/// - [`WorldEntityFetch::fetch_deferred_mut`] returns only non-structurally-mutable references.
///
/// [`World`]: crate::world::World
pub unsafe trait WorldEntityFetch {
    /// The read-only reference type returned by [`WorldEntityFetch::fetch_ref`].
    type Ref<'w>;

    /// The mutable reference type returned by [`WorldEntityFetch::fetch_mut`].
    type Mut<'w>;

    /// The mutable reference type returned by [`WorldEntityFetch::fetch_deferred_mut`],
    /// but without structural mutability.
    type DeferredMut<'w>;

    /// Returns read-only reference(s) to the entities with the given
    /// [`Entity`] IDs, as determined by `self`.
    ///
    /// # Safety
    ///
    /// It is the caller's responsibility to ensure that:
    /// - The given [`UnsafeWorldCell`] has read-only access to the fetched entities.
    /// - No other mutable references to the fetched entities exist at the same time.
    ///
    /// # Errors
    ///
    /// - Returns [`EntityNotSpawnedError`] if the entity does not exist.
    unsafe fn fetch_ref(
        self,
        cell: UnsafeWorldCell<'_>,
    ) -> Result<Self::Ref<'_>, EntityNotSpawnedError>;

    /// Returns mutable reference(s) to the entities with the given [`Entity`]
    /// IDs, as determined by `self`.
    ///
    /// # Safety
    ///
    /// It is the caller's responsibility to ensure that:
    /// - The given [`UnsafeWorldCell`] has mutable access to the fetched entities.
    /// - No other references to the fetched entities exist at the same time.
    ///
    /// # Errors
    ///
    /// - Returns [`EntityMutableFetchError::NotSpawned`] if the entity does not exist.
    /// - Returns [`EntityMutableFetchError::AliasedMutability`] if the entity was
    ///   requested mutably more than once.
    unsafe fn fetch_mut(
        self,
        cell: UnsafeWorldCell<'_>,
    ) -> Result<Self::Mut<'_>, EntityMutableFetchError>;

    /// Returns mutable reference(s) to the entities with the given [`Entity`]
    /// IDs, as determined by `self`, but without structural mutability.
    ///
    /// No structural mutability means components cannot be removed from the
    /// entity, new components cannot be added to the entity, and the entity
    /// cannot be despawned.
    ///
    /// # Safety
    ///
    /// It is the caller's responsibility to ensure that:
    /// - The given [`UnsafeWorldCell`] has mutable access to the fetched entities.
    /// - No other references to the fetched entities exist at the same time.
    ///
    /// # Errors
    ///
    /// - Returns [`EntityMutableFetchError::NotSpawned`] if the entity does not exist.
    /// - Returns [`EntityMutableFetchError::AliasedMutability`] if the entity was
    ///   requested mutably more than once.
    unsafe fn fetch_deferred_mut(
        self,
        cell: UnsafeWorldCell<'_>,
    ) -> Result<Self::DeferredMut<'_>, EntityMutableFetchError>;
}

// SAFETY:
// - No aliased mutability is caused because a single reference is returned.
// - No mutable references are returned by `fetch_ref`.
// - No structurally-mutable references are returned by `fetch_deferred_mut`.
unsafe impl WorldEntityFetch for Entity {
    type Ref<'w> = EntityRef<'w>;

    type Mut<'w> = EntityWorldMut<'w>;

    type DeferredMut<'w> = EntityMut<'w>;

    #[inline]
    unsafe fn fetch_ref(
        self,
        cell: UnsafeWorldCell<'_>,
    ) -> Result<Self::Ref<'_>, EntityNotSpawnedError> {
        let ecell = cell.get_entity(self)?;

        Ok(unsafe { EntityRef::new(ecell) })
    }

    #[inline]
    unsafe fn fetch_mut(
        self,
        cell: UnsafeWorldCell<'_>,
    ) -> Result<Self::Mut<'_>, EntityMutableFetchError> {
        let location = cell.entities().get_spawned(self)?;
        // SAFETY: caller ensures that the world cell has mutable access to the entity.
        let world = unsafe { cell.world_mut() };
        // SAFETY: location was fetched from the same world's `Entities`.
        Ok(unsafe { EntityWorldMut::new(world, self, Some(location)) })
    }

    #[inline]
    unsafe fn fetch_deferred_mut(
        self,
        cell: UnsafeWorldCell<'_>,
    ) -> Result<Self::DeferredMut<'_>, EntityMutableFetchError> {
        let ecell = cell.get_entity(self)?;
        // SAFETY: caller ensures that the world cell has mutable access to the entity.
        Ok(unsafe { EntityMut::new(ecell) })
    }
}

/// Provides a safe interface for non-structural access to the entities in a [`World`].
///
/// This cannot add or remove components, or spawn or despawn entities,
/// making it relatively safe to access in concert with other ECS data.
/// This type can be constructed via [`World::entities_and_commands`],
/// or [`DeferredWorld::entities_and_commands`].
///
/// [`World`]: crate::world::World
/// [`World::entities_and_commands`]: crate::world::World::entities_and_commands
/// [`DeferredWorld::entities_and_commands`]: crate::world::DeferredWorld::entities_and_commands
pub struct EntityFetcher<'w> {
    cell: UnsafeWorldCell<'w>,
}

impl<'w> EntityFetcher<'w> {
    // SAFETY:
    // - The given `cell` has mutable access to all entities.
    // - No other references to entities exist at the same time.
    pub(crate) unsafe fn new(cell: UnsafeWorldCell<'w>) -> Self {
        Self { cell }
    }

    /// Returns [`EntityRef`]s that expose read-only operations for the given
    /// `entities`, returning [`Err`] if any of the given entities do not exist.
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
    /// [`World::entity`]: crate::world::World::entity
    #[inline]
    pub fn get<F: WorldEntityFetch>(
        &self,
        entities: F,
    ) -> Result<F::Ref<'_>, EntityNotSpawnedError> {
        // SAFETY: `&self` gives read access to all entities, and prevents mutable access.
        unsafe { entities.fetch_ref(self.cell) }
    }
}

// TODO!
