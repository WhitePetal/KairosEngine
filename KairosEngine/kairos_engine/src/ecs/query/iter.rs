use std::{cmp::Ordering, iter::FusedIterator, ops::Range};

use nonmax::NonMaxU32;

use crate::{
    debug::DebugCheckedUnwrap,
    ecs::{
        archetype::{Archetype, ArchetypeEntity, Archetypes},
        change_detection::Tick,
        entity::{Entities, Entity, EntitySetIterator},
        query::{
            IterQueryData, QueryData, QueryFilter, QueryState, ReadOnlyQueryData,
            SingleEntityQueryData, StorageId,
        },
        storage::{Table, TableRow, Tables},
        world::{EntityMut, EntityRef, unsafe_world_cell::UnsafeWorldCell},
    },
};

/// An [`Iterator`] over query results of a [`Query`](crate::system::Query).
///
/// This struct is created by the [`Query::iter`](crate::system::Query::iter) and
/// [`Query::iter_mut`](crate::system::Query::iter_mut) methods.
pub struct QueryIter<'w, 's, D: QueryData, F: QueryFilter> {
    world: UnsafeWorldCell<'w>,
    tables: &'w Tables,
    archetypes: &'w Archetypes,
    query_state: &'s QueryState<D, F>,
    cursor: QueryIterationCursor<'w, 's, D, F>,
}

impl<'w, 's, D: QueryData, F: QueryFilter> QueryIter<'w, 's, D, F> {
    /// # Safety
    /// - `world` must have permission to access any of the components registered in `query_state`.
    /// - `world` must be the same one used to initialize `query_state`.
    pub(crate) unsafe fn new(
        world: UnsafeWorldCell<'w>,
        query_state: &'s QueryState<D, F>,
        last_run: Tick,
        this_run: Tick,
    ) -> Self {
        QueryIter {
            world,
            query_state,
            // SAFETY: We only access table data that has been registered in `query_state`.
            tables: unsafe { &world.storages().tables },
            archetypes: world.archetypes(),
            // SAFETY: The invariants are upheld by the caller.
            cursor: unsafe { QueryIterationCursor::init(world, query_state, last_run, this_run) },
        }
    }

    /// Creates a new separate iterator yielding the same remaining items of the current one.
    /// Advancing the new iterator will not advance the original one, which will resume at the
    /// point it was left at.
    ///
    /// Differently from [`remaining_mut`](QueryIter::remaining_mut) the new iterator does not
    /// borrow from the original one. However it can only be called from an iterator over read only
    /// items.
    ///
    /// # Example
    ///
    /// ```
    /// # use bevy_ecs::prelude::*;
    /// #
    /// # #[derive(Component)]
    /// # struct ComponentA;
    ///
    /// fn combinations(query: Query<&ComponentA>) {
    ///     let mut iter = query.iter();
    ///     while let Some(a) = iter.next() {
    ///         for b in iter.remaining() {
    ///             // Check every combination (a, b)
    ///         }
    ///     }
    /// }
    /// ```
    pub fn remaining(&self) -> QueryIter<'w, 's, D, F>
    where
        D: ReadOnlyQueryData,
    {
        QueryIter {
            world: self.world,
            tables: self.tables,
            archetypes: self.archetypes,
            query_state: self.query_state,
            cursor: self.cursor.clone(),
        }
    }

    /// Creates a new separate iterator yielding the same remaining items of the current one.
    /// Advancing the new iterator will not advance the original one, which will resume at the
    /// point it was left at.
    ///
    /// This method can be called on iterators over mutable items. However the original iterator
    /// will be borrowed while the new iterator exists and will thus not be usable in that timespan.
    ///
    /// # Example
    ///
    /// ```
    /// # use bevy_ecs::prelude::*;
    /// #
    /// # #[derive(Component)]
    /// # struct ComponentA;
    ///
    /// fn combinations(mut query: Query<&mut ComponentA>) {
    ///     let mut iter = query.iter_mut();
    ///     while let Some(a) = iter.next() {
    ///         for b in iter.remaining_mut() {
    ///             // Check every combination (a, b)
    ///         }
    ///     }
    /// }
    /// ```
    pub fn remaining_mut(&mut self) -> QueryIter<'_, 's, D, F> {
        QueryIter {
            world: self.world,
            tables: self.tables,
            archetypes: self.archetypes,
            query_state: self.query_state,
            cursor: self.cursor.reborrow(),
        }
    }

