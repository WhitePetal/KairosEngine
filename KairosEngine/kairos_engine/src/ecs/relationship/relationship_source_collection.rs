use std::{
    collections::{BTreeSet, btree_set},
    hash::BuildHasher,
    ops::{Deref, DerefMut},
};

use indexmap::IndexSet;
use smallvec::SmallVec;

use crate::ecs::entity::{Entity, EntityHashSet, EntityIndexSet};

#[cfg(test)]
mod tests;

/// The internal [`Entity`] collection used by a [`RelationshipTarget`](crate::relationship::RelationshipTarget) component.
/// This is not intended to be modified directly by users, as it could invalidate the correctness of relationships.
pub trait RelationshipSourceCollection {
    /// The type of iterator returned by the `iter` method.
    ///
    /// This is an associated type (rather than using a method that returns an opaque return-position impl trait)
    /// to ensure that all methods and traits (like [`DoubleEndedIterator`]) of the underlying collection's iterator
    /// are available to the user when implemented without unduly restricting the possible collections.
    ///
    /// The [`SourceIter`](super::SourceIter) type alias can be helpful to reduce confusion when working with this associated type.
    type SourceIter<'a>: Iterator<Item = Entity>
    where
        Self: 'a;

    /// Creates a new empty instance.
    fn new() -> Self;

    /// Returns an instance with the given pre-allocated entity `capacity`.
    ///
    /// Some collections will ignore the provided `capacity` and return a default instance.
    fn with_capacity(capacity: usize) -> Self;

    /// Reserves capacity for at least `additional` more entities to be inserted.
    ///
    /// Not all collections support this operation, in which case it is a no-op.
    fn reserve(&mut self, additional: usize);

    /// Adds the given `entity` to the collection.
    ///
    /// Returns whether the entity was added to the collection.
    /// Mainly useful when dealing with collections that don't allow
    /// multiple instances of the same entity ([`EntityHashSet`]).
    fn add(&mut self, entity: Entity) -> bool;

    /// Removes the given `entity` from the collection.
    ///
    /// Returns whether the collection actually contained
    /// the entity.
    fn remove(&mut self, entity: Entity) -> bool;

    /// Iterates all entities in the collection.
    fn iter(&self) -> Self::SourceIter<'_>;

    /// Returns the current length of the collection.
    fn len(&self) -> usize;

    /// Clears the collection.
    fn clear(&mut self);

    /// Attempts to save memory by shrinking the capacity to fit the current length.
    ///
    /// This operation is a no-op for collections that do not support it.
    fn shrink_to_fit(&mut self);

    /// Returns true if the collection contains no entities.
    #[inline]
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// For one-to-one relationships, returns the entity that should be removed before adding a new one.
    /// Returns `None` for one-to-many relationships or when no entity needs to be removed.
    fn source_to_remove_before_add(&self) -> Option<Entity> {
        None
    }

    /// Add multiple entities to collection at once.
    ///
    /// May be faster than repeatedly calling [`Self::add`].
    fn extend_from_iter(&mut self, entities: impl IntoIterator<Item = Entity>);
}

/// This trait signals that a [`RelationshipSourceCollection`] is ordered.
pub trait OrderedRelationshipSourceCollection: RelationshipSourceCollection {
    /// Inserts the entity at a specific index.
    /// If the index is too large, the entity will be added to the end of the collection.
    fn insert(&mut self, index: usize, entity: Entity);

    /// Removes the entity at the specified index if it exists.
    fn remove_at(&mut self, index: usize) -> Option<Entity>;

    /// Inserts the entity at a specific index.
    /// This will never reorder other entities.
    /// If the index is too large, the entity will be added to the end of the collection.
    fn insert_stable(&mut self, index: usize, entity: Entity);

    /// Removes the entity at the specified index if it exists.
    /// This will never reorder other entities.
    fn remove_at_stable(&mut self, index: usize) -> Option<Entity>;

    /// Sorts the source collection.
    fn sort(&mut self);

    /// Inserts the entity at the proper place to maintain sorting.
    fn insert_stored(&mut self, entity: Entity);

