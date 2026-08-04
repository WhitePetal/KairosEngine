use crate::ecs::{
    entity::{Entity, EntityLocation},
    world::World,
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