    /// Get the next result from the query.
    ///
    /// If the [`QueryData`] does not implement [`IterQueryData`],
    /// then it is not sound to yield multiple items concurrently
    /// and the resulting [`QueryIter`] will not implement [`Iterator`].
    /// In that case, this method can be used to iterate over the items
    /// while ensuring only one is alive at a time.
    ///
    /// Most queries do implement [`IterQueryData`],
    /// and can use the ordinary [`Iterator::next`]
    /// method or a `for` loop.
    ///
    /// # Example
    ///
    /// ```
    /// # use bevy_ecs::prelude::*;
    /// # #[derive(Component)]
    /// # struct C;
    /// fn system(mut query: Query<&mut C>) {
    ///     let mut iter = query.iter_mut();
    ///     while let Some(mut c) = iter.fetch_next() {
    ///         //
    ///     }
    /// }
    /// # bevy_ecs::system::assert_is_system(system);
    /// ```
    pub fn fetch_next(&mut self) -> Option<D::Item<'_, 's>> {
        // SAFETY:
        // - `tables` and `archetypes` belong to the same world that the cursor was initialized for.
        // - `query_state` is the state that was passed to `QueryIterationCursor::init`.
        // - `self` is mutably borrowed, so there are no other items alive for any entity.
        unsafe {
            self.cursor
                .next(self.tables, self.archetypes, self.query_state)
                .map(D::shrink)
        }
    }
}

impl<'w, 's, D: IterQueryData, F: QueryFilter> QueryIter<'w, 's, D, F> {
    /// Executes the equivalent of [`Iterator::fold`] over a contiguous segment
    /// from a storage.
    ///
    ///  # Safety
    ///  - `range` must be in `[0, storage::entity_count)` or None.
    #[inline]
    pub(super) unsafe fn fold_over_storage_range<B, Func>(
        &mut self,
        mut accum: B,
        func: &mut Func,
        storage: StorageId,
        range: Option<Range<u32>>,
    ) -> B
    where
        Func: FnMut(B, D::Item<'w, 's>) -> B,
    {
        if self.cursor.is_dense {
            // SAFETY: `self.cursor.is_dense` is true, so storage ids are guaranteed to be table ids.
            let table_id = unsafe { storage.table_id };
            // SAFETY: Matched table IDs are guaranteed to still exist.
            let table = unsafe { self.tables.get(table_id).debug_checked_unwrap() };

            let range = range.unwrap_or(0..table.entity_count());
            // SAFETY:
            // - The fetched table matches both D and F
            // - caller ensures `range` is within `[0, table.entity_count)`
            // - The if block ensures that the query iteration is dense
            accum = unsafe { self.fold_over_table_range(accum, func, table, range) }
        } else {
            // SAFETY: `self.cursor.is_dense` is false, so storage ids are guaranteed to be archetype ids.
            let archetype_id = unsafe { storage.archetype_id };
            // SAFETY: Matched archetype IDs are guaranteed to still exist.
            let archetype = unsafe { self.archetypes.get(archetype_id).debug_checked_unwrap() };
            // SAFETY: Matched table IDs are guaranteed to still exist.
            let table = unsafe { self.tables.get(archetype.table_id()).debug_checked_unwrap() };

            let range = range.unwrap_or(0..archetype.len());

            if table.entity_count() == archetype.len() {
                // SAFETY:
                // - The fetched archetype matches both D and F
                // - The provided archetype and its' table have the same length.
                // - caller ensures `range` is within `[0, archetype.len)`
                // - The if block ensures that the query iteration is not dense.
                accum =
                    unsafe { self.fold_over_dense_archetype_range(accum, func, archetype, range) }
            } else {
                // SAFETY:
                // - The fetched archetype matches both D and F
                // - caller ensures `range` is within `[0, archetype.len)`
                // - The if block ensures that the query iteration is not dense.
                accum = unsafe { self.fold_over_archetype_range(accum, func, archetype, range) }
            }
        }
        accum
    }

    /// Executes the equivalent of [`Iterator::fold`] over a contiguous segment
    /// from a table.
    ///
    /// # Safety
    ///  - all `rows` must be in `[0, table.entity_count)`.
    ///  - `table` must match D and F
    ///  - The query iteration must be dense (i.e. `self.query_state.is_dense` must be true).
    #[inline]
    pub(super) unsafe fn fold_over_table_range<B, Func>(
        &mut self,
        mut accum: B,
        func: &mut Func,
        table: &'w Table,
        rows: Range<u32>,
    ) -> B
    where
        Func: FnMut(B, D::Item<'w, 's>) -> B,
    {
        if table.is_empty() {
            return accum;
        }

        unsafe {
            D::set_table(&mut self.cursor.fetch, &self.query_state.fetch_state, table);
            F::set_table(
                &mut self.cursor.filter,
                &self.query_state.filter_state,
                table,
            );
        }

        let entities = table.entities();
        for row in rows {
            // SAFETY: Caller assures `row` in range of the current archetype.
            let entity = unsafe { entities.get_unchecked(row as usize) };
            // SAFETY: This is from an exclusive range, so it can't be max.
            let row = unsafe { TableRow::new(NonMaxU32::new_unchecked(row)) };

            // SAFETY: set_table was called prior.
            // Caller assures `row` in range of the current archetype.
            let fetched = unsafe {
                !F::filter_fetch(
                    &self.query_state.filter_state,
                    &mut self.cursor.filter,
                    *entity,
                    row,
                )
            };
            if fetched {
                continue;
            }

            // SAFETY:
            // - set_table was called prior.
            // - Caller assures `row` in range of the current archetype.
            // - Each row is unique, so each entity is only alive once
            // - `D: IterQueryData`
            if let Some(item) = unsafe {
                D::fetch(
                    &self.query_state.fetch_state,
                    &mut self.cursor.fetch,
                    *entity,
                    row,
                )
            } {
                accum = func(accum, item);
            }
        }
        accum
    }

