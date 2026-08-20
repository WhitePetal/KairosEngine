use std::fmt;

use fixedbitset::FixedBitSet;
#[cfg(feature = "trace")]
use tracing::Span;

use crate::ecs::{
    archetype::{Archetype, ArchetypeGeneration, ArchetypeId},
    component::ComponentId,
    entity_disabling::DefaultQueryFilters,
    query::{FilteredAccess, NopWorldQuery, QueryData, QueryFilter, ReadOnlyQueryData, WorldQuery},
    storage::TableId,
    world::{FromWorld, World, WorldId, unsafe_world_cell::UnsafeWorldCell},
};

/// An ID for either a table or an archetype. Used for Query iteration.
///
/// Query iteration is exclusively dense (over tables) or archetypal (over archetypes) based on whether
/// the query filters are dense or not. This is represented by the [`QueryState::is_dense`] field.
///
/// Note that `D::IS_DENSE` and `F::IS_DENSE` have no relationship with `QueryState::is_dense` and
/// any combination of their values can happen.
///
/// This is a union instead of an enum as the usage is determined at compile time, as all [`StorageId`]s for
/// a [`QueryState`] will be all [`TableId`]s or all [`ArchetypeId`]s, and not a mixture of both. This
/// removes the need for discriminator to minimize memory usage and branching during iteration, but requires
/// a safety invariant be verified when disambiguating them.
///
/// # Safety
/// Must be initialized and accessed as a [`TableId`], if both generic parameters to the query are dense.
/// Must be initialized and accessed as an [`ArchetypeId`] otherwise.
#[derive(Clone, Copy)]
pub(super) union StorageId {
    pub(super) table_id: TableId,
    pub(super) archetype_id: ArchetypeId,
}

/// Provides scoped access to a [`World`] state according to a given [`QueryData`] and [`QueryFilter`].
///
/// This data is cached between system runs, and is used to:
/// - store metadata about which [`Table`] or [`Archetype`] are matched by the query. "Matched" means
///   that the query will iterate over the data in the matched table/archetype.
/// - cache the [`State`] needed to compute the [`Fetch`] struct used to retrieve data
///   from a specific [`Table`] or [`Archetype`]
/// - build iterators that can iterate over the query results
///
/// [`State`]: crate::query::world_query::WorldQuery::State
/// [`Fetch`]: crate::query::world_query::WorldQuery::Fetch
/// [`Table`]: crate::storage::Table
///
/// # Safety
///
/// If the query is not read-only,
/// then before calling any other methods on a new `QueryState`
/// other than [`QueryState::update_archetypes`], [`QueryState::update_archetypes_unsafe_world_cell`],
/// [`Self::init_access`] must be called.
#[repr(C)]
// SAFETY NOTE:
// Do not add any new fields that use the `D` or `F` generic parameters as this may
// make `QueryState::as_transmuted_state` unsound if not done with care.
pub struct QueryState<D: QueryData, F: QueryFilter = ()> {
    world_id: WorldId,
    pub(crate) archetype_generation: ArchetypeGeneration,
    /// Metadata about the [`Table`](crate::storage::Table)s matched by this query.
    pub(crate) matched_tables: FixedBitSet,
    /// Metadata about the [`Archetype`]s matched by this query.
    pub(crate) matched_archetypes: FixedBitSet,
    /// [`FilteredAccess`] computed by combining the `D` and `F` access. Used to check which other queries
    /// this query can run in parallel with.
    /// Note that because we do a zero-cost reference conversion in `Query::as_readonly`,
    /// the access for a read-only query may include accesses for the original mutable version,
    /// but the `Query` does not have exclusive access to those components.
    pub(crate) component_access: FilteredAccess,
    // NOTE: we maintain both a bitset and a vec because iterating the vec is faster
    pub(super) matched_storage_ids: Vec<StorageId>,
    // Represents whether this query iteration is dense or not. When this is true
    // `matched_storage_ids` stores `TableId`s, otherwise it stores `ArchetypeId`s.
    pub(super) is_dense: bool,
    pub(crate) fetch_state: D::State,
    pub(crate) filter_state: F::State,
    #[cfg(feature = "trace")]
    par_iter_span: Span,
}

