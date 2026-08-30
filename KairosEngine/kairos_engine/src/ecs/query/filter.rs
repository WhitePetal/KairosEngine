use std::marker::PhantomData;

use wgpu::wgc::id;

use crate::ecs::{component::{Component, ComponentId, StorageType}, entity::Entity, query::WorldQuery, storage::TableRow};

/// Types that filter the results of a [`Query`].
///
/// There are many types that natively implement this trait:
/// - **Component filters.**
///   [`With`] and [`Without`] filters can be applied to check if the queried entity does or does not contain a particular component.
/// - **Change detection filters.**
///   [`Added`] and [`Changed`] filters can be applied to detect component changes to an entity.
/// - **Spawned filter.**
///   [`Spawned`] filter can be applied to check if the queried entity was spawned recently.
/// - **`QueryFilter` tuples.**
///   If every element of a tuple implements `QueryFilter`, then the tuple itself also implements the same trait.
///   This enables a single `Query` to filter over multiple conditions.
///   Due to the current lack of variadic generics in Rust, the trait has been implemented for tuples from 0 to 15 elements,
///   but nesting of tuples allows infinite `QueryFilter`s.
/// - **Filter disjunction operator.**
///   By default, tuples compose query filters in such a way that all conditions must be satisfied to generate a query item for a given entity.
///   Wrapping a tuple inside an [`Or`] operator will relax the requirement to just one condition.
///
/// Implementing the trait manually can allow for a fundamentally new type of behavior.
///
/// Query design can be easily structured by deriving `QueryFilter` for custom types.
/// Despite the added complexity, this approach has several advantages over using `QueryFilter` tuples.
/// The most relevant improvements are:
///
/// - Reusability across multiple systems.
/// - Filters can be composed together to create a more complex filter.
///
/// This trait can only be derived for structs if each field also implements `QueryFilter`.
///
/// ```
/// # use bevy_ecs::prelude::*;
/// # use bevy_ecs::{query::QueryFilter, component::Component};
/// #
/// # #[derive(Component)]
/// # struct ComponentA;
/// # #[derive(Component)]
/// # struct ComponentB;
/// # #[derive(Component)]
/// # struct ComponentC;
/// # #[derive(Component)]
/// # struct ComponentD;
/// # #[derive(Component)]
/// # struct ComponentE;
/// #
/// #[derive(QueryFilter)]
/// struct MyFilter<T: Component, P: Component> {
///     // Field names are not relevant, since they are never manually accessed.
///     with_a: With<ComponentA>,
///     or_filter: Or<(With<ComponentC>, Added<ComponentB>)>,
///     generic_tuple: (With<T>, Without<P>),
/// }
///
/// fn my_system(query: Query<Entity, MyFilter<ComponentD, ComponentE>>) {
///     // ...
/// }
/// # bevy_ecs::system::assert_is_system(my_system);
/// ```
///
/// [`Query`]: crate::system::Query
///
/// # Safety
///
/// The [`WorldQuery`] implementation must not take any mutable access.
/// This is the same safety requirement as [`ReadOnlyQueryData`](crate::query::ReadOnlyQueryData).
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a valid `Query` filter",
    label = "invalid `Query` filter",
    note = "a `QueryFilter` typically uses a combination of `With<T>` and `Without<T>` statements"
)]
pub unsafe trait QueryFilter: WorldQuery {
    /// Returns true if (and only if) this Filter relies strictly on archetypes to limit which
    /// components are accessed by the Query.
    ///
    /// This enables optimizations for [`QueryIter`](`crate::query::QueryIter`) that rely on knowing exactly how
    /// many elements are being iterated (such as `Iterator::collect()`).
    ///
    /// If this is `true`, then [`QueryFilter::filter_fetch`] must always return true.
    const IS_ARCHETYPAL: bool;

    /// Returns true if the provided [`Entity`] and [`TableRow`] should be included in the query results.
    /// If false, the entity will be skipped.
    ///
    /// Note that this is called after already restricting the matched [`Table`]s and [`Archetype`]s to the
    /// ones that are compatible with the Filter's access.
    ///
    /// Implementors of this method will generally either have a trivial `true` body (required for archetypal filters),
    /// or access the necessary data within this function to make the final decision on filter inclusion.
    ///
    /// # Safety
    ///
    /// Must always be called _after_ [`WorldQuery::set_table`] or [`WorldQuery::set_archetype`]. `entity` and
    /// `table_row` must be in the range of the current table and archetype.
    unsafe fn filter_fetch(
        state: &Self::State,
        fetch: &mut Self::Fetch<'_>,
        entity: Entity,
        table_row: TableRow,
    ) -> bool;
}

