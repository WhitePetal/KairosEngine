use std::fmt;

use fixedbitset::FixedBitSet;
use log::warn;
#[cfg(feature = "trace")]
use tracing::Span;

use crate::{
    debug::DebugName,
    ecs::{
        archetype::{Archetype, ArchetypeGeneration, ArchetypeId},
        change_detection::Tick,
        component::ComponentId,
        entity::{Entity, UniqueEntityArray},
        entity_disabling::DefaultQueryFilters,
        query::{
            FilteredAccess, FilteredAccessSet, IterQueryData, NopWorldQuery, QueryBuilder,
            QueryData, QueryEntityError, QueryFilter, QueryIter, ROQueryItem, ReadOnlyQueryData,
            SingleEntityQueryData, WorldQuery,
        },
        storage::TableId,
        system::Query,
        world::{FromWorld, World, WorldId, unsafe_world_cell::UnsafeWorldCell},
    },
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

    /// Adds all access from this query and any nested queries to the `component_access_set`.
    /// Panics if the access from this query and any nested queries conflict with each other
    /// or with any previous access.
    pub fn init_access(
        &self,
        system_name: Option<&str>,
        component_access_set: &mut FilteredAccessSet,
        world: UnsafeWorldCell,
    ) {
        let conflicts = component_access_set.get_conflicts_single(&self.component_access);
        if !conflicts.is_empty() {
            let mut accesses = conflicts.format_conflict_list(world);
            // Access list may be empty (if access to all components requested)
            if !accesses.is_empty() {
                accesses.push(' ');
            }
            let type_name = DebugName::type_name::<Query<D, F>>();
            let type_name = type_name.shortname();
            let system = system_name
                .map(|name| format!(" in system {name}"))
                .unwrap_or_default();
            panic!(
                "error[B0001]: {type_name}{system} access component(s) {accesses}in a way that conflicts with a previous system parameter. Consider using `Without<T>` to create disjoint Queries or merging conflicting Queries into a `ParamSet`. See: https://bevy.org/learn/errors/b0001"
            );
        }

        component_access_set.add(self.component_access.clone());
        D::init_nested_access(&self.fetch_state, system_name, component_access_set, world);
        F::init_nested_access(&self.filter_state, system_name, component_access_set, world);
    }

    /// Creates a new [`QueryState`] from a given [`World`] and inherits the result of `world.id()`.
    pub fn new(world: &mut World) -> Self {
        // SAFETY: We immediately call `init_access`
        let state = unsafe { Self::new_unchecked(world) };
        state.init_access(None, &mut FilteredAccessSet::new(), world.into());
        state
    }

    /// Creates a new [`QueryState`] from an immutable [`World`] reference and inherits the result of `world.id()`.
    ///
    /// This function may fail if, for example,
    /// the components that make up this query have not been registered into the world.
    pub fn try_new(world: &World) -> Option<Self> {
        let fetch_state = D::get_state(world.components())?;
        let filter_state = F::get_state(world.components())?;
        // SAFETY: We immediately call `init_access`
        let mut state =
            unsafe { Self::from_states_uninitialized(world, fetch_state, filter_state) };
        state.init_access(None, &mut FilteredAccessSet::new(), world.into());
        state.update_archetypes(world);
        Some(state)
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

    /// Creates a new [`QueryState`] from a given [`QueryBuilder`] and inherits its [`FilteredAccess`].
    pub fn from_builder(builder: &mut QueryBuilder<D, F>) -> Self {
        let mut fetch_state = D::init_state(builder.world_mut());
        let filter_state = F::init_state(builder.world_mut());

        let mut component_access = FilteredAccess::default();
        D::update_component_access(&fetch_state, &mut component_access);
        D::provide_extra_access(
            &mut fetch_state,
            component_access.access_mut(),
            builder.access().access(),
        );

        let mut component_access = builder.access().clone();

        // For dynamic queries the dense-ness is given by the query builder.
        let mut is_dense = builder.is_dense();

        if let Some(default_filters) = builder.world().get_resource::<DefaultQueryFilters>() {
            default_filters.modify_access(&mut component_access);
            is_dense &= default_filters.is_dense(builder.world().components());
        }

        // SAFETY: We immediately call `init_access`
        let mut state = Self {
            world_id: builder.world().id(),
            archetype_generation: ArchetypeGeneration::initial(),
            matched_storage_ids: Vec::new(),
            is_dense,
            fetch_state,
            filter_state,
            component_access,
            matched_tables: Default::default(),
            matched_archetypes: Default::default(),
            par_iter_span: tracing::info_span!(
                "par_for_each",
                data = std::any::type_name::<D>(),
                filter = std::any::type_name::<F>()
            ),
        };
        state.init_access(None, &mut FilteredAccessSet::new(), builder.world().into());
        state.update_archetypes(builder.world());
        state
    }

    /// Creates a [`Query`] from the given [`QueryState`] and [`World`].
    ///
    /// This will create read-only queries, see [`Self::query_mut`] for mutable queries.
    pub fn query<'w, 's>(&'s mut self, world: &'w World) -> Query<'w, 's, D::ReadOnly, F> {
        self.update_archetypes(world);
        self.query_manual(world)
    }

    /// Creates a [`Query`] from the given [`QueryState`] and [`World`].
    ///
    /// This method is slightly more efficient than [`QueryState::query`] in some situations, since
    /// it does not update this instance's internal cache. The resulting query may skip an entity that
    /// belongs to an archetype that has not been cached.
    ///
    /// To ensure that the cache is up to date, call [`QueryState::update_archetypes`] before this method.
    /// The cache is also updated in [`QueryState::new`], [`QueryState::get`], or any method with mutable
    /// access to `self`.
    ///
    /// This will create read-only queries, see [`Self::query_mut`] for mutable queries.
    pub fn query_manual<'w, 's>(&'s self, world: &'w World) -> Query<'w, 's, D::ReadOnly, F> {
        self.validate_world(world.id());
        // SAFETY:
        // - We have read access to the entire world, and we call `as_readonly()` so the query only performs read access.
        // - We called `validate_world`.
        unsafe {
            self.as_readonly()
                .query_unchecked_manual(world.as_unsafe_world_cell_readonly())
        }
    }

    /// Creates a [`Query`] from the given [`QueryState`] and [`World`].
    pub fn query_mut<'w, 's>(&'s mut self, world: &'w mut World) -> Query<'w, 's, D, F> {
        let last_run = world.last_change_tick();
        let this_run = world.change_tick();
        // SAFETY: We have exclusive access to the entire world.
        unsafe { self.query_unchecked_with_ticks(world.as_unsafe_world_cell(), last_run, this_run) }
    }

    /// Creates a [`Query`] from the given [`QueryState`] and [`World`].
    ///
    /// # Safety
    ///
    /// This does not check for mutable query correctness. To be safe, make sure mutable queries
    /// have unique access to the components they query.
    pub unsafe fn query_unchecked<'w, 's>(
        &'s mut self,
        world: UnsafeWorldCell<'w>,
    ) -> Query<'w, 's, D, F> {
        self.update_archetypes_unsafe_world_cell(world);
        // SAFETY: Caller ensures we have the required access
        unsafe { self.query_unchecked_manual(world) }
    }

    /// Creates a [`Query`] from the given [`QueryState`] and [`World`].
    ///
    /// This method is slightly more efficient than [`QueryState::query_unchecked`] in some situations, since
    /// it does not update this instance's internal cache. The resulting query may skip an entity that
    /// belongs to an archetype that has not been cached.
    ///
    /// To ensure that the cache is up to date, call [`QueryState::update_archetypes`] before this method.
    /// The cache is also updated in [`QueryState::new`], [`QueryState::get`], or any method with mutable
    /// access to `self`.
    ///
    /// # Safety
    ///
    /// This does not check for mutable query correctness. To be safe, make sure mutable queries
    /// have unique access to the components they query.
    /// This does not validate that `world.id()` matches `self.world_id`. Calling this on a `world`
    /// with a mismatched [`WorldId`] is unsound.
    pub unsafe fn query_unchecked_manual<'w, 's>(
        &'s self,
        world: UnsafeWorldCell<'w>,
    ) -> Query<'w, 's, D, F> {
        let last_run = world.last_change_tick();
        let this_run = world.change_tick();
        // SAFETY:
        // - The caller ensured we have the correct access to the world.
        // - The caller ensured that the world matches.
        unsafe { self.query_unchecked_manual_with_ticks(world, last_run, this_run) }
    }

    /// Creates a [`Query`] from the given [`QueryState`] and [`World`].
    ///
    /// # Safety
    ///
    /// This does not check for mutable query correctness. To be safe, make sure mutable queries
    /// have unique access to the components they query.
    pub unsafe fn query_unchecked_with_ticks<'w, 's>(
        &'s mut self,
        world: UnsafeWorldCell<'w>,
        last_run: Tick,
        this_run: Tick,
    ) -> Query<'w, 's, D, F> {
        self.update_archetypes_unsafe_world_cell(world);
        // SAFETY:
        // - The caller ensured we have the correct access to the world.
        // - We called `update_archetypes_unsafe_world_cell`, which calls `validate_world`.
        unsafe { self.query_unchecked_manual_with_ticks(world, last_run, this_run) }
    }

    /// Creates a [`Query`] from the given [`QueryState`] and [`World`].
    ///
    /// This method is slightly more efficient than [`QueryState::query_unchecked_with_ticks`] in some situations, since
    /// it does not update this instance's internal cache. The resulting query may skip an entity that
    /// belongs to an archetype that has not been cached.
    ///
    /// To ensure that the cache is up to date, call [`QueryState::update_archetypes`] before this method.
    /// The cache is also updated in [`QueryState::new`], [`QueryState::get`], or any method with mutable
    /// access to `self`.
    ///
    /// # Safety
    ///
    /// This does not check for mutable query correctness. To be safe, make sure mutable queries
    /// have unique access to the components they query.
    /// This does not validate that `world.id()` matches `self.world_id`. Calling this on a `world`
    /// with a mismatched [`WorldId`] is unsound.
    pub unsafe fn query_unchecked_manual_with_ticks<'w, 's>(
        &'s self,
        world: UnsafeWorldCell<'w>,
        last_run: Tick,
        this_run: Tick,
    ) -> Query<'w, 's, D, F> {
        // SAFETY:
        // - The caller ensured we have the correct access to the world.
        // - The caller ensured that the world matches.
        unsafe { Query::new(world, self, last_run, this_run) }
    }

    /// Checks if the query is empty for the given [`World`], where the last change and current tick are given.
    ///
    /// This is equivalent to `self.iter().next().is_none()`, and thus the worst case runtime will be `O(n)`
    /// where `n` is the number of *potential* matches. This can be notably expensive for queries that rely
    /// on non-archetypal filters such as [`Added`], [`Changed`] or [`Spawned`] which must individually check
    /// each query result for a match.
    ///
    /// # Panics
    ///
    /// If `world` does not match the one used to call `QueryState::new` for this instance.
    ///
    /// [`Added`]: crate::query::Added
    /// [`Changed`]: crate::query::Changed
    /// [`Spawned`]: crate::query::Spawned
    #[inline]
    pub fn is_empty(&self, world: &World, last_run: Tick, this_run: Tick) -> bool {
        self.validate_world(world.id());
        // SAFETY:
        // - We have read access to the entire world, and `is_empty()` only performs read access.
        // - We called `validate_world`.
        unsafe {
            self.query_unchecked_manual_with_ticks(
                world.as_unsafe_world_cell_readonly(),
                last_run,
                this_run,
            )
        }
        .is_empty()
    }

    /// Returns `true` if the given [`Entity`] matches the query.
    ///
    /// This is always guaranteed to run in `O(1)` time.
    #[inline]
    pub fn contains(&self, entity: Entity, world: &World, last_run: Tick, this_run: Tick) -> bool {
        self.validate_world(world.id());
        // SAFETY:
        // - We have read access to the entire world, and `is_empty()` only performs read access.
        // - We called `validate_world`.
        unsafe {
            self.query_unchecked_manual_with_ticks(
                world.as_unsafe_world_cell_readonly(),
                last_run,
                this_run,
            )
        }
        .contains(entity)
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

    /// Use this to transform a [`QueryState`] into a more generic [`QueryState`].
    /// This can be useful for passing to another function that might take the more general form.
    /// See [`Query::transmute_lens`](crate::system::Query::transmute_lens) for more details.
    ///
    /// You should not call [`update_archetypes`](Self::update_archetypes) on the returned [`QueryState`] as the result will be unpredictable.
    /// You might end up with a mix of archetypes that only matched the original query + archetypes that only match
    /// the new [`QueryState`]. Most of the safe methods on [`QueryState`] call [`QueryState::update_archetypes`] internally, so this
    /// best used through a [`Query`]
    pub fn transmute<'a, NewD: SingleEntityQueryData>(
        &self,
        world: impl Into<UnsafeWorldCell<'a>>,
    ) -> QueryState<NewD> {
        self.transmute_filtered::<NewD, ()>(world.into())
    }

    /// Creates a new [`QueryState`] with the same underlying [`FilteredAccess`], matched tables and archetypes
    /// as self but with a new type signature.
    ///
    /// Panics if `NewD` or `NewF` require accesses that this query does not have.
    pub fn transmute_filtered<'a, NewD: SingleEntityQueryData, NewF: QueryFilter>(
        &self,
        world: impl Into<UnsafeWorldCell<'a>>,
    ) -> QueryState<NewD, NewF> {
        let world = world.into();
        self.validate_world(world.id());

        let mut component_access = FilteredAccess::default();
        let mut fetch_state = NewD::get_state(world.components()).expect("Could not create fetch_state, Please initialize all referenced components before transmuting.");
        let filter_state = NewF::get_state(world.components()).expect("Could not create filter_state, Please initialize all referenced components before transmuting.");

        let mut self_access = self.component_access.clone();
        if D::IS_READ_ONLY {
            // The current state was transmuted from a mutable
            // `QueryData` to a read-only one.
            // Ignore any write access in the current state.
            self_access.access_mut().clear_writes();
        }

        NewD::update_component_access(&fetch_state, &mut component_access);
        NewD::provide_extra_access(
            &mut fetch_state,
            component_access.access_mut(),
            self_access.access(),
        );

        let mut filter_component_access = FilteredAccess::default();
        NewF::update_component_access(&filter_state, &mut filter_component_access);

        component_access.extend(&filter_component_access);
        assert!(
            component_access.is_subset(&self_access),
            "Transmuted state for {} attempts to access terms that are not allowed by original state {}.",
            DebugName::type_name::<(NewD, NewF)>(),
            DebugName::type_name::<(D, F)>()
        );

        // For transmuted queries, the dense-ness of the query is equal to the dense-ness of the original query.
        //
        // We ensure soundness using `FilteredAccess::required`.
        //
        // Any `WorldQuery` implementations that rely on a query being sparse for soundness,
        // including `&`, `&mut`, `Ref`, and `Mut`, will add a sparse set component to the `required` set.
        // (`Option<&Sparse>` and `Has<Sparse>` will incorrectly report a component as never being present
        // when doing dense iteration, but are not unsound.  See https://github.com/bevyengine/bevy/issues/16397)
        //
        // And any query with a sparse set component in the `required` set must have `is_dense = false`.
        // For static queries, the `WorldQuery` implementations ensure this.
        // For dynamic queries, anything that adds a `required` component also adds a `with` filter.
        //
        // The `component_access.is_subset()` check ensures that if the new query has a sparse set component in the `required` set,
        // then the original query must also have had that component in the `required` set.
        // Therefore, if the `WorldQuery` implementations rely on a query being sparse for soundness,
        // then there was a sparse set component in the `required` set, and the query has `is_dense = false`.
        let is_dense = self.is_dense;

        // SAFETY: `D: SingleEntityQueryData`, so we do not need to call `init_access`
        QueryState {
            world_id: self.world_id,
            archetype_generation: self.archetype_generation,
            matched_storage_ids: self.matched_storage_ids.clone(),
            is_dense,
            fetch_state,
            filter_state,
            component_access: self_access,
            matched_tables: self.matched_tables.clone(),
            matched_archetypes: self.matched_archetypes.clone(),
            #[cfg(feature = "trace")]
            par_iter_span: tracing::info_span!(
                "par_for_each",
                query = std::any::type_name::<NewD>(),
                filter = std::any::type_name::<NewF>(),
            ),
        }
    }

    /// Use this to combine two queries. The data accessed will be the intersection
    /// of archetypes included in both queries. This can be useful for accessing a
    /// subset of the entities between two queries.
    ///
    /// You should not call `update_archetypes` on the returned `QueryState` as the result
    /// could be unpredictable. You might end up with a mix of archetypes that only matched
    /// the original query + archetypes that only match the new `QueryState`. Most of the
    /// safe methods on `QueryState` call [`QueryState::update_archetypes`] internally, so
    /// this is best used through a `Query`.
    ///
    /// ## Performance
    ///
    /// This will have similar performance as constructing a new `QueryState` since much of internal state
    /// needs to be reconstructed. But it will be a little faster as it only needs to compare the intersection
    /// of matching archetypes rather than iterating over all archetypes.
    ///
    /// ## Panics
    ///
    /// Will panic if `NewD` contains accesses not in `Q` or `OtherQ`.
    pub fn join<'a, OtherD: QueryData, NewD: SingleEntityQueryData>(
        &self,
        world: impl Into<UnsafeWorldCell<'a>>,
        other: &QueryState<OtherD>,
    ) -> QueryState<NewD, ()> {
        self.join_filtered::<_, (), NewD, ()>(world, other)
    }

    /// Use this to combine two queries. The data accessed will be the intersection
    /// of archetypes included in both queries.
    ///
    /// ## Panics
    ///
    /// Will panic if `NewD` or `NewF` requires accesses not in `Q` or `OtherQ`.
    pub fn join_filtered<
        'a,
        OtherD: QueryData,
        OtherF: QueryFilter,
        NewD: SingleEntityQueryData,
        NewF: QueryFilter,
    >(
        &self,
        world: impl Into<UnsafeWorldCell<'a>>,
        other: &QueryState<OtherD, OtherF>,
    ) -> QueryState<NewD, NewF> {
        if self.world_id != other.world_id {
            panic!("Joining queries initialized on different worlds is not allowed.");
        }

        let world = world.into();

        self.validate_world(world.id());

        let mut component_access = FilteredAccess::default();
        let mut new_fetch_state = NewD::get_state(world.components())
            .expect("Could not create fetch_state, Please initialize all referenced components before transmuting.");
        let mut new_filter_state = NewF::get_state(world.components())
            .expect("Could not create filter_state, Please initialize all referenced components before transmuting.");

        let mut joined_component_access = self.component_access.clone();
        joined_component_access.extend(&other.component_access);

        if D::IS_READ_ONLY && self.component_access.access().has_any_write()
            || OtherD::IS_READ_ONLY && other.component_access.access().has_any_write()
        {
            // One of the input states was transmuted from a mutable
            // `QueryData` to a read-only one.
            // Ignore any write access in that current state.
            // The simplest way to do this is to clear *all* writes
            // and then add back in any writes that are valid
            joined_component_access.access_mut().clear_writes();
            if !D::IS_READ_ONLY {
                joined_component_access
                    .access_mut()
                    .extend(self.component_access.access());
            }
            if !OtherD::IS_READ_ONLY {
                joined_component_access
                    .access_mut()
                    .extend(other.component_access.access());
            }
        }

        NewD::update_component_access(&new_fetch_state, &mut component_access);
        NewD::provide_extra_access(
            &mut new_fetch_state,
            component_access.access_mut(),
            joined_component_access.access(),
        );

        let mut new_filter_component_access = FilteredAccess::default();
        NewF::update_component_access(&new_filter_state, &mut new_filter_component_access);

        component_access.extend(&new_filter_component_access);
        assert!(
            component_access.is_subset(&joined_component_access),
            "Joined state for {} attempts to access terms that are not allowed by state {} joined with {}.",
            DebugName::type_name::<(NewD, NewF)>(),
            DebugName::type_name::<(D, F)>(),
            DebugName::type_name::<(OtherD, OtherF)>()
        );

        if self.archetype_generation != other.archetype_generation {
            warn!(
                "You have tried to join queries with different archetype_generations. This could lead to unpredictable results."
            );
        }

        // the join is dense of both the queries were dense.
        let is_dense = self.is_dense && other.is_dense;

        // take the intersection of the matched ids
        let mut matched_tables = self.matched_tables.clone();
        let mut matched_archetypes = self.matched_archetypes.clone();
        matched_tables.intersect_with(&other.matched_tables);
        matched_archetypes.intersect_with(&other.matched_archetypes);
        let matched_storage_ids = if is_dense {
            matched_tables
                .ones()
                .map(|id| StorageId {
                    table_id: TableId::from_usize(id),
                })
                .collect()
        } else {
            matched_archetypes
                .ones()
                .map(|id| StorageId {
                    archetype_id: ArchetypeId::new(id),
                })
                .collect()
        };

        // SAFETY: `D: SingleEntityQueryData`, so we do not need to call `init_access`
        QueryState {
            world_id: self.world_id,
            archetype_generation: self.archetype_generation,
            matched_storage_ids,
            is_dense,
            fetch_state: new_fetch_state,
            filter_state: new_filter_state,
            component_access: joined_component_access,
            matched_tables,
            matched_archetypes,
            #[cfg(feature = "trace")]
            par_iter_span: tracing::info_span!(
                "par_for_each",
                query = std::any::type_name::<NewD>(),
                filter = std::any::type_name::<NewF>()
            ),
        }
    }

    /// Gets the query result for the given [`World`] and [`Entity`].
    ///
    /// This can only be called for read-only queries, see [`Self::get_mut`] for write-queries.
    ///
    /// If you need to get multiple items at once but get borrowing errors,
    /// consider using [`Self::update_archetypes`] followed by multiple [`Self::get_manual`] calls,
    /// or making a single call with [`Self::get_many`]  or [`Self::iter_many`].
    ///
    /// This is always guaranteed to run in `O(1)` time.
    #[inline]
    pub fn get<'w>(
        &mut self,
        world: &'w World,
        entity: Entity,
    ) -> Result<ROQueryItem<'w, '_, D>, QueryEntityError> {
        self.query(world).get_inner(entity)
    }

    /// Returns the read-only query results for the given array of [`Entity`].
    ///
    /// In case of a nonexisting entity or mismatched component, a [`QueryEntityError`] is
    /// returned instead.
    ///
    /// Note that the unlike [`QueryState::get_many_mut`], the entities passed in do not need to be unique.
    ///
    /// # Examples
    ///
    /// ```
    /// use bevy_ecs::prelude::*;
    /// use bevy_ecs::query::QueryEntityError;
    ///
    /// #[derive(Component, PartialEq, Debug)]
    /// struct A(usize);
    ///
    /// let mut world = World::new();
    /// let entity_vec: Vec<Entity> = (0..3).map(|i|world.spawn(A(i)).id()).collect();
    /// let entities: [Entity; 3] = entity_vec.try_into().unwrap();
    ///
    /// world.spawn(A(73));
    ///
    /// let mut query_state = world.query::<&A>();
    ///
    /// let component_values = query_state.get_many(&world, entities).unwrap();
    ///
    /// assert_eq!(component_values, [&A(0), &A(1), &A(2)]);
    ///
    /// let wrong_entity = Entity::from_raw_u32(365).unwrap();
    ///
    /// assert_eq!(match query_state.get_many(&mut world, [wrong_entity]).unwrap_err() {QueryEntityError::NotSpawned(error) => error.entity(), _ => panic!()}, wrong_entity);
    /// ```
    #[inline]
    pub fn get_many<'w, const N: usize>(
        &mut self,
        world: &'w World,
        entities: [Entity; N],
    ) -> Result<[ROQueryItem<'w, '_, D>; N], QueryEntityError> {
        self.query(world).get_many_inner(entities)
    }

    /// Returns the read-only query results for the given [`UniqueEntityArray`].
    ///
    /// In case of a nonexisting entity or mismatched component, a [`QueryEntityError`] is
    /// returned instead.
    ///
    /// # Examples
    ///
    /// ```
    /// use bevy_ecs::{prelude::*, query::QueryEntityError, entity::{EntitySetIterator, UniqueEntityArray, UniqueEntityVec}};
    ///
    /// #[derive(Component, PartialEq, Debug)]
    /// struct A(usize);
    ///
    /// let mut world = World::new();
    /// let entity_set: UniqueEntityVec = world.spawn_batch((0..3).map(A)).collect_set();
    /// let entity_set: UniqueEntityArray<3> = entity_set.try_into().unwrap();
    ///
    /// world.spawn(A(73));
    ///
    /// let mut query_state = world.query::<&A>();
    ///
    /// let component_values = query_state.get_many_unique(&world, entity_set).unwrap();
    ///
    /// assert_eq!(component_values, [&A(0), &A(1), &A(2)]);
    ///
    /// let wrong_entity = Entity::from_raw_u32(365).unwrap();
    ///
    /// assert_eq!(match query_state.get_many_unique(&mut world, UniqueEntityArray::from([wrong_entity])).unwrap_err() {QueryEntityError::NotSpawned(error) => error.entity(), _ => panic!()}, wrong_entity);
    /// ```
    #[inline]
    pub fn get_many_unique<'w, const N: usize>(
        &mut self,
        world: &'w World,
        entities: UniqueEntityArray<N>,
    ) -> Result<[ROQueryItem<'w, '_, D>; N], QueryEntityError> {
        self.query(world).get_many_unique_inner(entities)
    }

    /// Gets the query result for the given [`World`] and [`Entity`].
    ///
    /// This is always guaranteed to run in `O(1)` time.
    #[inline]
    pub fn get_mut<'w>(
        &mut self,
        world: &'w mut World,
        entity: Entity,
    ) -> Result<D::Item<'w, '_>, QueryEntityError> {
        self.query_mut(world).get_inner(entity)
    }

    /// Returns the query results for the given array of [`Entity`].
    ///
    /// In case of a nonexisting entity or mismatched component, a [`QueryEntityError`] is
    /// returned instead.
    ///
    /// ```
    /// use bevy_ecs::prelude::*;
    /// use bevy_ecs::query::QueryEntityError;
    ///
    /// #[derive(Component, PartialEq, Debug)]
    /// struct A(usize);
    ///
    /// let mut world = World::new();
    ///
    /// let entities: Vec<Entity> = (0..3).map(|i|world.spawn(A(i)).id()).collect();
    /// let entities: [Entity; 3] = entities.try_into().unwrap();
    ///
    /// world.spawn(A(73));
    ///
    /// let mut query_state = world.query::<&mut A>();
    ///
    /// let mut mutable_component_values = query_state.get_many_mut(&mut world, entities).unwrap();
    ///
    /// for mut a in &mut mutable_component_values {
    ///     a.0 += 5;
    /// }
    ///
    /// let component_values = query_state.get_many(&world, entities).unwrap();
    ///
    /// assert_eq!(component_values, [&A(5), &A(6), &A(7)]);
    ///
    /// let wrong_entity = Entity::from_raw_u32(57).unwrap();
    /// let invalid_entity = world.spawn_empty().id();
    ///
    /// assert_eq!(match query_state.get_many(&mut world, [wrong_entity]).unwrap_err() {QueryEntityError::NotSpawned(error) => error.entity(), _ => panic!()}, wrong_entity);
    /// assert_eq!(match query_state.get_many_mut(&mut world, [invalid_entity]).unwrap_err() {QueryEntityError::QueryDoesNotMatch(entity, _) => entity, _ => panic!()}, invalid_entity);
    /// assert_eq!(query_state.get_many_mut(&mut world, [entities[0], entities[0]]).unwrap_err(), QueryEntityError::AliasedMutability(entities[0]));
    /// ```
    #[inline]
    pub fn get_many_mut<'w, const N: usize>(
        &mut self,
        world: &'w mut World,
        entities: [Entity; N],
    ) -> Result<[D::Item<'w, '_>; N], QueryEntityError>
    where
        D: IterQueryData,
    {
        self.query_mut(world).get_many_mut_inner(entities)
    }

    /// Returns the query results for the given [`UniqueEntityArray`].
    ///
    /// In case of a nonexisting entity or mismatched component, a [`QueryEntityError`] is
    /// returned instead.
    ///
    /// ```
    /// use bevy_ecs::{prelude::*, query::QueryEntityError, entity::{EntitySetIterator, UniqueEntityArray, UniqueEntityVec}};
    ///
    /// #[derive(Component, PartialEq, Debug)]
    /// struct A(usize);
    ///
    /// let mut world = World::new();
    ///
    /// let entity_set: UniqueEntityVec = world.spawn_batch((0..3).map(A)).collect_set();
    /// let entity_set: UniqueEntityArray<3> = entity_set.try_into().unwrap();
    ///
    /// world.spawn(A(73));
    ///
    /// let mut query_state = world.query::<&mut A>();
    ///
    /// let mut mutable_component_values = query_state.get_many_unique_mut(&mut world, entity_set).unwrap();
    ///
    /// for mut a in &mut mutable_component_values {
    ///     a.0 += 5;
    /// }
    ///
    /// let component_values = query_state.get_many_unique(&world, entity_set).unwrap();
    ///
    /// assert_eq!(component_values, [&A(5), &A(6), &A(7)]);
    ///
    /// let wrong_entity = Entity::from_raw_u32(57).unwrap();
    /// let invalid_entity = world.spawn_empty().id();
    ///
    /// assert_eq!(match query_state.get_many_unique(&mut world, UniqueEntityArray::from([wrong_entity])).unwrap_err() {QueryEntityError::NotSpawned(error) => error.entity(), _ => panic!()}, wrong_entity);
    /// assert_eq!(match query_state.get_many_unique_mut(&mut world, UniqueEntityArray::from([invalid_entity])).unwrap_err() {QueryEntityError::QueryDoesNotMatch(entity, _) => entity, _ => panic!()}, invalid_entity);
    /// ```
    #[inline]
    pub fn get_many_unique_mut<'w, const N: usize>(
        &mut self,
        world: &'w mut World,
        entities: UniqueEntityArray<N>,
    ) -> Result<[D::Item<'w, '_>; N], QueryEntityError>
    where
        D: IterQueryData,
    {
        self.query_mut(world).get_many_unique_inner(entities)
    }

    /// Gets the query result for the given [`World`] and [`Entity`].
    ///
    /// This method is slightly more efficient than [`QueryState::get`] in some situations, since
    /// it does not update this instance's internal cache. This method will return an error if `entity`
    /// belongs to an archetype that has not been cached.
    ///
    /// To ensure that the cache is up to date, call [`QueryState::update_archetypes`] before this method.
    /// The cache is also updated in [`QueryState::new`], `QueryState::get`, or any method with mutable
    /// access to `self`.
    ///
    /// This can only be called for read-only queries, see [`Self::get_mut`] for mutable queries.
    ///
    /// This is always guaranteed to run in `O(1)` time.
    #[inline]
    pub fn get_manual<'w>(
        &self,
        world: &'w World,
        entity: Entity,
    ) -> Result<ROQueryItem<'w, '_, D>, QueryEntityError> {
        self.query_manual(world).get_inner(entity)
    }

    /// Gets the query result for the given [`World`] and [`Entity`].
    ///
    /// This is always guaranteed to run in `O(1)` time.
    ///
    /// # Safety
    ///
    /// This does not check for mutable query correctness. To be safe, make sure mutable queries
    /// have unique access to the components they query.
    #[inline]
    pub unsafe fn get_unchecked<'w>(
        &mut self,
        world: UnsafeWorldCell<'w>,
        entity: Entity,
    ) -> Result<D::Item<'w, '_>, QueryEntityError> {
        // SAFETY: Upheld by caller
        unsafe { self.query_unchecked(world) }.get_inner(entity)
    }

    /// Returns an [`Iterator`] over the query results for the given [`World`].
    ///
    /// This can only be called for read-only queries, see [`Self::iter_mut`] for write-queries.
    ///
    /// If you need to iterate multiple times at once but get borrowing errors,
    /// consider using [`Self::update_archetypes`] followed by multiple [`Self::iter_manual`] calls.
    #[inline]
    pub fn iter<'w, 's>(&'s mut self, world: &'w World) -> QueryIter<'w, 's, D::ReadOnly, F> {
        self.query(world).iter_inner()
    }

    /// Returns an [`Iterator`] over the query results for the given [`World`].
    ///
    /// This iterator is always guaranteed to return results from each matching entity once and only once.
    /// Iteration order is not guaranteed.
    #[inline]
    pub fn iter_mut<'w, 's>(&'s mut self, world: &'w mut World) -> QueryIter<'w, 's, D, F> {
        self.query_mut(world).iter_inner()
    }

    // /// Returns an [`Iterator`] over the query results for the given [`World`] without updating the query's archetypes.
    // /// Archetypes must be manually updated before by using [`Self::update_archetypes`].
    // ///
    // /// This iterator is always guaranteed to return results from each matching entity once and only once.
    // /// Iteration order is not guaranteed.
    // ///
    // /// This can only be called for read-only queries.
    // #[inline]
    // pub fn iter_manual<'w, 's>(&'s self, world: &'w World) -> QueryIter<'w, 's, D::ReadOnly, F> {
    //     self.query_manual(world).into_iter()
    // }
}

// TODO!
