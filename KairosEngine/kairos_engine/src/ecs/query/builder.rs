use std::marker::PhantomData;

use crate::ecs::{component::StorageType, query::{FilteredAccess, QueryData, QueryFilter}, world::World};



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
    _marker: PhantomData<(D, F)>
}

impl<'w, D: QueryData, F: QueryFilter> QueryBuilder<'w, D, F> {
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

    /// Returns a reference to the [`FilteredAccess`] that will be provided to the built [`Query`].
    pub fn access(&self) -> &FilteredAccess {
        &self.access
    }
}

// TODO!
