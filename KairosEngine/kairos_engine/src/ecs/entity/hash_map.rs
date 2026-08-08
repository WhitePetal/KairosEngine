//! Contains the [`EntityHashMap`] type, a [`HashMap`] pre-configured to use [`EntityHash`] hashing.
//!
//! This module is a lightweight wrapper around Bevy's [`HashMap`] that is more performant for [`Entity`] keys.

use std::{
    fmt::{self, Debug, Formatter},
    iter::FusedIterator,
    marker::PhantomData,
    ops::{Deref, DerefMut, Index},
};

use hashbrown::hash_map;
use serde::{Deserialize, Serialize};

use crate::{
    collections::FixedHashMap,
    ecs::entity::{Entity, EntityEquivalent, EntityHash, EntitySetIterator},
};

#[cfg(test)]
mod test;

/// A [`HashMap`] pre-configured to use [`EntityHash`] hashing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityHashMap<V>(FixedHashMap<Entity, V, EntityHash>);

impl<V> EntityHashMap<V> {
    /// Creates an empty `EntityHashMap`.
    ///
    /// Equivalent to [`HashMap::with_hasher(EntityHash)`].
    ///
    /// [`HashMap::with_hasher(EntityHash)`]: HashMap::with_hasher
    pub const fn new() -> Self {
        Self(FixedHashMap::with_hasher(EntityHash))
    }

    /// Creates an empty `EntityHashMap` with the specified capacity.
    ///
    /// Equivalent to [`HashMap::with_capacity_and_hasher(n, EntityHash)`].
    ///
    /// [`HashMap::with_capacity_and_hasher(n, EntityHash)`]: HashMap::with_capacity_and_hasher
    pub fn with_capacity(n: usize) -> Self {
        Self(FixedHashMap::with_capacity_and_hasher(n, EntityHash))
    }

    /// Constructs an `EntityHashMap` from an [`HashMap`].
    pub const fn from_index_map(set: FixedHashMap<Entity, V, EntityHash>) -> Self {
        Self(set)
    }

    /// Returns the inner [`HashMap`].
    pub fn into_inner(self) -> FixedHashMap<Entity, V, EntityHash> {
        self.0
    }

    /// An iterator visiting all keys in arbitrary order.
    /// The iterator element type is `&'a Entity`.
    ///
    /// Equivalent to [`HashMap::keys`].
    pub fn keys(&self) -> Keys<'_, V> {
        Keys(self.0.keys(), PhantomData)
    }

    /// Creates a consuming iterator visiting all the keys in arbitrary order.
    /// The map cannot be used after calling this.
    /// The iterator element type is [`Entity`].
    ///
    /// Equivalent to [`HashMap::into_keys`].
    pub fn into_keys(self) -> IntoKeys<V> {
        IntoKeys(self.0.into_keys(), PhantomData)
    }
}

impl<V> Default for EntityHashMap<V> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<V> Deref for EntityHashMap<V> {
    type Target = FixedHashMap<Entity, V, EntityHash>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<V> DerefMut for EntityHashMap<V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'a, V: Copy> Extend<&'a (Entity, V)> for EntityHashMap<V> {
    fn extend<T: IntoIterator<Item = &'a (Entity, V)>>(&mut self, iter: T) {
        self.0.extend(iter);
    }
}

impl<'a, V: Copy> Extend<(&'a Entity, &'a V)> for EntityHashMap<V> {
    fn extend<T: IntoIterator<Item = (&'a Entity, &'a V)>>(&mut self, iter: T) {
        self.0.extend(iter);
    }
}

impl<V> Extend<(Entity, V)> for EntityHashMap<V> {
    fn extend<T: IntoIterator<Item = (Entity, V)>>(&mut self, iter: T) {
        self.0.extend(iter);
    }
}

impl<V, const N: usize> From<[(Entity, V); N]> for EntityHashMap<V> {
    fn from(value: [(Entity, V); N]) -> Self {
        Self(FixedHashMap::from_iter(value))
    }
}

impl<V> FromIterator<(Entity, V)> for EntityHashMap<V> {
    fn from_iter<I: IntoIterator<Item = (Entity, V)>>(iterable: I) -> Self {
        Self(FixedHashMap::from_iter(iterable))
    }
}

impl<V> From<FixedHashMap<Entity, V, EntityHash>> for EntityHashMap<V> {
    fn from(value: FixedHashMap<Entity, V, EntityHash>) -> Self {
        Self(value)
    }
}

impl<V, Q: EntityEquivalent + ?Sized> Index<&Q> for EntityHashMap<V> {
    type Output = V;

    fn index(&self, key: &Q) -> &V {
        self.0.index(&key.entity())
    }
}

