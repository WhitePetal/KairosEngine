use std::{cell::UnsafeCell, num::NonZeroUsize, panic::Location};

use nonmax::NonMaxU32;

use crate::{debug::{DebugCheckedUnwrap, MaybeLocation}, ecs::{change_detection::{CheckChangeTicks, ComponentTicks, Tick}, component::{ComponentId, ComponentInfo}, entity::Entity, storage::{ImmutableSparseSet, SparseSet}}, on_drop::AbortOnPanic, ptr::{OwningPtr, Ptr}};

mod column;

pub use column::*;

/// An opaque unique ID for a [`Table`] within a [`World`].
///
/// Can be used with [`Tables::get`] to fetch the corresponding
/// table.
///
/// Each [`Archetype`] always points to a table via [`Archetype::table_id`].
/// Multiple archetypes can point to the same table so long as the components
/// stored in the table are identical, but do not share the same sparse set
/// components.
///
/// [`World`]: crate::world::World
/// [`Archetype`]: crate::archetype::Archetype
/// [`Archetype::table_id`]: crate::archetype::Archetype::table_id
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableId(u32);

impl TableId {
    /// Creates a new [`TableId`].
    ///
    /// `index` *must* be retrieved from calling [`TableId::as_u32`] on a `TableId` you got
    /// from a table of a given [`World`] or the created ID may be invalid.
    ///
    /// [`World`]: crate::world::World
    #[inline]
    pub const fn from_u32(index: u32) -> Self {
        Self(index)
    }

    /// Creates a new [`TableId`].
    ///
    /// `index` *must* be retrieved from calling [`TableId::as_usize`] on a `TableId` you got
    /// from a table of a given [`World`] or the created ID may be invalid.
    ///
    /// [`World`]: crate::world::World
    ///
    /// # Panics
    ///
    /// Will panic if the provided value does not fit within a [`u32`].
    #[inline]
    pub const fn from_usize(index: usize) -> Self {
        debug_assert!(index as u32 as usize == index);
        Self(index as u32)
    }

    /// Gets the underlying table index from the ID.
    #[inline]
    pub const fn as_u32(self) -> u32 {
        self.0
    }

    /// Gets the underlying table index from the ID.
    #[inline]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }

    /// The [`TableId`] of the [`Table`] without any components.
    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }
}

/// An opaque newtype for rows in [`Table`]s. Specifies a single row in a specific table.
///
/// Values of this type are retrievable from [`Archetype::entity_table_row`] and can be
/// used alongside [`Archetype::table_id`] to fetch the exact table and row where an
/// [`Entity`]'s components are stored.
///
/// Values of this type are only valid so long as entities have not moved around.
/// Adding and removing components from an entity, or despawning it will invalidate
/// potentially any table row in the table the entity was previously stored in. Users
/// should *always* fetch the appropriate row from the entity's [`Archetype`] before
/// fetching the entity's components.
///
/// [`Archetype`]: crate::archetype::Archetype
/// [`Archetype::entity_table_row`]: crate::archetype::Archetype::entity_table_row
/// [`Archetype::table_id`]: crate::archetype::Archetype::table_id
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct TableRow(NonMaxU32);

impl TableRow {
    /// Creates a [`TableRow`].
    #[inline]
    pub const fn new(index: NonMaxU32) -> Self {
        Self(index)
    }

    /// Gets the index of the row as a [`usize`].
    #[inline]
    pub const fn index(self) -> usize {
        // usize is at least u32 in Bevy
        self.0.get() as usize
    }

    /// Gets the index of the row as a [`usize`].
    #[inline]
    pub const fn index_u32(self) -> u32 {
        self.0.get()
    }
}

/// A builder type for constructing [`Table`]s.
///
///  - Use [`with_capacity`] to initialize the builder.
///  - Repeatedly call [`add_column`] to add columns for components.
///  - Finalize with [`build`] to get the constructed [`Table`].
///
/// [`with_capacity`]: Self::with_capacity
/// [`add_column`]: Self::add_column
/// [`build`]: Self::build
//
// # Safety
// The capacity of all columns is determined by that of the `entities` Vec. This means that
// it must be the correct capacity to allocate, reallocate, and deallocate all columns. This
// means the safety invariant must be enforced even in `TableBuilder`.
pub(crate) struct TableBuilder {
    columns: SparseSet<ComponentId, Column>,
    entities: Vec<Entity>,
}

