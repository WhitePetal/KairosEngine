use crate::ecs::query::WorldQuery;


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

    /// The item returned by this [`WorldQuery`]
    /// This will be the data retrieved by the query,
    /// and is visible to the end user when calling e.g. `Query<Self>::get`.
    type Item<'w, 's>;
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

/// A [`QueryData`] that only accesses data from the current entity, the one passed to [`QueryData::fetch`].
///
/// This is used as a bound in [`EntityRef::get_components`] and related APIs,
/// since they only have access to a single entity.
///
/// # Safety
///
/// This [`QueryData`] must only access data from the current entity, and not any other entities.
pub unsafe trait SingleEntityQueryData: IterQueryData {}


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

// TODO!
