use std::marker::PhantomData;

use crate::ecs::{
    change_detection::Tick,
    entity::{Entity, EntityEquivalent, EntitySet, UniqueEntityArray},
    query::{
        IterQueryData, NopWorldQuery, QueryCombinationIter, QueryData, QueryEntityError,
        QueryFilter, QueryIter, QueryManyIter, QueryManyUniqueIter, QueryState, ReadOnlyQueryData,
    },
    world::unsafe_world_cell::UnsafeWorldCell,
};

/// A [system parameter] that provides selective access to the [`Component`] data stored in a [`World`].
///
/// Queries enable systems to access [entity identifiers] and [components] without requiring direct access to the [`World`].
/// Its iterators and getter methods return *query items*, which are types containing data related to an entity.
///
/// `Query` is a generic data structure that accepts two type parameters:
///
/// - **`D` (query data)**:
///   The type of data fetched by the query, which will be returned as the query item.
///   Only entities that match the requested data will generate an item.
///   Must implement the [`QueryData`] trait.
/// - **`F` (query filter)**:
///   An optional set of conditions that determine whether query items should be kept or discarded.
///   This defaults to [`unit`], which means no additional filters will be applied.
///   Must implement the [`QueryFilter`] trait.
///
/// [system parameter]: crate::system::SystemParam
/// [`Component`]: crate::component::Component
/// [`World`]: crate::world::World
/// [entity identifiers]: Entity
/// [components]: crate::component::Component
///
/// # Similar parameters
///
/// `Query` has few sibling [`SystemParam`]s, which perform additional validation:
///
/// - [`Single`] - Exactly one matching query item.
/// - [`Option<Single>`] - Zero or one matching query item.
/// - [`Populated`] - At least one matching query item.
///
/// These parameters will prevent systems from running if their requirements are not met.
///
/// [`SystemParam`]: crate::system::system_param::SystemParam
/// [`Option<Single>`]: Single
///
/// # System parameter declaration
///
/// A query should always be declared as a system parameter.
/// This section shows the most common idioms involving the declaration of `Query`.
///
/// ## Component access
///
/// You can fetch an entity's component by specifying a reference to that component in the query's data parameter:
///
/// ```
/// # use bevy_ecs::prelude::*;
/// #
/// # #[derive(Component)]
/// # struct ComponentA;
/// #
/// // A component can be accessed by a shared reference...
/// fn immutable_query(query: Query<&ComponentA>) {
///     // ...
/// }
///
/// // ...or by a mutable reference.
/// fn mutable_query(query: Query<&mut ComponentA>) {
///     // ...
/// }
/// #
/// # bevy_ecs::system::assert_is_system(immutable_query);
/// # bevy_ecs::system::assert_is_system(mutable_query);
/// ```
///
/// Note that components need to be behind a reference (`&` or `&mut`), or the query will not compile:
///
/// ```compile_fail,E0277
/// # use bevy_ecs::prelude::*;
/// #
/// # #[derive(Component)]
/// # struct ComponentA;
/// #
/// // This needs to be `&ComponentA` or `&mut ComponentA` in order to compile.
/// fn invalid_query(query: Query<ComponentA>) {
///     // ...
/// }
/// ```
///
/// ## Query filtering
///
/// Setting the query filter type parameter will ensure that each query item satisfies the given condition:
///
/// ```
/// # use bevy_ecs::prelude::*;
/// #
/// # #[derive(Component)]
/// # struct ComponentA;
/// #
/// # #[derive(Component)]
/// # struct ComponentB;
/// #
/// // `ComponentA` data will be accessed, but only for entities that also contain `ComponentB`.
/// fn filtered_query(query: Query<&ComponentA, With<ComponentB>>) {
///     // ...
/// }
/// #
/// # bevy_ecs::system::assert_is_system(filtered_query);
/// ```
///
/// Note that the filter is `With<ComponentB>`, not `With<&ComponentB>`. Unlike query data, `With`
/// does not require components to be behind a reference.
///
/// ## `QueryData` or `QueryFilter` tuples
///
/// Using [`tuple`]s, each `Query` type parameter can contain multiple elements.
///
/// In the following example two components are accessed simultaneously, and the query items are
/// filtered on two conditions:
///
/// ```
/// # use bevy_ecs::prelude::*;
/// #
/// # #[derive(Component)]
/// # struct ComponentA;
/// #
/// # #[derive(Component)]
/// # struct ComponentB;
/// #
/// # #[derive(Component)]
/// # struct ComponentC;
/// #
/// # #[derive(Component)]
/// # struct ComponentD;
/// #
/// fn complex_query(
///     query: Query<(&mut ComponentA, &ComponentB), (With<ComponentC>, Without<ComponentD>)>
/// ) {
///     // ...
/// }
/// #
/// # bevy_ecs::system::assert_is_system(complex_query);
/// ```
///
/// Note that this currently only works on tuples with 15 or fewer items. You may nest tuples to
/// get around this limit:
///
/// ```
/// # use bevy_ecs::prelude::*;
/// #
/// # #[derive(Component)]
/// # struct ComponentA;
/// #
/// # #[derive(Component)]
/// # struct ComponentB;
/// #
/// # #[derive(Component)]
/// # struct ComponentC;
/// #
/// # #[derive(Component)]
/// # struct ComponentD;
/// #
/// fn nested_query(
///     query: Query<(&ComponentA, &ComponentB, (&mut ComponentC, &mut ComponentD))>
/// ) {
///     // ...
/// }
/// #
/// # bevy_ecs::system::assert_is_system(nested_query);
/// ```
///
/// ## Entity identifier access
///
/// You can access [`Entity`], the entity identifier, by including it in the query data parameter:
///
/// ```
/// # use bevy_ecs::prelude::*;
/// #
/// # #[derive(Component)]
/// # struct ComponentA;
/// #
/// fn entity_id_query(query: Query<(Entity, &ComponentA)>) {
///     // ...
/// }
/// #
/// # bevy_ecs::system::assert_is_system(entity_id_query);
/// ```
///
/// Be aware that [`Entity`] is not a component, so it does not need to be behind a reference.
///
/// ## Optional component access
///
/// A component can be made optional by wrapping it into an [`Option`]. In the following example, a
/// query item will still be generated even if the queried entity does not contain `ComponentB`.
/// When this is the case, `Option<&ComponentB>`'s corresponding value will be `None`.
///
/// ```
/// # use bevy_ecs::prelude::*;
/// #
/// # #[derive(Component)]
/// # struct ComponentA;
/// #
/// # #[derive(Component)]
/// # struct ComponentB;
/// #
/// // Queried items must contain `ComponentA`. If they also contain `ComponentB`, its value will
/// // be fetched as well.
/// fn optional_component_query(query: Query<(&ComponentA, Option<&ComponentB>)>) {
///     // ...
/// }
/// #
/// # bevy_ecs::system::assert_is_system(optional_component_query);
/// ```
///
/// Optional components can hurt performance in some cases, so please read the [performance]
/// section to learn more about them. Additionally, if you need to declare several optional
/// components, you may be interested in using [`AnyOf`].
///
/// [performance]: #performance
/// [`AnyOf`]: crate::query::AnyOf
///
/// ## Disjoint queries
///
/// A system cannot contain two queries that break Rust's mutability rules, or else it will panic
/// when initialized. This can often be fixed with the [`Without`] filter, which makes the queries
/// disjoint.
///
/// In the following example, the two queries can mutably access the same `&mut Health` component
/// if an entity has both the `Player` and `Enemy` components. Bevy will catch this and panic,
/// however, instead of breaking Rust's mutability rules:
///
/// ```should_panic
/// # use bevy_ecs::prelude::*;
/// #
/// # #[derive(Component)]
/// # struct Health;
/// #
/// # #[derive(Component)]
/// # struct Player;
/// #
/// # #[derive(Component)]
/// # struct Enemy;
/// #
/// fn randomize_health(
///     player_query: Query<&mut Health, With<Player>>,
///     enemy_query: Query<&mut Health, With<Enemy>>,
/// ) {
///     // ...
/// }
/// #
/// # bevy_ecs::system::assert_system_does_not_conflict(randomize_health);
/// ```
///
/// Adding a [`Without`] filter will disjoint the queries. In the following example, any entity
/// that has both the `Player` and `Enemy` components will be excluded from _both_ queries:
///
/// ```
/// # use bevy_ecs::prelude::*;
/// #
/// # #[derive(Component)]
/// # struct Health;
/// #
/// # #[derive(Component)]
/// # struct Player;
/// #
/// # #[derive(Component)]
/// # struct Enemy;
/// #
/// fn randomize_health(
///     player_query: Query<&mut Health, (With<Player>, Without<Enemy>)>,
///     enemy_query: Query<&mut Health, (With<Enemy>, Without<Player>)>,
/// ) {
///     // ...
/// }
/// #
/// # bevy_ecs::system::assert_system_does_not_conflict(randomize_health);
/// ```
///
/// An alternative solution to this problem would be to wrap the conflicting queries in
/// [`ParamSet`].
///
/// [`Without`]: crate::query::Without
/// [`ParamSet`]: crate::system::ParamSet
///
/// ## Whole Entity Access
///
/// [`EntityRef`] can be used in a query to gain read-only access to all components of an entity.
/// This is useful when dynamically fetching components instead of baking them into the query type.
///
/// ```
/// # use bevy_ecs::prelude::*;
/// #
/// # #[derive(Component)]
/// # struct ComponentA;
/// #
/// fn all_components_query(query: Query<(EntityRef, &ComponentA)>) {
///     // ...
/// }
/// #
/// # bevy_ecs::system::assert_is_system(all_components_query);
/// ```
///
/// As [`EntityRef`] can read any component on an entity, a query using it will conflict with *any*
/// mutable component access.
///
/// ```should_panic
/// # use bevy_ecs::prelude::*;
/// #
/// # #[derive(Component)]
/// # struct ComponentA;
/// #
/// // `EntityRef` provides read access to *all* components on an entity. When combined with
/// // `&mut ComponentA` in the same query, it creates a conflict because `EntityRef` could read
/// // `&ComponentA` while `&mut ComponentA` attempts to modify it - violating Rust's borrowing
/// // rules.
/// fn invalid_query(query: Query<(EntityRef, &mut ComponentA)>) {
///     // ...
/// }
/// #
/// # bevy_ecs::system::assert_system_does_not_conflict(invalid_query);
/// ```
///
/// It is strongly advised to couple [`EntityRef`] queries with the use of either [`With`] /
/// [`Without`] filters or [`ParamSet`]s. Not only does this improve the performance and
/// parallelization of the system, but it enables systems to gain mutable access to other
/// components:
///
/// ```
/// # use bevy_ecs::prelude::*;
/// #
/// # #[derive(Component)]
/// # struct ComponentA;
/// #
/// # #[derive(Component)]
/// # struct ComponentB;
/// #
/// // The first query only reads entities that have `ComponentA`, while the second query only
/// // modifies entities that *don't* have `ComponentA`. Because neither query will access the same
/// // entity, this system does not conflict.
/// fn disjoint_query(
///     query_a: Query<EntityRef, With<ComponentA>>,
///     query_b: Query<&mut ComponentB, Without<ComponentA>>,
/// ) {
///     // ...
/// }
/// #
/// # bevy_ecs::system::assert_system_does_not_conflict(disjoint_query);
/// ```
///
/// The fundamental rule: [`EntityRef`]'s ability to read all components means it can never
/// coexist with mutable access. [`With`] / [`Without`] filters can guarantee this by keeping the
/// queries on completely separate entities.
///
/// [`EntityRef`]: crate::world::EntityRef
/// [`With`]: crate::query::With
///
/// # Accessing query items
///
/// The following table summarizes the behavior of safe methods that can be used to get query
/// items:
///
/// |Query methods|Effect|
/// |-|-|
/// |[`iter`]\[[`_mut`][`iter_mut`]\]|Returns an iterator over all query items.|
/// |[`iter[_mut]().for_each()`][`for_each`],<br />[`par_iter`]\[[`_mut`][`par_iter_mut`]\]|Runs a specified function for each query item.|
/// |[`iter_many`]\[[`_unique`][`iter_many_unique`]\]\[[`_mut`][`iter_many_mut`]\]|Iterates over query items that match a list of entities.|
/// |[`iter_combinations`]\[[`_mut`][`iter_combinations_mut`]\]|Iterates over all combinations of query items.|
/// |[`single`](Self::single)\[[`_mut`][`single_mut`]\]|Returns a single query item if only one exists.|
/// |[`get`]\[[`_mut`][`get_mut`]\]|Returns the query item for a specified entity.|
/// |[`get_many`]\[[`_unique`][`get_many_unique`]\]\[[`_mut`][`get_many_mut`]\]|Returns all query items that match a list of entities.|
///
/// There are two methods for each type of query operation: immutable and mutable (ending with `_mut`).
/// When using immutable methods, the query items returned are of type [`ROQueryItem`], a read-only version of the query item.
/// In this circumstance, every mutable reference in the query fetch type parameter is substituted by a shared reference.
///
/// [`iter`]: Self::iter
/// [`iter_mut`]: Self::iter_mut
/// [`for_each`]: #iteratorfor_each
/// [`par_iter`]: Self::par_iter
/// [`par_iter_mut`]: Self::par_iter_mut
/// [`iter_many`]: Self::iter_many
/// [`iter_many_unique`]: Self::iter_many_unique
/// [`iter_many_mut`]: Self::iter_many_mut
/// [`iter_combinations`]: Self::iter_combinations
/// [`iter_combinations_mut`]: Self::iter_combinations_mut
/// [`single_mut`]: Self::single_mut
/// [`get`]: Self::get
/// [`get_mut`]: Self::get_mut
/// [`get_many`]: Self::get_many
/// [`get_many_unique`]: Self::get_many_unique
/// [`get_many_mut`]: Self::get_many_mut
///
/// # Performance
///
/// Creating a `Query` is a low-cost constant operation. Iterating it, on the other hand, fetches
/// data from the world and generates items, which can have a significant computational cost.
///
/// Two systems cannot be executed in parallel if both access the same component type where at
/// least one of the accesses is mutable. Because of this, it is recommended for queries to only
/// fetch mutable access to components when necessary, since immutable access can be parallelized.
///
/// Query filters ([`With`] / [`Without`]) can improve performance because they narrow the kinds of
/// entities that can be fetched. Systems that access fewer kinds of entities are more likely to be
/// parallelized by the scheduler.
///
/// On the other hand, be careful using optional components (`Option<&ComponentA>`) and
/// [`EntityRef`] because they broaden the amount of entities kinds that can be accessed. This is
/// especially true of a query that _only_ fetches optional components or [`EntityRef`], as the
/// query would iterate over all entities in the world.
///
/// There are two types of [component storage types]: [`Table`] and [`SparseSet`]. [`Table`] offers
/// fast iteration speeds, but slower insertion and removal speeds. [`SparseSet`] is the opposite:
/// it offers fast component insertion and removal speeds, but slower iteration speeds.
///
/// The following table compares the computational complexity of the various methods and
/// operations, where:
///
/// - **n** is the number of entities that match the query.
/// - **r** is the number of elements in a combination.
/// - **k** is the number of involved entities in the operation.
/// - **a** is the number of archetypes in the world.
/// - **C** is the [binomial coefficient], used to count combinations. <sub>n</sub>C<sub>r</sub> is
///   read as "*n* choose *r*" and is equivalent to the number of distinct unordered subsets of *r*
///   elements that can be taken from a set of *n* elements.
///
/// |Query operation|Computational complexity|
/// |-|-|
/// |[`iter`]\[[`_mut`][`iter_mut`]\]|O(n)|
/// |[`iter[_mut]().for_each()`][`for_each`],<br/>[`par_iter`]\[[`_mut`][`par_iter_mut`]\]|O(n)|
/// |[`iter_many`]\[[`_mut`][`iter_many_mut`]\]|O(k)|
/// |[`iter_combinations`]\[[`_mut`][`iter_combinations_mut`]\]|O(<sub>n</sub>C<sub>r</sub>)|
/// |[`single`](Self::single)\[[`_mut`][`single_mut`]\]|O(a)|
/// |[`get`]\[[`_mut`][`get_mut`]\]|O(1)|
/// |[`get_many`]|O(k)|
/// |[`get_many_mut`]|O(k<sup>2</sup>)|
/// |Archetype-based filtering ([`With`], [`Without`], [`Or`])|O(a)|
/// |Change detection filtering ([`Added`], [`Changed`], [`Spawned`])|O(a + n)|
///
/// [component storage types]: crate::component::StorageType
/// [`Table`]: crate::storage::Table
/// [`SparseSet`]: crate::storage::SparseSet
/// [binomial coefficient]: https://en.wikipedia.org/wiki/Binomial_coefficient
/// [`Or`]: crate::query::Or
/// [`Added`]: crate::query::Added
/// [`Changed`]: crate::query::Changed
/// [`Spawned`]: crate::query::Spawned
///
/// # `Iterator::for_each`
///
/// The `for_each` methods appear to be generally faster than `for`-loops when run on worlds with
/// high archetype fragmentation, and may enable additional optimizations like [autovectorization]. It
/// is strongly advised to only use [`Iterator::for_each`] if it tangibly improves performance.
/// *Always* profile or benchmark before and after the change!
///
/// ```rust
/// # use bevy_ecs::prelude::*;
/// #
/// # #[derive(Component)]
/// # struct ComponentA;
/// #
/// fn system(query: Query<&ComponentA>) {
///     // This may result in better performance...
///     query.iter().for_each(|component| {
///         // ...
///     });
///
///     // ...than this. Always benchmark to validate the difference!
///     for component in query.iter() {
///         // ...
///     }
/// }
/// #
/// # bevy_ecs::system::assert_is_system(system);
/// ```
///
/// [autovectorization]: https://en.wikipedia.org/wiki/Automatic_vectorization
pub struct Query<'world, 'state, D: QueryData, F: QueryFilter = ()> {
    // SAFETY: Must have access to the components registered in `state`.
    world: UnsafeWorldCell<'world>,
    state: &'state QueryState<D, F>,
    last_run: Tick,
    this_run: Tick,
}

