use std::{hash::Hash, marker::PhantomData};

use nonmax::NonMaxUsize;

#[cfg(debug_assertions)]
use crate::ecs::entity::Entity;
use crate::ecs::{component::ComponentId, entity::EntityIndex, storage::TableRow};

/// Represents something that can be stored in a [`SparseSet`] as an integer.
///
/// Ideally, the `usize` values should be very small (ie: incremented starting from
/// zero), as the number of bits needed to represent a `SparseSetIndex` in a `FixedBitSet`
/// is proportional to the **value** of those `usize`.
pub trait SparseSetIndex: Clone + PartialEq + Eq + Hash {
    /// Gets the sparse set index corresponding to this instance.
    fn sparse_set_index(&self) -> usize;
    /// Creates a new instance of this type with the specified index.
    fn get_sparse_set_index(value: usize) -> Self;
}

macro_rules! impl_sparse_set_index {
    ($($ty:ty),+) => {
        $(impl SparseSetIndex for $ty {
            #[inline]
            fn sparse_set_index(&self) -> usize {
                *self as usize
            }

            #[inline]
            fn get_sparse_set_index(value: usize) -> Self {
                value as $ty
            }
        })*
    };
}

impl_sparse_set_index!(u8, u16, u32, u64, usize);

/// A map from `I` to `V` implemented as a `Vec<Option<V>>`.
///
/// The key type, `I`, must implement [`SparseSetIndex`]
/// to allow conversion to and from array indexes.
///
/// This supports fast O(1) lookups, since they are simple
/// array indexing operations with no calculations.
///
/// However, it may use a lot of excess memory if the
/// values are large or the set is sparsely populated.
#[derive(Debug)]
pub(crate) struct SparseArray<I, V = I> {
    values: Vec<Option<V>>,
    marker: PhantomData<I>,
}

/// A map from `I` to `V` implemented as a `Box<[Option<V>]>`.
///
/// This uses less space than [`SparseArray`] because it does not
/// need to store both length and capacity,
/// but it cannot be changed after construction.
///
/// The key type, `I`, must implement [`SparseSetIndex`]
/// to allow conversion to and from array indexes.
///
/// This supports fast O(1) lookups, since they are simple
/// array indexing operations with no calculations.
///
/// However, it may use a lot of excess memory if the
/// values are large or the set is sparsely populated.
#[derive(Debug)]
pub(crate) struct ImmutableSparseArray<I, V = I> {
    values: Box<[Option<V>]>,
    marker: PhantomData<I>,
}

impl<I: SparseSetIndex, V> Default for SparseArray<I, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<I, V> SparseArray<I, V> {
    #[inline]
    pub const fn new() -> Self {
        Self {
            values: Vec::new(),
            marker: PhantomData,
        }
    }
}

macro_rules! impl_sparse_array {
    ($ty:ident) => {
        impl<I: SparseSetIndex, V> $ty<I, V> {
            /// Returns `true` if the collection contains a value for the specified `index`.
            #[inline]
            pub fn contains(&self, index: I) -> bool {
                let index = index.sparse_set_index();
                self.values.get(index).is_some_and(Option::is_some)
            }

            /// Returns a reference to the value at `index`.
            ///
            /// Returns `None` if `index` does not have a value or if `index` is out of bounds.
            #[inline]
            pub fn get(&self, index: I) -> Option<&V> {
                let index = index.sparse_set_index();
                self.values.get(index).and_then(Option::as_ref)
            }
        }
    };
}

impl_sparse_array!(SparseArray);
impl_sparse_array!(ImmutableSparseArray);

impl<I: SparseSetIndex, V> SparseArray<I, V> {
    /// Inserts `value` at `index` in the array.
    ///
    /// # Panics
    /// - Panics if the insertion forces a reallocation, and any of the new capacity overflows `isize::MAX` bytes.
    /// - Panics if the insertion forces a reallocation, and any of the new the reallocations causes an out-of-memory error.
    ///
    /// If `index` is out-of-bounds, this will enlarge the buffer to accommodate it.
    #[inline]
    pub fn insert(&mut self, index: I, value: V) {
        let index = index.sparse_set_index();
        if index >= self.values.len() {
            self.values.resize_with(index + 1, || None);
        }
        self.values[index] = Some(value)
    }

