use std::{cell::UnsafeCell, iter, marker::PhantomData, panic::Location};

use crate::{
    debug::{DebugCheckedUnwrap, DebugName, MaybeLocation},
    ecs::{
        archetype::{Archetype, Archetypes},
        bundle::Bundle,
        change_detection::{
            ComponentTicksMut, ComponentTicksRef, ContiguousComponentTicksMut,
            ContiguousComponentTicksRef, ContiguousMut, ContiguousRef, Mut, Ref, Tick,
        },
        component::{Component, ComponentId, Components, Mutable, StorageType},
        entity::{Entities, Entity, EntityLocation},
        query::{Access, EcsAccessLevel, EcsAccessType, QueryFilter, QueryState, WorldQuery},
        storage::{ComponentSparseSet, Table, TableRow},
        system::Query,
        world::{
            EntityMut, EntityMutExcept, EntityRef, EntityRefExcept, FilteredEntityMut,
            FilteredEntityRef, World, unsafe_world_cell::UnsafeWorldCell,
        },
    },
    ptr::{ThinSlicePtr, UnsafeCellDeref},
};

/// Types that can be fetched from a [`World`] using a [`Query`].
///
/// There are many types that natively implement this trait:
///
/// - **Component references. (&T and &mut T)**
///   Fetches a component by reference (immutably or mutably).
/// - **`QueryData` tuples.**
///   If every element of a tuple implements `QueryData`, then the tuple itself also implements the same trait.
///   This enables a single `Query` to access multiple components.
///   Due to the current lack of variadic generics in Rust, the trait has been implemented for tuples from 0 to 15 elements,
///   but nesting of tuples allows infinite `WorldQuery`s.
/// - **[`Entity`].**
///   Gets the identifier of the queried entity.
/// - **[`EntityLocation`].**
///   Gets the location metadata of the queried entity.
/// - **[`SpawnDetails`].**
///   Gets the tick the entity was spawned at.
/// - **[`EntityRef`].**
///   Read-only access to arbitrary components on the queried entity.
/// - **[`EntityMut`].**
///   Mutable access to arbitrary components on the queried entity.
/// - **[`&Archetype`](Archetype).**
///   Read-only access to the archetype-level metadata of the queried entity.
/// - **[`Option`].**
///   By default, a world query only tests entities that have the matching component types.
///   Wrapping it into an `Option` will increase the query search space, and it will return `None` if an entity doesn't satisfy the `WorldQuery`.
/// - **[`AnyOf`].**
///   Equivalent to wrapping each world query inside it into an `Option`.
/// - **[`Ref`].**
///   Similar to change detection filters but it is used as a query fetch parameter.
///   It exposes methods to check for changes to the wrapped component.
/// - **[`Mut`].**
///   Mutable component access, with change detection data.
/// - **[`Has`].**
///   Returns a bool indicating whether the entity has the specified component.
///
/// Implementing the trait manually can allow for a fundamentally new type of behavior.
///
/// # Trait derivation
///
/// Query design can be easily structured by deriving `QueryData` for custom types.
/// Despite the added complexity, this approach has several advantages over using `QueryData` tuples.
/// The most relevant improvements are:
///
/// - Reusability across multiple systems.
/// - There is no need to destructure a tuple since all fields are named.
/// - Subqueries can be composed together to create a more complex query.
/// - Methods can be implemented for the query items.
/// - There is no hardcoded limit on the number of elements.
///
/// This trait can only be derived for structs, if each field also implements `QueryData`.
///
/// ```
/// # use bevy_ecs::prelude::*;
/// use bevy_ecs::query::QueryData;
/// #
/// # #[derive(Component)]
/// # struct ComponentA;
/// # #[derive(Component)]
/// # struct ComponentB;
///
/// #[derive(QueryData)]
/// struct MyQuery {
///     entity: Entity,
///     // It is required that all reference lifetimes are explicitly annotated, just like in any
///     // struct. Each lifetime should be 'static.
///     component_a: &'static ComponentA,
///     component_b: &'static ComponentB,
/// }
///
/// fn my_system(query: Query<MyQuery>) {
///     for q in &query {
///         q.component_a;
///     }
/// }
/// # bevy_ecs::system::assert_is_system(my_system);
/// ```
///
/// ## Macro expansion
///
/// Expanding the macro will declare one to five additional structs, depending on whether or not the struct is marked as mutable or as contiguous.
/// For a struct named `X`, the additional structs will be:
///
/// |Struct name|`mutable` only|`contiguous` target|Description|
/// |:---:|:---:|:---:|---|
/// |`XItem`|---|---|The type of the query item for `X`|
/// |`XReadOnlyItem`|✓|---|The type of the query item for `XReadOnly`|
/// |`XReadOnly`|✓|---|[`ReadOnly`] variant of `X`|
/// |`XContiguousItem`|---|`mutable` or `all`|The type of the contiguous query item for `X`|
/// |`XReadOnlyContiguousItem`|✓|`immutable` or `all`|The type of the contiguous query item for `XReadOnly`|
///
/// ## Adding mutable references
///
/// Simply adding mutable references to a derived `QueryData` will result in a compilation error:
///
/// ```compile_fail
/// # use bevy_ecs::prelude::*;
/// # use bevy_ecs::query::QueryData;
/// #
/// # #[derive(Component)]
/// # struct ComponentA;
/// #
/// #[derive(QueryData)]
/// struct CustomQuery {
///     component_a: &'static mut ComponentA,
/// }
/// ```
///
/// To grant mutable access to components, the struct must be marked with the `#[query_data(mutable)]` attribute.
/// This will also create three more structs that will be used for accessing the query immutably (see table above).
///
/// ```
/// # use bevy_ecs::prelude::*;
/// # use bevy_ecs::query::QueryData;
/// #
/// # #[derive(Component)]
/// # struct ComponentA;
/// #
/// #[derive(QueryData)]
/// #[query_data(mutable)]
/// struct CustomQuery {
///     component_a: &'static mut ComponentA,
/// }
/// ```
///
/// ## Supporting contiguous iteration
///
/// To create contiguous items additionally (to support contiguous iteration), the struct must be marked with the `#[query_data(contiguous(target))]` attribute,
/// where the target may be `all`, `mutable` or `immutable` (see the table above).
///
/// For mutable queries it may be done like this:
/// ```
/// # use bevy_ecs::prelude::*;
/// # use bevy_ecs::query::QueryData;
/// #
/// # #[derive(Component)]
/// # struct ComponentA;
/// #
/// #[derive(QueryData)]
/// /// - contiguous(all) will create contiguous items for both read and mutable versions
/// /// - contiguous(mutable) will only create a contiguous item for the mutable version
/// /// - contiguous(immutable) will only create a contiguous item for the read only version
/// #[query_data(mutable, contiguous(all))]
/// struct CustomQuery {
///     component_a: &'static mut ComponentA,
/// }
/// ```
///
/// For immutable queries `contiguous(immutable)` attribute will be **ignored**, meanwhile `contiguous(mutable)` and `contiguous(all)`
/// will only generate a contiguous item for the (original) read only version.
///
/// To understand contiguous iteration refer to
/// [`Query::contiguous_iter`](`crate::system::Query::contiguous_iter`)
///
/// ## Adding methods to query items
///
/// It is possible to add methods to query items in order to write reusable logic about related components.
/// This will often make systems more readable because low level logic is moved out from them.
/// It is done by adding `impl` blocks with methods for the `-Item`, `-ReadOnlyItem`, `-ContiguousItem` or `ContiguousReadOnlyItem`
/// generated structs.
///
/// ```
/// # use bevy_ecs::prelude::*;
/// # use bevy_ecs::query::QueryData;
/// #
/// #[derive(Component)]
/// struct Health(f32);
///
/// #[derive(Component)]
/// struct Buff(f32);
///
/// #[derive(QueryData)]
/// #[query_data(mutable)]
/// struct HealthQuery {
///     health: &'static mut Health,
///     buff: Option<&'static mut Buff>,
/// }
///
/// // `HealthQueryItem` is only available when accessing the query with mutable methods.
/// impl<'w, 's> HealthQueryItem<'w, 's> {
///     fn damage(&mut self, value: f32) {
///         self.health.0 -= value;
///     }
///
///     fn total(&self) -> f32 {
///         self.health.0 + self.buff.as_deref().map_or(0.0, |Buff(buff)| *buff)
///     }
/// }
///
/// // `HealthQueryReadOnlyItem` is only available when accessing the query with immutable methods.
/// impl<'w, 's> HealthQueryReadOnlyItem<'w, 's> {
///     fn total(&self) -> f32 {
///         self.health.0 + self.buff.map_or(0.0, |Buff(buff)| *buff)
///     }
/// }
///
/// fn my_system(mut health_query: Query<HealthQuery>) {
///     // The item returned by the iterator is of type `HealthQueryReadOnlyItem`.
///     for health in health_query.iter() {
///         println!("Total: {}", health.total());
///     }
///     // The item returned by the iterator is of type `HealthQueryItem`.
///     for mut health in &mut health_query {
///         health.damage(1.0);
///         println!("Total (mut): {}", health.total());
///     }
/// }
/// # bevy_ecs::system::assert_is_system(my_system);
/// ```
///
/// ## Deriving traits for query items
///
/// The `QueryData` derive macro does not automatically implement the traits of the struct to the query item types.
/// Something similar can be done by using the `#[query_data(derive(...))]` attribute.
/// This will apply the listed derivable traits to the query item structs.
///
/// ```
/// # use bevy_ecs::prelude::*;
/// # use bevy_ecs::query::QueryData;
/// #
/// # #[derive(Component, Debug)]
/// # struct ComponentA;
/// #
/// #[derive(QueryData)]
/// #[query_data(mutable, derive(Debug), contiguous(all))]
/// struct CustomQuery {
///     component_a: &'static ComponentA,
/// }
///
/// // This function statically checks that `T` implements `Debug`.
/// fn assert_debug<T: std::fmt::Debug>() {}
///
/// assert_debug::<CustomQueryItem>();
/// assert_debug::<CustomQueryReadOnlyItem>();
/// assert_debug::<CustomQueryContiguousItem>();
/// assert_debug::<CustomQueryReadOnlyContiguousItem>();
/// ```
///
/// ## Query composition
///
/// It is possible to use any `QueryData` as a field of another one.
/// This means that a `QueryData` can also be used as a subquery, potentially in multiple places.
///
/// ```
/// # use bevy_ecs::prelude::*;
/// # use bevy_ecs::query::QueryData;
/// #
/// # #[derive(Component)]
/// # struct ComponentA;
/// # #[derive(Component)]
/// # struct ComponentB;
/// # #[derive(Component)]
/// # struct ComponentC;
/// #
/// #[derive(QueryData)]
/// struct SubQuery {
///     component_a: &'static ComponentA,
///     component_b: &'static ComponentB,
/// }
///
/// #[derive(QueryData)]
/// struct MyQuery {
///     subquery: SubQuery,
///     component_c: &'static ComponentC,
/// }
/// ```
///
/// # Generic Queries
///
/// When writing generic code, it is often necessary to use [`PhantomData`]
/// to constrain type parameters. Since `QueryData` is implemented for all
/// `PhantomData<T>` types, this pattern can be used with this macro.
///
/// ```
/// # use bevy_ecs::{prelude::*, query::QueryData};
/// # use std::marker::PhantomData;
/// #[derive(QueryData)]
/// pub struct GenericQuery<T> {
///     id: Entity,
///     marker: PhantomData<T>,
/// }
/// # fn my_system(q: Query<GenericQuery<()>>) {}
/// # bevy_ecs::system::assert_is_system(my_system);
/// ```
///
/// # Safety
///
/// - Component access of `Self::ReadOnly` must be a subset of `Self`
///   and `Self::ReadOnly` must match exactly the same archetypes/tables as `Self`
/// - `IS_READ_ONLY` must be `true` if and only if `Self: ReadOnlyQueryData`
///
/// [`ReadOnly`]: Self::ReadOnly
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not valid to request as data in a `Query`",
    label = "invalid `Query` data",
    note = "if `{Self}` is a component type, try using `&{Self}` or `&mut {Self}`"
)]
pub unsafe trait QueryData: WorldQuery {
    /// True if this query is read-only and may not perform mutable access.
    const IS_READ_ONLY: bool;

    /// Returns true if (and only if) this query data relies strictly on archetypes to limit which
    /// entities are accessed by the Query.
    ///
    /// This enables optimizations for [`QueryIter`](`crate::query::QueryIter`) that rely on knowing exactly how
    /// many elements are being iterated (such as `Iterator::collect()`).
    ///
    /// If this is `true`, then [`QueryData::fetch`] must always return `Some`.
    const IS_ARCHETYPAL: bool;

    /// The read-only variant of this [`QueryData`], which satisfies the [`ReadOnlyQueryData`] trait.
    type ReadOnly: ReadOnlyQueryData<State = <Self as WorldQuery>::State>;

    /// The item returned by this [`WorldQuery`]
    /// This will be the data retrieved by the query,
    /// and is visible to the end user when calling e.g. `Query<Self>::get`.
    type Item<'w, 's>;

    /// This function manually implements subtyping for the query items.
    fn shrink<'wlong: 'wshort, 'wshort, 's>(
        item: Self::Item<'wlong, 's>,
    ) -> Self::Item<'wshort, 's>;