impl<D: ReadOnlyQueryData, F: QueryFilter> Clone for Query<'_, '_, D, F> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D: ReadOnlyQueryData, F: QueryFilter> Copy for Query<'_, '_, D, F> {}

impl<D: QueryData, F: QueryFilter> std::fmt::Debug for Query<'_, '_, D, F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Query")
            .field("matched_entities", &self.iter().count())
            .field("state", &self.state)
            .field("last_runt", &self.last_run)
            .field("this_run", &self.this_run)
            .field("world", &self.world)
            .finish()
    }
}

impl<'w, 's, D: QueryData, F: QueryFilter> Query<'w, 's, D, F> {
    /// Creates a new query.
    ///
    /// # Safety
    ///
    /// * This will create a query that could violate memory safety rules. Make sure that this is only
    ///   called in ways that ensure the queries have unique mutable access.
    /// * `world` must be the world used to create `state`.
    #[inline]
    pub(crate) unsafe fn new(
        world: UnsafeWorldCell<'w>,
        state: &'s QueryState<D, F>,
        last_run: Tick,
        this_run: Tick,
    ) -> Self {
        Self {
            world,
            state,
            last_run,
            this_run,
        }
    }

    /// Returns another `Query` from this that fetches the read-only version of the query items.
    ///
    /// For example, `Query<(&mut D1, &D2, &mut D3), With<F>>` will become `Query<(&D1, &D2, &D3), With<F>>`.
    /// This can be useful when working around the borrow checker,
    /// or reusing functionality between systems via functions that accept query types.
    ///
    /// # See also
    ///
    /// [`into_readonly`](Self::into_readonly) for a version that consumes the `Query` to return one with the full `'world` lifetime.
    pub fn as_readonly(&self) -> Query<'_, 's, D::ReadOnly, F> {
        // SAFETY: The reborrowed query is converted to read-only, so it cannot perform mutable access,
        // and the original query is held with a shared borrow, so it cannot perform mutable access either.
        unsafe { self.reborrow_unsafe() }.into_readonly()
    }

