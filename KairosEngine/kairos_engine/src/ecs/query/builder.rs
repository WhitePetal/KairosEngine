use std::marker::PhantomData;

use crate::ecs::{
    component::{Component, ComponentId, StorageType},
    query::{FilteredAccess, QueryData, QueryFilter, QueryState, With, Without},
    world::World,
};

#[cfg(test)]
mod tests;

/// Builder struct to create [`QueryState`] instances at runtime.
///
/// ```
/// # use bevy_ecs::prelude::*;
/// #
/// # #[derive(Component)]
/// # struct A;
/// #
/// # #[derive(Component)]
/// # struct B;
/// #
/// # #[derive(Component)]
/// # struct C;
/// #
/// let mut world = World::new();
/// let entity_a = world.spawn((A, B)).id();
/// let entity_b = world.spawn((A, C)).id();
///
/// // Instantiate the builder using the type signature of the iterator you will consume
/// let mut query = QueryBuilder::<(Entity, &B)>::new(&mut world)
/// // Add additional terms through builder methods
///     .with::<A>()
///     .without::<C>()
///     .build();
///
/// // Consume the QueryState
/// let (entity, b) = query.single(&world).unwrap();
/// ```
pub struct QueryBuilder<'w, D: QueryData = (), F: QueryFilter = ()> {
    access: FilteredAccess,
    world: &'w mut World,
    or: bool,
    first: bool,
    _marker: PhantomData<(D, F)>,
}

impl<'w, D: QueryData, F: QueryFilter> QueryBuilder<'w, D, F> {
    /// Creates a new builder with the accesses required for `D` and `F`
    pub fn new(world: &'w mut World) -> Self {
        let fetch_state = D::init_state(world);
        let filter_state = F::init_state(world);

        let mut access = FilteredAccess::default();
        D::update_component_access(&fetch_state, &mut access);

        // Use a temporary empty FilteredAccess for filters. This prevents them from conflicting with the
        // main Query's `fetch_state` access. Filters are allowed to conflict with the main query fetch
        // because they are evaluated *before* a specific reference is constructed.
        let mut filter_access = FilteredAccess::default();
        F::update_component_access(&filter_state, &mut filter_access);

        // Merge the temporary filter access with the main access. This ensures that filter access is
        // properly considered in a global "cross-query" context (both within systems and across systems).
        access.extend(&filter_access);

        Self {
            access,
            world,
            or: false,
            first: false,
            _marker: PhantomData,
        }
    }

    pub(super) fn is_dense(&self) -> bool {
        // Note: `component_id` comes from the user in safe code, so we cannot trust it to
        // exist. If it doesn't exist we pessimistically assume it's sparse.
        let is_dense = |component_id| {
            self.world()
                .components()
                .get_info(component_id)
                .is_some_and(|info| info.storage_type() == StorageType::Table)
        };

        // Use dense iteration if possible, but fall back to sparse if we need to.
        // Both `D` and `F` must allow dense iteration, just as for queries without dynamic filters.
        // All `with` and `without` filters must be dense to ensure that we match all archetypes in a table.
        // We also need to ensure that any sparse set components in `access.required` cause sparse iteration,
        // but anything that adds a `required` component also adds a `with` filter.
        //
        // Note that `EntityRef` and `EntityMut` types, including `FilteredEntityRef` and `FilteredEntityMut`, have `D::IS_DENSE = true`.
        // Calling `builder.data::<&Sparse>()` will add a filter and force sparse iteration,
        // but calling `builder.data::<Option<&Sparse>>()` will still allow them to use dense iteration!
        D::IS_DENSE
            && F::IS_DENSE
            && self.access.with_filters().all(is_dense)
            && self.access.without_filters().all(is_dense)
    }

    /// Returns a reference to the world passed to [`Self::new`].
    pub fn world(&self) -> &World {
        self.world
    }

    /// Returns a mutable reference to the world passed to [`Self::new`].
    pub fn world_mut(&mut self) -> &mut World {
        self.world
    }

    /// Adds access to self's underlying [`FilteredAccess`] respecting [`Self::or`] and [`Self::and`]
    pub fn extend_access(&mut self, mut access: FilteredAccess) {
        if self.or {
            if self.first {
                access.required.clear();
                self.access.extend(&access);
                self.first = false;
            } else {
                self.access.append_or(&access);
            }
        } else {
            self.access.extend(&access);
        }
    }

    /// Adds accesses required for `T` to self.
    pub fn data<T: QueryData>(&mut self) -> &mut Self {
        let state = T::init_state(self.world);
        let mut access = FilteredAccess::default();
        T::update_component_access(&state, &mut access);
        self.extend_access(access);
        self
    }

    /// Adds filter from `T` to self.
    pub fn filter<T: QueryFilter>(&mut self) -> &mut Self {
        let state = T::init_state(self.world);
        let mut access = FilteredAccess::default();
        T::update_component_access(&state, &mut access);
        self.extend_access(access);
        self
    }

