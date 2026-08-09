//! Resources are unique, singleton-like data types that can be accessed from systems and stored in the [`World`](crate::world::World).

use std::ops::Deref;

use crate::{
    cell::SyncUnsafeCell,
    ecs::{
        component::{Component, ComponentId},
        entity::Entity,
        storage::SparseArray,
    },
};

/// A type that can be inserted into a [`World`] as a singleton.
///
/// You can access resource data in systems using the [`Res`] and [`ResMut`] system parameters
///
/// Only one resource of each type can be stored in a [`World`] at any given time.
///
/// # Examples
///
/// ```
/// # let mut world = World::default();
/// # let mut schedule = Schedule::default();
/// # use bevy_ecs::prelude::*;
/// #[derive(Resource)]
/// struct MyResource { value: u32 }
///
/// world.insert_resource(MyResource { value: 42 });
///
/// fn read_resource_system(resource: Res<MyResource>) {
///     assert_eq!(resource.value, 42);
/// }
///
/// fn write_resource_system(mut resource: ResMut<MyResource>) {
///     assert_eq!(resource.value, 42);
///     resource.value = 0;
///     assert_eq!(resource.value, 0);
/// }
/// # schedule.add_systems((read_resource_system, write_resource_system).chain());
/// # schedule.run(&mut world);
/// ```
///
/// # `!Sync` Resources
/// A `!Sync` type cannot implement `Resource`. However, it is possible to wrap a `Send` but not `Sync`
/// type in [`SyncCell`] or the currently unstable [`Exclusive`] to make it `Sync`. This forces only
/// having mutable access (`&mut T` only, never `&T`), but makes it safe to reference across multiple
/// threads.
///
/// This will fail to compile since `RefCell` is `!Sync`.
/// ```compile_fail
/// # use std::cell::RefCell;
/// # use bevy_ecs::resource::Resource;
///
/// #[derive(Resource)]
/// struct NotSync {
///    counter: RefCell<usize>,
/// }
/// ```
///
/// This will compile since the `RefCell` is wrapped with `SyncCell`.
/// ```
/// # use std::cell::RefCell;
/// # use bevy_ecs::resource::Resource;
/// use bevy_platform::cell::SyncCell;
///
/// #[derive(Resource)]
/// struct ActuallySync {
///    counter: SyncCell<RefCell<usize>>,
/// }
/// ```
///
/// [`Exclusive`]: https://doc.rust-lang.org/nightly/std/sync/struct.Exclusive.html
/// [`World`]: crate::world::World
/// [`Res`]: crate::system::Res
/// [`ResMut`]: crate::system::ResMut
/// [`SyncCell`]: bevy_platform::cell::SyncCell
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a `Resource`",
    label = "invalid `Resource`",
    note = "consider annotating `{Self}` with `#[derive(Resource)]`"
)]
pub trait Resource: Component {}

/// A cache that links each `ComponentId` from a resource to the corresponding entity.
#[derive(Default)]
pub struct ResourceEntities(SyncUnsafeCell<SparseArray<ComponentId, Entity>>);

impl ResourceEntities {
    #[inline]
    fn deref(&self) -> &SparseArray<ComponentId, Entity> {
        // SAFETY: There are no other mutable references to the map.
        // The underlying `SyncUnsafeCell` is never exposed outside this module,
        // so mutable references are only created by the resource hooks.
        // We only expose `&ResourceCache` to code with access to a resource (such as `&World`),
        // and that would conflict with the `DeferredWorld` passed to the resource hook.
        unsafe { &*self.0.get() }
    }

    /// Returns an iterator over all registered resource components and their corresponding entity.
    ///
    /// This must scan the entire array of components to find non-empty values,
    /// which may be slow even if there are few resources.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (ComponentId, Entity)> {
        self.deref().iter().map(|(id, entity)| (id, *entity))
    }

    /// Returns the entity for the given resource component, or `None` if there is no entity.
    #[inline]
    pub fn get(&self, id: ComponentId) -> Option<Entity> {
        self.deref().get(id).copied()
    }
}

/// [`ComponentId`] of the [`IsResource`] component.
pub const IS_RESOURCE: ComponentId = ComponentId::new(crate::ecs::component::IS_RESOURCE);

// TODO!

// /// A marker component for entities that have a Resource component.
// #[derive(Component, Debug)]
// #[component(on_insert, on_discard, on_despawn)]
// pub struct IsResource(ComponentId);

// impl IsResource {
//     /// Creates a new instance with the given `component_id`
//     pub fn new(component_id: ComponentId) -> Self {
//         Self(component_id)
//     }

//     /// The [`ComponentId`] of the resource component (the _actual_ resource value component, not the [`IsResource`] component).
//     pub fn resource_component_id(&self) -> ComponentId {
//         self.0
//     }

// }