    /// This places the most recently added entity at the particular index.
    fn place_most_recent(&mut self, index: usize);

    /// This places the given entity at the particular index.
    /// This will do nothing if the entity is not in the collection.
    /// If the index is out of bounds, this will put the entity at the end.
    fn place(&mut self, entity: Entity, index: usize);

    /// Adds the entity at index 0.
    fn push_front(&mut self, entity: Entity) {
        self.insert(0, entity);
    }

    /// Adds the entity to the back of the collection.
    fn push_back(&mut self, entity: Entity) {
        self.insert(usize::MAX, entity);
    }

    /// Removes the first entity.
    fn pop_front(&mut self) -> Option<Entity> {
        self.remove_at(0)
    }

    /// Removes the last entity.
    fn pop_back(&mut self) -> Option<Entity> {
        if self.is_empty() {
            None
        } else {
            self.remove_at(self.len() - 1)
        }
    }
}

impl RelationshipSourceCollection for Vec<Entity> {
    type SourceIter<'a> = std::iter::Copied<std::slice::Iter<'a, Entity>>;

    fn new() -> Self {
        Vec::new()
    }

    fn with_capacity(capacity: usize) -> Self {
        Vec::with_capacity(capacity)
    }

    fn reserve(&mut self, additional: usize) {
        Vec::reserve(self, additional);
    }

    fn add(&mut self, entity: Entity) -> bool {
        Vec::push(self, entity);

        true
    }

    fn remove(&mut self, entity: Entity) -> bool {
        // Scan from the back. Recently added entities live at the tail and are more likely to be
        // despawned. This exploits temporal locality to keep the search cheap.
        if let Some(index) = <[Entity]>::iter(self).rposition(|e| *e == entity) {
            Vec::remove(self, index);
            return true;
        }

        false
    }

    fn iter(&self) -> Self::SourceIter<'_> {
        <[Entity]>::iter(self).copied()
    }

    fn len(&self) -> usize {
        Vec::len(self)
    }

    fn clear(&mut self) {
        self.clear();
    }

    fn shrink_to_fit(&mut self) {
        Vec::shrink_to_fit(self);
    }

    fn extend_from_iter(&mut self, entities: impl IntoIterator<Item = Entity>) {
        self.extend(entities);
    }
}

impl OrderedRelationshipSourceCollection for Vec<Entity> {
    fn insert(&mut self, index: usize, entity: Entity) {
        self.push(entity);
        let len = self.len();
        if index < len {
            self.swap(index, len - 1);
        }
    }

    fn remove_at(&mut self, index: usize) -> Option<Entity> {
        (index < self.len()).then(|| self.swap_remove(index))
    }

    fn insert_stable(&mut self, index: usize, entity: Entity) {
        if index < self.len() {
            Vec::insert(self, index, entity);
        } else {
            self.push(entity);
        }
    }

    fn remove_at_stable(&mut self, index: usize) -> Option<Entity> {
        (index < self.len()).then(|| self.remove(index))
    }

    fn sort(&mut self) {
        self.sort_unstable();
    }

    fn insert_stored(&mut self, entity: Entity) {
        let index = self.partition_point(|e| e <= &entity);
        self.insert_stable(index, entity);
    }

    fn place_most_recent(&mut self, index: usize) {
        if let Some(entity) = self.pop() {
            let index = index.min(self.len());
            self.insert(index, entity);
        }
    }

    fn place(&mut self, entity: Entity, index: usize) {
        if let Some(current) = <[Entity]>::iter(self).position(|e| *e == entity) {
            let index = index.min(self.len());
            Vec::remove(self, current);
            self.insert(index, entity);
        }
    }
}

impl RelationshipSourceCollection for EntityHashSet {
    type SourceIter<'a> = std::iter::Copied<crate::ecs::entity::hash_set::Iter<'a>>;

    fn new() -> Self {
        EntityHashSet::new()
    }

    fn with_capacity(capacity: usize) -> Self {
        EntityHashSet::with_capacity(capacity)
    }

    fn reserve(&mut self, additional: usize) {
        self.deref_mut().reserve(additional);
    }