    /// Returns another `Query` from this does not return any data, which can be faster.
    ///
    /// The resulting query will ignore any non-archetypal filters in `D`,
    /// so this is only equivalent if `D::IS_ARCHETYPAL` is `true`.
    fn as_nop(&self) -> Query<'_, 's, NopWorldQuery<D>, F> {
        let new_state = self.state.as_nop();
        // SAFETY:
        // - The reborrowed query is converted to read-only, so it cannot perform mutable access,
        //   and the original query is held with a shared borrow, so it cannot perform mutable access either.
        //   Note that although `NopWorldQuery` itself performs *no* access and could soundly alias a mutable query,
        //   it has the original `QueryState::component_access` and could be `transmute`d to a read-only query.
        // - The world matches because it was the same one used to construct self.
        unsafe { Query::new(self.world, new_state, self.last_run, self.this_run) }
    }

    /// Returns another `Query` from this that fetches the read-only version of the query items.
    ///
    /// For example, `Query<(&mut D1, &D2, &mut D3), With<F>>` will become `Query<(&D1, &D2, &D3), With<F>>`.
    /// This can be useful when working around the borrow checker,
    /// or reusing functionality between systems via functions that accept query types.
    ///
    /// # See also
    ///
    /// [`as_readonly`](Self::as_readonly) for a version that borrows the `Query` instead of consuming it.
    pub fn into_readonly(self) -> Query<'w, 's, D::ReadOnly, F> {
        let new_state = self.state.as_readonly();
        // SAFETY:
        // - This is memory safe because it turns the query immutable.
        // - The world matches because it was the same one used to construct self.
        unsafe { Query::new(self.world, new_state, self.last_run, self.this_run) }
    }