    /// Executes the equivalent of [`Iterator::fold`] over a contiguous segment
    /// from an archetype.
    ///
    /// # Safety
    ///  - all `indices` must be in `[0, archetype.len())`.
    ///  - `archetype` must match D and F
    ///  - The query iteration must not be dense (i.e. `self.query_state.is_dense` must be false).
    #[inline]
    pub(super) unsafe fn fold_over_archetype_range<B, Func>(
        &mut self,
        mut accum: B,
        func: &mut Func,
        archetype: &'w Archetype,
        indices: Range<u32>,
    ) -> B
    where
        Func: FnMut(B, D::Item<'w, 's>) -> B,
    {
        if archetype.is_empty() {
            return accum;
        }
        let table = unsafe { self.tables.get(archetype.table_id()).debug_checked_unwrap() };
        unsafe {
            D::set_archetype(
                &mut self.cursor.fetch,
                &self.query_state.fetch_state,
                archetype,
                table,
            );
            F::set_archetype(
                &mut self.cursor.filter,
                &self.query_state.filter_state,
                archetype,
                table,
            );
        }

        let entities = archetype.entities();
        for index in indices {
            // SAFETY: Caller assures `index` in range of the current archetype.
            let archetype_entity = unsafe { entities.get_unchecked(index as usize) };

            // SAFETY: set_archetype was called prior.
            // Caller assures `index` in range of the current archetype.
            let fetched = unsafe {
                !F::filter_fetch(
                    &self.query_state.filter_state,
                    &mut self.cursor.filter,
                    archetype_entity.id(),
                    archetype_entity.table_row(),
                )
            };
            if fetched {
                continue;
            }

            // SAFETY:
            // - set_archetype was called prior, `index` is an archetype index in range of the current archetype
            // - Caller assures `index` in range of the current archetype.
            // - Each row is unique, so each entity is only alive once
            // - `D: IterQueryData`
            if let Some(item) = unsafe {
                D::fetch(
                    &self.query_state.fetch_state,
                    &mut self.cursor.fetch,
                    archetype_entity.id(),
                    archetype_entity.table_row(),
                )
            } {
                accum = func(accum, item);
            }
        }
        accum
    }

    /// Executes the equivalent of [`Iterator::fold`] over a contiguous segment
    /// from an archetype which has the same entity count as its table.
    ///
    /// # Safety
    ///  - all `indices` must be in `[0, archetype.len())`.
    ///  - `archetype` must match D and F
    ///  - `archetype` must have the same length as its table.
    ///  - The query iteration must not be dense (i.e. `self.query_state.is_dense` must be false).
    #[inline]
    pub(super) unsafe fn fold_over_dense_archetype_range<B, Func>(
        &mut self,
        mut accum: B,
        func: &mut Func,
        archetype: &'w Archetype,
        rows: Range<u32>,
    ) -> B
    where
        Func: FnMut(B, D::Item<'w, 's>) -> B,
    {
        if archetype.is_empty() {
            return accum;
        }
        let table = unsafe { self.tables.get(archetype.table_id()).debug_checked_unwrap() };
        debug_assert!(
            archetype.len() == table.entity_count(),
            "archetype and its table must have the same length."
        );

        unsafe {
            D::set_archetype(
                &mut self.cursor.fetch,
                &self.query_state.fetch_state,
                archetype,
                table,
            );
            F::set_archetype(
                &mut self.cursor.filter,
                &self.query_state.filter_state,
                archetype,
                table,
            );
        }
        let entities = table.entities();
        for row in rows {
            // SAFETY: Caller assures `row` in range of the current archetype.
            let entity = unsafe { *entities.get_unchecked(row as usize) };
            // SAFETY: This is from an exclusive range, so it can't be max.
            let row = unsafe { TableRow::new(NonMaxU32::new_unchecked(row)) };

            // SAFETY: set_table was called prior.
            // Caller assures `row` in range of the current archetype.
            let filter_matched = unsafe {
                F::filter_fetch(
                    &self.query_state.filter_state,
                    &mut self.cursor.filter,
                    entity,
                    row,
                )
            };
            if !filter_matched {
                continue;
            }

            // SAFETY:
            // - set_table was called prior.
            // - Caller assures `row` in range of the current archetype.
            // - Each row is unique, so each entity is only alive once
            // - `D: IterQueryData`
            if let Some(item) = unsafe {
                D::fetch(
                    &self.query_state.fetch_state,
                    &mut self.cursor.fetch,
                    entity,
                    row,
                )
            } {
                accum = func(accum, item)
            }
        }
        accum
    }
}