    /// Offers additional access above what we requested in `update_component_access`.
    /// Implementations may add additional access that is a subset of `available_access`
    /// and does not conflict with anything in `access`,
    /// and must update `access` to include that access.
    ///
    /// This is used by [`WorldQuery`] types like [`FilteredEntityRef`]
    /// and [`FilteredEntityMut`] to support dynamic access.
    ///
    /// Called when constructing a [`QueryLens`](crate::system::QueryLens) or calling [`QueryState::from_builder`](super::QueryState::from_builder)
    fn provide_extra_access(
        _state: &mut Self::State,
        _access: &mut Access,
        _available_access: &Access,
    ) {
    }

    /// Fetch [`Self::Item`](`QueryData::Item`) for either the given `entity` in the current [`Table`],
    /// or for the given `entity` in the current [`Archetype`]. This must always be called after
    /// [`WorldQuery::set_table`] with a `table_row` in the range of the current [`Table`] or after
    /// [`WorldQuery::set_archetype`]  with an `entity` in the current archetype.
    /// Accesses components registered in [`WorldQuery::update_component_access`].
    ///
    /// This method returns `None` if the entity does not match the query.
    /// If `Self` implements [`ArchetypeQueryData`], this must always return `Some`.
    ///
    /// # Safety
    ///
    /// - Must always be called _after_ [`WorldQuery::set_table`] or [`WorldQuery::set_archetype`]. `entity` and
    ///   `table_row` must be in the range of the current table and archetype.
    /// - There must not be simultaneous conflicting component access registered in `update_component_access`.
    /// - If `Self` does not impl `ReadOnlyQueryData`, then there must not be any other `Item`s alive for the current entity
    /// - If `Self` does not impl `IterQueryData`, then there must not be any other `Item`s alive for *any* entity
    unsafe fn fetch<'w, 's>(
        state: &'s Self::State,
        fetch: &mut Self::Fetch<'w>,
        entity: Entity,
        table_row: TableRow,
    ) -> Option<Self::Item<'w, 's>>;

    /// Returns an iterator over the access needed by [`QueryData::fetch`]. Access conflicts are usually
    /// checked in [`WorldQuery::update_component_access`], but in certain cases this method can be useful to implement
    /// a way of checking for access conflicts in a non-allocating way.
    fn iter_access(state: &Self::State) -> impl Iterator<Item = EcsAccessType<'_>>;
}

/// A [`QueryData`] which allows getting a direct access to contiguous chunks of components'
/// values, which may be used to apply simd-operations.
///
/// Contiguous iteration may be done via:
/// - [`Query::contiguous_iter`](crate::system::Query::contiguous_iter),
/// - [`Query::contiguous_iter_mut`](crate::system::Query::contiguous_iter_mut),
///
// NOTE: Even though all component references (&T, &mut T) implement this trait, it won't be executed for
// SparseSet components because in that case the query is not dense.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be iterated contiguously",
    label = "invalid contiguous `Query` data",
    note = "if `{Self}` is a custom query type, using `QueryData` derive macro, ensure that the `#[query_data(contiguous(target))]` attribute is added"
)]
pub trait ContiguousQueryData: ArchetypeQueryData + IterQueryData {
    /// Item returned by [`ContiguousQueryData::fetch_contiguous`].
    /// Represents a contiguous chunk of memory.
    type Contiguous<'w, 's>;

    /// Fetch [`ContiguousQueryData::Contiguous`] which represents a contiguous chunk of memory (e.g., an array) in the current [`Table`].
    /// This must always be called after [`WorldQuery::set_table`].
    ///
    /// # Safety
    ///
    /// - Must always be called _after_ [`WorldQuery::set_table`].
    /// - `entities`'s length must match the length of the set table.
    /// - `entities` must match the entities of the set table.
    /// - There must not be simultaneous conflicting component access registered in `update_component_access`.
    unsafe fn fetch_contiguous<'w, 's>(
        state: &'s Self::State,
        fetch: &mut Self::Fetch<'w>,
        entities: &'w [Entity],
    ) -> Self::Contiguous<'w, 's>;
}

/// A [`QueryData`] for which instances may be alive for different entities concurrently.
///
/// Rust [`Iterator`]s don't connect the lifetime in [`Iterator::next`] to anything in [`Iterator::Item`],
/// so later calls don't invalidate earlier items.
/// This is how methods like [`Iterator::collect`] work.
/// It is therefore unsound to offer an [`Iterator`] for a [`QueryData`] for which only one instance may be alive concurrently.
///
/// To iterate over a [`QueryData`] that does not implement [`IterQueryData`],
/// use the [`QueryIter::fetch_next()`](crate::query::QueryIter::fetch_next) method.
///
/// For `QueryData` that implement this trait, [`QueryData::fetch`] may be called for one entity while an item is still alive for a different entity.
///
/// All [`SingleEntityQueryData`] types are [`IterQueryData`].
/// They only access data on the current entity, the one passed to [`QueryData::fetch`],
/// so the access for different entities will always be disjoint.
///
/// All [`ReadOnlyQueryData`] types are [`IterQueryData`].
/// Even if they access data on entities other than the current one,
/// that access is read-only and it's sound for it to alias.
///
/// Queries with a nested query that performs mutable access should generally *not* be [`IterQueryData`],
/// although they can be if they have a way to prove that all accesses through the nested query are disjoint.
///
/// # Safety
///
/// This [`QueryData`] must not perform conflicting access when fetched for different entities.
pub unsafe trait IterQueryData: QueryData {}

/// A [`QueryData`] that is read only.
///
/// # Safety
///
/// This must only be implemented for read-only [`QueryData`]'s.
pub unsafe trait ReadOnlyQueryData: IterQueryData<ReadOnly = Self> {}

/// A [`QueryData`] that only accesses data from the current entity, the one passed to [`QueryData::fetch`].
///
/// This is used as a bound in [`EntityRef::get_components`] and related APIs,
/// since they only have access to a single entity.
///
/// # Safety
///
/// This [`QueryData`] must only access data from the current entity, and not any other entities.
pub unsafe trait SingleEntityQueryData: IterQueryData {}

/// The item type returned when a [`WorldQuery`] is iterated over
pub type QueryItem<'w, 's, Q> = <Q as QueryData>::Item<'w, 's>;
/// The read-only variant of the item type returned when a [`QueryData`] is iterated over immutably
pub type ROQueryItem<'w, 's, D> = QueryItem<'w, 's, <D as QueryData>::ReadOnly>;

/// A [`QueryData`] that does not borrow from its [`QueryState`].
///
/// This is implemented by most `QueryData` types.
/// The main exceptions are [`FilteredEntityRef`], [`FilteredEntityMut`], [`EntityRefExcept`], and [`EntityMutExcept`],
/// which borrow an access list from their query state.
/// Consider using a full [`EntityRef`] or [`EntityMut`] if you would need those.
pub trait ReleaseStateQueryData: QueryData {
    /// Releases the borrow from the query state by converting an item to have a `'static` state lifetime.
    fn release_state<'w>(item: Self::Item<'w, '_>) -> Self::Item<'w, 'static>;
}

/// A marker trait to indicate that the query data filters at an archetype level.
///
/// This is needed to implement [`ExactSizeIterator`] for
/// [`QueryIter`](crate::query::QueryIter) that contains archetype-level filters.
///
/// The trait must only be implemented for query data where its corresponding [`QueryData::IS_ARCHETYPAL`] is [`prim@true`].
pub trait ArchetypeQueryData: QueryData {}

// SAFETY:
// `update_component_access` does nothing.
// This is sound because `fetch` does not access components.
unsafe impl WorldQuery for Entity {
    type Fetch<'w> = ();

    type State = ();

    fn shrink_fetch<'wlong: 'wshort, 'wshort>(_fetch: Self::Fetch<'wlong>) -> Self::Fetch<'wshort> {
    }

    unsafe fn init_fetch<'w, 's>(
        _world: UnsafeWorldCell<'w>,
        _state: &'s Self::State,
        _last_run: Tick,
        _this_run: Tick,
    ) -> Self::Fetch<'w> {
    }

    const IS_DENSE: bool = true;

    #[inline]
    unsafe fn set_archetype<'w, 's>(
        _fetch: &mut Self::Fetch<'w>,
        _state: &'s Self::State,
        _archetype: &'w Archetype,
        _table: &'w Table,
    ) {
    }

    #[inline]
    unsafe fn set_table<'w, 's>(
        _fetch: &mut Self::Fetch<'w>,
        _state: &'s Self::State,
        _table: &'w Table,
    ) {
    }

    fn update_component_access(_state: &Self::State, _access: &mut super::FilteredAccess) {}

    fn init_state(_world: &mut World) -> Self::State {}

    fn get_state(_components: &Components) -> Option<Self::State> {
        Some(())
    }

    fn matches_component_set(
        _state: &Self::State,
        _set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        true
    }
}

// SAFETY: `Self` is the same as `Self::ReadOnly`
unsafe impl QueryData for Entity {
    const IS_READ_ONLY: bool = true;

    const IS_ARCHETYPAL: bool = true;

    type ReadOnly = Self;

    type Item<'w, 's> = Entity;

    fn shrink<'wlong: 'wshort, 'wshort, 's>(
        item: Self::Item<'wlong, 's>,
    ) -> Self::Item<'wshort, 's> {
        item
    }

    #[inline(always)]
    unsafe fn fetch<'w, 's>(
        _state: &'s Self::State,
        _fetch: &mut Self::Fetch<'w>,
        entity: Entity,
        _table_row: TableRow,
    ) -> Option<Self::Item<'w, 's>> {
        Some(entity)
    }

    fn iter_access(_state: &Self::State) -> impl Iterator<Item = EcsAccessType<'_>> {
        iter::empty()
    }
}

// SAFETY: access is read only and only on the current entity
unsafe impl IterQueryData for Entity {}

// SAFETY: access is read only
unsafe impl ReadOnlyQueryData for Entity {}

// SAFETY: access is only on the current entity
unsafe impl SingleEntityQueryData for Entity {}

impl ReleaseStateQueryData for Entity {
    fn release_state<'w>(item: Self::Item<'w, '_>) -> Self::Item<'w, 'static> {
        item
    }
}

impl ArchetypeQueryData for Entity {}

impl ContiguousQueryData for Entity {
    type Contiguous<'w, 's> = &'w [Entity];

    unsafe fn fetch_contiguous<'w, 's>(
        _state: &'s Self::State,
        _fetch: &mut Self::Fetch<'w>,
        entities: &'w [Entity],
    ) -> Self::Contiguous<'w, 's> {
        entities
    }
}

// SAFETY:
// `update_component_access` does nothing.
// This is sound because `fetch` does not access components.
unsafe impl WorldQuery for EntityLocation {
    type Fetch<'w> = &'w Entities;

    type State = ();

    fn shrink_fetch<'wlong: 'wshort, 'wshort>(fetch: Self::Fetch<'wlong>) -> Self::Fetch<'wshort> {
        fetch
    }

    unsafe fn init_fetch<'w, 's>(
        world: UnsafeWorldCell<'w>,
        _state: &'s Self::State,
        _last_run: Tick,
        _this_run: Tick,
    ) -> Self::Fetch<'w> {
        world.entities()
    }

    // This is set to true to avoid forcing archetypal iteration in compound queries, is likely to be slower
    // in most practical use case.
    const IS_DENSE: bool = true;

    #[inline]
    unsafe fn set_archetype<'w, 's>(
        _fetch: &mut Self::Fetch<'w>,
        _state: &'s Self::State,
        _archetype: &'w Archetype,
        _table: &'w Table,
    ) {
    }

    #[inline]
    unsafe fn set_table<'w, 's>(
        _fetch: &mut Self::Fetch<'w>,
        _state: &'s Self::State,
        _table: &'w Table,
    ) {
    }

    fn update_component_access(_state: &Self::State, _access: &mut super::FilteredAccess) {}

    fn init_state(_world: &mut World) -> Self::State {}

    fn get_state(_components: &Components) -> Option<Self::State> {
        Some(())
    }

    fn matches_component_set(
        _state: &Self::State,
        _set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        true
    }
}

// SAFETY: `Self` is the same as `Self::ReadOnly`
unsafe impl QueryData for EntityLocation {
    const IS_READ_ONLY: bool = true;

    const IS_ARCHETYPAL: bool = true;

    type ReadOnly = Self;

    type Item<'w, 's> = EntityLocation;

    fn shrink<'wlong: 'wshort, 'wshort, 's>(
        item: Self::Item<'wlong, 's>,
    ) -> Self::Item<'wshort, 's> {
        item
    }

    #[inline(always)]
    unsafe fn fetch<'w, 's>(
        _state: &'s Self::State,
        fetch: &mut Self::Fetch<'w>,
        entity: Entity,
        _table_row: TableRow,
    ) -> Option<Self::Item<'w, 's>> {
        // SAFETY: `fetch` must be called with an entity that exists in the world
        Some(unsafe { fetch.get_spawned(entity).debug_checked_unwrap() })
    }

    fn iter_access(_state: &Self::State) -> impl Iterator<Item = EcsAccessType<'_>> {
        iter::empty()
    }
}

// SAFETY: access is read only and only on the current entity
unsafe impl IterQueryData for EntityLocation {}

// SAFETY: access is read only
unsafe impl ReadOnlyQueryData for EntityLocation {}