    /// Returns a new `Query` reborrowing the access from this one. The current query will be unusable
    /// while the new one exists.
    ///
    /// # Example
    ///
    /// For example this allows to call other methods or other systems that require an owned `Query` without
    /// completely giving up ownership of it.
    ///
    /// ```
    /// # use bevy_ecs::prelude::*;
    /// #
    /// # #[derive(Component)]
    /// # struct ComponentA;
    ///
    /// fn helper_system(query: Query<&ComponentA>) { /* ... */}
    ///
    /// fn system(mut query: Query<&ComponentA>) {
    ///     helper_system(query.reborrow());
    ///     // Can still use query here:
    ///     for component in &query {
    ///         // ...
    ///     }
    /// }
    /// ```
    pub fn reborrow(&mut self) -> Query<'_, 's, D, F> {
        // SAFETY: this query is exclusively borrowed while the new one exists, so
        // no overlapping access can occur.
        unsafe { self.reborrow_unsafe() }
    }

    // Returns a new `Query` reborrowing the access from this one.
    /// The current query will still be usable while the new one exists, but must not be used in a way that violates aliasing.
    ///
    /// # Safety
    ///
    /// This function makes it possible to violate Rust's aliasing guarantees.
    /// You must make sure this call does not result in a mutable or shared reference to a component with a mutable reference.
    ///
    /// # See also
    ///
    /// - [`reborrow`](Self::reborrow) for the safe versions.
    pub unsafe fn reborrow_unsafe(&self) -> Query<'_, 's, D, F> {
        // SAFETY:
        // - This is memory safe because the caller ensures that there are no conflicting references.
        // - The world matches because it was the same one used to construct self.
        unsafe { self.copy_unsafe() }
    }

    /// Returns a new `Query` copying the access from this one.
    /// The current query will still be usable while the new one exists, but must not be used in a way that violates aliasing.
    ///
    /// # Safety
    ///
    /// This function makes it possible to violate Rust's aliasing guarantees.
    /// You must make sure this call does not result in a mutable or shared reference to a component with a mutable reference.
    ///
    /// # See also
    ///
    /// - [`reborrow_unsafe`](Self::reborrow_unsafe) for a safer version that constrains the returned `'w` lifetime to the length of the borrow.
    unsafe fn copy_unsafe(&self) -> Query<'w, 's, D, F> {
        // SAFETY:
        // - This is memory safe because the caller ensures that there are no conflicting references.
        // - The world matches because it was the same one used to construct self.
        unsafe { Query::new(self.world, self.state, self.last_run, self.this_run) }
    }

    /// Returns an [`Iterator`] over the read-only query items.
    ///
    /// This iterator is always guaranteed to return results from each matching entity once and only once.
    /// Iteration order is not guaranteed.
    ///
    /// # Example
    ///
    /// Here, the `report_names_system` iterates over the `Player` component of every entity that contains it:
    ///
    /// ```
    /// # use bevy_ecs::prelude::*;
    /// #
    /// # #[derive(Component)]
    /// # struct Player { name: String }
    /// #
    /// fn report_names_system(query: Query<&Player>) {
    ///     for player in &query {
    ///         println!("Say hello to {}!", player.name);
    ///     }
    /// }
    /// # bevy_ecs::system::assert_is_system(report_names_system);
    /// ```
    ///
    /// # See also
    ///
    /// [`iter_mut`](Self::iter_mut) for mutable query items.
    #[inline]
    pub fn iter(&self) -> QueryIter<'_, 's, D::ReadOnly, F> {
        self.as_readonly().into_iter()
    }

    /// Returns an [`Iterator`] over the query items.
    ///
    /// This iterator is always guaranteed to return results from each matching entity once and only once.
    /// Iteration order is not guaranteed.
    ///
    /// If the [`QueryData`] does not implement [`IterQueryData`],
    /// then it is not sound to yield multiple items concurrently
    /// and the resulting [`QueryIter`] will not implement [`Iterator`].
    /// To iterate over the items in that case,
    /// use the [`QueryIter::fetch_next()`](crate::query::QueryIter::fetch_next) method,
    /// which ensures only one item is alive at a time.
    ///
    /// # Example
    ///
    /// Here, the `gravity_system` updates the `Velocity` component of every entity that contains it:
    ///
    /// ```
    /// # use bevy_ecs::prelude::*;
    /// #
    /// # #[derive(Component)]
    /// # struct Velocity { x: f32, y: f32, z: f32 }
    /// fn gravity_system(mut query: Query<&mut Velocity>) {
    ///     const DELTA: f32 = 1.0 / 60.0;
    ///     for mut velocity in &mut query {
    ///         velocity.y -= 9.8 * DELTA;
    ///     }
    /// }
    /// # bevy_ecs::system::assert_is_system(gravity_system);
    /// ```
    ///
    /// # See also
    ///
    /// [`iter`](Self::iter) for read-only query items.
    #[inline]
    pub fn iter_mut(&mut self) -> QueryIter<'_, 's, D, F> {
        self.reborrow().iter_inner()
    }