impl<'a, V> IntoIterator for &'a EntityHashMap<V> {
    type Item = (&'a Entity, &'a V);
    type IntoIter = hash_map::Iter<'a, Entity, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a, V> IntoIterator for &'a mut EntityHashMap<V> {
    type Item = (&'a Entity, &'a mut V);
    type IntoIter = hash_map::IterMut<'a, Entity, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

impl<V> IntoIterator for EntityHashMap<V> {
    type Item = (Entity, V);
    type IntoIter = hash_map::IntoIter<Entity, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

/// An iterator over the keys of a [`EntityHashMap`] in arbitrary order.
/// The iterator element type is `&'a Entity`.
///
/// This struct is created by the [`keys`] method on [`EntityHashMap`]. See its documentation for more.
///
/// [`keys`]: EntityHashMap::keys
pub struct Keys<'a, V, S = EntityHash>(hash_map::Keys<'a, Entity, V>, PhantomData<S>);

impl<'a, V> Keys<'a, V> {
    /// Constructs a [`Keys<'a, V, S>`] from a [`hash_map::Keys<'a, V>`] unsafely.
    ///
    /// # Safety
    ///
    /// `keys` must either be empty, or have been obtained from a
    /// [`hash_map::HashMap`] using the `S` hasher.
    pub const unsafe fn from_keys_unchecked<S>(
        keys: hash_map::Keys<'a, Entity, V>,
    ) -> Keys<'a, V, S> {
        Keys(keys, PhantomData)
    }

    /// Returns the inner [`Keys`](hash_map::Keys).
    pub const fn into_inner(self) -> hash_map::Keys<'a, Entity, V> {
        self.0
    }
}

impl<'a, V> Deref for Keys<'a, V> {
    type Target = hash_map::Keys<'a, Entity, V>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a, V> Iterator for Keys<'a, V> {
    type Item = &'a Entity;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<V> ExactSizeIterator for Keys<'_, V> {}

impl<V> FusedIterator for Keys<'_, V> {}

impl<V> Clone for Keys<'_, V> {
    fn clone(&self) -> Self {
        // SAFETY: We are cloning an already valid `Keys`.
        unsafe { Self::from_keys_unchecked(self.0.clone()) }
    }
}

impl<V: Debug> Debug for Keys<'_, V> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Keys").field(&self.0).field(&self.1).finish()
    }
}

impl<V> Default for Keys<'_, V> {
    fn default() -> Self {
        // SAFETY: `Keys` is empty.
        unsafe { Self::from_keys_unchecked(Default::default()) }
    }
}

// SAFETY: Keys stems from a correctly behaving `HashMap<Entity, V, EntityHash>`.
unsafe impl<V> EntitySetIterator for Keys<'_, V> {}

/// An owning iterator over the keys of a [`EntityHashMap`] in arbitrary order.
/// The iterator element type is [`Entity`].
///
/// This struct is created by the [`into_keys`] method on [`EntityHashMap`].
/// See its documentation for more.
/// The map cannot be used after calling that method.
///
/// [`into_keys`]: EntityHashMap::into_keys
pub struct IntoKeys<V, S = EntityHash>(hash_map::IntoKeys<Entity, V>, PhantomData<S>);

impl<V> IntoKeys<V> {
    /// Constructs a [`IntoKeys<V, S>`] from a [`hash_map::IntoKeys<V>`] unsafely.
    ///
    /// # Safety
    ///
    /// `into_keys` must either be empty, or have been obtained from a
    /// [`hash_map::HashMap`] using the `S` hasher.
    pub const unsafe fn from_into_keys_unchecked<S>(
        into_keys: hash_map::IntoKeys<Entity, V>,
    ) -> IntoKeys<V, S> {
        IntoKeys(into_keys, PhantomData)
    }

    /// Returns the inner [`IntoKeys`](hash_map::IntoKeys).
    pub fn into_inner(self) -> hash_map::IntoKeys<Entity, V> {
        self.0
    }
}

impl<V> Deref for IntoKeys<V> {
    type Target = hash_map::IntoKeys<Entity, V>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<V> Iterator for IntoKeys<V> {
    type Item = Entity;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<V> ExactSizeIterator for IntoKeys<V> {}

impl<V> FusedIterator for IntoKeys<V> {}

impl<V: Debug> Debug for IntoKeys<V> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_tuple("IntoKeys")
            .field(&self.0)
            .field(&self.1)
            .finish()
    }
}

impl<V> Default for IntoKeys<V> {
    fn default() -> Self {
        // SAFETY: `IntoKeys` is empty.
        unsafe { Self::from_into_keys_unchecked(Default::default()) }
    }
}

// SAFETY: IntoKeys stems from a correctly behaving `HashMap<Entity, V, EntityHash>`.
unsafe impl<V> EntitySetIterator for IntoKeys<V> {}