impl<D: QueryData, F: QueryFilter> fmt::Debug for QueryState<D, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QueryState")
            .field("world_id", &self.world_id)
            .field("matched_table_count", &self.matched_tables.count_ones(..))
            .field(
                "matched_archetype_count",
                &self.matched_archetypes.count_ones(..),
            )
            .finish_non_exhaustive()
    }
}

impl<D: QueryData, F: QueryFilter> FromWorld for QueryState<D, F> {
    fn from_world(world: &mut World) -> Self {
        world.query_filtered()
    }
}

impl<D: QueryData, F: QueryFilter> QueryState<D, F> {
    /// Converts this `QueryState` reference to a `QueryState` that does not access anything mutably.
    pub fn as_readonly(&self) -> &QueryState<D::ReadOnly, F> {
        // SAFETY: invariant on `WorldQuery` trait upholds that `D::ReadOnly` and `F::ReadOnly`
        // have a subset of the access, and match the exact same archetypes/tables as `D`/`F` respectively.
        unsafe { self.as_transmuted_state::<D::ReadOnly, F>() }
    }

    /// Converts this `QueryState` reference to a `QueryState` that does not return any data
    /// which can be faster.
    ///
    /// This doesn't use `NopWorldQuery` as it loses filter functionality, for example
    /// `NopWorldQuery<Changed<T>>` is functionally equivalent to `With<T>`.
    pub(crate) fn as_nop(&self) -> &QueryState<NopWorldQuery<D>, F> {
        // SAFETY: `NopWorldQuery` doesn't have any accesses and defers to
        // `D` for table/archetype matching
        unsafe { self.as_transmuted_state::<NopWorldQuery<D>, F>() }
    }

    /// Converts this `QueryState` reference to any other `QueryState` with
    /// the same `WorldQuery::State` associated types.
    ///
    /// Consider using `as_readonly` or `as_nop` instead which are safe functions.
    ///
    /// # Safety
    ///
    /// `NewD` must have a subset of the access that `D` does and match the exact same archetypes/tables
    /// `NewF` must have a subset of the access that `F` does and match the exact same archetypes/tables
    pub(crate) unsafe fn as_transmuted_state<
        NewD: ReadOnlyQueryData<State = D::State>,
        NewF: QueryFilter<State = F::State>,
    >(
        &self,
    ) -> &QueryState<NewD, NewF> {
        unsafe { &*std::ptr::from_ref(self).cast::<QueryState<NewD, NewF>>() }
    }

    /// Returns the components accessed by this query.
    pub fn component_access(&self) -> &FilteredAccess {
        &self.component_access
    }