    /// Returns an [`Iterator`] over the query items, with the actual "inner" world lifetime.
    ///
    /// This iterator is always guaranteed to return results from each matching entity once and only once.
    /// Iteration order is not guaranteed.
    ///
    /// If the [`QueryData`] does not implement [`IterQueryData`],
    /// then it is not sound to yield multiple items concurrently
    /// and the resulting [`QueryIter`] will not implement [`Iterator`].
    /// To iterate over the items in that case,
    /// use the [`QueryIter::fetch_next()`](crate::query::QueryIter::fetch_next) method,
    /// which ensures only one item is alive at a time.
    ///
    /// # Example
    ///
    /// Here, the `report_names_system` iterates over the `Player` component of every entity
    /// that contains it:
    ///
    /// ```
    /// # use bevy_ecs::prelude::*;
    /// #
    /// # #[derive(Component)]
    /// # struct Player { name: String }
    /// #
    /// fn report_names_system(query: Query<&Player>) {
    ///     for player in &query {
    ///         println!("Say hello to {}!", player.name);
    ///     }
    /// }
    /// # bevy_ecs::system::assert_is_system(report_names_system);
    /// ```
    #[inline]
    pub fn iter_inner(self) -> QueryIter<'w, 's, D, F> {
        // SAFETY:
        // - `self.world` has permission to access the required components.
        // - We consume the query, so mutable queries cannot alias.
        //   Read-only queries are `Copy`, but may alias themselves.
        unsafe { QueryIter::new(self.world, self.state, self.last_run, self.this_run) }
    }

    /// Returns a [`QueryCombinationIter`] over all combinations of `K` read-only query items without repetition.
    ///
    /// This iterator is always guaranteed to return results from each unique pair of matching entities.
    /// Iteration order is not guaranteed.
    ///
    /// # Example
    ///
    /// ```
    /// # use bevy_ecs::prelude::*;
    /// # #[derive(Component)]
    /// # struct ComponentA;
    /// #
    /// fn some_system(query: Query<&ComponentA>) {
    ///     for [a1, a2] in query.iter_combinations() {
    ///         // ...
    ///     }
    /// }
    /// ```
    ///
    /// # See also
    ///
    /// - [`iter_combinations_mut`](Self::iter_combinations_mut) for mutable query item combinations.
    /// - [`iter_combinations_inner`](Self::iter_combinations_inner) for mutable query item combinations with the full `'world` lifetime.
    #[inline]
    pub fn iter_combinations<const K: usize>(
        &self,
    ) -> QueryCombinationIter<'_, 's, D::ReadOnly, F, K> {
        self.as_readonly().iter_combinations_inner()
    }

    /// Returns a [`QueryCombinationIter`] over all combinations of `K` query items without repetition.
    ///
    /// This iterator is always guaranteed to return results from each unique pair of matching entities.
    /// Iteration order is not guaranteed.
    ///
    /// # Example
    ///
    /// ```
    /// # use bevy_ecs::prelude::*;
    /// # #[derive(Component)]
    /// # struct ComponentA;
    /// fn some_system(mut query: Query<&mut ComponentA>) {
    ///     let mut combinations = query.iter_combinations_mut();
    ///     while let Some([mut a1, mut a2]) = combinations.fetch_next() {
    ///         // mutably access components data
    ///     }
    /// }
    /// ```
    ///
    /// # See also
    ///
    /// - [`iter_combinations`](Self::iter_combinations) for read-only query item combinations.
    /// - [`iter_combinations_inner`](Self::iter_combinations_inner) for mutable query item combinations with the full `'world` lifetime.
    #[inline]
    pub fn iter_combinations_mut<const K: usize>(&mut self) -> QueryCombinationIter<'_, 's, D, F, K>
    where
        D: IterQueryData,
    {
        self.reborrow().iter_combinations_inner()
    }

    /// Returns a [`QueryCombinationIter`] over all combinations of `K` query items without repetition.
    /// This consumes the [`Query`] to return results with the actual "inner" world lifetime.
    ///
    /// This iterator is always guaranteed to return results from each unique pair of matching entities.
    /// Iteration order is not guaranteed.
    ///
    /// # Example
    ///
    /// ```
    /// # use bevy_ecs::prelude::*;
    /// # #[derive(Component)]
    /// # struct ComponentA;
    /// fn some_system(query: Query<&mut ComponentA>) {
    ///     let mut combinations = query.iter_combinations_inner();
    ///     while let Some([mut a1, mut a2]) = combinations.fetch_next() {
    ///         // mutably access components data
    ///     }
    /// }
    /// ```
    ///
    /// # See also
    ///
    /// - [`iter_combinations`](Self::iter_combinations) for read-only query item combinations.
    /// - [`iter_combinations_mut`](Self::iter_combinations_mut) for mutable query item combinations.
    #[inline]
    pub fn iter_combinations_inner<const K: usize>(self) -> QueryCombinationIter<'w, 's, D, F, K>
    where
        D: IterQueryData,
    {
        // SAFETY: `self.world` has permission to access the required components.
        unsafe { QueryCombinationIter::new(self.world, self.state, self.last_run, self.this_run) }
    }

    /// Returns an [`Iterator`] over the read-only query items generated from an [`Entity`] list.
    ///
    /// Items are returned in the order of the list of entities, and may not be unique if the input
    /// doesn't guarantee uniqueness. Entities that don't match the query are skipped.
    ///
    /// # Example
    ///
    /// ```
    /// # use bevy_ecs::prelude::*;
    /// # #[derive(Component)]
    /// # struct Counter {
    /// #     value: i32
    /// # }
    /// #
    /// // A component containing an entity list.
    /// #[derive(Component)]
    /// struct Friends {
    ///     list: Vec<Entity>,
    /// }
    ///
    /// fn system(
    ///     friends_query: Query<&Friends>,
    ///     counter_query: Query<&Counter>,
    /// ) {
    ///     for friends in &friends_query {
    ///         for counter in counter_query.iter_many(&friends.list) {
    ///             println!("Friend's counter: {}", counter.value);
    ///         }
    ///     }
    /// }
    /// # bevy_ecs::system::assert_is_system(system);
    /// ```
    ///
    /// # See also
    ///
    /// - [`iter_many_mut`](Self::iter_many_mut) to get mutable query items.
    /// - [`iter_many_inner`](Self::iter_many_inner) to get mutable query items with the full `'world` lifetime.
    #[inline]
    pub fn iter_many<EntityList: IntoIterator<Item: EntityEquivalent>>(
        &self,
        entities: EntityList,
    ) -> QueryManyIter<'_, 's, D::ReadOnly, F, EntityList::IntoIter> {
        self.as_readonly().iter_many_inner(entities)
    }