// SAFETY: access is only on the current entity
unsafe impl SingleEntityQueryData for EntityLocation {}

impl ReleaseStateQueryData for EntityLocation {
    fn release_state<'w>(item: Self::Item<'w, '_>) -> Self::Item<'w, 'static> {
        item
    }
}

impl ArchetypeQueryData for EntityLocation {}

/// The `SpawnDetails` query parameter fetches the [`Tick`] the entity was spawned at.
///
/// To evaluate whether the spawn happened since the last time the system ran, the system
/// param [`SystemChangeTick`](bevy_ecs::system::SystemChangeTick) needs to be used.
///
/// If the query should filter for spawned entities instead, use the
/// [`Spawned`](bevy_ecs::query::Spawned) query filter instead.
///
/// # Examples
///
/// ```
/// # use bevy_ecs::component::Component;
/// # use bevy_ecs::entity::Entity;
/// # use bevy_ecs::system::Query;
/// # use bevy_ecs::query::Spawned;
/// # use bevy_ecs::query::SpawnDetails;
///
/// fn print_spawn_details(query: Query<(Entity, SpawnDetails)>) {
///     for (entity, spawn_details) in &query {
///         if spawn_details.is_spawned() {
///             print!("new ");
///         }
///         print!(
///             "entity {:?} spawned at {:?}",
///             entity,
///             spawn_details.spawn_tick()
///         );
///         match spawn_details.spawned_by().into_option() {
///             Some(location) => println!(" by {:?}", location),
///             None => println!()
///         }
///     }
/// }
///
/// # bevy_ecs::system::assert_is_system(print_spawn_details);
/// ```
#[derive(Clone, Copy, Debug)]
pub struct SpawnDetails {
    spawned_by: MaybeLocation,
    spawn_tick: Tick,
    last_run: Tick,
    this_run: Tick,
}

impl SpawnDetails {
    /// Returns `true` if the entity spawned since the last time this system ran.
    /// Otherwise, returns `false`.
    pub fn is_spawned(self) -> bool {
        self.is_spawned_after(self.last_run)
    }

    /// Returns `true` if the entity spawned after the `other` tick.
    /// Otherwise, returns `false`.
    #[inline]
    pub fn is_spawned_after(self, other: Tick) -> bool {
        self.spawn_tick.is_newer_than(other, self.this_run)
    }

    /// Returns the `Tick` this entity spawned at.
    pub fn spawn_tick(self) -> Tick {
        self.spawn_tick
    }

    /// Returns the source code location from which this entity has been spawned.
    pub fn spawned_by(self) -> MaybeLocation {
        self.spawned_by
    }
}

#[doc(hidden)]
#[derive(Clone)]
pub struct SpawnDetailsFetch<'w> {
    entities: &'w Entities,
    last_run: Tick,
    this_run: Tick,
}

// SAFETY:
// No components are accessed.
unsafe impl WorldQuery for SpawnDetails {
    type Fetch<'w> = SpawnDetailsFetch<'w>;

    type State = ();

    fn shrink_fetch<'wlong: 'wshort, 'wshort>(fetch: Self::Fetch<'wlong>) -> Self::Fetch<'wshort> {
        fetch
    }

    unsafe fn init_fetch<'w, 's>(
        world: UnsafeWorldCell<'w>,
        _state: &'s Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Self::Fetch<'w> {
        SpawnDetailsFetch {
            entities: world.entities(),
            last_run,
            this_run,
        }
    }

    const IS_DENSE: bool = true;

    #[inline]
    unsafe fn set_archetype<'w, 's>(
        _fetch: &mut Self::Fetch<'w>,
        _state: &'s Self::State,
        _archetype: &'w Archetype,
        _table: &'w Table,
    ) {
    }

    #[inline]
    unsafe fn set_table<'w, 's>(
        _fetch: &mut Self::Fetch<'w>,
        _state: &'s Self::State,
        _table: &'w Table,
    ) {
    }

    fn update_component_access(_state: &Self::State, _access: &mut super::FilteredAccess) {}

    fn init_state(_world: &mut World) -> Self::State {}

    fn get_state(_components: &Components) -> Option<Self::State> {
        Some(())
    }

    fn matches_component_set(
        _state: &Self::State,
        _set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        true
    }
}

// SAFETY:
// No components are accessed.
// Is its own ReadOnlyQueryData.
unsafe impl QueryData for SpawnDetails {
    const IS_READ_ONLY: bool = true;

    const IS_ARCHETYPAL: bool = true;

    type ReadOnly = Self;

    type Item<'w, 's> = Self;

    fn shrink<'wlong: 'wshort, 'wshort, 's>(
        item: Self::Item<'wlong, 's>,
    ) -> Self::Item<'wshort, 's> {
        item
    }

    #[inline(always)]
    unsafe fn fetch<'w, 's>(
        _state: &'s Self::State,
        fetch: &mut Self::Fetch<'w>,
        entity: Entity,
        _table_row: TableRow,
    ) -> Option<Self::Item<'w, 's>> {
        let (spawned_by, spawn_tick) = unsafe {
            fetch
                .entities
                .entity_get_spawned_or_despawned_unchecked(entity)
        };
        Some(Self {
            spawned_by,
            spawn_tick,
            last_run: fetch.last_run,
            this_run: fetch.this_run,
        })
    }

    fn iter_access(_state: &Self::State) -> impl Iterator<Item = EcsAccessType<'_>> {
        iter::empty()
    }
}

// SAFETY: access is read only and only on the current entity
unsafe impl IterQueryData for SpawnDetails {}

// SAFETY: access is read only
unsafe impl ReadOnlyQueryData for SpawnDetails {}

// SAFETY: access is only on the current entity
unsafe impl SingleEntityQueryData for SpawnDetails {}

impl ReleaseStateQueryData for SpawnDetails {
    fn release_state<'w>(item: Self::Item<'w, '_>) -> Self::Item<'w, 'static> {
        item
    }
}

impl ArchetypeQueryData for SpawnDetails {}

/// The [`WorldQuery::Fetch`] type for WorldQueries that can fetch multiple components from an entity
/// ([`EntityRef`], [`EntityMut`], etc.)
#[derive(Copy, Clone)]
#[doc(hidden)]
pub struct EntityFetch<'w> {
    world: UnsafeWorldCell<'w>,
    last_run: Tick,
    this_run: Tick,
}

// SAFETY:
// `fetch` accesses all components in a readonly way.
// This is sound because `update_component_access` sets read access for all components and panic when appropriate.
// Filters are unchanged.
unsafe impl<'a> WorldQuery for EntityRef<'a> {
    type Fetch<'w> = EntityFetch<'w>;

    type State = ();

    fn shrink_fetch<'wlong: 'wshort, 'wshort>(fetch: Self::Fetch<'wlong>) -> Self::Fetch<'wshort> {
        fetch
    }

    unsafe fn init_fetch<'w, 's>(
        world: UnsafeWorldCell<'w>,
        _state: &'s Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Self::Fetch<'w> {
        EntityFetch {
            world,
            last_run,
            this_run,
        }
    }

    const IS_DENSE: bool = true;

    #[inline]
    unsafe fn set_archetype<'w, 's>(
        _fetch: &mut Self::Fetch<'w>,
        _state: &'s Self::State,
        _archetype: &'w Archetype,
        _table: &'w Table,
    ) {
    }

    #[inline]
    unsafe fn set_table<'w, 's>(
        _fetch: &mut Self::Fetch<'w>,
        _state: &'s Self::State,
        _table: &'w Table,
    ) {
    }

    fn update_component_access(_state: &Self::State, access: &mut super::FilteredAccess) {
        assert!(
            !access.access().has_any_write(),
            "EntityRef conflicts with a previous access in this query. Shared access cannot coincide with exclusive access."
        );
        access.read_all();
    }

    fn init_state(_world: &mut World) -> Self::State {}

    fn get_state(_components: &Components) -> Option<Self::State> {
        Some(())
    }

    fn matches_component_set(
        _state: &Self::State,
        _set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        true
    }
}

unsafe impl<'a> QueryData for EntityRef<'a> {
    const IS_READ_ONLY: bool = true;

    const IS_ARCHETYPAL: bool = true;

    type ReadOnly = Self;

    type Item<'w, 's> = EntityRef<'w>;

    fn shrink<'wlong: 'wshort, 'wshort, 's>(
        item: Self::Item<'wlong, 's>,
    ) -> Self::Item<'wshort, 's> {
        item
    }

    #[inline(always)]
    unsafe fn fetch<'w, 's>(
        _state: &'s Self::State,
        fetch: &mut Self::Fetch<'w>,
        entity: Entity,
        _table_row: TableRow,
    ) -> Option<Self::Item<'w, 's>> {
        // SAFETY: `fetch` must be called with an entity that exists in the world
        let cell = unsafe {
            fetch
                .world
                .get_entity_with_ticks(entity, fetch.last_run, fetch.this_run)
                .debug_checked_unwrap()
        };
        // SAFETY: Read-only access to every component has been registered.
        Some(unsafe { EntityRef::new(cell) })
    }

    fn iter_access(_state: &Self::State) -> impl Iterator<Item = EcsAccessType<'_>> {
        iter::once(EcsAccessType::Component(EcsAccessLevel::ReadAll))
    }
}

// SAFETY: access is read only and only on the current entity
unsafe impl IterQueryData for EntityRef<'_> {}

// SAFETY: access is read only
unsafe impl ReadOnlyQueryData for EntityRef<'_> {}

// SAFETY: access is only on the current entity
unsafe impl SingleEntityQueryData for EntityRef<'_> {}

impl ReleaseStateQueryData for EntityRef<'_> {
    fn release_state<'w>(item: Self::Item<'w, '_>) -> Self::Item<'w, 'static> {
        item
    }
}

impl ArchetypeQueryData for EntityRef<'_> {}

unsafe impl<'a> WorldQuery for EntityMut<'a> {
    type Fetch<'w> = EntityFetch<'w>;

    type State = ();

    fn shrink_fetch<'wlong: 'wshort, 'wshort>(fetch: Self::Fetch<'wlong>) -> Self::Fetch<'wshort> {
        fetch
    }

    unsafe fn init_fetch<'w, 's>(
        world: UnsafeWorldCell<'w>,
        _state: &'s Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Self::Fetch<'w> {
        EntityFetch {
            world,
            last_run,
            this_run,
        }
    }

    const IS_DENSE: bool = true;

    #[inline]
    unsafe fn set_archetype<'w, 's>(
        _fetch: &mut Self::Fetch<'w>,
        _state: &'s Self::State,
        _archetype: &'w Archetype,
        _table: &'w Table,
    ) {
    }

    #[inline]
    unsafe fn set_table<'w, 's>(
        _fetch: &mut Self::Fetch<'w>,
        _state: &'s Self::State,
        _table: &'w Table,
    ) {
    }

    fn update_component_access(_state: &Self::State, access: &mut super::FilteredAccess) {
        assert!(
            !access.access().has_any_read(),
            "EntityMut conflicts with a previous access in this query. Exclusive access cannot coincide with any other accesses.",
        );
        access.write_all();
    }

    fn init_state(_world: &mut World) -> Self::State {}

    fn get_state(_components: &Components) -> Option<Self::State> {
        Some(())
    }

    fn matches_component_set(
        _state: &Self::State,
        _set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        true
    }
}

// SAFETY: access of `EntityRef` is a subset of `EntityMut`
unsafe impl<'a> QueryData for EntityMut<'a> {
    const IS_READ_ONLY: bool = false;

    const IS_ARCHETYPAL: bool = true;

    type ReadOnly = EntityRef<'a>;

    type Item<'w, 's> = EntityMut<'w>;

    fn shrink<'wlong: 'wshort, 'wshort, 's>(
        item: Self::Item<'wlong, 's>,
    ) -> Self::Item<'wshort, 's> {
        item
    }

    #[inline(always)]
    unsafe fn fetch<'w, 's>(
        _state: &'s Self::State,
        fetch: &mut Self::Fetch<'w>,
        entity: Entity,
        _table_row: TableRow,
    ) -> Option<Self::Item<'w, 's>> {
        // SAFETY: `fetch` must be called with an entity that exists in the world
        let cell = unsafe {
            fetch
                .world
                .get_entity_with_ticks(entity, fetch.last_run, fetch.this_run)
                .debug_checked_unwrap()
        };
        // SAFETY: mutable access to every component has been registered.
        Some(unsafe { EntityMut::new(cell) })
    }

    fn iter_access(_state: &Self::State) -> impl Iterator<Item = EcsAccessType<'_>> {
        iter::once(EcsAccessType::Component(EcsAccessLevel::WriteAll))
    }
}

// SAFETY: access is only on the current entity
unsafe impl IterQueryData for EntityMut<'_> {}

// SAFETY: access is only on the current entity
unsafe impl SingleEntityQueryData for EntityMut<'_> {}

impl ReleaseStateQueryData for EntityMut<'_> {
    fn release_state<'w>(item: Self::Item<'w, '_>) -> Self::Item<'w, 'static> {
        item
    }
}

impl ArchetypeQueryData for EntityMut<'_> {}

unsafe impl WorldQuery for FilteredEntityRef<'_, '_> {
    type Fetch<'w> = EntityFetch<'w>;

    type State = Access;

