use crate::ecs::{
    change_detection::Tick, query::Access, world::unsafe_world_cell::UnsafeWorldCell,
};

/// Provides read-only access to a set of [`Resource`]s defined by the contained [`Access`].
///
/// Use [`FilteredResourcesMut`] if you need mutable access to some resources.
///
/// To be useful as a [`SystemParam`](crate::system::SystemParam),
/// this must be configured using a [`FilteredResourcesParamBuilder`](crate::system::FilteredResourcesParamBuilder)
/// to build the system using a [`SystemParamBuilder`](crate::prelude::SystemParamBuilder).
///
/// # Examples
///
/// ```
/// # use bevy_ecs::{prelude::*, system::*};
/// #
/// # #[derive(Default, Resource)]
/// # struct A;
/// #
/// # #[derive(Default, Resource)]
/// # struct B;
/// #
/// # #[derive(Default, Resource)]
/// # struct C;
/// #
/// # let mut world = World::new();
/// // Use `FilteredResourcesParamBuilder` to declare access to resources.
/// let system = (FilteredResourcesParamBuilder::new(|builder| {
///     builder.add_read::<B>().add_read::<C>();
/// }),)
///     .build_state(&mut world)
///     .build_system(resource_system);
///
/// world.init_resource::<A>();
/// world.init_resource::<C>();
///
/// fn resource_system(res: FilteredResources) {
///     // The resource exists, but we have no access, so we can't read it.
///     assert!(res.get::<A>().is_err());
///     // The resource doesn't exist, so we can't read it.
///     assert!(res.get::<B>().is_err());
///     // The resource exists and we have access, so we can read it.
///     let c = res.get::<C>().unwrap();
///     // The type parameter can be left out if it can be determined from use.
///     let c: Ref<C> = res.get().unwrap();
/// }
/// #
/// # world.run_system_once(system);
/// ```
///
/// This can be used alongside ordinary [`Res`](crate::system::Res) and [`ResMut`](crate::system::ResMut) parameters if they do not conflict.
///
/// ```
/// # use bevy_ecs::{prelude::*, system::*};
/// #
/// # #[derive(Default, Resource)]
/// # struct A;
/// #
/// # #[derive(Default, Resource)]
/// # struct B;
/// #
/// # let mut world = World::new();
/// # world.init_resource::<A>();
/// # world.init_resource::<B>();
/// #
/// let system = (
///     FilteredResourcesParamBuilder::new(|builder| {
///         builder.add_read::<A>();
///     }),
///     ParamBuilder,
///     ParamBuilder,
/// )
///     .build_state(&mut world)
///     .build_system(resource_system);
///
/// // Read access to A does not conflict with read access to A or write access to B.
/// fn resource_system(filtered: FilteredResources, res_a: Res<A>, res_mut_b: ResMut<B>) {
///     let res_a_2: Ref<A> = filtered.get::<A>().unwrap();
/// }
/// #
/// # world.run_system_once(system);
/// ```
///
/// But it will conflict if it tries to read the same resource that another parameter writes.
///
/// ```should_panic
/// # use bevy_ecs::{prelude::*, system::*};
/// #
/// # #[derive(Default, Resource)]
/// # struct A;
/// #
/// # let mut world = World::new();
/// # world.init_resource::<A>();
/// #
/// let system = (
///     FilteredResourcesParamBuilder::new(|builder| {
///         builder.add_read::<A>();
///     }),
///     ParamBuilder,
/// )
///     .build_state(&mut world)
///     .build_system(invalid_resource_system);
///
/// // Read access to A conflicts with write access to A.
/// fn invalid_resource_system(filtered: FilteredResources, res_mut_a: ResMut<A>) { }
/// #
/// # world.run_system_once(system);
/// ```
#[derive(Clone, Copy)]
pub struct FilteredResources<'w, 's> {
    world: UnsafeWorldCell<'w>,
    access: &'s Access,
    last_run: Tick,
    this_run: Tick,
}

impl<'w, 's> FilteredResources<'w, 's> {
    /// Creates a new [`FilteredResources`].
    /// # Safety
    /// It is the callers responsibility to ensure that nothing else may access the any resources in the `world` in a way that conflicts with `access`.
    pub(crate) unsafe fn new(
        world: UnsafeWorldCell<'w>,
        access: &'s Access,
        last_run: Tick,
        this_run: Tick,
    ) -> Self {
        Self {
            world,
            access,
            last_run,
            this_run,
        }
    }
}