    fn add(&mut self, entity: Entity) -> bool {
        self.insert(entity)
    }

    fn remove(&mut self, entity: Entity) -> bool {
        self.deref_mut().remove(&entity)
    }

    fn iter(&self) -> Self::SourceIter<'_> {
        self.iter().copied()
    }

    fn len(&self) -> usize {
        self.deref().len()
    }

    fn clear(&mut self) {
        self.deref_mut().clear();
    }

    fn shrink_to_fit(&mut self) {
        self.deref_mut().shrink_to_fit();
    }

    fn extend_from_iter(&mut self, entities: impl IntoIterator<Item = Entity>) {
        self.extend(entities);
    }
}

impl<const N: usize> RelationshipSourceCollection for SmallVec<[Entity; N]> {
    type SourceIter<'a> = std::iter::Copied<std::slice::Iter<'a, Entity>>;

    fn new() -> Self {
        SmallVec::new()
    }

    fn with_capacity(capacity: usize) -> Self {
        SmallVec::with_capacity(capacity)
    }

    fn reserve(&mut self, additional: usize) {
        SmallVec::reserve(self, additional);
    }

    fn add(&mut self, entity: Entity) -> bool {
        SmallVec::push(self, entity);

        true
    }

    fn remove(&mut self, entity: Entity) -> bool {
        if let Some(index) = <[Entity]>::iter(self).position(|e| *e == entity) {
            SmallVec::remove(self, index);
            return true;
        }

        false
    }

    fn iter(&self) -> Self::SourceIter<'_> {
        <[Entity]>::iter(self).copied()
    }

    fn len(&self) -> usize {
        SmallVec::len(self)
    }

    fn clear(&mut self) {
        self.clear();
    }

    fn shrink_to_fit(&mut self) {
        SmallVec::shrink_to_fit(self);
    }

    fn extend_from_iter(&mut self, entities: impl IntoIterator<Item = Entity>) {
        self.extend(entities);
    }
}

impl RelationshipSourceCollection for Entity {
    type SourceIter<'a> = std::option::IntoIter<Entity>;

    fn new() -> Self {
        Entity::PLACEHOLDER
    }

    fn with_capacity(_capacity: usize) -> Self {
        Self::new()
    }

    fn reserve(&mut self, _additional: usize) {}

    fn add(&mut self, entity: Entity) -> bool {
        *self = entity;
        true
    }

    fn remove(&mut self, entity: Entity) -> bool {
        if *self == entity {
            *self = Entity::PLACEHOLDER;

            return true;
        }

        false
    }

    fn iter(&self) -> Self::SourceIter<'_> {
        if *self == Entity::PLACEHOLDER {
            None.into_iter()
        } else {
            Some(*self).into_iter()
        }
    }

    fn len(&self) -> usize {
        if *self == Entity::PLACEHOLDER {
            return 0;
        }
        1
    }

    fn clear(&mut self) {
        *self = Entity::PLACEHOLDER;
    }

    fn shrink_to_fit(&mut self) {}

    fn extend_from_iter(&mut self, entities: impl IntoIterator<Item = Entity>) {
        for entity in entities {
            *self = entity;
        }
    }

    fn source_to_remove_before_add(&self) -> Option<Entity> {
        if *self != Entity::PLACEHOLDER {
            Some(*self)
        } else {
            None
        }
    }
}

impl<const N: usize> OrderedRelationshipSourceCollection for SmallVec<[Entity; N]> {
    fn insert(&mut self, index: usize, entity: Entity) {
        self.push(entity);
        let len = self.len();
        if index < len {
            self.swap(index, len - 1);
        }
    }

    fn remove_at(&mut self, index: usize) -> Option<Entity> {
        (index < self.len()).then(|| self.swap_remove(index))
    }

    fn insert_stable(&mut self, index: usize, entity: Entity) {
        if index < self.len() {
            SmallVec::<[Entity; N]>::insert(self, index, entity);
        } else {
            self.push(entity);
        }
    }

    fn remove_at_stable(&mut self, index: usize) -> Option<Entity> {
        (index < self.len()).then(|| self.remove(index))
    }

    fn sort(&mut self) {
        self.sort_unstable();
    }