/// Filter that selects entities with a component `T`.
///
/// This can be used in a [`Query`](crate::system::Query) if entities are required to have the
/// component `T` but you don't actually care about components value.
///
/// This is the negation of [`Without`].
///
/// # Examples
///
/// ```
/// # use bevy_ecs::component::Component;
/// # use bevy_ecs::query::With;
/// # use bevy_ecs::system::IntoSystem;
/// # use bevy_ecs::system::Query;
/// #
/// # #[derive(Component)]
/// # struct IsBeautiful;
/// # #[derive(Component)]
/// # struct Name { name: &'static str };
/// #
/// fn compliment_entity_system(query: Query<&Name, With<IsBeautiful>>) {
///     for name in &query {
///         println!("{} is looking lovely today!", name.name);
///     }
/// }
/// # bevy_ecs::system::assert_is_system(compliment_entity_system);
/// ```
pub struct With<T>(PhantomData<T>);

// SAFETY:
// `update_component_access` does not add any accesses.
// This is sound because [`QueryFilter::filter_fetch`] does not access any components.
// `update_component_access` adds a `With` filter for `T`.
// This is sound because `matches_component_set` returns whether the set contains the component.
unsafe impl<T: Component> WorldQuery for With<T> {
    type Fetch<'w> = ();

    type State = ComponentId;

    fn shrink_fetch<'wlong: 'wshort, 'wshort>(_fetch: Self::Fetch<'wlong>) -> Self::Fetch<'wshort> {}

    #[inline]
    unsafe fn init_fetch<'w, 's>(
        _world: crate::ecs::world::unsafe_world_cell::UnsafeWorldCell<'w>,
        _state: &'s Self::State,
        _last_run: crate::ecs::change_detection::Tick,
        _this_run: crate::ecs::change_detection::Tick,
    ) -> Self::Fetch<'w> {
    }

    const IS_DENSE: bool = {
        match T::STORAGE_TYPE {
            StorageType::Table => true,
            StorageType::SparseSet => false,
        }
    };

    #[inline]
    unsafe fn set_archetype<'w, 's>(
        _fetch: &mut Self::Fetch<'w>,
        _state: &'s Self::State,
        _archetype: &'w crate::ecs::archetype::Archetype,
        _table: &'w crate::ecs::storage::Table,
    ) {
    }

    #[inline]
    unsafe fn set_table<'w, 's>(
        _fetch: &mut Self::Fetch<'w>,
        _state: &'s Self::State,
        _table: &'w crate::ecs::storage::Table,
    ) {
    }

    #[inline]
    fn update_component_access(id: &Self::State, access: &mut super::FilteredAccess) {
        access.and_with(*id);
    }

    fn init_state(world: &mut crate::ecs::world::World) -> Self::State {
        world.register_component::<T>()
    }

    fn get_state(components: &crate::ecs::component::Components) -> Option<Self::State> {
        components.component_id::<T>()
    }

    fn matches_component_set(
        id: &Self::State,
        set_contains_id: &impl Fn(crate::ecs::component::ComponentId) -> bool,
    ) -> bool {
        set_contains_id(*id)
    }
}

// SAFETY: WorldQuery impl performs no access at all
unsafe impl<T: Component> QueryFilter for With<T> {
    const IS_ARCHETYPAL: bool = true;

     #[inline(always)]
    unsafe fn filter_fetch(
        _state: &Self::State,
        _fetch: &mut Self::Fetch<'_>,
        _entity: Entity,
        _table_row: TableRow,
    ) -> bool {
        true
    }
}

unsafe impl QueryFilter for () {
    const IS_ARCHETYPAL: bool = true;

    unsafe fn filter_fetch(
        state: &Self::State,
        fetch: &mut Self::Fetch<'_>,
        entity: Entity,
        table_row: TableRow,
    ) -> bool {
        true
    }
}

/// A marker trait to indicate that the filter works at an archetype level.
///
/// This is needed to:
/// - implement [`ExactSizeIterator`] for [`QueryIter`](crate::query::QueryIter) that contains archetype-level filters.
/// - ensure table filtering for [`QueryContiguousIter`](crate::query::QueryContiguousIter).
///
/// The trait must only be implemented for filters where its corresponding [`QueryFilter::IS_ARCHETYPAL`]
/// is [`prim@true`]. As such, only the [`With`] and [`Without`] filters can implement the trait.
/// [Tuples](prim@tuple) and [`Or`] filters are automatically implemented with the trait only if its containing types
/// also implement the same trait.
///
/// [`Added`], [`Changed`] and [`Spawned`] work with entities, and therefore are not archetypal. As such
/// they do not implement [`ArchetypeFilter`].
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a valid `Query` filter based on archetype information",
    label = "invalid `Query` filter",
    note = "an `ArchetypeFilter` typically uses a combination of `With<T>` and `Without<T>` statements"
)]
pub trait ArchetypeFilter: QueryFilter {}

// TODO!