    fn shrink_fetch<'wlong: 'wshort, 'wshort>(fetch: Self::Fetch<'wlong>) -> Self::Fetch<'wshort> {
        fetch
    }

    unsafe fn init_fetch<'w, 's>(
        world: UnsafeWorldCell<'w>,
        _state: &'s Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Self::Fetch<'w> {
        EntityFetch {
            world,
            last_run,
            this_run,
        }
    }

    const IS_DENSE: bool = true;

    unsafe fn set_archetype<'w, 's>(
        _fetch: &mut Self::Fetch<'w>,
        _state: &'s Self::State,
        _archetype: &'w Archetype,
        _table: &'w Table,
    ) {
    }

    unsafe fn set_table<'w, 's>(
        _fetch: &mut Self::Fetch<'w>,
        _state: &'s Self::State,
        _table: &'w Table,
    ) {
    }

    fn update_component_access(state: &Self::State, filtered_access: &mut super::FilteredAccess) {
        assert!(
            filtered_access.access().is_compatible(state),
            "FilteredEntityRef conflicts with a previous access in this query. Exclusive access cannot coincide with any other accesses."
        );
        filtered_access.access.extend(state);
    }

    fn init_state(_world: &mut World) -> Self::State {
        Access::default()
    }

    fn get_state(_components: &Components) -> Option<Self::State> {
        Some(Access::default())
    }

    fn matches_component_set(
        _state: &Self::State,
        _set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        true
    }
}

// SAFETY: `Self` is the same as `Self::ReadOnly`
unsafe impl<'a, 'b> QueryData for FilteredEntityRef<'a, 'b> {
    const IS_READ_ONLY: bool = true;

    const IS_ARCHETYPAL: bool = true;

    type ReadOnly = Self;

    type Item<'w, 's> = FilteredEntityRef<'w, 's>;

    fn shrink<'wlong: 'wshort, 'wshort, 's>(
        item: Self::Item<'wlong, 's>,
    ) -> Self::Item<'wshort, 's> {
        item
    }

    #[inline]
    fn provide_extra_access(
        state: &mut Self::State,
        access: &mut Access,
        available_access: &Access,
    ) {
        // Claim any extra access that doesn't conflict with other subqueries
        // This is used when constructing a `QueryLens` or creating a query from a `QueryBuilder`
        // Start with the entire available access, since that is the most we can possibly access
        state.clone_from(available_access);
        // Prevent all writes, since `FilteredEntityRef` only performs read access
        state.clear_writes();
        // Prevent any access that would conflict with other accesses in the current query
        state.remove_conflicting_access(access);
        // Finally, add the resulting access to the query access
        // to make sure a later `FilteredEntityMut` won't conflict with this.
        access.extend(state);
    }

    #[inline(always)]
    unsafe fn fetch<'w, 's>(
        access: &'s Self::State,
        fetch: &mut Self::Fetch<'w>,
        entity: Entity,
        _table_row: TableRow,
    ) -> Option<Self::Item<'w, 's>> {
        // SAFETY: `fetch` must be called with an entity that exists in the world
        let cell = unsafe {
            fetch
                .world
                .get_entity_with_ticks(entity, fetch.last_run, fetch.this_run)
                .debug_checked_unwrap()
        };
        // SAFETY: mutable access to every component has been registered.
        Some(unsafe { FilteredEntityRef::new(cell, access) })
    }

    fn iter_access(state: &Self::State) -> impl Iterator<Item = EcsAccessType<'_>> {
        iter::once(EcsAccessType::Access(state))
    }
}

// SAFETY: access is read only and only on the current entity
unsafe impl IterQueryData for FilteredEntityRef<'_, '_> {}

// SAFETY: access is read only
unsafe impl ReadOnlyQueryData for FilteredEntityRef<'_, '_> {}

// SAFETY: access is only on the current entity
unsafe impl SingleEntityQueryData for FilteredEntityRef<'_, '_> {}

impl ArchetypeQueryData for FilteredEntityRef<'_, '_> {}

// SAFETY: The accesses of `Self::ReadOnly` are a subset of the accesses of `Self`
unsafe impl WorldQuery for FilteredEntityMut<'_, '_> {
    type Fetch<'w> = EntityFetch<'w>;

    type State = Access;

    fn shrink_fetch<'wlong: 'wshort, 'wshort>(fetch: Self::Fetch<'wlong>) -> Self::Fetch<'wshort> {
        fetch
    }

    unsafe fn init_fetch<'w, 's>(
        world: UnsafeWorldCell<'w>,
        _state: &'s Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Self::Fetch<'w> {
        EntityFetch {
            world,
            last_run,
            this_run,
        }
    }

    const IS_DENSE: bool = true;

    #[inline]
    unsafe fn set_archetype<'w, 's>(
        _fetch: &mut Self::Fetch<'w>,
        _state: &'s Self::State,
        _archetype: &'w Archetype,
        _table: &'w Table,
    ) {
    }

    #[inline]
    unsafe fn set_table<'w, 's>(
        _fetch: &mut Self::Fetch<'w>,
        _state: &'s Self::State,
        _table: &'w Table,
    ) {
    }

    fn update_component_access(state: &Self::State, filtered_access: &mut super::FilteredAccess) {
        assert!(
            filtered_access.access().is_compatible(state),
            "FilteredEntityMut conflicts with a previous access in the query. Exclusice access cannot coincide with any other accesses."
        );
        filtered_access.access.extend(state);
    }

    fn init_state(_world: &mut World) -> Self::State {
        Access::default()
    }

    fn get_state(_components: &Components) -> Option<Self::State> {
        Some(Access::default())
    }

    fn matches_component_set(
        _state: &Self::State,
        _set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        true
    }
}

// SAFETY: access of `FilteredEntityRef` is a subset of `FilteredEntityMut`
unsafe impl<'a, 'b> QueryData for FilteredEntityMut<'a, 'b> {
    const IS_READ_ONLY: bool = false;

    const IS_ARCHETYPAL: bool = true;

    type ReadOnly = FilteredEntityRef<'a, 'b>;

    type Item<'w, 's> = FilteredEntityMut<'w, 's>;

    fn shrink<'wlong: 'wshort, 'wshort, 's>(
        item: Self::Item<'wlong, 's>,
    ) -> Self::Item<'wshort, 's> {
        item
    }

    #[inline]
    fn provide_extra_access(
        state: &mut Self::State,
        access: &mut Access,
        available_access: &Access,
    ) {
        // Claim any extra access that doesn't conflict with other subqueries
        // This is used when constructing a `QueryLens` or creating a query from a `QueryBuilder`
        // Start with the entire available access, since that is the most we can possibly access
        state.clone_from(available_access);
        // Prevent any access that would conflict with other accesses in the current query
        state.remove_conflicting_access(access);
        // Finally, add the resulting access to the query access
        // to make sure a later `FilteredEntityRef` or `FilteredEntityMut` won't conflict with this.
        access.extend(state);
    }

    #[inline(always)]
    unsafe fn fetch<'w, 's>(
        access: &'s Self::State,
        fetch: &mut Self::Fetch<'w>,
        entity: Entity,
        _table_row: TableRow,
    ) -> Option<Self::Item<'w, 's>> {
        let cell = unsafe {
            fetch
                .world
                .get_entity_with_ticks(entity, fetch.last_run, fetch.this_run)
                .debug_checked_unwrap()
        };
        // SAFETY: mutable access to every component has been registered.
        Some(unsafe { FilteredEntityMut::new(cell, access) })
    }

    fn iter_access(state: &Self::State) -> impl Iterator<Item = EcsAccessType<'_>> {
        iter::once(EcsAccessType::Access(state))
    }
}

// SAFETY: access is only on the current entity
unsafe impl IterQueryData for FilteredEntityMut<'_, '_> {}

// SAFETY: access is only on the current entity
unsafe impl SingleEntityQueryData for FilteredEntityMut<'_, '_> {}

impl ArchetypeQueryData for FilteredEntityMut<'_, '_> {}

unsafe impl<'a, 'b, B> WorldQuery for EntityRefExcept<'a, 'b, B>
where
    B: Bundle,
{
    type Fetch<'w> = EntityFetch<'w>;

    type State = Access;

    fn shrink_fetch<'wlong: 'wshort, 'wshort>(fetch: Self::Fetch<'wlong>) -> Self::Fetch<'wshort> {
        fetch
    }

    unsafe fn init_fetch<'w, 's>(
        world: UnsafeWorldCell<'w>,
        _state: &'s Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Self::Fetch<'w> {
        EntityFetch {
            world,
            last_run,
            this_run,
        }
    }

    const IS_DENSE: bool = true;

    unsafe fn set_archetype<'w, 's>(
        _fetch: &mut Self::Fetch<'w>,
        _state: &'s Self::State,
        _archetype: &'w Archetype,
        _table: &'w Table,
    ) {
    }

    unsafe fn set_table<'w, 's>(
        _fetch: &mut Self::Fetch<'w>,
        _state: &'s Self::State,
        _table: &'w Table,
    ) {
    }

    fn update_component_access(state: &Self::State, filtered_access: &mut super::FilteredAccess) {
        let access = filtered_access.access_mut();
        assert!(
            access.is_compatible(state),
            "`EntityRefExcept<{}>` conflicts with a previous access in this query.",
            DebugName::type_name::<B>()
        );
        access.extend(state);
    }

    fn init_state(world: &mut World) -> Self::State {
        let mut access = Access::new();
        access.read_all();
        for id in B::component_ids(&mut world.components_registrator()) {
            access.remove_read(id);
        }
        access
    }

    fn get_state(components: &Components) -> Option<Self::State> {
        let mut access = Access::new();
        access.read_all();
        // If the component isn't registered, we don't have a `ComponentId`
        // to use to exclude its access.
        // Rather than fail, just try to take additional access.
        // This is sound because access checks will run on the resulting access.
        // Since the component isn't registered, there are no entities with that
        // component, and the extra access will usually have no effect.
        for id in B::get_component_ids(components).flatten() {
            access.remove_read(id);
        }
        Some(access)
    }

    fn matches_component_set(
        _state: &Self::State,
        _set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        true
    }
}

// SAFETY: `Self` is the same as `Self::ReadOnly`.
unsafe impl<'a, 'b, B> QueryData for EntityRefExcept<'a, 'b, B>
where
    B: Bundle,
{
    const IS_READ_ONLY: bool = true;

    const IS_ARCHETYPAL: bool = true;

    type ReadOnly = Self;

    type Item<'w, 's> = EntityRefExcept<'w, 's, B>;

    fn shrink<'wlong: 'wshort, 'wshort, 's>(
        item: Self::Item<'wlong, 's>,
    ) -> Self::Item<'wshort, 's> {
        item
    }

    unsafe fn fetch<'w, 's>(
        access: &'s Self::State,
        fetch: &mut Self::Fetch<'w>,
        entity: Entity,
        _table_row: TableRow,
    ) -> Option<Self::Item<'w, 's>> {
        // SAFETY: `fetch` must be called with an entity that exists in the world
        let cell = unsafe {
            fetch
                .world
                .get_entity_with_ticks(entity, fetch.last_run, fetch.this_run)
                .debug_checked_unwrap()
        };
        // SAFETY: mutable access to every component has been registered.
        Some(unsafe { EntityRefExcept::new(cell, access) })
    }

    fn iter_access(state: &Self::State) -> impl Iterator<Item = EcsAccessType<'_>> {
        iter::once(EcsAccessType::Access(state))
    }
}

// SAFETY: access is read only and only on the current entity
unsafe impl<B> IterQueryData for EntityRefExcept<'_, '_, B> where B: Bundle {}

// SAFETY: access is read only
unsafe impl<B> ReadOnlyQueryData for EntityRefExcept<'_, '_, B> where B: Bundle {}

// SAFETY: access is only on the current entity
unsafe impl<B> SingleEntityQueryData for EntityRefExcept<'_, '_, B> where B: Bundle {}

impl<B: Bundle> ArchetypeQueryData for EntityRefExcept<'_, '_, B> {}

// SAFETY: `EntityMutExcept` guards access to all components in the bundle `B`
// and populates `Access` values so that queries that conflict with this access
// are rejected.
unsafe impl<'a, 'b, B> WorldQuery for EntityMutExcept<'a, 'b, B>
where
    B: Bundle,
{
    type Fetch<'w> = EntityFetch<'w>;

    type State = Access;

    fn shrink_fetch<'wlong: 'wshort, 'wshort>(fetch: Self::Fetch<'wlong>) -> Self::Fetch<'wshort> {
        fetch
    }

    unsafe fn init_fetch<'w, 's>(
        world: UnsafeWorldCell<'w>,
        _state: &'s Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Self::Fetch<'w> {
        EntityFetch {
            world,
            last_run,
            this_run,
        }
    }

    const IS_DENSE: bool = true;

    unsafe fn set_archetype<'w, 's>(
        _fetch: &mut Self::Fetch<'w>,
        _state: &'s Self::State,
        _archetype: &'w Archetype,
        _table: &'w Table,
    ) {
    }

    unsafe fn set_table<'w, 's>(
        _fetch: &mut Self::Fetch<'w>,
        _state: &'s Self::State,
        _table: &'w Table,
    ) {
    }

    fn update_component_access(state: &Self::State, filtered_access: &mut super::FilteredAccess) {
        let access = filtered_access.access_mut();
        assert!(
            access.is_compatible(state),
            "`EntityMutExcept<{}>` conflicts with a previous access in this query.",
            DebugName::type_name::<B>()
        );
        access.extend(state);
    }

    fn init_state(world: &mut World) -> Self::State {
        let mut access = Access::new();
        access.write_all();
        for id in B::component_ids(&mut world.components_registrator()) {
            access.remove_read(id);
        }
        access
    }

    fn get_state(components: &Components) -> Option<Self::State> {
        let mut access = Access::new();
        access.write_all();
        // If the component isn't registered, we don't have a `ComponentId`
        // to use to exclude its access.
        // Rather than fail, just try to take additional access.
        // This is sound because access checks will run on the resulting access.
        // Since the component isn't registered, there are no entities with that
        // component, and the extra access will usually have no effect.
        for id in B::get_component_ids(components).flatten() {
            access.remove_read(id);
        }
        Some(access)
    }

    fn matches_component_set(
        _state: &Self::State,
        _set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        true
    }
}