    /// Returns a mutable reference to the value at `index`.
    ///
    /// Returns `None` if `index` does not have a value or if `index` is out of bounds.
    #[inline]
    pub fn get_mut(&mut self, index: I) -> Option<&mut V> {
        let index = index.sparse_set_index();
        self.values.get_mut(index).and_then(Option::as_mut)
    }

    /// Removes and returns the value stored at `index`.
    ///
    /// Returns `None` if `index` did not have a value or if `index` is out of bounds.
    #[inline]
    pub fn remove(&mut self, index: I) -> Option<V> {
        let index = index.sparse_set_index();
        self.values.get_mut(index).and_then(Option::take)
    }

    /// Removes all of the values stored within.
    pub fn clear(&mut self) {
        self.values.clear();
    }

    /// Converts the [`SparseArray`] into an immutable variant.
    pub(crate) fn into_immutable(self) -> ImmutableSparseArray<I, V> {
        ImmutableSparseArray {
            values: self.values.into_boxed_slice(),
            marker: PhantomData,
        }
    }

    /// Returns an iterator over the non-empty values in the array.
    ///
    /// This must scan the entire array to find non-empty values,
    /// which may be slow even if the array is sparsely populated.
    #[inline]
    pub(crate) fn iter(&self) -> impl Iterator<Item = (I, &V)> {
        self.values.iter().enumerate().filter_map(|(index, value)| {
            value
                .as_ref()
                .map(|value| (SparseSetIndex::get_sparse_set_index(index), value))
        })
    }
}

/// A sparse data structure of [`Component`](crate::component::Component)s.
///
/// Designed for relatively fast insertions and deletions.
#[derive(Debug)]
pub struct ComponentSparseSet {
    // Internally this only relies on the Entity index to keep track of where the component data is
    // stored for entities that are alive. The generation is not required, but is stored
    // in debug builds to validate that access is correct.
    #[cfg(not(debug_assertions))]
    entities: Vec<EntityIndex>,
    #[cfg(debug_assertions)]
    entities: Vec<Entity>,
    sparse: SparseArray<EntityIndex, TableRow>,
}

/// A map from `I` to `V` that combines dense and sparse storage.
///
/// This is implemented as a sparse array mapping keys to dense indexes,
/// plus dense arrays of indexes and keys.
///
/// The key type, `I`, must implement [`SparseSetIndex`]
/// to allow conversion to and from array indexes.
///
/// This supports fast O(1) lookups, since they consist of one array index to map
/// the key to a dense index, followed by a second array index to find the value.
///
/// This may use a lot of excess memory if the set is sparsely populated,
/// since it stores an empty entry for each key.
///
/// Compared to a simple `Vec<Option<V>>`,
/// the dense storage of values takes less memory when `V` is large,
/// although the overhead of tracking which entries have values
/// may make it larger when `V` is small or the set is densely populated.
#[derive(Debug)]
pub struct SparseSet<I, V: 'static> {
    /// The mapping from dense index to value.
    ///
    /// `dense[sparse[k]]` holds the value for `k`.
    dense: Vec<V>,

    /// The reverse mapping from dense index to key.
    ///
    /// `indices[sparse[k]] == k`
    indices: Vec<I>,

    /// The mapping from keys to dense indexes.
    sparse: SparseArray<I, NonMaxUsize>,
}

/// A collection of [`ComponentSparseSet`] storages, indexed by [`ComponentId`]
///
/// Can be accessed via [`Storages`](crate::storage::Storages)
#[derive(Default)]
pub struct SparseSets {
    sets: SparseSet<ComponentId, ComponentSparseSet>,
}