impl<'w, 's, D: QueryData, F: QueryFilter> QueryIter<'w, 's, D, F> {
    /// Sorts all query items into a new iterator, using the query lens as a key.
    ///
    /// This sort is stable (i.e., does not reorder equal elements).
    ///
    /// This uses [`slice::sort`] internally.
    ///
    /// Defining the lens works like [`transmute_lens`](crate::system::Query::transmute_lens).
    /// This includes the allowed parameter type changes listed under [allowed transmutes].
    /// However, the lens uses the filter of the original query when present.
    ///
    /// The lens needs to be a [`SingleEntityQueryData`] because the current implementation
    /// of query transmutes does not support nested queries.
    /// This restriction may be lifted in the future.
    ///
    /// The sort is not cached across system runs.
    ///
    /// If the [`QueryData`] does not implement [`IterQueryData`],
    /// then it is not sound to yield multiple items concurrently
    /// and the resulting [`QuerySortedIter`] will not implement [`Iterator`].
    /// To iterate over the items in that case,
    /// use the [`QuerySortedIter::fetch_next()`](crate::query::QuerySortedIter::fetch_next) method,
    /// which ensures only one item is alive at a time.
    ///
    /// [allowed transmutes]: crate::system::Query#allowed-transmutes
    ///
    /// # Panics
    ///
    /// This will panic if `next` has been called on `QueryIter` before, unless the underlying `Query` is empty.
    ///
    /// # Examples
    /// ```rust
    /// # use bevy_ecs::prelude::*;
    /// # use std::{ops::{Deref, DerefMut}, iter::Sum};
    /// #
    /// # #[derive(Component)]
    /// # struct PartMarker;
    /// #
    /// # #[derive(Component, PartialEq, Eq, PartialOrd, Ord)]
    /// # struct PartIndex(usize);
    /// #
    /// # #[derive(Component, Clone, Copy)]
    /// # struct PartValue(f32);
    /// #
    /// # impl Deref for PartValue {
    /// #     type Target = f32;
    /// #
    /// #     fn deref(&self) -> &Self::Target {
    /// #         &self.0
    /// #     }
    /// # }
    /// #
    /// # #[derive(Component)]
    /// # struct ParentValue(f32);
    /// #
    /// # impl Deref for ParentValue {
    /// #     type Target = f32;
    /// #
    /// #     fn deref(&self) -> &Self::Target {
    /// #         &self.0
    /// #     }
    /// # }
    /// #
    /// # impl DerefMut for ParentValue {
    /// #     fn deref_mut(&mut self) -> &mut Self::Target {
    /// #         &mut self.0
    /// #     }
    /// # }
    /// #
    /// # #[derive(Component, Debug, PartialEq, Eq, PartialOrd, Ord)]
    /// # struct Length(usize);
    /// #
    /// # #[derive(Component, Debug, PartialEq, Eq, PartialOrd, Ord)]
    /// # struct Width(usize);
    /// #
    /// # #[derive(Component, Debug, PartialEq, Eq, PartialOrd, Ord)]
    /// # struct Height(usize);
    /// #
    /// # #[derive(Component, PartialEq, Eq, PartialOrd, Ord)]
    /// # struct ParentEntity(Entity);
    /// #
    /// # #[derive(Component, Clone, Copy)]
    /// # struct ChildPartCount(usize);
    /// #
    /// # impl Deref for ChildPartCount {
    /// #     type Target = usize;
    /// #
    /// #     fn deref(&self) -> &Self::Target {
    /// #         &self.0
    /// #     }
    /// # }
    /// # let mut world = World::new();
    /// // We can ensure that a query always returns in the same order.
    /// fn system_1(query: Query<(Entity, &PartIndex)>) {
    ///     let parts: Vec<(Entity, &PartIndex)> = query.iter().sort::<&PartIndex>().collect();
    /// }
    ///
    /// // We can freely rearrange query components in the key.
    /// fn system_2(query: Query<(&Length, &Width, &Height), With<PartMarker>>) {
    ///     for (length, width, height) in query.iter().sort::<(&Height, &Length, &Width)>() {
    ///         println!("height: {height:?}, width: {width:?}, length: {length:?}")
    ///     }
    /// }
    ///
    /// // We can sort by Entity without including it in the original Query.
    /// // Here, we match iteration orders between query iterators.
    /// fn system_3(
    ///     part_query: Query<(&PartValue, &ParentEntity)>,
    ///     mut parent_query: Query<(&ChildPartCount, &mut ParentValue)>,
    /// ) {
    ///     let part_values = &mut part_query
    ///         .into_iter()
    ///         .sort::<&ParentEntity>()
    ///         .map(|(&value, parent_entity)| *value);
    ///
    ///     for (&child_count, mut parent_value) in parent_query.iter_mut().sort::<Entity>() {
    ///         **parent_value = part_values.take(*child_count).sum();
    ///     }
    /// }
    /// #
    /// # let mut schedule = Schedule::default();
    /// # schedule.add_systems((system_1, system_2, system_3));
    /// # schedule.run(&mut world);
    /// ```
    pub fn sort<L: ReadOnlyQueryData + SingleEntityQueryData + 'w>(
        self,
    ) -> QuerySortedIter<
        'w,
        's,
        D,
        F,
        impl ExactSizeIterator<Item = Entity> + DoubleEndedIterator + FusedIterator + 'w,
    >
    where
        for<'lw, 'ls> L::Item<'lw, 'ls>: Ord,
    {
        self.sort_impl::<L>(|keyed_query| keyed_query.sort())
    }

    /// Shared implementation for the various `sort` methods.
    /// This uses the lens to collect the items for sorting, but delegates the actual sorting to the provided closure.
    ///
    /// Defining the lens works like [`transmute_lens`](crate::system::Query::transmute_lens).
    /// This includes the allowed parameter type changes listed under [allowed transmutes].
    /// However, the lens uses the filter of the original query when present.
    ///
    /// The lens needs to be a [`SingleEntityQueryData`] because the current implementation
    /// of query transmutes does not support nested queries.
    /// This restriction may be lifted in the future.
    ///
    /// The sort is not cached across system runs.
    ///
    /// If the [`QueryData`] does not implement [`IterQueryData`],
    /// then it is not sound to yield multiple items concurrently
    /// and the resulting [`QuerySortedIter`] will not implement [`Iterator`].
    /// To iterate over the items in that case,
    /// use the [`QuerySortedIter::fetch_next()`](crate::query::QuerySortedIter::fetch_next) method,
    /// which ensures only one item is alive at a time.
    ///
    /// [allowed transmutes]: crate::system::Query#allowed-transmutes
    ///
    /// # Panics
    ///
    /// This will panic if `next` has been called on `QueryIter` before, unless the underlying `Query` is empty.
    fn sort_impl<L: ReadOnlyQueryData + SingleEntityQueryData + 'w>(
        self,
        f: impl FnOnce(&mut Vec<(L::Item<'_, '_>, NeutralOrd<Entity>)>),
    ) -> QuerySortedIter<
        'w,
        's,
        D,
        F,
        impl ExactSizeIterator<Item = Entity> + DoubleEndedIterator + FusedIterator + 'w,
    > {
        // On the first successful iteration of `QueryIterationCursor`, `archetype_entities` or `table_entities`
        // will be set to a non-zero value. The correctness of this method relies on this.
        // I.e. this sort method will execute if and only if `next` on `QueryIterationCursor` of a
        // non-empty `QueryIter` has not yet been called. When empty, this sort method will not panic.
        if !self.cursor.archetype_entities.is_empty() || !self.cursor.table_entities.is_empty() {
            panic!("it is not valid to call sort() after next()")
        }

        let world = self.world;

        let query_lens_state = self.query_state.transmute_filtered::<(L, Entity), F>(world);

        // SAFETY:
        // `self.world` has permission to access the required components.
        // The original query iter has not been iterated on, so no items are aliased from it.
        // `QueryIter::new` ensures `world` is the same one used to initialize `query_state`.
        let query_lens = unsafe { query_lens_state.query_unchecked_manual(world) }.into_iter();
        let mut keyed_query: Vec<_> = query_lens
            .map(|(key, entity)| (key, NeutralOrd(entity)))
            .collect();
        f(&mut keyed_query);
        let entity_iter = keyed_query
            .into_iter()
            .map(|(.., entity)| entity.0)
            .collect::<Vec<_>>()
            .into_iter();
        // SAFETY:
        // `self.world` has permission to access the required components.
        // Each lens query item is dropped before the respective actual query item is accessed.
        unsafe {
            QuerySortedIter::new(
                world,
                self.query_state,
                entity_iter,
                world.last_change_tick(),
                world.change_tick(),
            )
        }
    }
}