    /// Returns an iterator over the query items generated from an [`Entity`] list.
    ///
    /// Items are returned in the order of the list of entities, and may not be unique if the input
    /// doesn't guarantee uniqueness. Entities that don't match the query are skipped.
    ///
    /// # Examples
    ///
    /// ```
    /// # use bevy_ecs::prelude::*;
    /// #[derive(Component)]
    /// struct Counter {
    ///     value: i32
    /// }
    ///
    /// #[derive(Component)]
    /// struct Friends {
    ///     list: Vec<Entity>,
    /// }
    ///
    /// fn system(
    ///     friends_query: Query<&Friends>,
    ///     mut counter_query: Query<&mut Counter>,
    /// ) {
    ///     for friends in &friends_query {
    ///         let mut iter = counter_query.iter_many_mut(&friends.list);
    ///         while let Some(mut counter) = iter.fetch_next() {
    ///             println!("Friend's counter: {}", counter.value);
    ///             counter.value += 1;
    ///         }
    ///     }
    /// }
    /// # bevy_ecs::system::assert_is_system(system);
    /// ```
    /// # See also
    ///
    /// - [`iter_many`](Self::iter_many) to get read-only query items.
    /// - [`iter_many_inner`](Self::iter_many_inner) to get mutable query items with the full `'world` lifetime.
    #[inline]
    pub fn iter_many_mut<EntityList: IntoIterator<Item: EntityEquivalent>>(
        &mut self,
        entities: EntityList,
    ) -> QueryManyIter<'_, 's, D, F, EntityList::IntoIter> {
        self.reborrow().iter_many_inner(entities)
    }

    /// Returns an iterator over the query items generated from an [`Entity`] list.
    /// This consumes the [`Query`] to return results with the actual "inner" world lifetime.
    ///
    /// Items are returned in the order of the list of entities, and may not be unique if the input
    /// doesn't guarantee uniqueness. Entities that don't match the query are skipped.
    ///
    /// # See also
    ///
    /// - [`iter_many`](Self::iter_many) to get read-only query items.
    /// - [`iter_many_mut`](Self::iter_many_mut) to get mutable query items.
    #[inline]
    pub fn iter_many_inner<EntityList: IntoIterator<Item: EntityEquivalent>>(
        self,
        entities: EntityList,
    ) -> QueryManyIter<'w, 's, D, F, EntityList::IntoIter> {
        // SAFETY: `self.world` has permission to access the required components.
        unsafe {
            QueryManyIter::new(
                self.world,
                self.state,
                entities,
                self.last_run,
                self.this_run,
            )
        }
    }

    /// Returns an [`Iterator`] over the unique read-only query items generated from an [`EntitySet`].
    ///
    /// Items are returned in the order of the list of entities. Entities that don't match the query are skipped.
    ///
    /// # Example
    ///
    /// ```
    /// # use bevy_ecs::{prelude::*, entity::{EntitySet, UniqueEntityIter}};
    /// # use core::slice;
    /// # #[derive(Component)]
    /// # struct Counter {
    /// #     value: i32
    /// # }
    /// #
    /// // `Friends` ensures that it only lists unique entities.
    /// #[derive(Component)]
    /// struct Friends {
    ///     unique_list: Vec<Entity>,
    /// }
    ///
    /// impl<'a> IntoIterator for &'a Friends {
    ///
    ///     type Item = &'a Entity;
    ///     type IntoIter = UniqueEntityIter<slice::Iter<'a, Entity>>;
    ///
    ///     fn into_iter(self) -> Self::IntoIter {
    ///         // SAFETY: `Friends` ensures that it unique_list contains only unique entities.
    ///        unsafe { UniqueEntityIter::from_iter_unchecked(self.unique_list.iter()) }
    ///     }
    /// }
    ///
    /// fn system(
    ///     friends_query: Query<&Friends>,
    ///     counter_query: Query<&Counter>,
    /// ) {
    ///     for friends in &friends_query {
    ///         for counter in counter_query.iter_many_unique(friends) {
    ///             println!("Friend's counter: {:?}", counter.value);
    ///         }
    ///     }
    /// }
    /// # bevy_ecs::system::assert_is_system(system);
    /// ```
    ///
    /// # See also
    ///
    /// - [`iter_many_unique_mut`](Self::iter_many_unique_mut) to get mutable query items.
    /// - [`iter_many_unique_inner`](Self::iter_many_unique_inner) to get with the actual "inner" world lifetime.
    #[inline]
    pub fn iter_many_unique<EntityList: EntitySet>(
        &self,
        entities: EntityList,
    ) -> QueryManyUniqueIter<'_, 's, D::ReadOnly, F, EntityList::IntoIter> {
        self.as_readonly().iter_many_unique_inner(entities)
    }

    /// Returns an iterator over the unique query items generated from an [`EntitySet`].
    ///
    /// Items are returned in the order of the list of entities. Entities that don't match the query are skipped.
    ///
    /// # Examples
    ///
    /// ```
    /// # use bevy_ecs::{prelude::*, entity::{EntitySet, UniqueEntityIter}};
    /// # use core::slice;
    /// #[derive(Component)]
    /// struct Counter {
    ///     value: i32
    /// }
    ///
    /// // `Friends` ensures that it only lists unique entities.
    /// #[derive(Component)]
    /// struct Friends {
    ///     unique_list: Vec<Entity>,
    /// }
    ///
    /// impl<'a> IntoIterator for &'a Friends {
    ///     type Item = &'a Entity;
    ///     type IntoIter = UniqueEntityIter<slice::Iter<'a, Entity>>;
    ///
    ///     fn into_iter(self) -> Self::IntoIter {
    ///         // SAFETY: `Friends` ensures that it unique_list contains only unique entities.
    ///         unsafe { UniqueEntityIter::from_iter_unchecked(self.unique_list.iter()) }
    ///     }
    /// }
    ///
    /// fn system(
    ///     friends_query: Query<&Friends>,
    ///     mut counter_query: Query<&mut Counter>,
    /// ) {
    ///     for friends in &friends_query {
    ///         for mut counter in counter_query.iter_many_unique_mut(friends) {
    ///             println!("Friend's counter: {:?}", counter.value);
    ///             counter.value += 1;
    ///         }
    ///     }
    /// }
    /// # bevy_ecs::system::assert_is_system(system);
    /// ```
    /// # See also
    ///
    /// - [`iter_many_unique`](Self::iter_many_unique) to get read-only query items.
    /// - [`iter_many_unique_inner`](Self::iter_many_unique_inner) to get with the actual "inner" world lifetime.
    #[inline]
    pub fn iter_many_unique_mut<EntityList: EntitySet>(
        &mut self,
        entities: EntityList,
    ) -> QueryManyUniqueIter<'_, 's, D, F, EntityList::IntoIter>
    where
        D: IterQueryData,
    {
        self.reborrow().iter_many_unique_inner(entities)
    }

