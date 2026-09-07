use std::{ops::Deref, slice};

use kairos_ecs_macros::FromTemplate;

use crate::ecs::{
    bundle::Bundle,
    component::Component,
    entity::Entity,
    relationship::{RelatedSpawner, RelatedSpawnerCommands},
    system::EntityCommands,
    world::{EntityWorldMut, FromWorld},
};

#[cfg(test)]
mod tests;

/// Stores the parent entity of this child entity with this component.
///
/// This is a [`Relationship`] component, and creates the canonical
/// "parent / child" hierarchy. This is the "source of truth" component, and it pairs with
/// the [`Children`] [`RelationshipTarget`](crate::relationship::RelationshipTarget).
///
/// This relationship should be used for things like:
///
/// 1. Organizing entities in a scene
/// 2. Propagating configuration or data inherited from a parent, such as "visibility" or "world-space global transforms".
/// 3. Ensuring a hierarchy is despawned when an entity is despawned.
///
/// [`ChildOf`] contains a single "target" [`Entity`]. When [`ChildOf`] is inserted on a "source" entity,
/// the "target" entity will automatically (and immediately, via a component hook) have a [`Children`]
/// component inserted, and the "source" entity will be added to that [`Children`] instance.
///
/// If the [`ChildOf`] component is replaced with a different "target" entity, the old target's [`Children`]
/// will be automatically (and immediately, via a component hook) be updated to reflect that change.
///
/// Likewise, when the [`ChildOf`] component is removed, the "source" entity will be removed from the old
/// target's [`Children`]. If this results in [`Children`] being empty, [`Children`] will be automatically removed.
///
/// When a parent is despawned, all children (and their descendants) will _also_ be despawned.
///
/// You can create parent-child relationships in a variety of ways. The most direct way is to insert a [`ChildOf`] component:
///
/// ```
/// # use bevy_ecs::prelude::*;
/// # let mut world = World::new();
/// let root = world.spawn_empty().id();
/// let child1 = world.spawn(ChildOf(root)).id();
/// let child2 = world.spawn(ChildOf(root)).id();
/// let grandchild = world.spawn(ChildOf(child1)).id();
///
/// assert_eq!(&**world.entity(root).get::<Children>().unwrap(), &[child1, child2]);
/// assert_eq!(&**world.entity(child1).get::<Children>().unwrap(), &[grandchild]);
///
/// world.entity_mut(child2).remove::<ChildOf>();
/// assert_eq!(&**world.entity(root).get::<Children>().unwrap(), &[child1]);
///
/// world.entity_mut(root).despawn();
/// assert!(world.get_entity(root).is_err());
/// assert!(world.get_entity(child1).is_err());
/// assert!(world.get_entity(grandchild).is_err());
/// ```
///
/// However if you are spawning many children, you might want to use the [`EntityWorldMut::with_children`] helper instead:
///
/// ```
/// # use bevy_ecs::prelude::*;
/// # let mut world = World::new();
/// let mut child1 = Entity::PLACEHOLDER;
/// let mut child2 = Entity::PLACEHOLDER;
/// let mut grandchild = Entity::PLACEHOLDER;
/// let root = world.spawn_empty().with_children(|p| {
///     child1 = p.spawn_empty().with_children(|p| {
///         grandchild = p.spawn_empty().id();
///     }).id();
///     child2 = p.spawn_empty().id();
/// }).id();
///
/// assert_eq!(&**world.entity(root).get::<Children>().unwrap(), &[child1, child2]);
/// assert_eq!(&**world.entity(child1).get::<Children>().unwrap(), &[grandchild]);
/// ```
///
/// [`Relationship`]: crate::relationship::Relationship
#[derive(Component, FromTemplate, Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "kairos_reflect", derive(bevy_reflect::Reflect))]
#[cfg_attr(
    feature = "kairos_reflect",
    reflect(Component, PartialEq, Debug, FromWorld, Clone)
)]
#[derive(serde::Serialize, serde::Deserialize)]
#[cfg_attr(all(feature = "kairos_reflect"), reflect(Serialize, Deserialize))]
#[relationship(relationship_target = Children)]
#[doc(alias = "IsChild", alias = "Parent")]
pub struct ChildOf(pub Entity);

impl ChildOf {
    /// The parent entity of this child entity.
    #[inline]
    pub fn parent(&self) -> Entity {
        self.0
    }
}

// TODO: We need to impl either FromWorld or Default so ChildOf can be registered as Reflect.
// This is because Reflect deserialize by creating an instance and apply a patch on top.
// However ChildOf should only ever be set with a real user-defined entity.  Its worth looking into
// better ways to handle cases like this.
impl FromWorld for ChildOf {
    fn from_world(_world: &mut super::world::World) -> Self {
        ChildOf(Entity::PLACEHOLDER)
    }
}

