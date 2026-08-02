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
pub struct SparseArray<I, V = I> {
    values: Vec<Option<V>>,
    marker: PhantomData<I>,
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

/// A collection of [`ComponentSparseSet`] storages, indexed by [`ComponentId`]
///
/// Can be accessed via [`Storages`](crate::storage::Storages)
#[derive(Default)]
pub struct SparseSets {
    sets: SparseSet<ComponentId, ComponentSparseSet>,
}
