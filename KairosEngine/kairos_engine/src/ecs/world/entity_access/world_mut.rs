use crate::{
    ecs::{
        component::{Component, ComponentId},
        entity::{Entity, EntityLocation},
        relationship::RelationshipHookMode,
        world::{World, entity_access::EntityRef},
    },
    ptr::OwningPtr,
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
}