// SAFETY: All accesses that `EntityRefExcept` provides are also accesses that
// `EntityMutExcept` provides.
unsafe impl<'a, 'b, B> QueryData for EntityMutExcept<'a, 'b, B>
where
    B: Bundle,
{
    const IS_READ_ONLY: bool = false;

    const IS_ARCHETYPAL: bool = true;

    type ReadOnly = EntityRefExcept<'a, 'b, B>;

    type Item<'w, 's> = EntityMutExcept<'w, 's, B>;

    fn shrink<'wlong: 'wshort, 'wshort, 's>(
        item: Self::Item<'wlong, 's>,
    ) -> Self::Item<'wshort, 's> {
        item
    }

    unsafe fn fetch<'w, 's>(
        access: &'s Self::State,
        fetch: &mut Self::Fetch<'w>,
        entity: Entity,
        _table_row: TableRow,
    ) -> Option<Self::Item<'w, 's>> {
        // SAFETY: `fetch` must be called with an entity that exists in the world
        let cell = unsafe {
            fetch
                .world
                .get_entity_with_ticks(entity, fetch.last_run, fetch.this_run)
                .debug_checked_unwrap()
        };
        // SAFETY: mutable access to every component has been registered.
        Some(unsafe { EntityMutExcept::new(cell, access) })
    }

    fn iter_access(state: &Self::State) -> impl Iterator<Item = EcsAccessType<'_>> {
        iter::once(EcsAccessType::Access(state))
    }
}

// SAFETY: access is only on the current entity
unsafe impl<B> IterQueryData for EntityMutExcept<'_, '_, B> where B: Bundle {}

// SAFETY: access is only on the current entity
unsafe impl<B> SingleEntityQueryData for EntityMutExcept<'_, '_, B> where B: Bundle {}

impl<B: Bundle> ArchetypeQueryData for EntityMutExcept<'_, '_, B> {}

// SAFETY:
// `update_component_access` does nothing.
// This is sound because `fetch` does not access components.
unsafe impl WorldQuery for &Archetype {
    type Fetch<'w> = (&'w Entities, &'w Archetypes);

    type State = ();

    fn shrink_fetch<'wlong: 'wshort, 'wshort>(fetch: Self::Fetch<'wlong>) -> Self::Fetch<'wshort> {
        fetch
    }

    unsafe fn init_fetch<'w, 's>(
        world: UnsafeWorldCell<'w>,
        _state: &'s Self::State,
        _last_run: Tick,
        _this_run: Tick,
    ) -> Self::Fetch<'w> {
        (world.entities(), world.archetypes())
    }

    // This could probably be a non-dense query and just set a Option<&Archetype> fetch value in
    // set_archetypes, but forcing archetypal iteration is likely to be slower in any compound query.
    const IS_DENSE: bool = true;

    #[inline]
    unsafe fn set_archetype<'w, 's>(
        _fetch: &mut Self::Fetch<'w>,
        _state: &'s Self::State,
        _archetype: &'w Archetype,
        _table: &'w Table,
    ) {
    }

    #[inline]
    unsafe fn set_table<'w, 's>(
        _fetch: &mut Self::Fetch<'w>,
        _state: &'s Self::State,
        _table: &'w Table,
    ) {
    }

    fn update_component_access(_state: &Self::State, _access: &mut super::FilteredAccess) {}

    fn init_state(_world: &mut World) -> Self::State {}

    fn get_state(_components: &Components) -> Option<Self::State> {
        Some(())
    }

    fn matches_component_set(
        _state: &Self::State,
        _set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        true
    }
}

unsafe impl QueryData for &Archetype {
    const IS_READ_ONLY: bool = true;

    const IS_ARCHETYPAL: bool = true;

    type ReadOnly = Self;

    type Item<'w, 's> = &'w Archetype;

    fn shrink<'wlong: 'wshort, 'wshort, 's>(
        item: Self::Item<'wlong, 's>,
    ) -> Self::Item<'wshort, 's> {
        item
    }

    #[inline(always)]
    unsafe fn fetch<'w, 's>(
        _state: &'s Self::State,
        fetch: &mut Self::Fetch<'w>,
        entity: Entity,
        _table_row: TableRow,
    ) -> Option<Self::Item<'w, 's>> {
        let (entities, archetypes) = *fetch;
        // SAFETY: `fetch` must be called with an entity that exists in the world
        let location = unsafe { entities.get_spawned(entity).debug_checked_unwrap() };
        // SAFETY: The assigned archetype for a living entity must always be valid.
        Some(unsafe { archetypes.get(location.archetype_id).debug_checked_unwrap() })
    }

    fn iter_access(_state: &Self::State) -> impl Iterator<Item = EcsAccessType<'_>> {
        iter::empty()
    }
}

// SAFETY: access is read only and only on the current entity
unsafe impl IterQueryData for &Archetype {}

// SAFETY: access is read only
unsafe impl ReadOnlyQueryData for &Archetype {}

// SAFETY: access is only on the current entity
unsafe impl SingleEntityQueryData for &Archetype {}

impl ReleaseStateQueryData for &Archetype {
    fn release_state<'w>(item: Self::Item<'w, '_>) -> Self::Item<'w, 'static> {
        item
    }
}

impl ArchetypeQueryData for &Archetype {}

/// The [`WorldQuery::Fetch`] type for `& T`.
pub struct ReadFetch<'w, T: Component> {
    components: StorageSwitch<
        T,
        // T::STORAGE_TYPE = StorageType::Table
        Option<ThinSlicePtr<'w, UnsafeCell<T>>>,
        // T::STORAGE_TYPE = StorageType::SparseSet
        Option<&'w ComponentSparseSet>,
    >,
}

impl<T: Component> Clone for ReadFetch<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: Component> Copy for ReadFetch<'_, T> {}

// SAFETY:
// `fetch` accesses a single component in a readonly way.
// This is sound because `update_component_access` adds read access for that component and panic when appropriate.
// `update_component_access` adds a `With` filter for a component.
// This is sound because `matches_component_set` returns whether the set contains that component.
unsafe impl<T: Component> WorldQuery for &T {
    type Fetch<'w> = ReadFetch<'w, T>;

    type State = ComponentId;

    fn shrink_fetch<'wlong: 'wshort, 'wshort>(fetch: Self::Fetch<'wlong>) -> Self::Fetch<'wshort> {
        fetch
    }

    #[inline]
    unsafe fn init_fetch<'w, 's>(
        world: UnsafeWorldCell<'w>,
        &component_id: &'s Self::State,
        _last_run: Tick,
        _this_run: Tick,
    ) -> Self::Fetch<'w> {
        ReadFetch {
            components: StorageSwitch::new(
                || None,
                || {
                    // SAFETY: The underlying type associated with `component_id` is `T`,
                    // which we are allowed to access since we registered it in `update_component_access`.
                    // Note that we do not actually access any components in this function, we just get a shared
                    // reference to the sparse set, which is used to access the components in `Self::fetch`.
                    unsafe { world.storages().sparse_sets.get(component_id) }
                },
            ),
        }
    }

    const IS_DENSE: bool = {
        match T::STORAGE_TYPE {
            StorageType::Table => true,
            StorageType::SparseSet => false,
        }
    };

    #[inline]
    unsafe fn set_archetype<'w, 's>(
        fetch: &mut Self::Fetch<'w>,
        component_id: &'s Self::State,
        _archetype: &'w Archetype,
        table: &'w Table,
    ) {
        if Self::IS_DENSE {
            unsafe {
                Self::set_table(fetch, component_id, table);
            }
        }
    }

    #[inline]
    unsafe fn set_table<'w, 's>(
        fetch: &mut Self::Fetch<'w>,
        component_id: &'s Self::State,
        table: &'w Table,
    ) {
        let table_data = Some(unsafe {
            table
                .get_data_slice_for(*component_id)
                .debug_checked_unwrap()
                .into()
        });
        // SAFETY: set_table is only called when T::STORAGE_TYPE = StorageType::Table
        unsafe {
            fetch.components.set_table(table_data);
        }
    }

    fn update_component_access(component_id: &Self::State, access: &mut super::FilteredAccess) {
        assert!(
            !access.access().has_write(*component_id),
            "&{} conflicts with a previous acces in this query. Shared access cannot coincide with exclusive access.",
            DebugName::type_name::<T>()
        );
        access.add_read(*component_id);
    }

    fn init_state(world: &mut World) -> Self::State {
        world.register_component::<T>()
    }

    fn get_state(components: &Components) -> Option<Self::State> {
        components.component_id::<T>()
    }

    fn matches_component_set(
        state: &Self::State,
        set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        set_contains_id(*state)
    }
}

// SAFETY: `Self` is the same as `Self::ReadOnly`
unsafe impl<T: Component> QueryData for &T {
    const IS_READ_ONLY: bool = true;

    const IS_ARCHETYPAL: bool = true;

    type ReadOnly = Self;

    type Item<'w, 's> = &'w T;

    fn shrink<'wlong: 'wshort, 'wshort, 's>(
        item: Self::Item<'wlong, 's>,
    ) -> Self::Item<'wshort, 's> {
        item
    }

    #[inline(always)]
    unsafe fn fetch<'w, 's>(
        _state: &'s Self::State,
        fetch: &mut Self::Fetch<'w>,
        entity: Entity,
        table_row: TableRow,
    ) -> Option<Self::Item<'w, 's>> {
        Some(fetch.components.extract(
            |table| {
                // SAFETY: set_table was previously called
                let table = unsafe { table.debug_checked_unwrap() };
                // SAFETY: Caller ensures `table_row` is in range.
                let item = unsafe { table.get_unchecked(table_row.index()) };
                unsafe { item.deref() }
            },
            |sparse_set| {
                let item = unsafe {
                    // SAFETY: Caller ensures `entity` is in range.
                    sparse_set
                        .debug_checked_unwrap()
                        .get(entity)
                        .debug_checked_unwrap()
                };
                unsafe { item.deref() }
            },
        ))
    }

    fn iter_access(state: &Self::State) -> impl Iterator<Item = EcsAccessType<'_>> {
        iter::once(EcsAccessType::Component(EcsAccessLevel::Read(*state)))
    }
}

impl<T: Component> ContiguousQueryData for &T {
    type Contiguous<'w, 's> = &'w [T];

    unsafe fn fetch_contiguous<'w, 's>(
        _state: &'s Self::State,
        fetch: &mut Self::Fetch<'w>,
        entities: &'w [Entity],
    ) -> Self::Contiguous<'w, 's> {
        fetch.components.extract(
            |table| {
                // SAFETY: The caller ensures `set_table` was previously called
                let table = unsafe { table.debug_checked_unwrap() };
                // SAFETY:
                // - `table` is `entities.len()` long
                // - `UnsafeCell<T>` has the same layout as `T`
                unsafe { table.cast().as_slice_unchecked(entities.len()) }
            },
            |_| {
                #[cfg(debug_assertions)]
                unreachable!();
                // SAFETY: The caller ensures query is dense
                #[cfg(not(debug_assertions))]
                core::hint::unreachable_unchecked();
            },
        )
    }
}

// SAFETY: access is read only and only on the current entity
unsafe impl<T: Component> IterQueryData for &T {}

// SAFETY: access is read only
unsafe impl<T: Component> ReadOnlyQueryData for &T {}

// SAFETY: access is only on the current entity
unsafe impl<T: Component> SingleEntityQueryData for &T {}

impl<T: Component> ReleaseStateQueryData for &T {
    fn release_state<'w>(item: Self::Item<'w, '_>) -> Self::Item<'w, 'static> {
        item
    }
}

impl<T: Component> ArchetypeQueryData for &T {}

#[doc(hidden)]
pub struct RefFetch<'w, T: Component> {
    components: StorageSwitch<
        T,
        // T::STORAGE_TYPE = StorageType::Table
        Option<(
            ThinSlicePtr<'w, UnsafeCell<T>>,
            ThinSlicePtr<'w, UnsafeCell<Tick>>,
            ThinSlicePtr<'w, UnsafeCell<Tick>>,
            MaybeLocation<ThinSlicePtr<'w, UnsafeCell<&'static Location<'static>>>>,
        )>,
        // T::STORAGE_TYPE = StorageType::SparseSet
        // Can be `None` when the component has never been inserted
        Option<&'w ComponentSparseSet>,
    >,
    last_run: Tick,
    this_run: Tick,
}

impl<T: Component> Clone for RefFetch<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: Component> Copy for RefFetch<'_, T> {}