    fn insert_stored(&mut self, entity: Entity) {
        let index = self.partition_point(|e| e <= &entity);
        self.insert_stable(index, entity);
    }

    fn place_most_recent(&mut self, index: usize) {
        if let Some(entity) = self.pop() {
            let index = index.min(self.len() - 1);
            self.insert(index, entity);
        }
    }

    fn place(&mut self, entity: Entity, index: usize) {
        if let Some(current) = <[Entity]>::iter(self).position(|e| *e == entity) {
            // The len is at least 1, so the subtraction is safe.
            let index = index.min(self.len() - 1);
            SmallVec::<[Entity; N]>::remove(self, current);
            self.insert(index, entity);
        };
    }
}

impl<S: BuildHasher + Default> RelationshipSourceCollection for IndexSet<Entity, S> {
    type SourceIter<'a>
        = std::iter::Copied<indexmap::set::Iter<'a, Entity>>
    where
        S: 'a;

    fn new() -> Self {
        IndexSet::default()
    }

    fn with_capacity(capacity: usize) -> Self {
        IndexSet::with_capacity_and_hasher(capacity, S::default())
    }

    fn reserve(&mut self, additional: usize) {
        self.reserve(additional);
    }

    fn add(&mut self, entity: Entity) -> bool {
        self.insert(entity)
    }

    fn remove(&mut self, entity: Entity) -> bool {
        self.shift_remove(&entity)
    }

    fn iter(&self) -> Self::SourceIter<'_> {
        self.iter().copied()
    }

    fn len(&self) -> usize {
        self.len()
    }

    fn clear(&mut self) {
        self.clear();
    }

    fn shrink_to_fit(&mut self) {
        self.shrink_to_fit();
    }

    fn extend_from_iter(&mut self, entities: impl IntoIterator<Item = Entity>) {
        self.extend(entities);
    }
}

impl RelationshipSourceCollection for EntityIndexSet {
    type SourceIter<'a> = std::iter::Copied<crate::ecs::entity::index_set::Iter<'a>>;

    fn new() -> Self {
        EntityIndexSet::new()
    }

    fn with_capacity(capacity: usize) -> Self {
        EntityIndexSet::with_capacity(capacity)
    }

    fn reserve(&mut self, additional: usize) {
        self.deref_mut().reserve(additional);
    }

    fn add(&mut self, entity: Entity) -> bool {
        self.insert(entity)
    }

    fn remove(&mut self, entity: Entity) -> bool {
        self.deref_mut().shift_remove(&entity)
    }

    fn iter(&self) -> Self::SourceIter<'_> {
        self.iter().copied()
    }

    fn len(&self) -> usize {
        self.deref().len()
    }

    fn clear(&mut self) {
        self.deref_mut().clear();
    }

    fn shrink_to_fit(&mut self) {
        self.deref_mut().shrink_to_fit();
    }

    fn extend_from_iter(&mut self, entities: impl IntoIterator<Item = Entity>) {
        self.extend(entities);
    }
}

impl RelationshipSourceCollection for BTreeSet<Entity> {
    type SourceIter<'a> = std::iter::Copied<btree_set::Iter<'a, Entity>>;

    fn new() -> Self {
        BTreeSet::new()
    }

    fn with_capacity(_capacity: usize) -> Self {
        // BTreeSet doesn't have a capacity
        Self::new()
    }

    fn reserve(&mut self, _additional: usize) {
        // BTreeSet doesn't have a capacity
    }

    fn add(&mut self, entity: Entity) -> bool {
        self.insert(entity)
    }

    fn remove(&mut self, entity: Entity) -> bool {
        self.remove(&entity)
    }

    fn iter(&self) -> Self::SourceIter<'_> {
        self.iter().copied()
    }

    fn len(&self) -> usize {
        self.len()
    }

    fn clear(&mut self) {
        self.clear();
    }

    fn shrink_to_fit(&mut self) {
        // BTreeSet doesn't have a capacity
    }

    fn extend_from_iter(&mut self, entities: impl IntoIterator<Item = Entity>) {
        self.extend(entities);
    }
}