impl<'w, 's, D: IterQueryData, F: QueryFilter> Iterator for QueryIter<'w, 's, D, F> {
    type Item = D::Item<'w, 's>;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        // SAFETY:
        // - `tables` and `archetypes` belong to the same world that the cursor was initialized for.
        // - `query_state` is the state that was passed to `QueryIterationCursor::init`.
        // - `D: IterQueryData`
        unsafe {
            self.cursor
                .next(self.tables, self.archetypes, self.query_state)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let max_size = self.cursor.max_remaining(self.tables, self.archetypes);
        let archetype_query = D::IS_ARCHETYPAL && F::IS_ARCHETYPAL;
        let min_size = if archetype_query { max_size } else { 0 };
        (min_size as usize, Some(max_size as usize))
    }

    #[inline]
    fn fold<B, Func>(mut self, init: B, mut func: Func) -> B
    where
        Func: FnMut(B, Self::Item) -> B,
    {
        let mut accum = init;
        // Empty any remaining uniterated values from the current table/archetype
        while self.cursor.current_row != self.cursor.current_len {
            let Some(item) = self.next() else { break };
            accum = func(accum, item);
        }

        for id in self.cursor.storage_id_iter.clone().copied() {
            // SAFETY:
            // - The range(None) is equivalent to [0, storage.entity_count)
            accum = unsafe { self.fold_over_storage_range(accum, &mut func, id, None) }
        }
        accum
    }
}