// SAFETY:
// `fetch` accesses a single component in a readonly way.
// This is sound because `update_component_access` adds read access for that component and panic when appropriate.
// `update_component_access` adds a `With` filter for a component.
// This is sound because `matches_component_set` returns whether the set contains that component.
unsafe impl<'__w, T: Component> WorldQuery for Ref<'__w, T> {
    type Fetch<'w> = RefFetch<'w, T>;

    type State = ComponentId;

    fn shrink_fetch<'wlong: 'wshort, 'wshort>(fetch: Self::Fetch<'wlong>) -> Self::Fetch<'wshort> {
        fetch
    }

    #[inline]
    unsafe fn init_fetch<'w, 's>(
        world: UnsafeWorldCell<'w>,
        component_id: &'s Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Self::Fetch<'w> {
        RefFetch {
            components: StorageSwitch::new(
                || None,
                || {
                    // SAFETY: The underlying type associated with `component_id` is `T`,
                    // which we are allowed to access since we registered it in `update_component_access`.
                    // Note that we do not actually access any components in this function, we just get a shared
                    // reference to the sparse set, which is used to access the components in `Self::fetch`.
                    unsafe { world.storages().sparse_sets.get(*component_id) }
                },
            ),
            last_run,
            this_run,
        }
    }

    const IS_DENSE: bool = {
        match T::STORAGE_TYPE {
            StorageType::Table => true,
            StorageType::SparseSet => false,
        }
    };

    #[inline]
    unsafe fn set_archetype<'w, 's>(
        fetch: &mut Self::Fetch<'w>,
        component_id: &'s Self::State,
        _archetype: &'w Archetype,
        table: &'w Table,
    ) {
        if Self::IS_DENSE {
            unsafe {
                Self::set_table(fetch, component_id, table);
            }
        }
    }

    #[inline]
    unsafe fn set_table<'w, 's>(
        fetch: &mut Self::Fetch<'w>,
        component_id: &'s Self::State,
        table: &'w Table,
    ) {
        let column = unsafe { table.get_column(*component_id).debug_checked_unwrap() };
        let table_data = unsafe {
            Some((
                column.get_data_slice(table.entity_count() as usize).into(),
                column
                    .get_added_ticks_slice(table.entity_count() as usize)
                    .into(),
                column
                    .get_changed_ticks_slice(table.entity_count() as usize)
                    .into(),
                column
                    .get_changed_by_slice(table.entity_count() as usize)
                    .map(Into::into),
            ))
        };
        // SAFETY: set_table is only called when T::STORAGE_TYPE = StorageType::Table
        unsafe {
            fetch.components.set_table(table_data);
        }
    }

    fn update_component_access(component_id: &Self::State, access: &mut super::FilteredAccess) {
        assert!(
            !access.access().has_write(*component_id),
            "&{} conflicts with a previous access in this query. Shared access cannot coincide with exclusice access.",
            DebugName::type_name::<T>()
        );
        access.add_read(*component_id);
    }

    fn init_state(world: &mut World) -> Self::State {
        world.register_component::<T>()
    }

    fn get_state(components: &Components) -> Option<Self::State> {
        components.component_id::<T>()
    }

    fn matches_component_set(
        component_id: &Self::State,
        set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        set_contains_id(*component_id)
    }
}

// SAFETY: `Self` is the same as `Self::ReadOnly`
unsafe impl<'__w, T: Component> QueryData for Ref<'__w, T> {
    const IS_READ_ONLY: bool = true;

    const IS_ARCHETYPAL: bool = true;

    type ReadOnly = Self;

    type Item<'w, 's> = Ref<'w, T>;

    fn shrink<'wlong: 'wshort, 'wshort, 's>(
        item: Self::Item<'wlong, 's>,
    ) -> Self::Item<'wshort, 's> {
        item
    }

    #[inline(always)]
    unsafe fn fetch<'w, 's>(
        _state: &'s Self::State,
        fetch: &mut Self::Fetch<'w>,
        entity: Entity,
        table_row: TableRow,
    ) -> Option<Self::Item<'w, 's>> {
        Some(fetch.components.extract(
            |table| {
                // SAFETY: set_table was previously called
                let (table_components, added_ticks, changed_ticks, callers) =
                    unsafe { table.debug_checked_unwrap() };

                // SAFETY: The caller ensures `table_row` is in range.
                let component = unsafe { table_components.get_unchecked(table_row.index()) };
                // SAFETY: The caller ensures `table_row` is in range.
                let added = unsafe { added_ticks.get_unchecked(table_row.index()) };
                // SAFETY: The caller ensures `table_row` is in range.
                let changed = unsafe { changed_ticks.get_unchecked(table_row.index()) };
                // SAFETY: The caller ensures `table_row` is in range.
                let caller =
                    callers.map(|callers| unsafe { callers.get_unchecked(table_row.index()) });

                unsafe {
                    Ref {
                        value: component.deref(),
                        ticks: ComponentTicksRef {
                            added: added.deref(),
                            changed: changed.deref(),
                            changed_by: caller.map(|caller| caller.deref()),
                            this_run: fetch.this_run,
                            last_run: fetch.last_run,
                        },
                    }
                }
            },
            |sparse_set| {
                // SAFETY: The caller ensures `entity` is in range and has the component.
                let (component, ticks) = unsafe {
                    sparse_set
                        .debug_checked_unwrap()
                        .get_with_ticks(entity)
                        .debug_checked_unwrap()
                };

                unsafe {
                    Ref {
                        value: component.deref(),
                        ticks: ComponentTicksRef::from_tick_cells(
                            ticks,
                            fetch.last_run,
                            fetch.this_run,
                        ),
                    }
                }
            },
        ))
    }

    fn iter_access(state: &Self::State) -> impl Iterator<Item = EcsAccessType<'_>> {
        iter::once(EcsAccessType::Component(EcsAccessLevel::Read(*state)))
    }
}

// SAFETY: access is read only and only on the current entity
unsafe impl<'__w, T: Component> IterQueryData for Ref<'__w, T> {}

// SAFETY: access is read only
unsafe impl<'__w, T: Component> ReadOnlyQueryData for Ref<'__w, T> {}

// SAFETY: access is only on the current entity
unsafe impl<'__w, T: Component> SingleEntityQueryData for Ref<'__w, T> {}

impl<T: Component> ReleaseStateQueryData for Ref<'_, T> {
    fn release_state<'w>(item: Self::Item<'w, '_>) -> Self::Item<'w, 'static> {
        item
    }
}

impl<T: Component> ArchetypeQueryData for Ref<'_, T> {}

impl<T: Component> ContiguousQueryData for Ref<'_, T> {
    type Contiguous<'w, 's> = ContiguousRef<'w, T>;

    unsafe fn fetch_contiguous<'w, 's>(
        _state: &'s Self::State,
        fetch: &mut Self::Fetch<'w>,
        entities: &'w [Entity],
    ) -> Self::Contiguous<'w, 's> {
        fetch.components.extract(
            |table| {
                let (table_components, added_ticks, changed_ticks, callers) =
                    unsafe { table.debug_checked_unwrap() };

                ContiguousRef {
                    // SAFETY: `entities` has the same length as the rows in the set table.
                    value: unsafe { table_components.cast().as_slice_unchecked(entities.len()) },
                    // SAFETY:
                    // - The caller ensures the permission to access ticks.
                    // - `entities` has the same length as the rows in the set table hence the
                    // ticks.
                    ticks: unsafe {
                        ContiguousComponentTicksRef::from_slice_ptrs(
                            added_ticks,
                            changed_ticks,
                            callers,
                            entities.len(),
                            fetch.this_run,
                            fetch.last_run,
                        )
                    },
                }
            },
            |_| {
                #[cfg(debug_assertions)]
                unreachable!();
                // SAFETY: the caller ensures that [`Self::set_table`] was called beforehand.
                #[cfg(not(debug_assertions))]
                unsafe {
                    std::hint::unreachable_unchecked();
                }
            },
        )
    }
}

pub struct WriteFetch<'w, T: Component> {
    components: StorageSwitch<
        T,
        // T::STORAGE_TYPE = StorageType::Table
        Option<(
            ThinSlicePtr<'w, UnsafeCell<T>>,
            ThinSlicePtr<'w, UnsafeCell<Tick>>,
            ThinSlicePtr<'w, UnsafeCell<Tick>>,
            MaybeLocation<ThinSlicePtr<'w, UnsafeCell<&'static Location<'static>>>>,
        )>,
        // T::STORAGE_TYPE = StorageType::SparseSet
        // Can be `None` when the component has never been inserted
        Option<&'w ComponentSparseSet>,
    >,
    last_run: Tick,
    this_run: Tick,
}

impl<T: Component> Clone for WriteFetch<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: Component> Copy for WriteFetch<'_, T> {}

// SAFETY:
// `fetch` accesses a single component mutably.
// This is sound because `update_component_access` adds write access for that component and panic when appropriate.
// `update_component_access` adds a `With` filter for a component.
// This is sound because `matches_component_set` returns whether the set contains that component.
unsafe impl<'__w, T: Component> WorldQuery for &'__w mut T {
    type Fetch<'w> = WriteFetch<'w, T>;

    type State = ComponentId;

    fn shrink_fetch<'wlong: 'wshort, 'wshort>(fetch: Self::Fetch<'wlong>) -> Self::Fetch<'wshort> {
        fetch
    }

    #[inline]
    unsafe fn init_fetch<'w, 's>(
        world: UnsafeWorldCell<'w>,
        component_id: &'s Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Self::Fetch<'w> {
        WriteFetch {
            components: StorageSwitch::new(
                || None,
                || {
                    // SAFETY: The underlying type associated with `component_id` is `T`,
                    // which we are allowed to access since we registered it in `update_component_access`.
                    // Note that we do not actually access any components in this function, we just get a shared
                    // reference to the sparse set, which is used to access the components in `Self::fetch`.
                    unsafe { world.storages().sparse_sets.get(*component_id) }
                },
            ),
            last_run,
            this_run,
        }
    }

    const IS_DENSE: bool = {
        match T::STORAGE_TYPE {
            StorageType::Table => true,
            StorageType::SparseSet => false,
        }
    };

    #[inline]
    unsafe fn set_archetype<'w, 's>(
        fetch: &mut Self::Fetch<'w>,
        component_id: &'s Self::State,
        _archetype: &'w Archetype,
        table: &'w Table,
    ) {
        if Self::IS_DENSE {
            // SAFETY: `set_archetype`'s safety rules are a super set of the `set_table`'s ones.
            unsafe {
                Self::set_table(fetch, component_id, table);
            }
        }
    }

    #[inline]
    unsafe fn set_table<'w, 's>(
        fetch: &mut Self::Fetch<'w>,
        component_id: &'s Self::State,
        table: &'w Table,
    ) {
        let column = unsafe { table.get_column(*component_id).debug_checked_unwrap() };
        let table_data = unsafe {
            Some((
                column.get_data_slice(table.entity_count() as usize).into(),
                column
                    .get_added_ticks_slice(table.entity_count() as usize)
                    .into(),
                column
                    .get_changed_ticks_slice(table.entity_count() as usize)
                    .into(),
                column
                    .get_changed_by_slice(table.entity_count() as usize)
                    .map(Into::into),
            ))
        };
        // SAFETY: set_table is only called when T::STORAGE_TYPE = StorageType::Table
        unsafe {
            fetch.components.set_table(table_data);
        }
    }

    fn update_component_access(component_id: &Self::State, access: &mut super::FilteredAccess) {
        assert!(
            !access.access().has_read(*component_id),
            "&mut {} conflicts with a previous access in this query. Mutable component access must be unique.",
            DebugName::type_name::<T>()
        );
        access.add_write(*component_id);
    }

    fn init_state(world: &mut World) -> Self::State {
        world.register_component::<T>()
    }

    fn get_state(components: &Components) -> Option<Self::State> {
        components.component_id::<T>()
    }

    fn matches_component_set(
        component_id: &Self::State,
        set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        set_contains_id(*component_id)
    }
}

// SAFETY: access of `&T` is a subset of `&mut T`
unsafe impl<'__w, T: Component<Mutability = Mutable>> QueryData for &'__w mut T {
    const IS_READ_ONLY: bool = false;

    const IS_ARCHETYPAL: bool = true;

    type ReadOnly = &'__w T;

    type Item<'w, 's> = Mut<'w, T>;

    fn shrink<'wlong: 'wshort, 'wshort, 's>(
        item: Self::Item<'wlong, 's>,
    ) -> Self::Item<'wshort, 's> {
        item
    }