impl TableBuilder {
    /// Start building a new [`Table`] with a specified
    /// `column_capacity` (How many columns?) and `capacity` (How many entities per column?).
    pub fn with_capacity(capacity: usize, column_capacity: usize) -> Self {
        Self {
            columns: SparseSet::with_capacity(column_capacity),
            entities: Vec::with_capacity(capacity)
        }
    }

    /// Add a new column to the [`Table`].
    ///
    /// Specify the component which will be stored in the [`column`](Column) using its [`ComponentId`]
    ///
    /// Columns must be added in order of increasing [`ComponentId`],
    /// or else [`TableBuilder::build`] will panic.
    #[must_use]
    pub fn add_column(mut self, component_info: &ComponentInfo) -> Self {
        self.columns.insert(
            component_info.id(),
            Column::with_capacity(component_info, self.entities.capacity())
        );
        self
    }

    /// Build the [`Table`].
    ///
    /// After this operation, the caller won't be able to add more columns.
    ///
    /// # Panics
    /// - If the table's columns were not added in order, sorted by [`ComponentId`].
    #[must_use]
    pub fn build(self) -> Table {
        assert!(self.columns.indices().is_sorted());
        Table {
            columns: self.columns.into_immutable(),
            entities: self.entities
        }
    }
}

/// A column-oriented [structure-of-arrays] based storage for [`Component`]s of entities
/// in a [`World`].
///
/// Conceptually, a `Table` can be thought of as a `HashMap<ComponentId, Column>`, where
/// each [`Column`] is a type-erased `Vec<T: Component>`. Each row corresponds to a single entity
/// (i.e. index 3 in Column A and index 3 in Column B point to different components on the same
/// entity). Fetching components from a table involves fetching the associated column for a
/// component type (via its [`ComponentId`]), then fetching the entity's row within that column.
///
/// [structure-of-arrays]: https://en.wikipedia.org/wiki/AoS_and_SoA#Structure_of_arrays
/// [`Component`]: crate::component::Component
/// [`World`]: crate::world::World
//
// # Safety
// The capacity of all columns is determined by that of the `entities` Vec. This means that
// it must be the correct capacity to allocate, reallocate, and deallocate all columns. This
// means the safety invariant must be enforced even in `TableBuilder`.
pub struct Table {
    columns: ImmutableSparseSet<ComponentId, Column>,
    entities: Vec<Entity>,
}

