use crate::ecs::{archetype::{ArchetypeEntity, Archetypes}, change_detection::Tick, entity::Entity, query::{IterQueryData, QueryData, QueryFilter, QueryState, StorageId}, storage::Tables, world::unsafe_world_cell::UnsafeWorldCell};


/// An [`Iterator`] over query results of a [`Query`](crate::system::Query).
///
/// This struct is created by the [`Query::iter`](crate::system::Query::iter) and
/// [`Query::iter_mut`](crate::system::Query::iter_mut) methods.
pub struct QueryIter<'w, 's, D: QueryData, F: QueryFilter> {
    world: UnsafeWorldCell<'w>,
    tables: &'w Tables,
    archetypes: &'w Archetypes,
    query_state: &'s QueryState<D, F>,
    cursor: QueryIterationCursor<'w, 's, D, F>
}

impl<'w, 's, D: QueryData, F: QueryFilter> QueryIter<'w, 's, D, F> {
    pub(crate) unsafe fn new(
        world: UnsafeWorldCell<'w>,
        query_state: &'s QueryState<D, F>,
        last_run: Tick,
        this_run: Tick
    ) -> Self {
        QueryIter {
            world,
            query_state,
            // SAFETY: We only access table data that has been registered in `query_state`.
            tables: unsafe {
                &world.storages().tables
            },
            archetypes: world.archetypes(),
            // SAFETY: The invariants are upheld by the caller.
            cursor: unsafe {
                QueryIterationCursor::init(world, query_state, last_run, this_run)
            }
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
    current_row: u32
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
        this_run: Tick
    ) -> Self {
        let fetch = unsafe {
            D::init_fetch(world, &query_state.fetch_state, last_run, this_run)
        };
        let filter = unsafe {
            F::init_fetch(world, &query_state.filter_state, last_run, this_run)
        };
        QueryIterationCursor {
            fetch,
            filter,
            table_entities: &[],
            archetype_entities: &[],
            storage_id_iter: query_state.matched_storage_ids.iter(),
            is_dense: query_state.is_dense,
            current_len: 0,
            current_row: 0
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
}