    /// Returns the tables matched by this query.
    pub fn matched_tables(&self) -> impl Iterator<Item = TableId> + '_ {
        self.matched_tables.ones().map(TableId::from_usize)
    }

    /// Returns the archetypes matched by this query.
    pub fn mathced_archetypes(&self) -> impl Iterator<Item = ArchetypeId> + '_ {
        self.matched_archetypes.ones().map(ArchetypeId::new)
    }

    /// Creates a new [`QueryState`] from a given [`World`] and inherits the result of `world.id()`.
    ///
    /// Unlike [`QueryState::new`], this does not check access of nested queries,
    /// so [`Self::init_access`] must be called before querying using this state or returning it to safe code.
    ///
    /// # Safety
    ///
    /// If the query is not read-only,
    /// then before calling any other methods on the returned `QueryState`
    /// other than [`QueryState::update_archetypes`], [`QueryState::update_archetypes_unsafe_world_cell`],
    /// [`Self::init_access`] must be called.
    pub unsafe fn new_unchecked(world: &mut World) -> Self {
        let fetch_state = D::init_state(world);
        let filter_state = F::init_state(world);

        let mut state =
            unsafe { Self::from_states_uninitialized(world, fetch_state, filter_state) };
        state.update_archetypes(world);
        state
    }

    /// Creates a new [`QueryState`] but does not populate it with the matched results from the World yet
    ///
    /// `new_archetype` and its variants must be called on all of the World's archetypes before the
    /// state can return valid query results.
    ///
    /// # Safety
    ///
    /// If the query is not read-only,
    /// then before calling any other methods on the returned `QueryState`
    /// other than [`QueryState::update_archetypes`], [`QueryState::update_archetypes_unsafe_world_cell`],
    /// [`Self::init_access`] must be called.
    unsafe fn from_states_uninitialized(
        world: &World,
        fetch_state: <D as WorldQuery>::State,
        filter_state: <F as WorldQuery>::State,
    ) -> Self {
        let mut component_access = FilteredAccess::default();
        D::update_component_access(&fetch_state, &mut component_access);

        // Use a temporary empty FilteredAccess for filters. This prevents them from conflicting with the
        // main Query's `fetch_state` access. Filters are allowed to conflict with the main query fetch
        // because they are evaluated *before* a specific reference is constructed.
        let mut filter_component_access = FilteredAccess::default();
        F::update_component_access(&filter_state, &mut filter_component_access);

        // Merge the temporary filter access with the main access. This ensures that filter access is
        // properly considered in a global "cross-query" context (both within systems and across systems).
        component_access.extend(&filter_component_access);

        // For queries without dynamic filters the dense-ness of the query is equal to the dense-ness
        // of its static type parameters.
        let mut is_dense = D::IS_DENSE && F::IS_DENSE;

        if let Some(default_filters) = world.get_resource::<DefaultQueryFilters>() {
            default_filters.modify_access(&mut component_access);
            is_dense &= default_filters.is_dense(world.components());
        }

        Self {
            world_id: world.id(),
            archetype_generation: ArchetypeGeneration::initial(),
            matched_storage_ids: Vec::new(),
            is_dense,
            fetch_state,
            filter_state,
            component_access,
            matched_tables: Default::default(),
            matched_archetypes: Default::default(),
            #[cfg(feature = "trace")]
            par_iter_span: tracing::info_span!(
                "par_fro_each",
                query = std::any::type_name::<D>(),
                filter = std::any::type_name::<F>()
            ),
        }
    }

    /// Updates the state's internal view of the [`World`]'s archetypes. If this is not called before querying data,
    /// the results may not accurately reflect what is in the `world`.
    ///
    /// This is only required if a `manual` method (such as [`Self::get_manual`]) is being called, and it only needs to
    /// be called if the `world` has been structurally mutated (i.e. added/removed a component or resource). Users using
    /// non-`manual` methods such as [`QueryState::get`] do not need to call this as it will be automatically called for them.
    ///
    /// If you have an [`UnsafeWorldCell`] instead of `&World`, consider using [`QueryState::update_archetypes_unsafe_world_cell`].
    ///
    /// # Panics
    ///
    /// If `world` does not match the one used to call `QueryState::new` for this instance.
    #[inline]
    pub fn update_archetypes(&mut self, world: &World) {
        self.update_archetypes_unsafe_world_cell(world.as_unsafe_world_cell_readonly());
    }

    /// Updates the state's internal view of the `world`'s archetypes. If this is not called before querying data,
    /// the results may not accurately reflect what is in the `world`.
    ///
    /// This is only required if a `manual` method (such as [`Self::get_manual`]) is being called, and it only needs to
    /// be called if the `world` has been structurally mutated (i.e. added/removed a component or resource). Users using
    /// non-`manual` methods such as [`QueryState::get`] do not need to call this as it will be automatically called for them.
    ///
    /// # Note
    ///
    /// This method only accesses world metadata.
    ///
    /// # Panics
    ///
    /// If `world` does not match the one used to call `QueryState::new` for this instance.
    pub fn update_archetypes_unsafe_world_cell(&mut self, world: UnsafeWorldCell) {
        self.validate_world(world.id());
        D::update_archetypes(&mut self.fetch_state, world);
        F::update_archetypes(&mut self.filter_state, world);
        if self.component_access.required.is_clear() {
            let archetypes = world.archetypes();
            let old_generation =
                std::mem::replace(&mut self.archetype_generation, archetypes.generation());

            for archetype in &archetypes[old_generation..] {
                // SAFETY: The validate_world call ensures that the world is the same the QueryState
                // was initialized from.
                unsafe { self.new_archetype(archetype) }
            }
        } else {
            // skip if we are already up to date
            if self.archetype_generation == world.archetypes().generation() {
                return;
            }

            let potential_archetypes = self
                .component_access
                .required
                .iter()
                .filter_map(|component_id| {
                    world
                        .archetypes()
                        .component_index()
                        .get(&component_id)
                        .map(|index| index.keys())
                })
                // select the component with the fewest archetypes
                .min_by_key(ExactSizeIterator::len);
            if let Some(archetypes) = potential_archetypes {
                for archetype_id in archetypes {
                    if archetype_id < &self.archetype_generation.0 {
                        continue;
                    }
                    // SAFETY: get_potential_archetypes only returns archetype ids that are valid for the world
                    let archetype = &world.archetypes()[*archetype_id];
                    // SAFETY: The validate_world call ensures that the world is the same the QueryState
                    // was initialized from.
                    unsafe {
                        self.new_archetype(archetype);
                    }
                }
            }
            self.archetype_generation = world.archetypes().generation()
        }
    }

    /// # Panics
    ///
    /// If `world_id` does not match the [`World`] used to call `QueryState::new` for this instance.
    ///
    /// Many unsafe query methods require the world to match for soundness. This function is the easiest
    /// way of ensuring that it matches.
    #[inline]
    #[track_caller]
    pub fn validate_world(&self, world_id: WorldId) {
        #[inline(never)]
        #[track_caller]
        #[cold]
        fn panic_mismatched(this: WorldId, other: WorldId) -> ! {
            panic!(
                "Encountered a mismatched World. This QueryState was created from {this:?}, but a method was called using {other:?}."
            )
        }

        if self.world_id != world_id {
            panic_mismatched(self.world_id, world_id)
        }
    }

    /// Update the current [`QueryState`] with information from the provided [`Archetype`]
    /// (if applicable, i.e. if the archetype has any intersecting [`ComponentId`] with the current [`QueryState`]).
    ///
    /// # Safety
    /// `archetype` must be from the `World` this state was initialized from.
    pub unsafe fn new_archetype(&mut self, archetype: &Archetype) {
        if D::matches_component_set(&self.fetch_state, &|id| archetype.contains(id))
            && F::matches_component_set(&self.filter_state, &|id| archetype.contains(id))
            && self.matches_component_set(&|id| archetype.contains(id))
        {
            let archetype_index = archetype.id().index();
            if !self.matched_archetypes.contains(archetype_index) {
                self.matched_archetypes.grow_and_insert(archetype_index);
                if !self.is_dense {
                    self.matched_storage_ids.push(StorageId {
                        archetype_id: archetype.id(),
                    });
                }
            }
            let table_index = archetype.table_id().as_usize();
            if !self.matched_tables.contains(table_index) {
                self.matched_tables.grow_and_insert(table_index);
                if self.is_dense {
                    self.matched_storage_ids.push(StorageId {
                        table_id: archetype.table_id(),
                    });
                }
            }
        }
    }

    /// Returns `true` if this query matches a set of components. Otherwise, returns `false`.
    pub fn matches_component_set(&self, set_contains_id: &impl Fn(ComponentId) -> bool) -> bool {
        self.component_access.filter_sets.iter().any(|set| {
            set.with.iter().all(set_contains_id)
                && set.without.iter().all(|index| !set_contains_id(index))
        })
    }
}

// TODO!