/// Tracks which entities are children of this parent entity.
///
/// A [`RelationshipTarget`] collection component that is populated
/// with entities that "target" this entity with the [`ChildOf`] [`Relationship`] component.
///
/// Together, these components form the "canonical parent-child hierarchy". See the [`ChildOf`] component for the full
/// description of this relationship and instructions on how to use it.
///
/// # Usage
///
/// Like all [`RelationshipTarget`] components, this data should not be directly manipulated to avoid desynchronization.
/// Instead, modify the [`ChildOf`] components on the "source" entities.
///
/// To access the children of an entity, you can iterate over the [`Children`] component,
/// using the [`IntoIterator`] trait.
/// For more complex access patterns, see the [`RelationshipTarget`] trait.
///
/// [`Relationship`]: crate::relationship::Relationship
/// [`RelationshipTarget`]: crate::relationship::RelationshipTarget
#[derive(Component, Default, Debug, PartialEq, Eq)]
#[relationship_target(relationship = ChildOf, linked_spawn)]
#[cfg_attr(feature = "kairos_reflect", derive(kairos_reflect::Reflect))]
#[cfg_attr(feature = "kairos_reflect", reflect(Component, FromWorld, Default))]
#[doc(alias = "IsParent")]
pub struct Children(Vec<Entity>);

impl Children {
    /// Swaps the child at `a_index` with the child at `b_index`.
    #[inline]
    pub fn swap(&mut self, a_index: usize, b_index: usize) {
        self.0.swap(a_index, b_index);
    }

    /// Sorts children [stably](https://en.wikipedia.org/wiki/Sorting_algorithm#Stability)
    /// in place using the provided comparator function.
    ///
    /// For the underlying implementation, see [`slice::sort_by`].
    ///
    /// For the unstable version, see [`sort_unstable_by`](Children::sort_unstable_by).
    ///
    /// See also [`sort_by_key`](Children::sort_by_key), [`sort_by_cached_key`](Children::sort_by_cached_key).
    #[inline]
    pub fn sort_by<F>(&mut self, compare: F)
    where
        F: FnMut(&Entity, &Entity) -> core::cmp::Ordering,
    {
        self.0.sort_by(compare);
    }

    /// Sorts children [stably](https://en.wikipedia.org/wiki/Sorting_algorithm#Stability)
    /// in place using the provided key extraction function.
    ///
    /// For the underlying implementation, see [`slice::sort_by_key`].
    ///
    /// For the unstable version, see [`sort_unstable_by_key`](Children::sort_unstable_by_key).
    ///
    /// See also [`sort_by`](Children::sort_by), [`sort_by_cached_key`](Children::sort_by_cached_key).
    #[inline]
    pub fn sort_by_key<K, F>(&mut self, compare: F)
    where
        F: FnMut(&Entity) -> K,
        K: Ord,
    {
        self.0.sort_by_key(compare);
    }

    /// Sorts children [stably](https://en.wikipedia.org/wiki/Sorting_algorithm#Stability)
    /// in place using the provided key extraction function. Only evaluates each key at most
    /// once per sort, caching the intermediate results in memory.
    ///
    /// For the underlying implementation, see [`slice::sort_by_cached_key`].
    ///
    /// See also [`sort_by`](Children::sort_by), [`sort_by_key`](Children::sort_by_key).
    #[inline]
    pub fn sort_by_cached_key<K, F>(&mut self, compare: F)
    where
        F: FnMut(&Entity) -> K,
        K: Ord,
    {
        self.0.sort_by_cached_key(compare);
    }

    /// Sorts children [unstably](https://en.wikipedia.org/wiki/Sorting_algorithm#Stability)
    /// in place using the provided comparator function.
    ///
    /// For the underlying implementation, see [`slice::sort_unstable_by`].
    ///
    /// For the stable version, see [`sort_by`](Children::sort_by).
    ///
    /// See also [`sort_unstable_by_key`](Children::sort_unstable_by_key).
    #[inline]
    pub fn sort_unstable_by<F>(&mut self, compare: F)
    where
        F: FnMut(&Entity, &Entity) -> core::cmp::Ordering,
    {
        self.0.sort_unstable_by(compare);
    }

