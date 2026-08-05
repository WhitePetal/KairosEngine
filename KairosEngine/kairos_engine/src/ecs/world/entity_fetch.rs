use crate::ecs::{entity::EntityNotSpawnedError, world::{error::EntityMutableFetchError, unsafe_world_cell::UnsafeWorldCell}};


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
        cell: UnsafeWorldCell<'_>
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
        cell: UnsafeWorldCell<'_>
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
        cell: UnsafeWorldCell<'_>
    ) -> Result<Self::DeferredMut<'_>, EntityMutableFetchError>;
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

    pub fn get
}