    /// Adds [`With<T>`] to the [`FilteredAccess`] of self.
    pub fn with<T: Component>(&mut self) -> &mut Self {
        self.filter::<With<T>>();
        self
    }

    /// Adds [`With<T>`] to the [`FilteredAccess`] of self from a runtime [`ComponentId`].
    pub fn with_id(&mut self, id: ComponentId) -> &mut Self {
        let mut access = FilteredAccess::default();
        access.and_with(id);
        self.extend_access(access);
        self
    }

    /// Adds [`Without<T>`] to the [`FilteredAccess`] of self.
    pub fn without<T: Component>(&mut self) -> &mut Self {
        self.filter::<Without<T>>();
        self
    }

    /// Adds [`Without<T>`] to the [`FilteredAccess`] of self from a runtime [`ComponentId`].
    pub fn without_id(&mut self, id: ComponentId) -> &mut Self {
        let mut access = FilteredAccess::default();
        access.and_without(id);
        self.extend_access(access);
        self
    }

    /// Adds `&T` to the [`FilteredAccess`] of self.
    pub fn ref_id(&mut self, id: ComponentId) -> &mut Self {
        self.with_id(id);
        self.access.add_read(id);
        self
    }

    /// Adds `&mut T` to the [`FilteredAccess`] of self.
    pub fn mut_id(&mut self, id: ComponentId) -> &mut Self {
        self.with_id(id);
        self.access.add_write(id);
        self
    }

    /// Takes a function over mutable access to a [`QueryBuilder`], calls that function
    /// on an empty builder and then adds all accesses from that builder to self as optional.
    pub fn optional(&mut self, f: impl Fn(&mut QueryBuilder)) -> &mut Self {
        let mut builder = QueryBuilder::new(self.world);
        f(&mut builder);
        self.access.extend_access(builder.access());
        self
    }

    /// Takes a function over mutable access to a [`QueryBuilder`], calls that function
    /// on an empty builder and then adds all accesses from that builder to self.
    ///
    /// Primarily used when inside a [`Self::or`] closure to group several terms.
    pub fn and(&mut self, f: impl Fn(&mut QueryBuilder)) -> &mut Self {
        let mut builder = QueryBuilder::new(self.world);
        f(&mut builder);
        let access = builder.access().clone();
        self.extend_access(access);
        self
    }

    /// Takes a function over mutable access to a [`QueryBuilder`], calls that function
    /// on an empty builder, all accesses added to that builder will become terms in an or expression.
    ///
    /// ```
    /// # use bevy_ecs::prelude::*;
    /// #
    /// # #[derive(Component)]
    /// # struct A;
    /// #
    /// # #[derive(Component)]
    /// # struct B;
    /// #
    /// # let mut world = World::new();
    /// #
    /// QueryBuilder::<Entity>::new(&mut world).or(|builder| {
    ///     builder.with::<A>();
    ///     builder.with::<B>();
    /// });
    /// // is equivalent to
    /// QueryBuilder::<Entity>::new(&mut world).filter::<Or<(With<A>, With<B>)>>();
    /// ```
    pub fn or(&mut self, f: impl Fn(&mut QueryBuilder)) -> &mut Self {
        let mut builder = QueryBuilder::new(self.world);
        builder.or = true;
        builder.first = true;
        f(&mut builder);
        self.access.extend(builder.access());
        self
    }

    /// Returns a reference to the [`FilteredAccess`] that will be provided to the built [`Query`].
    pub fn access(&self) -> &FilteredAccess {
        &self.access
    }

    /// Transmute the existing builder adding required accesses.
    /// This will maintain all existing accesses.
    ///
    /// If including a filter type see [`Self::transmute_filtered`]
    pub fn transmute<NewD: QueryData>(&mut self) -> &mut QueryBuilder<'w, NewD> {
        self.transmute_filtered::<NewD, ()>()
    }

    /// Transmute the existing builder adding required accesses.
    /// This will maintain all existing accesses.
    pub fn transmute_filtered<NewD: QueryData, NewF: QueryFilter>(
        &mut self,
    ) -> &mut QueryBuilder<'w, NewD, NewF> {
        let fetch_state = NewD::init_state(self.world);
        let filter_state = NewF::init_state(self.world);

        let mut access = FilteredAccess::default();
        NewD::update_component_access(&fetch_state, &mut access);
        NewF::update_component_access(&filter_state, &mut access);

        self.extend_access(access);
        // SAFETY:
        // - We have included all required accesses for NewQ and NewF
        // - The layout of all QueryBuilder instances is the same
        unsafe { std::mem::transmute(self) }
    }

    /// Create a [`QueryState`] with the accesses of the builder.
    ///
    /// Takes `&mut self` to access the inner world reference while initializing
    /// state for the new [`QueryState`]
    pub fn build(&mut self) -> QueryState<D, F> {
        QueryState::<D, F>::from_builder(self)
    }
}