    /// Returns an iterator over the unique query items generated from an [`EntitySet`].
    /// This consumes the [`Query`] to return results with the actual "inner" world lifetime.
    ///
    /// Items are returned in the order of the list of entities. Entities that don't match the query are skipped.
    ///
    /// # Examples
    ///
    /// ```
    /// # use bevy_ecs::{prelude::*, entity::{EntitySet, UniqueEntityIter}};
    /// # use core::slice;
    /// #[derive(Component)]
    /// struct Counter {
    ///     value: i32
    /// }
    ///
    /// // `Friends` ensures that it only lists unique entities.
    /// #[derive(Component)]
    /// struct Friends {
    ///     unique_list: Vec<Entity>,
    /// }
    ///
    /// impl<'a> IntoIterator for &'a Friends {
    ///     type Item = &'a Entity;
    ///     type IntoIter = UniqueEntityIter<slice::Iter<'a, Entity>>;
    ///
    ///     fn into_iter(self) -> Self::IntoIter {
    ///         // SAFETY: `Friends` ensures that it unique_list contains only unique entities.
    ///         unsafe { UniqueEntityIter::from_iter_unchecked(self.unique_list.iter()) }
    ///     }
    /// }
    ///
    /// fn system(
    ///     friends_query: Query<&Friends>,
    ///     mut counter_query: Query<&mut Counter>,
    /// ) {
    ///     let friends = friends_query.single().unwrap();
    ///     for mut counter in counter_query.iter_many_unique_inner(friends) {
    ///         println!("Friend's counter: {:?}", counter.value);
    ///         counter.value += 1;
    ///     }
    /// }
    /// # bevy_ecs::system::assert_is_system(system);
    /// ```
    /// # See also
    ///
    /// - [`iter_many_unique`](Self::iter_many_unique) to get read-only query items.
    /// - [`iter_many_unique_mut`](Self::iter_many_unique_mut) to get mutable query items.
    #[inline]
    pub fn iter_many_unique_inner<EntityList: EntitySet>(
        self,
        entities: EntityList,
    ) -> QueryManyUniqueIter<'w, 's, D, F, EntityList::IntoIter>
    where
        D: IterQueryData,
    {
        // SAFETY: `self.world` has permission to access the required components.
        unsafe {
            QueryManyUniqueIter::new(
                self.world,
                self.state,
                entities,
                self.last_run,
                self.this_run,
            )
        }
    }

    /// Returns an [`Iterator`] over the query items.
    ///
    /// This iterator is always guaranteed to return results from each matching entity once and only once.
    /// Iteration order is not guaranteed.
    ///
    /// If the [`QueryData`] does not implement [`IterQueryData`],
    /// then it is not sound to yield multiple items concurrently
    /// and the resulting [`QueryIter`] will not implement [`Iterator`].
    /// To iterate over the items in that case,
    /// use the [`QueryIter::fetch_next()`](crate::query::QueryIter::fetch_next) method,
    /// which ensures only one item is alive at a time.
    ///
    /// # Safety
    ///
    /// This function makes it possible to violate Rust's aliasing guarantees.
    /// You must make sure this call does not result in multiple mutable references to the same component.
    ///
    /// # See also
    ///
    /// - [`iter`](Self::iter) and [`iter_mut`](Self::iter_mut) for the safe versions.
    #[inline]
    pub unsafe fn iter_unsafe(&self) -> QueryIter<'_, 's, D, F>
    where
        D: IterQueryData,
    {
        // SAFETY: The caller promises that this will not result in multiple mutable references.
        unsafe { self.reborrow_unsafe() }.into_iter()
    }

    /// Iterates over all possible combinations of `K` query items without repetition.
    ///
    /// This iterator is always guaranteed to return results from each unique pair of matching entities.
    /// Iteration order is not guaranteed.
    ///
    /// # Safety
    ///
    /// This allows aliased mutability.
    /// You must make sure this call does not result in multiple mutable references to the same component.
    ///
    /// # See also
    ///
    /// - [`iter_combinations`](Self::iter_combinations) and [`iter_combinations_mut`](Self::iter_combinations_mut) for the safe versions.
    #[inline]
    pub unsafe fn iter_combinations_unsafe<const K: usize>(
        &self,
    ) -> QueryCombinationIter<'_, 's, D, F, K>
    where
        D: IterQueryData,
    {
        // SAFETY: The caller promises that this will not result in multiple mutable references.
        unsafe { self.reborrow_unsafe() }.iter_combinations_inner()
    }

    /// Returns an [`Iterator`] over the query items generated from an [`Entity`] list.
    ///
    /// Items are returned in the order of the list of entities, and may not be unique if the input
    /// doesnn't guarantee uniqueness. Entities that don't match the query are skipped.
    ///
    /// # Safety
    ///
    /// This allows aliased mutability and does not check for entity uniqueness.
    /// You must make sure this call does not result in multiple mutable references to the same component.
    /// Particular care must be taken when collecting the data (rather than iterating over it one item at a time) such as via [`Iterator::collect`].
    ///
    /// # See also
    ///
    /// - [`iter_many_mut`](Self::iter_many_mut) to safely access the query items.
    pub unsafe fn iter_many_unsafe<EntityList: IntoIterator<Item: EntityEquivalent>>(
        &self,
        entities: EntityList,
    ) -> QueryManyIter<'_, 's, D, F, EntityList::IntoIter> {
        // SAFETY: The caller promises that this will not result in multiple mutable references.
        unsafe { self.reborrow_unsafe() }.iter_many_inner(entities)
    }