// This is correct as [`QueryIter`] always returns `None` once exhausted.
impl<'w, 's, D: IterQueryData, F: QueryFilter> FusedIterator for QueryIter<'w, 's, D, F> {}

// SAFETY: [`QueryIter`] is guaranteed to return every matching entity once and only once.
unsafe impl<'w, 's, F: QueryFilter> EntitySetIterator for QueryIter<'w, 's, Entity, F> {}

// SAFETY: [`QueryIter`] is guaranteed to return every matching entity once and only once.
unsafe impl<'w, 's, F: QueryFilter> EntitySetIterator for QueryIter<'w, 's, EntityRef<'_>, F> {}

// SAFETY: [`QueryIter`] is guaranteed to return every matching entity once and only once.
unsafe impl<'w, 's, F: QueryFilter> EntitySetIterator for QueryIter<'w, 's, EntityMut<'_>, F> {}

/// An [`Iterator`] over sorted query results of a [`Query`](crate::system::Query).
///
/// This struct is created by the [`QueryIter::sort`], [`QueryIter::sort_unstable`],
/// [`QueryIter::sort_by`], [`QueryIter::sort_unstable_by`], [`QueryIter::sort_by_key`],
/// [`QueryIter::sort_unstable_by_key`], and [`QueryIter::sort_by_cached_key`] methods.
pub struct QuerySortedIter<'w, 's, D: QueryData, F: QueryFilter, I>
where
    I: Iterator<Item = Entity>,
{
    entity_iter: I,
    entities: &'w Entities,
    tables: &'w Tables,
    archetypes: &'w Archetypes,
    fetch: D::Fetch<'w>,
    query_state: &'s QueryState<D, F>,
}

impl<'w, 's, D: QueryData, F: QueryFilter, I: Iterator> QuerySortedIter<'w, 's, D, F, I>
where
    I: Iterator<Item = Entity>,
{
    pub(crate) unsafe fn new<EntityList: IntoIterator<IntoIter = I>>(
        world: UnsafeWorldCell<'w>,
        query_state: &'s QueryState<D, F>,
        entity_list: EntityList,
        last_run: Tick,
        this_run: Tick,
    ) -> QuerySortedIter<'w, 's, D, F, I> {
        let fetch = unsafe { D::init_fetch(world, &query_state.fetch_state, last_run, this_run) };
        QuerySortedIter {
            query_state,
            entities: world.entities(),
            archetypes: world.archetypes(),
            // SAFETY: We only access table data that has been registered in `query_state`.
            // This means `world` has permission to access the data we use.
            tables: &unsafe { world.storages() }.tables,
            fetch,
            entity_iter: entity_list.into_iter(),
        }
    }
}

struct QueryIterationCursor<'w, 's, D: QueryData, F: QueryFilter> {
    // whether the query iteration is dense or not. Mirrors QueryState's `is_dense` field.
    is_dense: bool,
    storage_id_iter: std::slice::Iter<'s, StorageId>,
    table_entities: &'w [Entity],
    archetype_entities: &'w [ArchetypeEntity],
    fetch: D::Fetch<'w>,
    filter: F::Fetch<'w>,
    // length of the table or length of the archetype, depending on whether both `D`'s and `F`'s fetches are dense
    current_len: u32,
    // either table row or archetype index, depending on whether both `D`'s and `F`'s fetches are dense
    current_row: u32,
}

impl<D: QueryData, F: QueryFilter> Clone for QueryIterationCursor<'_, '_, D, F> {
    fn clone(&self) -> Self {
        Self {
            is_dense: self.is_dense,
            storage_id_iter: self.storage_id_iter.clone(),
            table_entities: self.table_entities,
            archetype_entities: self.archetype_entities,
            fetch: self.fetch.clone(),
            filter: self.filter.clone(),
            current_len: self.current_len,
            current_row: self.current_row,
        }
    }
}

impl<'w, 's, D: QueryData, F: QueryFilter> QueryIterationCursor<'w, 's, D, F> {
    /// # Safety
    /// - `world` must have permission to access any of the components registered in `query_state`.
    /// - `world` must be the same one used to initialize `query_state`.
    unsafe fn init_empty(
        world: UnsafeWorldCell<'w>,
        query_state: &'s QueryState<D, F>,
        last_run: Tick,
        this_run: Tick,
    ) -> Self {
        QueryIterationCursor {
            storage_id_iter: [].iter(),
            ..unsafe { Self::init(world, query_state, last_run, this_run) }
        }
    }

