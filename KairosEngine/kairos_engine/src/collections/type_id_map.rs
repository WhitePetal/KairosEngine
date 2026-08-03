use std::any::TypeId;

use indexmap::map::{Entry, IndexMap};

use crate::hash::NoOpHash;

/// A specialized map type with Key of [`TypeId`]
/// Iteration order only depends on the order of insertions and deletions.
pub type TypeIdMap<V> = IndexMap<TypeId, V, NoOpHash>;

pub type TypeIdMapEntry<'a, K, V> = Entry<'a, K, V>;

/// Extension trait to make use of [`TypeIdMap`] more ergonomic.
///
/// Each function on this trait is a trivial wrapper for a function
/// on [`IndexMap`], replacing a `TypeId` key with a
/// generic parameter `T`.
///
/// # Examples
///
/// ```rust
/// # use std::any::TypeId;
/// # use bevy_utils::TypeIdMap;
/// use bevy_utils::TypeIdMapExt;
///
/// struct MyType;
///
/// // Using the built-in `HashMap` functions requires manually looking up `TypeId`s.
/// let mut map = TypeIdMap::default();
/// map.insert(TypeId::of::<MyType>(), 7);
/// assert_eq!(map.get(&TypeId::of::<MyType>()), Some(&7));
///
/// // Using `TypeIdMapExt` functions does the lookup for you.
/// map.insert_type::<MyType>(7);
/// assert_eq!(map.get_type::<MyType>(), Some(&7));
/// ```
pub trait TypeIdMapExt<V> {
    /// Inserts a value for the type `T`.
    ///
    /// If the map did not previously contain this key then [`None`] is returned,
    /// otherwise the value for this key is updated and the old value returned.
    fn insert_type<T: ?Sized + 'static>(&mut self, v: V) -> Option<V>;

    /// Returns a reference to the value for type `T`, if one exists.
    fn get_type<T: ?Sized + 'static>(&self) -> Option<&V>;

    /// Returns a mutable reference to the value for type `T`, if one exists.
    fn get_type_mut<T: ?Sized + 'static>(&mut self) -> Option<&mut V>;

    /// Removes type `T` from the map, returning the value for this
    /// key if it was previously present.
    fn remove_type<T: ?Sized + 'static>(&mut self) -> Option<V>;

    /// Gets the type `T`'s entry in the map for in-place manipulation.
    fn entry_type<T: ?Sized + 'static>(&mut self) -> TypeIdMapEntry<'_, TypeId, V>;
}

impl<V> TypeIdMapExt<V> for TypeIdMap<V> {
    #[inline]
    fn insert_type<T: ?Sized + 'static>(&mut self, v: V) -> Option<V> {
        self.insert(TypeId::of::<T>(), v)
    }

    #[inline]
    fn get_type<T: ?Sized + 'static>(&self) -> Option<&V> {
        self.get(&TypeId::of::<T>())
    }

    #[inline]
    fn get_type_mut<T: ?Sized + 'static>(&mut self) -> Option<&mut V> {
        self.get_mut(&TypeId::of::<T>())
    }

    #[inline]
    fn remove_type<T: ?Sized + 'static>(&mut self) -> Option<V> {
        self.shift_remove(&TypeId::of::<T>())
    }

    #[inline]
    fn entry_type<T: ?Sized + 'static>(&mut self) -> TypeIdMapEntry<'_, TypeId, V> {
        self.entry(TypeId::of::<T>())
    }
}