    /// Returns an [`Iterator`] over the unique query items generated from an [`Entity`] list.
    ///
    /// Items are returned in the order of the list of entities. Entities that don't match the query are skipped.
    ///
    /// # Safety
    ///
    /// This allows aliased mutability.
    /// You must make sure this call does not result in multiple mutable references to the same component.
    ///
    /// # See also
    ///
    /// - [`iter_many_unique`](Self::iter_many_unique) to get read-only query items.
    /// - [`iter_many_unique_mut`](Self::iter_many_unique_mut) to get mutable query items.
    /// - [`iter_many_unique_inner`](Self::iter_many_unique_inner) to get with the actual "inner" world lifetime.
    pub unsafe fn iter_many_unique_unsafe<EntityList: EntitySet>(
        &self,
        entities: EntityList,
    ) -> QueryManyUniqueIter<'_, 's, D, F, EntityList::IntoIter>
    where
        D: IterQueryData,
    {
        // SAFETY: The caller promises that this will not result in multiple mutable references.
        unsafe { self.reborrow_unsafe() }.iter_many_unique_inner(entities)
    }

    /// Returns `true` if there are no query items.
    ///
    /// This is equivalent to `self.iter().next().is_none()`, and thus the worst case runtime will be `O(n)`
    /// where `n` is the number of *potential* matches. This can be notably expensive for queries that rely
    /// on non-archetypal filters such as [`Added`], [`Changed`] or [`Spawned`] which must individually check
    /// each query result for a match.
    ///
    /// # Example
    ///
    /// Here, the score is increased only if an entity with a `Player` component is present in the world:
    ///
    /// ```
    /// # use bevy_ecs::prelude::*;
    /// #
    /// # #[derive(Component)]
    /// # struct Player;
    /// # #[derive(Resource)]
    /// # struct Score(u32);
    /// fn update_score_system(query: Query<(), With<Player>>, mut score: ResMut<Score>) {
    ///     if !query.is_empty() {
    ///         score.0 += 1;
    ///     }
    /// }
    /// # bevy_ecs::system::assert_is_system(update_score_system);
    /// ```
    ///
    /// [`Added`]: crate::query::Added
    /// [`Changed`]: crate::query::Changed
    /// [`Spawned`]: crate::query::Spawned
    #[inline]
    pub fn is_empty(&self) -> bool {
        // If the query data matches every entity, then `as_nop()` can safely
        // skip the cost of initializing the fetch for data that won't be used.
        // if D::IS_ARCHETYPAL {
        //     self.as_nop().iter().next().is_none()
        // } else {
        //     self.iter().next().is_none()
        // }
        todo!()
    }

    /// Returns `true` if the given [`Entity`] matches the query.
    ///
    /// This is always guaranteed to run in `O(1)` time.
    ///
    /// # Example
    ///
    /// ```
    /// # use bevy_ecs::prelude::*;
    /// #
    /// # #[derive(Component)]
    /// # struct InRange;
    /// #
    /// # #[derive(Resource)]
    /// # struct Target {
    /// #     entity: Entity,
    /// # }
    /// #
    /// fn targeting_system(in_range_query: Query<&InRange>, target: Res<Target>) {
    ///     if in_range_query.contains(target.entity) {
    ///         println!("Bam!")
    ///     }
    /// }
    /// # bevy_ecs::system::assert_is_system(targeting_system);
    /// ```
    #[inline]
    pub fn contains(&self, entity: Entity) -> bool {
        todo!()
    }

    /// Returns the query item for the given [`Entity`].
    /// This consumes the [`Query`] to return results with the actual "inner" world lifetime.
    ///
    /// In case of a nonexisting entity or mismatched component, a [`QueryEntityError`] is returned instead.
    ///
    /// This is always guaranteed to run in `O(1)` time.
    ///
    /// # See also
    ///
    /// - [`get_mut`](Self::get_mut) to get the item using a mutable borrow of the [`Query`].
    #[inline]
    pub fn get_inner(self, entity: Entity) -> Result<D::Item<'w, 's>, QueryEntityError> {
        todo!()
    }

    /// Returns the query items for the given array of [`Entity`].
    /// This consumes the [`Query`] to return results with the actual "inner" world lifetime.
    ///
    /// The returned query items are in the same order as the input.
    /// In case of a nonexisting entity or mismatched component, a [`QueryEntityError`] is returned instead.
    ///
    /// # See also
    ///
    /// - [`get_many`](Self::get_many) to get read-only query items without checking for duplicate entities.
    /// - [`get_many_mut`](Self::get_many_mut) to get items using a mutable reference.
    /// - [`get_many_mut_inner`](Self::get_many_mut_inner) to get mutable query items with the actual "inner" world lifetime.
    #[inline]
    pub fn get_many_inner<const N: usize>(
        self,
        entities: [Entity; N],
    ) -> Result<[D::Item<'w, 's>; N], QueryEntityError>
    where
        D: ReadOnlyQueryData,
    {
        todo!()
    }

    /// Returns the query items for the given [`UniqueEntityArray`].
    /// This consumes the [`Query`] to return results with the actual "inner" world lifetime.
    ///
    /// The returned query items are in the same order as the input.
    /// In case of a nonexisting entity, duplicate entities or mismatched component, a [`QueryEntityError`] is returned instead.
    ///
    /// # See also
    ///
    /// - [`get_many_unique`](Self::get_many_unique) to get read-only query items without checking for duplicate entities.
    /// - [`get_many_unique_mut`](Self::get_many_unique_mut) to get items using a mutable reference.
    #[inline]
    pub fn get_many_unique_inner<const N: usize>(
        self,
        entities: UniqueEntityArray<N>,
    ) -> Result<[D::Item<'w, 's>; N], QueryEntityError>
    where
        D: IterQueryData,
    {
        todo!()
    }

    /// Returns the query items for the given array of [`Entity`].
    /// This consumes the [`Query`] to return results with the actual "inner" world lifetime.
    ///
    /// The returned query items are in the same order as the input.
    /// In case of a nonexisting entity, duplicate entities or mismatched component, a [`QueryEntityError`] is returned instead.
    ///
    /// # See also
    ///
    /// - [`get_many`](Self::get_many) to get read-only query items without checking for duplicate entities.
    /// - [`get_many_mut`](Self::get_many_mut) to get items using a mutable reference.
    /// - [`get_many_inner`](Self::get_many_mut_inner) to get read-only query items with the actual "inner" world lifetime.
    #[inline]
    pub fn get_many_mut_inner<const N: usize>(
        self,
        entities: [Entity; N],
    ) -> Result<[D::Item<'w, 's>; N], QueryEntityError>
    where
        D: IterQueryData,
    {
        todo!()
    }
}

impl<'w, 's, D: IterQueryData, F: QueryFilter> IntoIterator for Query<'w, 's, D, F> {
    type Item = D::Item<'w, 's>;
    type IntoIter = QueryIter<'w, 's, D, F>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_inner()
    }
}

// TODO!
