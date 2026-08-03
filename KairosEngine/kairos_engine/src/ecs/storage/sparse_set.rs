use std::{cell::UnsafeCell, hash::Hash, marker::PhantomData, num::NonZero, panic::Location};

use nonmax::{NonMaxU32, NonMaxUsize};

#[cfg(debug_assertions)]
use crate::ecs::entity::Entity;
use crate::{
    debug::{DebugCheckedUnwrap, MaybeLocation},
    ecs::{
        change_detection::{CheckChangeTicks, ComponentTickCells, ComponentTicks, Tick},
        component::{ComponentId, ComponentInfo},
        entity::EntityIndex,
        storage::{Column, TableRow, VecExtensions},
    },
    on_drop::AbortOnPanic,
    ptr::{OwningPtr, Ptr},
};

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
    /// Capacity and length match those of `entities`.
    dense: Column,
    // Internally this only relies on the Entity index to keep track of where the component data is
    // stored for entities that are alive. The generation is not required, but is stored
    // in debug builds to validate that access is correct.
    #[cfg(not(debug_assertions))]
    entities: Vec<EntityIndex>,
    #[cfg(debug_assertions)]
    entities: Vec<Entity>,
    sparse: SparseArray<EntityIndex, TableRow>,
}

impl ComponentSparseSet {
    /// Creates a new [`ComponentSparseSet`] with a given component type layout and
    /// initial `capacity`.
    pub(crate) fn new(component_info: &ComponentInfo, capacity: usize) -> Self {
        let entities = Vec::with_capacity(capacity);
        Self {
            dense: Column::with_capacity(component_info, entities.capacity()),
            entities,
            sparse: Default::default(),
        }
    }

    /// Returns the number of component values in the sparse set.
    #[inline]
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    /// Removes all of the values stored within.
    pub(crate) fn clear(&mut self) {
        // SAFETY: This is using the size of the ComponentSparseSet.
        unsafe {
            self.dense.clear(self.len());
        }
        self.entities.clear();
        self.sparse.clear();
    }

    /// Returns `true` if the sparse set contains no component values.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    /// Inserts the `entity` key and component `value` pair into this sparse
    /// set.
    ///
    /// # Aborts
    /// - Aborts the process if the insertion forces a reallocation, and any of the new capacity overflows `isize::MAX` bytes.
    /// - Aborts the process if the insertion forces a reallocation, and any of the new the reallocations causes an out-of-memory error.
    ///
    /// # Safety
    /// The `value` pointer must point to a valid address that matches the [`Layout`](std::alloc::Layout)
    /// inside the [`ComponentInfo`] given when constructing this sparse set.
    pub(crate) unsafe fn insert(
        &mut self,
        entity: Entity,
        value: OwningPtr<'_>,
        change_tick: Tick,
        caller: MaybeLocation,
    ) {
        if let Some(&dense_index) = self.sparse.get(entity.index()) {
            #[cfg(debug_assertions)]
            assert_eq!(entity, self.entities[dense_index.index()]);
            self.dense.replace(dense_index, value, change_tick, caller);
        } else {
            let dense_index = self.entities.len();
            let capacity = self.entities.capacity();

            #[cfg(not(debug_assertions))]
            self.entities.push(entity.index());
            #[cfg(debug_assertions)]
            self.entities.push(entity);

            // If any of the following operations panic due to an allocation error, the state
            // of the `ComponentSparseSet` will be left in an invalid state and potentially cause UB.
            // We create an AbortOnPanic guard to force panics to terminate the process if this occurs.
            let _guard = AbortOnPanic;
            if capacity != self.entities.capacity() {
                // SAFETY: An entity was just pushed onto `entities`, its capacity cannot be zero.
                let new_capacity = unsafe { NonZero::new_unchecked(self.entities.capacity()) };
                if let Some(capacity) = NonZero::new(capacity) {
                    // SAFETY: This is using the capacity of the previous allocation.
                    unsafe {
                        self.dense.realloc(capacity, new_capacity);
                    }
                } else {
                    self.dense.alloc(new_capacity);
                }
            }

            // SAFETY: This entity index does not exist here yet, so there are no duplicates,
            // and the entity index is `NonMaxU32` so the length must not be max either.
            let table_row = unsafe { TableRow::new(NonMaxU32::new_unchecked(dense_index as u32)) };
            self.dense.initialize(table_row, value, change_tick, caller);
            self.sparse.insert(entity.index(), table_row);

            std::mem::forget(_guard);
        }
    }