    #[inline(always)]
    unsafe fn fetch<'w, 's>(
        _state: &'s Self::State,
        fetch: &mut Self::Fetch<'w>,
        entity: Entity,
        table_row: TableRow,
    ) -> Option<Self::Item<'w, 's>> {
        Some(fetch.components.extract(
            |table| {
                let (table_components, added_ticks, changed_ticks, callers) =
                    unsafe { table.debug_checked_unwrap() };

                // SAFETY: The caller ensures `table_row` is in range.
                let component = unsafe { table_components.get_unchecked(table_row.index()) };
                // SAFETY: The caller ensures `table_row` is in range.
                let added = unsafe { added_ticks.get_unchecked(table_row.index()) };
                // SAFETY: The caller ensures `table_row` is in range.
                let changed = unsafe { changed_ticks.get_unchecked(table_row.index()) };
                // SAFETY: The caller ensures `table_row` is in range.
                let caller =
                    callers.map(|callers| unsafe { callers.get_unchecked(table_row.index()) });

                unsafe {
                    Mut {
                        value: component.deref_mut(),
                        ticks: ComponentTicksMut {
                            added: added.deref_mut(),
                            changed: changed.deref_mut(),
                            changed_by: caller.map(|caller| caller.deref_mut()),
                            this_run: fetch.this_run,
                            last_run: fetch.last_run,
                        },
                    }
                }
            },
            |sparse_set| {
                let (component, ticks) = unsafe {
                    // SAFETY: The caller ensures `entity` is in range and has the component.
                    sparse_set
                        .debug_checked_unwrap()
                        .get_with_ticks(entity)
                        .debug_checked_unwrap()
                };

                unsafe {
                    Mut {
                        value: component.assert_unique().deref_mut(),
                        ticks: ComponentTicksMut::from_tick_cells(
                            ticks,
                            fetch.last_run,
                            fetch.this_run,
                        ),
                    }
                }
            },
        ))
    }

    fn iter_access(state: &Self::State) -> impl Iterator<Item = EcsAccessType<'_>> {
        iter::once(EcsAccessType::Component(EcsAccessLevel::Write(*state)))
    }
}

// SAFETY: access is only on the current entity
unsafe impl<T: Component<Mutability = Mutable>> IterQueryData for &mut T {}

// SAFETY: access is only on the current entity
unsafe impl<T: Component<Mutability = Mutable>> SingleEntityQueryData for &mut T {}

impl<T: Component<Mutability = Mutable>> ReleaseStateQueryData for &mut T {
    fn release_state<'w>(item: Self::Item<'w, '_>) -> Self::Item<'w, 'static> {
        item
    }
}

impl<T: Component<Mutability = Mutable>> ArchetypeQueryData for &mut T {}

impl<T: Component<Mutability = Mutable>> ContiguousQueryData for &mut T {
    type Contiguous<'w, 's> = ContiguousMut<'w, T>;

    unsafe fn fetch_contiguous<'w, 's>(
        _state: &'s Self::State,
        fetch: &mut Self::Fetch<'w>,
        entities: &'w [Entity],
    ) -> Self::Contiguous<'w, 's> {
        fetch.components.extract(
            |table| {
                // SAFETY: set_table was previously called
                let (table_components, added_ticks, changed_ticks, callers) =
                    unsafe { table.debug_checked_unwrap() };

                ContiguousMut {
                    // SAFETY: `entities` has the same length as the rows in the set table.
                    value: unsafe { table_components.as_mut_slice_unchecked(entities.len()) },
                    // SAFETY:
                    // - The caller ensures the permission to access ticks.
                    // - `entities` has the same length as the rows in the set table hence the
                    // ticks.
                    ticks: unsafe {
                        ContiguousComponentTicksMut::from_slice_ptrs(
                            added_ticks,
                            changed_ticks,
                            callers,
                            entities.len(),
                            fetch.this_run,
                            fetch.last_run,
                        )
                    },
                }
            },
            |_| {
                #[cfg(debug_assertions)]
                unreachable!();
                // SAFETY: the caller ensures that [`Self::set_table`] was called beforehand.
                #[cfg(not(debug_assertions))]
                unsafe {
                    std::hint::unreachable_unchecked();
                }
            },
        )
    }
}

/// When `Mut<T>` is used in a query, it will be converted to `Ref<T>` when transformed into its read-only form, providing access to change detection methods.
///
/// By contrast `&mut T` will result in a `Mut<T>` item in mutable form to record mutations, but result in a bare `&T` in read-only form.
//
// SAFETY:
// `fetch` accesses a single component mutably.
// This is sound because `update_component_access` adds write access for that component and panic when appropriate.
// `update_component_access` adds a `With` filter for a component.
// This is sound because `matches_component_set` returns whether the set contains that component.
unsafe impl<'__w, T: Component> WorldQuery for Mut<'__w, T> {
    type Fetch<'w> = WriteFetch<'w, T>;

    type State = ComponentId;

    fn shrink_fetch<'wlong: 'wshort, 'wshort>(fetch: Self::Fetch<'wlong>) -> Self::Fetch<'wshort> {
        fetch
    }

    #[inline]
    // Forwarded to `&mut T`
    unsafe fn init_fetch<'w, 's>(
        world: UnsafeWorldCell<'w>,
        state: &'s Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Self::Fetch<'w> {
        unsafe { <&mut T as WorldQuery>::init_fetch(world, state, last_run, this_run) }
    }

    // Forwarded to `&mut T`
    const IS_DENSE: bool = <&mut T as WorldQuery>::IS_DENSE;

    #[inline]
    // Forwarded to `&mut T`
    unsafe fn set_archetype<'w, 's>(
        fetch: &mut Self::Fetch<'w>,
        state: &'s Self::State,
        archetype: &'w Archetype,
        table: &'w Table,
    ) {
        unsafe {
            <&mut T as WorldQuery>::set_archetype(fetch, state, archetype, table);
        }
    }

    #[inline]
    // Forwarded to `&mut T`
    unsafe fn set_table<'w, 's>(
        fetch: &mut Self::Fetch<'w>,
        state: &'s Self::State,
        table: &'w Table,
    ) {
        unsafe {
            <&mut T as WorldQuery>::set_table(fetch, state, table);
        }
    }

    // NOT forwarded to `&mut T`
    fn update_component_access(component_id: &Self::State, access: &mut super::FilteredAccess) {
        // Update component access here instead of in `<&mut T as WorldQuery>` to avoid erroneously referencing
        // `&mut T` in error message.
        assert!(
            !access.access().has_read(*component_id),
            "Mut<{}> conflicts with a previous access in this query. Mutable component access mut be unique.",
            DebugName::type_name::<T>()
        );
        access.add_write(*component_id);
    }

    // Forwarded to `&mut T`
    fn init_state(world: &mut World) -> Self::State {
        <&mut T as WorldQuery>::init_state(world)
    }

    // Forwarded to `&mut T`
    fn get_state(components: &Components) -> Option<Self::State> {
        <&mut T as WorldQuery>::get_state(components)
    }

    // Forwarded to `&mut T`
    fn matches_component_set(
        state: &Self::State,
        set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        <&mut T as WorldQuery>::matches_component_set(state, set_contains_id)
    }
}

// SAFETY: access of `Ref<T>` is a subset of `Mut<T>`
unsafe impl<'__w, T: Component<Mutability = Mutable>> QueryData for Mut<'__w, T> {
    const IS_READ_ONLY: bool = false;

    const IS_ARCHETYPAL: bool = true;

    type ReadOnly = Ref<'__w, T>;

    type Item<'w, 's> = Mut<'w, T>;

    // Forwarded to `&mut T`
    fn shrink<'wlong: 'wshort, 'wshort, 's>(
        item: Self::Item<'wlong, 's>,
    ) -> Self::Item<'wshort, 's> {
        <&mut T as QueryData>::shrink(item)
    }

    #[inline(always)]
    // Forwarded to `&mut T`
    unsafe fn fetch<'w, 's>(
        state: &'s Self::State,
        // Rust complains about lifetime bounds not matching the trait if I directly use `WriteFetch<'w, T>` right here.
        // But it complains nowhere else in the entire trait implementation.
        fetch: &mut Self::Fetch<'w>,
        entity: Entity,
        table_row: TableRow,
    ) -> Option<Self::Item<'w, 's>> {
        unsafe { <&mut T as QueryData>::fetch(state, fetch, entity, table_row) }
    }

    fn iter_access(state: &Self::State) -> impl Iterator<Item = EcsAccessType<'_>> {
        iter::once(EcsAccessType::Component(EcsAccessLevel::Write(*state)))
    }
}

// SAFETY: access is only on the current entity
unsafe impl<T: Component<Mutability = Mutable>> IterQueryData for Mut<'_, T> {}

// SAFETY: access is only on the current entity
unsafe impl<T: Component<Mutability = Mutable>> SingleEntityQueryData for Mut<'_, T> {}

impl<T: Component<Mutability = Mutable>> ReleaseStateQueryData for Mut<'_, T> {
    fn release_state<'w>(item: Self::Item<'w, '_>) -> Self::Item<'w, 'static> {
        item
    }
}

impl<T: Component<Mutability = Mutable>> ArchetypeQueryData for Mut<'_, T> {}

impl<'__w, T: Component<Mutability = Mutable>> ContiguousQueryData for Mut<'__w, T> {
    type Contiguous<'w, 's> = ContiguousMut<'w, T>;

    unsafe fn fetch_contiguous<'w, 's>(
        state: &'s Self::State,
        fetch: &mut Self::Fetch<'w>,
        entities: &'w [Entity],
    ) -> Self::Contiguous<'w, 's> {
        unsafe { <&mut T as ContiguousQueryData>::fetch_contiguous(state, fetch, entities) }
    }
}

