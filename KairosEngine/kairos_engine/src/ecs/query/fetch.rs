use std::iter;

use crate::{
    debug::{DebugCheckedUnwrap, MaybeLocation},
    ecs::{
        archetype::Archetype,
        change_detection::Tick,
        component::{ComponentId, Components},
        entity::{Entities, Entity, EntityLocation},
        query::{Access, EcsAccessLevel, EcsAccessType, WorldQuery},
        storage::{Table, TableRow},
        world::{
            EntityMut, EntityRef, FilteredEntityRef, World, unsafe_world_cell::UnsafeWorldCell,
        },
    },
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

    fn shrink_fetch<'wlong: 'wshort, 'wshort>(fethc: Self::Fetch<'wlong>) -> Self::Fetch<'wshort> {}

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
        _fethc: &mut Self::Fetch<'w>,
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
        _fethc: &mut Self::Fetch<'w>,
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
        _fethc: &mut Self::Fetch<'w>,
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

    fn iter_access(state: &Self::State) -> impl Iterator<Item = EcsAccessType<'_>> {
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
        fetch: &mut Self::Fetch<'w>,
        state: &'s Self::State,
        archetype: &'w Archetype,
        table: &'w Table,
    ) {
    }

    #[inline]
    unsafe fn set_table<'w, 's>(
        _fethc: &mut Self::Fetch<'w>,
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
        state: &'s Self::State,
        fetch: &mut Self::Fetch<'w>,
        entity: Entity,
        table_row: TableRow,
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

    fn iter_access(state: &Self::State) -> impl Iterator<Item = EcsAccessType<'_>> {
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
        _fethc: &mut Self::Fetch<'w>,
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
        table_row: TableRow,
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

    fn iter_access(state: &Self::State) -> impl Iterator<Item = EcsAccessType<'_>> {
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
        todo!()
    }

    unsafe fn init_fetch<'w, 's>(
        world: UnsafeWorldCell<'w>,
        state: &'s Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Self::Fetch<'w> {
        todo!()
    }

    const IS_DENSE: bool = true;

    unsafe fn set_archetype<'w, 's>(
        fetch: &mut Self::Fetch<'w>,
        state: &'s Self::State,
        archetype: &'w Archetype,
        table: &'w Table,
    ) {
        todo!()
    }

    unsafe fn set_table<'w, 's>(
        fethc: &mut Self::Fetch<'w>,
        state: &'s Self::State,
        table: &'w Table,
    ) {
        todo!()
    }

    fn update_component_access(state: &Self::State, access: &mut super::FilteredAccess) {
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

// TODO!