    /// Returns `true` if the sparse set has a component value for the provided `entity`.
    #[inline]
    pub fn contains(&self, entity: Entity) -> bool {
        #[cfg(debug_assertions)]
        {
            if let Some(&dense_index) = self.sparse.get(entity.index()) {
                #[cfg(debug_assertions)]
                assert_eq!(entity, self.entities[dense_index.index()]);
                true
            } else {
                false
            }
        }
        #[cfg(not(debug_assertions))]
        self.sparse.contains(entity.index())
    }

    /// Returns a reference to the entity's component value.
    ///
    /// Returns `None` if `entity` does not have a component in the sparse set.
    #[inline]
    pub fn get(&self, entity: Entity) -> Option<Ptr<'_>> {
        self.sparse.get(entity.index()).map(|&dense_index| {
            #[cfg(debug_assertions)]
            assert_eq!(entity, self.entities[dense_index.index()]);
            // SAFETY: if the sparse index points to something in the dense vec, it exists
            unsafe { self.dense.get_data_unchecked(dense_index) }
        })
    }

    /// Returns references to the entity's component value and its added and changed ticks.
    ///
    /// Returns `None` if `entity` does not have a component in the sparse set.
    #[inline]
    pub fn get_with_ticks(&self, entity: Entity) -> Option<(Ptr<'_>, ComponentTickCells<'_>)> {
        let dense_index = *self.sparse.get(entity.index())?;
        #[cfg(debug_assertions)]
        assert_eq!(entity, self.entities[dense_index.index()]);
        // SAFETY: if the sparse index points to something in the dense vec, it exists
        unsafe {
            Some((
                self.dense.get_data_unchecked(dense_index),
                ComponentTickCells {
                    added: self.dense.get_added_tick_unchecked(dense_index),
                    changed: self.dense.get_changed_tick_unchecked(dense_index),
                    changed_by: self.dense.get_changed_by_unchecked(dense_index),
                },
            ))
        }
    }

    /// Returns a reference to the "added" tick of the entity's component value.
    ///
    /// Returns `None` if `entity` does not have a component in the sparse set.
    #[inline]
    pub fn get_added_tick(&self, entity: Entity) -> Option<&UnsafeCell<Tick>> {
        let dense_index = *self.sparse.get(entity.index())?;
        #[cfg(debug_assertions)]
        assert_eq!(entity, self.entities[dense_index.index()]);
        // SAFETY: if the sparse index points to something in the dense vec, it exists
        unsafe { Some(self.dense.get_added_tick_unchecked(dense_index)) }
    }

    /// Returns a reference to the "changed" tick of the entity's component value.
    ///
    /// Returns `None` if `entity` does not have a component in the sparse set.
    #[inline]
    pub fn get_changed_tick(&self, entity: Entity) -> Option<&UnsafeCell<Tick>> {
        let dense_index = *self.sparse.get(entity.index())?;
        #[cfg(debug_assertions)]
        assert_eq!(entity, self.entities[dense_index.index()]);
        // SAFETY: if the sparse index points to something in the dense vec, it exists
        unsafe { Some(self.dense.get_changed_tick_unchecked(dense_index)) }
    }

    /// Returns a reference to the "added" and "changed" ticks of the entity's component value.
    ///
    /// Returns `None` if `entity` does not have a component in the sparse set.
    #[inline]
    pub fn get_ticks(&self, entity: Entity) -> Option<ComponentTicks> {
        let dense_index = *self.sparse.get(entity.index())?;
        #[cfg(debug_assertions)]
        assert_eq!(entity, self.entities[dense_index.index()]);
        // SAFETY: if the sparse index points to something in the dense vec, it exists
        unsafe { Some(self.dense.get_ticks_unchecked(dense_index)) }
    }

    /// Returns a reference to the calling location that last changed the entity's component value.
    ///
    /// Returns `None` if `entity` does not have a component in the sparse set.
    #[inline]
    pub fn get_changed_by(
        &self,
        entity: Entity,
    ) -> MaybeLocation<Option<&UnsafeCell<&'static Location<'static>>>> {
        MaybeLocation::new_with_flattened(|| {
            let dense_index = *self.sparse.get(entity.index())?;
            #[cfg(debug_assertions)]
            assert_eq!(entity, self.entities[dense_index.index()]);
            // SAFETY: if the sparse index points to something in the dense vec, it exists
            unsafe { Some(self.dense.get_changed_by_unchecked(dense_index)) }
        })
    }

    /// Returns the drop function for the component type stored in the sparse set,
    /// or `None` if it doesn't need to be dropped.
    #[inline]
    pub fn get_drop(&self) -> Option<unsafe fn(OwningPtr<'_>)> {
        self.dense.get_drop()
    }

    /// Removes the `entity` from this sparse set and returns a pointer to the associated value (if
    /// it exists).
    #[must_use = "The returned pointer must be used to drop the removed component."]
    pub(crate) fn remove_and_forget(&mut self, entity: Entity) -> Option<OwningPtr<'_>> {
        self.sparse.remove(entity.index()).map(|dense_index| {
            #[cfg(debug_assertions)]
            assert_eq!(entity, self.entities[dense_index.index()]);
            let last = self.entities.len() - 1;
            if dense_index.index() >= last {
                #[cfg(debug_assertions)]
                assert_eq!(dense_index.index(), last);
                // SAFETY: This is strictly decreasing the length, so it cannot outgrow
                // it also cannot underflow as an item was just removed from the sparse array.
                unsafe {
                    self.entities.set_len(last);
                }
                // SAFETY: `last` is guaranteed to be the last element in `dense` as the length is synced with
                // the `entities` store.
                unsafe {
                    self.dense
                        .get_data_unchecked(dense_index)
                        .assert_unique()
                        .promote()
                }
            } else {
                // SAFETY: The above check ensures that `dense_index` and the last element are not
                // overlapping, and thus also within bounds.
                unsafe {
                    self.entities
                        .swap_remove_nonoverlapping_unchecked(dense_index.index());
                }
                // SAFETY: The above check ensures that `dense_index` is in bounds.
                let swapped_entity = unsafe { self.entities.get_unchecked(dense_index.index()) };
                #[cfg(not(debug_assertions))]
                let index = *swapped_entity;
                #[cfg(debug_assertions)]
                let index = swapped_entity.index();
                // SAFETY: The swapped entity was just fetched from the entity Vec, it must have already
                // been inserted and in bounds.
                unsafe {
                    *self.sparse.get_mut(index).debug_checked_unwrap() = dense_index;
                }
                // SAFETY: The above check ensures that `dense_index` and the last element are not
                // overlapping, and thus also within bounds.
                unsafe {
                    self.dense
                        .swap_remove_and_forget_unchecked_nonoverlapping(last, dense_index)
                }
            }
        })
    }

    /// Removes (and drops) the entity's component value from the sparse set.
    ///
    /// Returns `true` if `entity` had a component value in the sparse set.
    pub(crate) fn remove(&mut self, entity: Entity) -> bool {
        self.sparse
            .remove(entity.index())
            .map(|dense_index| {
                #[cfg(debug_assertions)]
                assert_eq!(entity, self.entities[dense_index.index()]);
                let last = self.entities.len() - 1;
                if dense_index.index() >= last {
                    #[cfg(debug_assertions)]
                    assert_eq!(dense_index.index(), last);
                    // SAFETY: This is strictly decreasing the length, so it cannot outgrow
                    // it also cannot underflow as an item was just removed from the sparse array.
                    unsafe {
                        self.entities.set_len(last);
                    }
                    // SAFETY: `last` is guaranteed to be the last element in `dense` as the length is synced with
                    // the `entities` store.
                    unsafe {
                        self.dense.drop_last_component(last);
                    }
                } else {
                    // SAFETY: The above check ensures that `dense_index` and the last element are not
                    // overlapping, and thus also within bounds.
                    unsafe {
                        self.entities
                            .swap_remove_nonoverlapping_unchecked(dense_index.index());
                    }
                    // SAFETY: The above check ensures that `dense_index` is in bounds.
                    let swapped_entity =
                        unsafe { self.entities.get_unchecked(dense_index.index()) };
                    #[cfg(not(debug_assertions))]
                    let index = *swapped_entity;
                    #[cfg(debug_assertions)]
                    let index = swapped_entity.index();
                    // SAFETY: The swapped entity was just fetched from the entity Vec, it must have already
                    // been inserted and in bounds.
                    unsafe {
                        *self.sparse.get_mut(index).debug_checked_unwrap() = dense_index;
                    }
                    // SAFETY: The above check ensures that `dense_index` and the last element are not
                    // overlapping, and thus also within bounds.
                    unsafe {
                        self.dense
                            .swap_remove_and_drop_unchecked_nonoverlapping(last, dense_index);
                    }
                }
            })
            .is_some()
    }

    pub(crate) fn check_change_ticks(&mut self, check: CheckChangeTicks) {
        // SAFETY: This is using the valid size of the column.
        unsafe {
            self.dense.check_change_ticks(self.len(), check);
        }
    }
}

impl Drop for ComponentSparseSet {
    fn drop(&mut self) {
        let len = self.entities.len();
        self.entities.clear();
        // SAFETY: `cap` and `len` are correct. `dense` is never accessed again after this call.
        unsafe {
            self.dense.drop(self.entities.capacity(), len);
        }
    }
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