/// Provides mutable access to a set of [`Resource`]s defined by the contained [`Access`].
///
/// Use [`FilteredResources`] if you only need read-only access to resources.
///
/// To be useful as a [`SystemParam`](crate::system::SystemParam),
/// this must be configured using a [`FilteredResourcesMutParamBuilder`](crate::system::FilteredResourcesMutParamBuilder)
/// to build the system using a [`SystemParamBuilder`](crate::prelude::SystemParamBuilder).
///
/// # Examples
///
/// ```
/// # use bevy_ecs::{prelude::*, system::*};
/// #
/// # #[derive(Default, Resource)]
/// # struct A;
/// #
/// # #[derive(Default, Resource)]
/// # struct B;
/// #
/// # #[derive(Default, Resource)]
/// # struct C;
/// #
/// # #[derive(Default, Resource)]
/// # struct D;
/// #
/// # let mut world = World::new();
/// // Use `FilteredResourcesMutParamBuilder` to declare access to resources.
/// let system = (FilteredResourcesMutParamBuilder::new(|builder| {
///     builder.add_write::<B>().add_read::<C>().add_write::<D>();
/// }),)
///     .build_state(&mut world)
///     .build_system(resource_system);
///
/// world.init_resource::<A>();
/// world.init_resource::<C>();
/// world.init_resource::<D>();
///
/// fn resource_system(mut res: FilteredResourcesMut) {
///     // The resource exists, but we have no access, so we can't read it or write it.
///     assert!(res.get::<A>().is_err());
///     assert!(res.get_mut::<A>().is_err());
///     // The resource doesn't exist, so we can't read it or write it.
///     assert!(res.get::<B>().is_err());
///     assert!(res.get_mut::<B>().is_err());
///     // The resource exists and we have read access, so we can read it but not write it.
///     let c = res.get::<C>().unwrap();
///     assert!(res.get_mut::<C>().is_err());
///     // The resource exists and we have write access, so we can read it or write it.
///     let d = res.get::<D>().unwrap();
///     let d = res.get_mut::<D>().unwrap();
///     // The type parameter can be left out if it can be determined from use.
///     let c: Ref<C> = res.get().unwrap();
/// }
/// #
/// # world.run_system_once(system);
/// ```
///
/// This can be used alongside ordinary [`Res`](crate::system::ResMut) and [`ResMut`](crate::system::ResMut) parameters if they do not conflict.
///
/// ```
/// # use bevy_ecs::{prelude::*, system::*};
/// #
/// # #[derive(Default, Resource)]
/// # struct A;
/// #
/// # #[derive(Default, Resource)]
/// # struct B;
/// #
/// # #[derive(Default, Resource)]
/// # struct C;
/// #
/// # let mut world = World::new();
/// # world.init_resource::<A>();
/// # world.init_resource::<B>();
/// # world.init_resource::<C>();
/// #
/// let system = (
///     FilteredResourcesMutParamBuilder::new(|builder| {
///         builder.add_read::<A>().add_write::<B>();
///     }),
///     ParamBuilder,
///     ParamBuilder,
/// )
///     .build_state(&mut world)
///     .build_system(resource_system);
///
/// // Read access to A does not conflict with read access to A or write access to C.
/// // Write access to B does not conflict with access to A or C.
/// fn resource_system(mut filtered: FilteredResourcesMut, res_a: Res<A>, res_mut_c: ResMut<C>) {
///     let res_a_2: Ref<A> = filtered.get::<A>().unwrap();
///     let res_mut_b: Mut<B> = filtered.get_mut::<B>().unwrap();
/// }
/// #
/// # world.run_system_once(system);
/// ```
///
/// But it will conflict if it tries to read the same resource that another parameter writes,
/// or write the same resource that another parameter reads.
///
/// ```should_panic
/// # use bevy_ecs::{prelude::*, system::*};
/// #
/// # #[derive(Default, Resource)]
/// # struct A;
/// #
/// # let mut world = World::new();
/// # world.init_resource::<A>();
/// #
/// let system = (
///     FilteredResourcesMutParamBuilder::new(|builder| {
///         builder.add_write::<A>();
///     }),
///     ParamBuilder,
/// )
///     .build_state(&mut world)
///     .build_system(invalid_resource_system);
///
/// // Read access to A conflicts with write access to A.
/// fn invalid_resource_system(filtered: FilteredResourcesMut, res_a: Res<A>) { }
/// #
/// # world.run_system_once(system);
/// ```
pub struct FilteredResourcesMut<'w, 's> {
    world: UnsafeWorldCell<'w>,
    access: &'s Access,
    last_run: Tick,
    this_run: Tick,
}

impl<'w, 's> FilteredResourcesMut<'w, 's> {
    /// Creates a new [`FilteredResources`].
    /// # Safety
    /// It is the callers responsibility to ensure that nothing else may access the any resources in the `world` in a way that conflicts with `access`.
    pub(crate) unsafe fn new(
        world: UnsafeWorldCell<'w>,
        access: &'s Access,
        last_run: Tick,
        this_run: Tick,
    ) -> Self {
        Self {
            world,
            access,
            last_run,
            this_run,
        }
    }
}

// TODO!