impl Table {
    /// Fetches a read-only slice of the entities stored within the [`Table`].
    #[inline]
    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }

    /// Get the capacity of this table, in entities.
    /// Note that if an allocation is in process, this might not match the actual capacity of the columns, but it should once the allocation ends.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.entities.capacity()
    }

    /// Gets the number of entities currently being stored in the table.
    #[inline]
    pub fn entity_count(&self) -> u32 {
        // No entity may have more than one table row, so there are no duplicates,
        // and there may only ever be u32::MAX entities, so the length never exceeds u32's capacity.
        self.entities.len() as u32
    }

    /// Removes the entity at the given row and returns the entity swapped in to replace it (if an
    /// entity was swapped in)
    ///
    /// # Safety
    /// `row` must be in-bounds (`row.as_usize()` < `self.len()`)
    pub(crate) unsafe fn swap_remove_unchecked(&mut self, row: TableRow) -> Option<Entity> {
        debug_assert!(row.index_u32() < self.entity_count());
        let last_element_index = self.entity_count() - 1;
        if row.index_u32() != last_element_index {
            // Instead of checking this condition on every `swap_remove` call, we
            // check it here and use `swap_remove_nonoverlapping`.
            for col in self.columns.values_mut() {
                // SAFETY:
                // - `row` < `len`
                // - `last_element_index` = `len` - 1
                // - `row` != `last_element_index`
                // - the `len` is kept within `self.entities`, it will update accordingly.
                unsafe {
                    col.swap_remove_and_drop_unchecked_nonoverlapping(
                        last_element_index as usize,
                        row
                    );
                }
            }
        } else {
            // If `row.as_usize()` == `last_element_index` than there's no point in removing the component
            // at `row`, but we still need to drop it.
            for col in self.columns.values_mut() {
                col.drop_last_component(last_element_index as usize);
            }
        }
        let is_last = row.index_u32() == last_element_index;
        self.entities.swap_remove(row.index());
        if is_last {
            None
        } else {
            // SAFETY: This was swap removed and was not last, so it must be in bounds.
            unsafe { Some(*self.entities.get_unchecked(row.index())) }
        }
    }

    /// Fetches a read-only reference to the [`Column`] for a given [`Component`] within the table.
    ///
    /// Returns `None` if the corresponding component does not belong to the table.
    ///
    /// [`Component`]: crate::component::Component
    #[inline]
    pub fn get_column(&self, component_id: ComponentId) -> Option<&Column> {
        self.columns.get(component_id)
    }

    /// Fetches a mutable reference to the [`Column`] for a given [`Component`] within the
    /// table.
    ///
    /// Returns `None` if the corresponding component does not belong to the table.
    ///
    /// [`Component`]: crate::component::Component
    #[inline]
    pub(crate) fn get_column_mut(&mut self, component_id: ComponentId) -> Option<&mut Column> {
        self.columns.get_mut(component_id)
    }

    /// Get the data of the column matching `component_id` as a slice.
    ///
    /// # Safety
    /// `row.as_usize()` < `self.len()`
    /// - `T` must match the `component_id`
    pub unsafe fn get_data_slice_for<T>(
        &self,
        component_id: ComponentId
    ) -> Option<&[UnsafeCell<T>]> {
        self.get_column(component_id)
            .map(|col| col.get_data_slice(self.entity_count() as usize))
    }

    /// Get the added ticks of the column matching `component_id` as a slice.
    pub fn get_added_ticks_slice_for(
        &self,
        component_id: ComponentId
    ) -> Option<&[UnsafeCell<Tick>]> {
        self.get_column(component_id)
            // SAFETY: `self.len()` is guaranteed to be the len of the ticks array
            .map(|col| unsafe {
                col.get_added_ticks_slice(self.entity_count() as usize)
            })
    }

    /// Get the changed ticks of the column matching `component_id` as a slice.
    pub fn get_changed_ticks_slice_for(
        &self,
        component_id: ComponentId
    ) -> Option<&[UnsafeCell<Tick>]> {
        self.get_column(component_id)
            // SAFETY: `self.len()` is guaranteed to be the len of the ticks array
            .map(|col| unsafe {
                col.get_changed_ticks_slice(self.entity_count() as usize)
            })
    }

    /// Fetches the calling locations that last changed the each component
    pub fn get_changed_by_slice_for(
        &self,
        component_id: ComponentId
    ) -> MaybeLocation<Option<&[UnsafeCell<&'static Location<'static>>]>> {
        MaybeLocation::new_with_flattened(|| {
            self.get_column(component_id)
                // SAFETY: `self.len()` is guaranteed to be the len of the locations array
                .map(|col| unsafe {
                    col.get_changed_by_slice(self.entity_count() as usize)
                })
        })
    }

    /// Get the specific [`change tick`](Tick) of the component matching `component_id` in `row`.
    pub fn get_changed_tick(
        &self,
        component_id: ComponentId,
        row: TableRow
    ) -> Option<&UnsafeCell<Tick>> {
        if row.index_u32() >= self.entity_count() {
            return None;
        }

        // SAFETY: `row.index()` < `len`
        self.get_column(component_id)
            .map(|col| unsafe {
                col.changed_ticks.get_unchecked(row.index())
            })
    }

    /// Get the specific [`added tick`](Tick) of the component matching `component_id` in `row`.
    pub fn get_added_tick(
        &self,
        component_id: ComponentId,
        row: TableRow,
    ) -> Option<&UnsafeCell<Tick>> {
        if row.index_u32() >= self.entity_count() {
            return None;
        }

        // SAFETY: `row.index()` < `len`
        self.get_column(component_id)
            .map(|col| unsafe { col.added_ticks.get_unchecked(row.index()) })
    }

    /// Get the specific calling location that changed the component matching `component_id` in `row`
    pub fn get_changed_by(
        &self,
        component_id: ComponentId,
        row: TableRow,
    ) -> MaybeLocation<Option<&UnsafeCell<&'static Location<'static>>>> {
        MaybeLocation::new_with_flattened(|| {
            if row.index_u32() >= self.entity_count() {
                return None;
            }

            self.get_column(component_id).map(|col| {
                // SAFETY: `row.index()` < `len`
                col.changed_by
                    .as_ref()
                    .map(|changed_by| unsafe { changed_by.get_unchecked(row.index()) })
            })
        })
    }

    /// Get the [`ComponentTicks`] of the component matching `component_id` in `row`.
    ///
    /// # Safety
    /// - `row.as_usize()` < `self.len()`
    pub unsafe fn get_ticks_unchecked(
        &self,
        component_id: ComponentId,
        row: TableRow,
    ) -> Option<ComponentTicks> {
        self.get_column(component_id).map(|col| ComponentTicks {
            added: col.added_ticks.get_unchecked(row.index()).read(),
            changed: col.changed_ticks.get_unchecked(row.index()).read(),
        })
    }

    /// Checks if the table contains a [`Column`] for a given [`Component`].
    ///
    /// Returns `true` if the column is present, `false` otherwise.
    ///
    /// [`Component`]: crate::component::Component
    #[inline]
    pub fn has_column(&self, component_id: ComponentId) -> bool {
        self.columns.contains(component_id)
    }

    /// Allocate memory for the columns in the [`Table`]
    ///
    /// # Panics
    /// - Panics if any of the new capacity overflows `isize::MAX` bytes.
    /// - Panics if any of the new allocations causes an out-of-memory error.
    ///
    /// The current capacity of the columns should be 0, if it's not 0, then the previous data will be overwritten and leaked.
    ///
    /// # Safety
    /// The capacity of all columns is determined by that of the `entities` Vec. This means that
    /// it must be the correct capacity to allocate, reallocate, and deallocate all columns. This
    /// means the safety invariant must be enforced even in `TableBuilder`.
    fn alloc_columns(&mut self, new_capacity: NonZeroUsize) {
        // If any of these allocations trigger an unwind, the wrong capacity will be used while dropping this table - UB.
        // To avoid this, we use `AbortOnPanic`. If the allocation triggered a panic, the `AbortOnPanic`'s Drop impl will be
        // called, and abort the program.
        let _guard = AbortOnPanic;
        for col in self.columns.values_mut() {
            col.alloc(new_capacity);
        }
        std::mem::forget(_guard); // The allocation was successful, so we don't drop the guard.
    }

    /// Reallocate memory for the columns in the [`Table`]
    ///
    /// # Panics
    /// - Panics if any of the new capacities overflows `isize::MAX` bytes.
    /// - Panics if any of the new reallocations causes an out-of-memory error.
    ///
    /// # Safety
    /// - `current_column_capacity` is indeed the capacity of the columns
    ///
    /// The capacity of all columns is determined by that of the `entities` Vec. This means that
    /// it must be the correct capacity to allocate, reallocate, and deallocate all columns. This
    /// means the safety invariant must be enforced even in `TableBuilder`.
    unsafe fn realloc_columns(
        &mut self,
        current_column_capacity: NonZeroUsize,
        new_capacity: NonZeroUsize
    ) {
        // If any of these allocations trigger an unwind, the wrong capacity will be used while dropping this table - UB.
        // To avoid this, we use `AbortOnPanic`. If the allocation triggered a panic, the `AbortOnPanic`'s Drop impl will be
        // called, and abort the program.
        let _guard = AbortOnPanic;

        // SAFETY:
        // - There's no overflow
        // - `current_capacity` is indeed the capacity - safety requirement
        // - current capacity > 0
        for col in self.columns.values_mut() {
            col.realloc(current_column_capacity, new_capacity);
        }
        std::mem::forget(_guard); // The allocation was successful, so we don't drop the guard.
    }

    /// Reserves `additional` elements worth of capacity within the table.
    pub(crate) fn reserve(&mut self, additional: usize) {
        if (self.capacity() - self.entity_count() as usize) < additional {
            let column_cap = self.capacity();
            self.entities.reserve(additional);

            // use entities vector capacity as driving capacity for all related allocations
            let new_capacity = self.entities.capacity();

            if column_cap == 0 {
                // SAFETY: the current capacity is 0
                unsafe { self.alloc_columns(NonZeroUsize::new_unchecked(new_capacity)); }
            } else {
                // SAFETY:
                // - `column_cap` is indeed the columns' capacity
                unsafe {
                    self.realloc_columns(NonZeroUsize::new_unchecked(column_cap), NonZeroUsize::new_unchecked(new_capacity));
                }
            }
        }
    }

    /// Allocates space for a new entity
    ///
    /// # Panics
    /// - Panics if the allocation forces a reallocation and the new capacities overflows `isize::MAX` bytes.
    /// - Panics if the allocation forces a reallocation and causes an out-of-memory error.
    ///
    /// # Safety
    ///
    /// The allocated row must be written to immediately with valid values in each column
    pub(crate) unsafe fn allocate(&mut self, entity: Entity) -> TableRow {
        self.reserve(1);
        let len = self.entity_count();
        // SAFETY: No entity index may be in more than one table row at once, so there are no duplicates,
        // and there can not be an entity index of u32::MAX. Therefore, this can not be max either.
        let row = unsafe {
            TableRow::new(NonMaxU32::new_unchecked(len))
        };
        let len = len as usize;
        self.entities.push(entity);
        for col in self.columns.values_mut() {
            col.added_ticks
                .initialize_unchecked(len, UnsafeCell::new(Tick::new(0)));
            col.changed_ticks
                .initialize_unchecked(len, UnsafeCell::new(Tick::new(0)));
            col.changed_by
                .as_mut()
                .zip(MaybeLocation::caller())
                .map(|(changed_by, caller)| {
                    changed_by.initialize_unchecked(len, UnsafeCell::new(caller));
                });
        }

        row
    }

    /// Get the drop function for some component that is stored in this table.
    #[inline]
    pub fn get_drop_for(&self, component_id: ComponentId) -> Option<unsafe fn(OwningPtr<'_>)> {
        self.get_column(component_id)?.data.drop
    }

    /// Gets the number of components being stored in the table.
    #[inline]
    pub fn component_count(&self) -> usize {
        self.columns.len()
    }

    /// Gets the maximum number of entities the table can currently store
    /// without reallocating the underlying memory.
    #[inline]
    pub fn entity_capacity(&self) -> usize {
        self.entities.capacity()
    }

    /// Checks if the [`Table`] is empty or not.
    ///
    /// Returns `true` if the table contains no entities, `false` otherwise.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    /// Call [`Tick::check_tick`] on all of the ticks in the [`Table`]
    pub(crate) fn check_change_ticks(&mut self, check: CheckChangeTicks) {
        let len = self.entity_count() as usize;
        for col in self.columns.values_mut() {
            // SAFETY: `len` is the actual length of the column
            unsafe { col.check_change_ticks(len, check); }
        }
    }

    /// Iterates over the [`Column`]s of the [`Table`].
    pub fn iter_columns(&self) -> impl Iterator<Item = &Column> {
        self.columns.values()
    }

    /// Clears all of the stored components in the [`Table`].
    ///
    /// # Panics
    /// - Panics if any of the components in any of the columns panics while being dropped.
    pub(crate) fn clear(&mut self) {
        let len = self.entity_count() as usize;
        // We must clear the entities first, because in the drop function causes a panic, it will result in a double free of the columns.
        self.entities.clear();
        for column in self.columns.values_mut() {
            // SAFETY: we defer `self.entities.clear()` until after clearing the columns,
            // so `self.len()` should match the columns' len
            unsafe { column.clear(len); }
        }
    }

    /// Moves component data out of the [`Table`].
    ///
    /// This function leaves the underlying memory unchanged, but the component behind
    /// returned pointer is semantically owned by the caller and will not be dropped in its original location.
    /// Caller is responsible to drop component data behind returned pointer.
    ///
    /// # Safety
    /// - This table must hold the component matching `component_id`
    /// - `row` must be in bounds
    /// - The row's inconsistent state that happens after taking the component must be resolved—either initialize a new component or remove the row.
    pub(crate) unsafe fn take_component(
        &mut self,
        component_id: ComponentId,
        row: TableRow
    ) -> OwningPtr<'_> {
        self.get_column_mut(component_id)
            .debug_checked_unwrap()
            .data
            .get_unchecked_mut(row.index())
            .promote()
    }

    /// Get the component at a given `row`, if the [`Table`] stores components with the given `component_id`
    ///
    /// # Safety
    /// `row.as_usize()` < `self.len()`
    pub unsafe fn get_component(
        &self,
        component_id: ComponentId,
        row: TableRow
    ) -> Option<Ptr<'_>> {
        self.get_column(component_id)
            .map(|col| col.data.get_unchecked(row.index()))
    }
}

// TODO!