    /// Sorts children [unstably](https://en.wikipedia.org/wiki/Sorting_algorithm#Stability)
    /// in place using the provided key extraction function.
    ///
    /// For the underlying implementation, see [`slice::sort_unstable_by_key`].
    ///
    /// For the stable version, see [`sort_by_key`](Children::sort_by_key).
    ///
    /// See also [`sort_unstable_by`](Children::sort_unstable_by).
    #[inline]
    pub fn sort_unstable_by_key<K, F>(&mut self, compare: F)
    where
        F: FnMut(&Entity) -> K,
        K: Ord,
    {
        self.0.sort_unstable_by_key(compare);
    }
}

impl<'a> IntoIterator for &'a Children {
    type Item = <Self::IntoIter as Iterator>::Item;

    type IntoIter = slice::Iter<'a, Entity>;

    #[inline(always)]
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl Deref for Children {
    type Target = [Entity];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A type alias over [`RelatedSpawner`] used to spawn child entities containing a [`ChildOf`] relationship.
pub type ChildSpawner<'w> = RelatedSpawner<'w, ChildOf>;

/// A type alias over [`RelatedSpawnerCommands`] used to spawn child entities containing a [`ChildOf`] relationship.
pub type ChildSpawnerCommands<'w> = RelatedSpawnerCommands<'w, ChildOf>;

impl<'w> EntityWorldMut<'w> {
    /// Spawns children of this entity (with a [`ChildOf`] relationship) by taking a function that operates on a [`ChildSpawner`].
    /// See also [`with_related`](Self::with_related).
    pub fn with_children(&mut self, func: impl FnOnce(&mut ChildSpawner)) -> &mut Self {
        self.with_related_entities(func);
        self
    }

    /// Adds the given children to this entity.
    /// See also [`add_related`](Self::add_related).
    pub fn add_children(&mut self, children: &[Entity]) -> &mut Self {
        self.add_related::<ChildOf>(children)
    }

    /// Removes all the parent-child relationships from this entity.
    /// To despawn the child entities, instead use [`EntityWorldMut::despawn_children`](EntityWorldMut::despawn_children).
    /// See also [`detach_all_related`](Self::detach_all_related)
    pub fn detach_all_children(&mut self) -> &mut Self {
        self.detach_all_related::<ChildOf>()
    }

    /// Insert children at specific index.
    /// See also [`insert_related`](Self::insert_related).
    pub fn insert_children(&mut self, index: usize, children: &[Entity]) -> &mut Self {
        self.insert_related::<ChildOf>(index, children)
    }

    /// Insert child at specific index.
    /// See also [`insert_related`](Self::insert_related).
    pub fn insert_child(&mut self, index: usize, child: Entity) -> &mut Self {
        self.insert_related::<ChildOf>(index, &[child])
    }

    /// Adds the given child to this entity.
    /// See also [`add_related`](Self::add_related).
    pub fn add_child(&mut self, child: Entity) -> &mut Self {
        self.add_related::<ChildOf>(&[child])
    }

    /// Removes the parent-child relationship between this entity and the given entities.
    /// Does not despawn the children.
    pub fn detach_children(&mut self, children: &[Entity]) -> &mut Self {
        self.remove_related::<ChildOf>(children)
    }

    /// Removes the parent-child relationship between this entity and the given entity.
    /// Does not despawn the child.
    pub fn detach_child(&mut self, child: Entity) -> &mut Self {
        self.remove_related::<ChildOf>(&[child])
    }

    /// Replaces all the related children with a new set of children.
    pub fn replace_children(&mut self, children: &[Entity]) -> &mut Self {
        self.replace_related::<ChildOf>(children)
    }

    /// Replaces all the related children with a new set of children.
    ///
    /// # Warning
    ///
    /// Failing to maintain the functions invariants may lead to erratic engine behavior including random crashes.
    /// Refer to [`Self::replace_related_with_difference`] for a list of these invariants.
    ///
    /// # Panics
    ///
    /// Panics when debug assertions are enabled if an invariant is broken and the command is executed.
    pub fn replace_children_with_difference(
        &mut self,
        entities_to_unrelate: &[Entity],
        entities_to_relate: &[Entity],
        newly_related_entities: &[Entity],
    ) -> &mut Self {
        self.replace_related_with_difference::<ChildOf>(
            entities_to_unrelate,
            entities_to_relate,
            newly_related_entities,
        )
    }

    /// Spawns the passed bundle and adds it to this entity as a child.
    ///
    /// For efficient spawning of multiple children, use [`with_children`].
    ///
    /// [`with_children`]: EntityWorldMut::with_children
    pub fn with_child(&mut self, bundle: impl Bundle) -> &mut Self {
        let parent = self.id();
        self.world_scope(|world| {
            world.spawn((bundle, ChildOf(parent)));
        });
        self
    }
}

impl<'a> EntityCommands<'a> {
    /// Spawns children of this entity (with a [`ChildOf`] relationship) by taking a function that operates on a [`ChildSpawner`].
    pub fn with_children(
        &mut self,
        func: impl FnOnce(&mut RelatedSpawnerCommands<ChildOf>),
    ) -> &mut Self {
        self.with_related_entities(func);
        self
    }

    /// Adds the given children to this entity.
    pub fn add_children(&mut self, children: &[Entity]) -> &mut Self {
        self.add_related::<ChildOf>(children)
    }

    /// Removes all the parent-child relationships from this entity.
    /// To despawn the child entities, instead use [`EntityWorldMut::despawn_children`](EntityWorldMut::despawn_children).
    /// See also [`detach_all_related`](Self::detach_all_related)
    pub fn detach_all_children(&mut self) -> &mut Self {
        self.detach_all_related::<ChildOf>()
    }

    /// Insert children at specific index.
    /// See also [`insert_related`](Self::insert_related).
    pub fn insert_children(&mut self, index: usize, children: &[Entity]) -> &mut Self {
        self.insert_related::<ChildOf>(index, children)
    }

    /// Insert children at specific index.
    /// See also [`insert_related`](Self::insert_related).
    pub fn insert_child(&mut self, index: usize, child: Entity) -> &mut Self {
        self.insert_related::<ChildOf>(index, &[child])
    }

    /// Adds the given child to this entity.
    pub fn add_child(&mut self, child: Entity) -> &mut Self {
        self.add_related::<ChildOf>(&[child])
    }

    /// Removes the parent-child relationship between this entity and the given entities.
    /// Does not despawn the children.
    pub fn detach_children(&mut self, children: &[Entity]) -> &mut Self {
        self.remove_related::<ChildOf>(children)
    }

    /// Removes the parent-child relationship between this entity and the given entity.
    /// Does not despawn the child.
    pub fn detach_child(&mut self, child: Entity) -> &mut Self {
        self.remove_related::<ChildOf>(&[child])
    }

    /// Replaces the children on this entity with a new list of children.
    pub fn replace_children(&mut self, children: &[Entity]) -> &mut Self {
        self.replace_related::<ChildOf>(children)
    }

    /// Replaces all the related entities with a new set of entities.
    ///
    /// # Warning
    ///
    /// Failing to maintain the functions invariants may lead to erratic engine behavior including random crashes.
    /// Refer to [`EntityWorldMut::replace_related_with_difference`] for a list of these invariants.
    ///
    /// # Panics
    ///
    /// Panics when debug assertions are enabled if an invariant is broken and the command is executed.
    pub fn replace_children_with_difference(
        &mut self,
        entities_to_unrelate: &[Entity],
        entities_to_relate: &[Entity],
        newly_related_entities: &[Entity],
    ) -> &mut Self {
        self.replace_related_with_difference::<ChildOf>(
            entities_to_unrelate,
            entities_to_relate,
            newly_related_entities,
        )
    }

    /// Spawns the passed bundle and adds it to this entity as a child.
    ///
    /// For efficient spawning of multiple children, use [`with_children`].
    ///
    /// [`with_children`]: EntityCommands::with_children
    pub fn with_child(&mut self, bundle: impl Bundle) -> &mut Self {
        self.with_related::<ChildOf>(bundle);
        self
    }
}

/// Returns a [`SpawnRelatedBundle`] that will insert the [`Children`] component, spawn a [`SpawnableList`] of entities with given bundles that
/// relate to the [`Children`] entity via the [`ChildOf`] component, and reserve space in the [`Children`] for each spawned entity.
///
/// Any additional arguments will be interpreted as bundles to be spawned.
///
/// Also see [`related`](crate::related) for a version of this that works with any [`RelationshipTarget`] type.
///
/// ```
/// # use bevy_ecs::hierarchy::Children;
/// # use bevy_ecs::name::Name;
/// # use bevy_ecs::world::World;
/// # use bevy_ecs::children;
/// let mut world = World::new();
/// world.spawn((
///     Name::new("Root"),
///     children![
///         Name::new("Child1"),
///         (
///             Name::new("Child2"),
///             children![Name::new("Grandchild")]
///         )
///     ]
/// ));
/// ```
///
/// [`RelationshipTarget`]: crate::relationship::RelationshipTarget
/// [`SpawnRelatedBundle`]: crate::spawn::SpawnRelatedBundle
/// [`SpawnableList`]: crate::spawn::SpawnableList
#[macro_export]
macro_rules! children {
    [$($child:expr),*$(,)?] => {
        $crate::related!($crate::ecs::hierarchy::Children [$($child),*])
    };
}