    /// # Safety
    /// - `world` must have permission to access any of the components registered in `query_state`.
    /// - `world` must be the same one used to initialize `query_state`.
    unsafe fn init(
        world: UnsafeWorldCell<'w>,
        query_state: &'s QueryState<D, F>,
        last_run: Tick,
        this_run: Tick,
    ) -> Self {
        let fetch = unsafe { D::init_fetch(world, &query_state.fetch_state, last_run, this_run) };
        let filter = unsafe { F::init_fetch(world, &query_state.filter_state, last_run, this_run) };
        QueryIterationCursor {
            fetch,
            filter,
            table_entities: &[],
            archetype_entities: &[],
            storage_id_iter: query_state.matched_storage_ids.iter(),
            is_dense: query_state.is_dense,
            current_len: 0,
            current_row: 0,
        }
    }

    fn reborrow(&mut self) -> QueryIterationCursor<'_, 's, D, F> {
        QueryIterationCursor {
            is_dense: self.is_dense,
            fetch: D::shrink_fetch(self.fetch.clone()),
            filter: F::shrink_fetch(self.filter.clone()),
            table_entities: self.table_entities,
            archetype_entities: self.archetype_entities,
            storage_id_iter: self.storage_id_iter.clone(),
            current_len: self.current_len,
            current_row: self.current_row,
        }
    }