/// A helper type for accessing a [`Query`] within a [`QueryData`].
///
/// This is intended to be used inside other implementations of [`QueryData`],
/// either for manual implementations or `#[derive(QueryData)]`.
/// It is not normally useful to query directly,
/// since it's equivalent to adding another [`Query`] parameter to a system.
///
/// Note that this requires the inner query to be a [`ReadOnlyQueryData`]
/// to prevent mutable aliasing.
///
/// ```
/// # use bevy_ecs::prelude::*;
/// # use bevy_ecs::query::NestedQuery;
/// #
/// # #[derive(Component)]
/// # struct A;
/// fn system(mut query: Query<NestedQuery<&A>>) {
///     // This works, because it performs read-only iteration
///     for a in &query {
///         let a: Query<&A> = a;
///     }
/// }
/// ```
///
/// ```compile_fail
/// # use bevy_ecs::prelude::*;
/// # use bevy_ecs::query::NestedQuery;
/// #
/// # #[derive(Component)]
/// # struct A;
/// fn system(mut query: Query<NestedQuery<&mut A>>) {
///     // This fails, because it would allow mutable aliasing of `&mut A`
///     for a in &mut query {
///         let a: Query<&mut A> = a;
///     }
/// }
/// ```
///
/// # Example
///
/// The simplest way to use a `NestedQuery` is with a `#[derive(QueryData)]` struct.
/// The `Query` will be available on the generated `Item` struct,
/// and we can use the query in methods on that struct.
///
/// ```
/// # use bevy_ecs::prelude::*;
/// # use bevy_ecs::query::{NestedQuery, QueryData, QueryFilter, ReadOnlyQueryData};
/// #
/// # #[derive(Component)]
/// # struct Data(usize);
/// #
/// # let mut world = World::new();
/// #
/// // We want to create a relational query data
/// // that lets us query components on an entity's parent,
/// // like this:
/// let root = world.spawn(Data(3)).id();
/// let child = world.spawn(ChildOf(root)).id();
///
/// let mut query = world.query::<Parent<&Data>>();
/// let &Data(data) = query.query(&mut world).get(child).unwrap().data().unwrap();
/// assert_eq!(data, 3);
///
/// // We derive a query data struct that contains the relation plus a `NestedQuery`
/// #[derive(QueryData)]
/// struct Parent<D: ReadOnlyQueryData + 'static, F: QueryFilter + 'static = ()> {
///     // This will query `ChildOf` on the entity itself,
///     // so we can find the parent entity
///     parent: &'static ChildOf,
///     // This will provide a `Query` that we can use to
///     // query data on the parent entity
///     nested_query: NestedQuery<D, F>,
/// }
///
/// // And add a method on the generated item struct to invoke the nested query.
/// impl<'w, 's, D: ReadOnlyQueryData + 'static, F: QueryFilter + 'static> ParentItem<'w, 's, D, F> {
///     fn data(&self) -> Option<D::Item<'w, 's>> {
///         // We need to use `_inner` methods to return the full `'w` lifetime.
///         self.nested_query.get_inner(self.parent.parent()).ok()
///     }
/// }
/// ```
///
/// In order to make a query that returns the inner query data directly,
/// instead of through an intermediate `Item` struct,
/// you can implement `QueryData` manually by delegating to `NestedQuery`.
///
/// ```
/// # use bevy_ecs::{
/// #     archetype::Archetype,
/// #     change_detection::Tick,
/// #     component::{ComponentId, Components},
/// #     prelude::*,
/// #     query::{
/// #         EcsAccessType, FilteredAccess, IterQueryData, NestedQuery, QueryData, QueryFilter,
/// #         ReadOnlyQueryData, ReleaseStateQueryData, WorldQuery,
/// #     },
/// #     storage::{Table, TableRow},
/// #     world::unsafe_world_cell::UnsafeWorldCell,
/// # };
/// #
/// # #[derive(Component)]
/// # struct Data(usize);
/// #
/// # let mut world = World::new();
/// #
/// // We want to create a relational query data
/// // that lets us query components on an entity's parent,
/// // like this:
/// let root = world.spawn(Data(3)).id();
/// let child = world.spawn(ChildOf(root)).id();
///
/// let mut query = world.query::<Parent<&Data>>();
/// let &Data(data) = query.query(&mut world).get(child).unwrap();
/// assert_eq!(data, 3);
///
/// // This is the relational query data.
/// // This will never actually be constructed,
/// // and is only used as a `QueryData` type.
/// pub struct Parent<D: ReadOnlyQueryData, F: QueryFilter = ()>(D, F);
///
/// // A type alias to delegate the `QueryData` impls to.
/// // We need to refer to this type a lot, so the alias will help.
/// // This could also be a `#[derive(QueryData)]` type.
/// type ParentInner<D, F> = (
///     // This will query `ChildOf` on the entity itself,
///     // so we can find the parent entity
///     &'static ChildOf,
///     // This will provide a `Query` that we can use to
///     // query data on the parent entity
///     NestedQuery<D, F>,
/// );
///
/// unsafe impl<D: ReadOnlyQueryData + 'static, F: QueryFilter + 'static> QueryData for Parent<D, F> {
///     // Set `Item` to what we need for this relational query.
///     // Here we use the output of `D`.
///     type Item<'w, 's> = D::Item<'w, 's>;
///
///     unsafe fn fetch<'w, 's>(state: &'s Self::State, fetch: &mut Self::Fetch<'w>, entity: Entity, table_row: TableRow) -> Option<Self::Item<'w, 's>> {
///         // In `fetch`, first delegate to the type alias to get the parts:
///         let (&ChildOf(parent), nested_query) =
///             <ParentInner<D, F> as QueryData>::fetch(state, fetch, entity, table_row)?;
///         // Then use the `NestedQuery` to get the data we need.
///         // We need to use `_inner` methods to return the full `'w` lifetime.
///         nested_query.get_inner(parent).ok()
///     }
///
///     fn shrink<'wlong: 'wshort, 'wshort, 's>(item: Self::Item<'wlong, 's>) -> Self::Item<'wshort, 's> {
///         D::shrink(item)
///     }
///
///     // Set `ReadOnly` to `Self`,
///     // as `NestedQuery` does not yet support mutable queries.
///     type ReadOnly = Self;
///
///     // Delegate everything else on `QueryData` and `WorldQuery` to the type alias.
///     // This is sound for `unsafe` items because they delegate to the
///     // sound implementations on the type alias.
///     const IS_READ_ONLY: bool = <ParentInner<D, F> as QueryData>::IS_READ_ONLY;
///     const IS_ARCHETYPAL: bool = <ParentInner<D, F> as QueryData>::IS_ARCHETYPAL;
///
///     fn iter_access(state: &Self::State) -> impl Iterator<Item = EcsAccessType<'_>> {
///         <ParentInner<D, F> as QueryData>::iter_access(state)
///     }
/// }
///
/// unsafe impl<D: ReadOnlyQueryData + 'static, F: QueryFilter + 'static> WorldQuery for Parent<D, F> {
///     type Fetch<'w> = <ParentInner<D, F> as WorldQuery>::Fetch<'w>;
///     type State = <ParentInner<D, F> as WorldQuery>::State;
///
///     fn shrink_fetch<'wlong: 'wshort, 'wshort>(fetch: Self::Fetch<'wlong>) -> Self::Fetch<'wshort> {
///         <ParentInner<D, F> as WorldQuery>::shrink_fetch(fetch)
///     }
///
///     unsafe fn init_fetch<'w, 's>(world: UnsafeWorldCell<'w>, state: &'s Self::State, last_run: Tick, this_run: Tick) -> Self::Fetch<'w> {
///         <ParentInner<D, F> as WorldQuery>::init_fetch(world, state, last_run, this_run)
///     }
///
///     const IS_DENSE: bool = <ParentInner<D, F> as WorldQuery>::IS_DENSE;
///
///     unsafe fn set_archetype<'w, 's>(fetch: &mut Self::Fetch<'w>, state: &'s Self::State, archetype: &'w Archetype, table: &'w Table) {
///         <ParentInner<D, F> as WorldQuery>::set_archetype(fetch, state, archetype, table)
///     }
///
///     unsafe fn set_table<'w, 's>(fetch: &mut Self::Fetch<'w>, state: &'s Self::State, table: &'w Table) {
///         <ParentInner<D, F> as WorldQuery>::set_table(fetch, state, table)
///     }
///
///     fn update_component_access(state: &Self::State, access: &mut FilteredAccess) {
///         <ParentInner<D, F> as WorldQuery>::update_component_access(state, access)
///     }
///
///     fn init_state(world: &mut World) -> Self::State {
///         <ParentInner<D, F> as WorldQuery>::init_state(world)
///     }
///
///     fn get_state(components: &Components) -> Option<Self::State> {
///         <ParentInner<D, F> as WorldQuery>::get_state(components)
///     }
///
///     fn matches_component_set(state: &Self::State, set_contains_id: &impl Fn(ComponentId) -> bool) -> bool {
///         <ParentInner<D, F> as WorldQuery>::matches_component_set(state, set_contains_id)
///     }
/// }
///
/// // Also impl `ReadOnlyQueryData`, `IterQueryData`, and `ReleaseStateQueryData`
/// // These are safe because they delegate to the type alias, which is also read-only.
/// // Do *not* impl `ArchetypeQueryData`, because `fetch` sometimes returns `None`,
/// // and do *not* impl `SingleEntityQueryData`, because `NestedQuery` accesses other entities.
/// unsafe impl<D: ReadOnlyQueryData + 'static, F: QueryFilter + 'static> ReadOnlyQueryData for Parent<D, F> {}
///
/// unsafe impl<D: ReadOnlyQueryData + 'static, F: QueryFilter + 'static> IterQueryData for Parent<D, F> {}
///
/// impl<D: ReadOnlyQueryData + ReleaseStateQueryData + 'static, F: QueryFilter + 'static>
///     ReleaseStateQueryData for Parent<D, F>
/// {
///     fn release_state<'w>(item: Self::Item<'w, '_>) -> Self::Item<'w, 'static> {
///         D::release_state(item)
///     }
/// }
/// ```
pub struct NestedQuery<D: QueryData + 'static, F: QueryFilter + 'static = ()>(
    PhantomData<Query<'static, 'static, D, F>>,
);

#[doc(hidden)]
#[derive(Clone)]
pub struct NestedQueryFetch<'w> {
    world: UnsafeWorldCell<'w>,
    last_run: Tick,
    this_run: Tick,
}

unsafe impl<D: ReadOnlyQueryData + 'static, F: QueryFilter + 'static> WorldQuery
    for NestedQuery<D, F>
{
    type Fetch<'w> = NestedQueryFetch<'w>;

    type State = QueryState<D, F>;

    fn shrink_fetch<'wlong: 'wshort, 'wshort>(fetch: Self::Fetch<'wlong>) -> Self::Fetch<'wshort> {
        fetch
    }

    #[inline]
    unsafe fn init_fetch<'w, 's>(
        world: UnsafeWorldCell<'w>,
        state: &'s Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Self::Fetch<'w> {
        NestedQueryFetch {
            world,
            last_run,
            this_run,
        }
    }

    const IS_DENSE: bool = true;

    #[inline]
    unsafe fn set_archetype<'w, 's>(
        _fetch: &mut Self::Fetch<'w>,
        _state: &'s Self::State,
        _archetype: &'w Archetype,
        _table: &'w Table,
    ) {
    }

    #[inline]
    unsafe fn set_table<'w, 's>(
        _fetch: &mut Self::Fetch<'w>,
        _state: &'s Self::State,
        _table: &'w Table,
    ) {
    }

    fn update_component_access(_state: &Self::State, _access: &mut super::FilteredAccess) {
        // This performs no access on the current entity
        // Access to the nested query is checked through `init_nested_access`
    }

    fn init_nested_access(
        state: &Self::State,
        system_name: Option<&str>,
        component_access_set: &mut super::FilteredAccessSet,
        world: UnsafeWorldCell,
    ) {
        todo!()
    }

    fn init_state(world: &mut World) -> Self::State {
        todo!()
    }

    fn get_state(components: &Components) -> Option<Self::State> {
        todo!()
    }

    fn matches_component_set(
        state: &Self::State,
        set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        todo!()
    }
}

/// A compile-time checked union of two different types that differs based on the
/// [`StorageType`] of a given component.
pub(super) union StorageSwitch<C: Component, T: Copy, S: Copy> {
    /// The table variant. Requires the component to be a table component.
    table: T,
    /// The sparse set variant. Requires the component to be a sparse set component.
    sparse_set: S,
    _marker: PhantomData<C>,
}

impl<C: Component, T: Copy, S: Copy> StorageSwitch<C, T, S> {
    /// Creates a new [`StorageSwitch`] using the given closures to initialize
    /// the variant corresponding to the component's [`StorageType`].
    pub fn new(table: impl FnOnce() -> T, sparse_set: impl FnOnce() -> S) -> Self {
        match C::STORAGE_TYPE {
            StorageType::Table => Self { table: table() },
            StorageType::SparseSet => Self {
                sparse_set: sparse_set(),
            },
        }
    }

    /// Creates a new [`StorageSwitch`] using a table variant.
    ///
    /// # Panics
    ///
    /// This will panic on debug builds if `C` is not a table component.
    ///
    /// # Safety
    ///
    /// `C` must be a table component.
    #[inline]
    pub unsafe fn set_table(&mut self, table: T) {
        match C::STORAGE_TYPE {
            StorageType::Table => self.table = table,
            _ => {
                #[cfg(debug_assertions)]
                unreachable!();
                #[cfg(not(debug_assertions))]
                unsafe {
                    std::hint::unreachable_unchecked()
                }
            }
        }
    }

    pub fn extract<R>(&self, table: impl FnOnce(T) -> R, sparse_set: impl FnOnce(S) -> R) -> R {
        match C::STORAGE_TYPE {
            StorageType::Table => table(
                // SAFETY: C::STORAGE_TYPE == StorageType::Table
                unsafe { self.table },
            ),
            StorageType::SparseSet => sparse_set(
                // SAFETY: C::STORAGE_TYPE == StorageType::SparseSet
                unsafe { self.sparse_set },
            ),
        }
    }
}

impl<C: Component, T: Copy, S: Copy> Clone for StorageSwitch<C, T, S> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<C: Component, T: Copy, S: Copy> Copy for StorageSwitch<C, T, S> {}

/// [`WorldQuery`] used to nullify queries by turning `Query<D>` into `Query<NopWorldQuery<D>>`
///
/// This will rarely be useful to consumers of `bevy_ecs`.
pub(crate) struct NopWorldQuery<D: QueryData>(PhantomData<D>);

unsafe impl<D: QueryData> WorldQuery for NopWorldQuery<D> {
    type Fetch<'w> = ();

    type State = D::State;

    fn shrink_fetch<'wlong: 'wshort, 'wshort>(_fetch: Self::Fetch<'wlong>) -> Self::Fetch<'wshort> {
    }

    #[inline(always)]
    unsafe fn init_fetch<'w, 's>(
        _world: UnsafeWorldCell<'w>,
        _state: &'s Self::State,
        _last_run: Tick,
        _this_run: Tick,
    ) -> Self::Fetch<'w> {
    }

    const IS_DENSE: bool = D::IS_DENSE;

    #[inline(always)]
    unsafe fn set_archetype<'w, 's>(
        _fetch: &mut Self::Fetch<'w>,
        _state: &'s Self::State,
        _archetype: &'w Archetype,
        _table: &'w Table,
    ) {
    }

    #[inline(always)]
    unsafe fn set_table<'w, 's>(
        _fetch: &mut Self::Fetch<'w>,
        _state: &'s Self::State,
        _table: &'w Table,
    ) {
    }

    fn update_component_access(state: &Self::State, access: &mut super::FilteredAccess) {}

    fn init_state(world: &mut World) -> Self::State {
        D::init_state(world)
    }

    fn get_state(components: &Components) -> Option<Self::State> {
        D::get_state(components)
    }

    fn matches_component_set(
        state: &Self::State,
        set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        D::matches_component_set(state, set_contains_id)
    }

    fn update_archetypes(state: &mut Self::State, world: UnsafeWorldCell) {
        D::update_archetypes(state, world);
    }
}

// SAFETY: `Self::ReadOnly` is `Self`
unsafe impl<D: QueryData> QueryData for NopWorldQuery<D> {
    const IS_READ_ONLY: bool = true;

    const IS_ARCHETYPAL: bool = true;

    type ReadOnly = Self;

    type Item<'w, 's> = ();

    fn shrink<'wlong: 'wshort, 'wshort, 's>(
        _item: Self::Item<'wlong, 's>,
    ) -> Self::Item<'wshort, 's> {
    }

    #[inline(always)]
    unsafe fn fetch<'w, 's>(
        _state: &'s Self::State,
        _fetch: &mut Self::Fetch<'w>,
        _entity: Entity,
        _table_row: TableRow,
    ) -> Option<Self::Item<'w, 's>> {
        Some(())
    }

    fn iter_access(_state: &Self::State) -> impl Iterator<Item = EcsAccessType<'_>> {
        iter::empty()
    }
}

// SAFETY: `NopFetch` never accesses any data
unsafe impl<D: QueryData> IterQueryData for NopWorldQuery<D> {}

// SAFETY: `NopFetch` never accesses any data
unsafe impl<D: QueryData> ReadOnlyQueryData for NopWorldQuery<D> {}

// SAFETY: `NopFetch` never accesses any data
unsafe impl<D: QueryData> SingleEntityQueryData for NopWorldQuery<D> {}

impl<D: QueryData> ReleaseStateQueryData for NopWorldQuery<D> {
    fn release_state<'w>(_item: Self::Item<'w, '_>) -> Self::Item<'w, 'static> {}
}

impl<D: QueryData> ArchetypeQueryData for NopWorldQuery<D> {}

// TODO!