    /// Retrieve item returned from most recent `next` call again.
    ///
    /// # Safety
    /// The result of `next` and any previous calls to `peek_last` with this row must have been
    /// dropped to prevent aliasing mutable references.
    unsafe fn peek_last(&mut self, query_state: &'s QueryState<D, F>) -> Option<D::Item<'w, 's>>
    where
        D: IterQueryData,
    {
        if self.current_row > 0 {
            let index = self.current_row - 1;
            if self.is_dense {
                // SAFETY: This must have been called previously in `next` as `current_row > 0`
                let entity = unsafe { self.table_entities.get_unchecked(index as usize) };
                // SAFETY:
                //  - `set_table` must have been called previously either in `next` or before it.
                //  - `*entity` and `index` are in the current table.
                //  - `D: IterQueryData`
                unsafe {
                    D::fetch(
                        &query_state.fetch_state,
                        &mut self.fetch,
                        *entity,
                        // SAFETY: This is from an exclusive range, so it can't be max.
                        TableRow::new(NonMaxU32::new_unchecked(index)),
                    )
                }
            } else {
                // SAFETY: This must have been called previously in `next` as `current_row > 0`
                let archetype_entity =
                    unsafe { self.archetype_entities.get_unchecked(index as usize) };
                // SAFETY:
                //  - `set_archetype` must have been called previously either in `next` or before it.
                //  - `archetype_entity.id()` and `archetype_entity.table_row()` are in the current archetype.
                //  - `D: IterQueryData`
                unsafe {
                    D::fetch(
                        &query_state.fetch_state,
                        &mut self.fetch,
                        archetype_entity.id(),
                        archetype_entity.table_row(),
                    )
                }
            }
        } else {
            None
        }
    }

    /// How many values will this cursor return at most?
    ///
    /// Note that if `D::IS_ARCHETYPAL && F::IS_ARCHETYPAL`, the return value
    /// will be **the exact count of remaining values**.
    fn max_remaining(&self, tables: &'w Tables, archetypes: &'w Archetypes) -> u32 {
        let ids = self.storage_id_iter.clone();
        let remaining_matched: u32 = if self.is_dense {
            // SAFETY: The if check ensures that storage_id_iter stores TableIds
            unsafe { ids.map(|id| tables[id.table_id].entity_count()).sum() }
        } else {
            // SAFETY: The if check ensures that storage_id_iter stores ArchetypeIds
            unsafe { ids.map(|id| archetypes[id.archetype_id].len()).sum() }
        };
        remaining_matched + self.current_len - self.current_row
    }

    // NOTE: If you are changing query iteration code, remember to update the following places, where relevant:
    // QueryIter, QueryIterationCursor, QuerySortedIter, QueryManyIter, QuerySortedManyIter, QueryCombinationIter,
    // QueryState::par_fold_init_unchecked_manual, QueryState::par_many_fold_init_unchecked_manual,
    // QueryState::par_many_unique_fold_init_unchecked_manual, QueryContiguousIter::next
    /// # Safety
    /// - `tables` and `archetypes` must belong to the same world that the [`QueryIterationCursor`]
    ///   was initialized for.
    /// - `query_state` must be the same [`QueryState`] that was passed to `init` or `init_empty`.
    /// - If `D` does not impl `ReadOnlyQueryData`, then there must not be any other `Item`s alive for the current entity
    /// - If `D` does not impl `IterQueryData`, then there must not be any other `Item`s alive for *any* entity
    #[inline(always)]
    unsafe fn next(
        &mut self,
        tables: &'w Tables,
        archetypes: &'w Archetypes,
        query_state: &'s QueryState<D, F>,
    ) -> Option<D::Item<'w, 's>> {
        if self.is_dense {
            // NOTE: if you are changing this branch you would probably have to change
            // QueryContiguousIter::next as well
            loop {
                // we are on the beginning of the query, or finished processing a table, so skip to the next
                if self.current_row == self.current_len {
                    let table_id = unsafe { self.storage_id_iter.next()?.table_id };
                    let table = unsafe { tables.get(table_id).debug_checked_unwrap() };
                    if table.is_empty() {
                        continue;
                    }
                    // SAFETY: `table` is from the world that `fetch/filter` were created for,
                    // `fetch_state`/`filter_state` are the states that `fetch/filter` were initialized with
                    unsafe {
                        D::set_table(&mut self.fetch, &query_state.fetch_state, table);
                        F::set_table(&mut self.filter, &query_state.filter_state, table);
                    }
                    self.table_entities = table.entities();
                    self.current_len = table.entity_count();
                    self.current_row = 0;
                }

                // SAFETY: set_table was called prior.
                // `current_row` is a table row in range of the current table, because if it was not, then the above would have been executed.
                let entity =
                    unsafe { self.table_entities.get_unchecked(self.current_row as usize) };
                // SAFETY: The row is less than the u32 len, so it must not be max.
                let row = unsafe { TableRow::new(NonMaxU32::new_unchecked(self.current_row)) };
                self.current_row += 1;

                if unsafe {
                    !F::filter_fetch(&query_state.filter_state, &mut self.filter, *entity, row)
                } {
                    continue;
                }

                // SAFETY:
                // - set_table was called prior.
                // - `current_row` must be a table row in range of the current table,
                //   because if it was not, then the above would have been executed.
                // - fetch is only called once for each `entity`.
                // - caller ensures no conflicting `Item`s are alive
                let item =
                    unsafe { D::fetch(&query_state.fetch_state, &mut self.fetch, *entity, row) };
                if let Some(item) = item {
                    return Some(item);
                }
            }
        } else {
            loop {
                if self.current_row == self.current_len {
                    let archetype_id = unsafe { self.storage_id_iter.next()?.archetype_id };
                    let archetype = unsafe { archetypes.get(archetype_id).debug_checked_unwrap() };
                    if archetype.is_empty() {
                        continue;
                    }
                    let table = unsafe { tables.get(archetype.table_id()).debug_checked_unwrap() };
                    // SAFETY: `archetype` and `tables` are from the world that `fetch/filter` were created for,
                    // `fetch_state`/`filter_state` are the states that `fetch/filter` were initialized with
                    unsafe {
                        D::set_archetype(
                            &mut self.fetch,
                            &query_state.fetch_state,
                            archetype,
                            table,
                        );
                        F::set_archetype(
                            &mut self.filter,
                            &query_state.filter_state,
                            archetype,
                            table,
                        );
                    }
                    self.archetype_entities = archetype.entities();
                    self.current_len = archetype.len();
                    self.current_row = 0;
                }

                // SAFETY: set_archetype was called prior.
                // `current_row` is an archetype index row in range of the current archetype, because if it was not, then the if above would have been executed.
                let archetype_entity = unsafe {
                    self.archetype_entities
                        .get_unchecked(self.current_row as usize)
                };
                self.current_row += 1;

                if unsafe {
                    !F::filter_fetch(
                        &query_state.filter_state,
                        &mut self.filter,
                        archetype_entity.id(),
                        archetype_entity.table_row(),
                    )
                } {
                    continue;
                }

                // SAFETY:
                // - set_archetype was called prior.
                // - `current_row` must be an archetype index row in range of the current archetype,
                //   because if it was not, then the if above would have been executed.
                // - fetch is only called once for each `archetype_entity`.
                // - caller ensures no conflicting `Item`s are alive
                let item = unsafe {
                    D::fetch(
                        &query_state.fetch_state,
                        &mut self.fetch,
                        archetype_entity.id(),
                        archetype_entity.table_row(),
                    )
                };
                if let Some(item) = item {
                    return Some(item);
                }
            }
        }
    }
}

/// A wrapper struct that gives its data a neutral ordering.
#[derive(Copy, Clone)]
struct NeutralOrd<T>(T);

impl<T> PartialEq for NeutralOrd<T> {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl<T> Eq for NeutralOrd<T> {}

#[expect(
    clippy::non_canonical_partial_ord_impl,
    reason = "`PartialOrd` and `Ord` on this struct must only ever return `Ordering::Equal`, so we prefer clarity"
)]
impl<T> PartialOrd for NeutralOrd<T> {
    fn partial_cmp(&self, _other: &Self) -> Option<Ordering> {
        Some(Ordering::Equal)
    }
}

impl<T> Ord for NeutralOrd<T> {
    fn cmp(&self, _other: &Self) -> Ordering {
        Ordering::Equal
    }
}
