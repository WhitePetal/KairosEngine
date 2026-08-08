mod identifier;

use std::fmt;

pub use identifier::WorldId;

use crate::{
    debug::DebugCheckedUnwrap,
    ecs::{
        archetype::Archetypes,
        bundle::{Bundle, BundleId, BundleInfo, Bundles},
        component::{Component, ComponentId, ComponentIds, Components, ComponentsRegistrator},
        entity::{Entities, EntityAllocator},
        lifecycle::RemovedComponentMessages,
        observer::Observers,
        resource::ResourceEntities,
        storage::Storages,
        world::unsafe_world_cell::UnsafeWorldCell,
    },
};

pub mod unsafe_world_cell;

mod deferred_world;
mod entity_access;
mod entity_fetch;

pub mod error;

pub use deferred_world::DeferredWorld;
pub use entity_access::EntityWorldMut;
pub use entity_fetch::{EntityFetcher, WorldEntityFetch};

/// Stores and exposes operations on [entities](Entity), [components](Component), resources,
/// and their associated metadata.
///
/// Each [`Entity`] has a set of unique components, based on their type.
/// Entity components can be created, updated, removed, and queried using a given [`World`].
///
/// For complex access patterns involving [`SystemParam`](crate::system::SystemParam),
/// consider using [`SystemState`](crate::system::SystemState).
///
/// To mutate different parts of the world simultaneously,
/// use [`World::resource_scope`] or [`SystemState`](crate::system::SystemState).
///
/// ## Resources
///
/// Worlds can also store [`Resource`]s,
/// which are unique instances of a given type that belong to a specific unique Entity.
/// There are also *non send resources*, which can only be accessed on the main thread.
/// These are stored outside of the ECS.
/// See [`Resource`] for usage.
pub struct World {
    id: WorldId,
    pub(crate) entities: Entities,
    pub(crate) entity_allocator: EntityAllocator,
    pub(crate) components: Components,
    pub(crate) component_ids: ComponentIds,
    pub(crate) resource_entities: ResourceEntities,
    pub(crate) archetypes: Archetypes,
    pub(crate) storages: Storages,
    pub(crate) bundles: Bundles,
    pub(crate) observers: Observers,
    pub(crate) removed_components: RemovedComponentMessages,
}

/// Creates an instance of the type this trait is implemented for
/// using data from the supplied [`World`].
///
/// This can be helpful for complex initialization or context-aware defaults.
///
/// [`FromWorld`] may be derived for:
/// - any struct whose fields all implement `FromWorld`
/// - any enum where one variant has the attribute `#[from_world]`
///
/// ```rs
///
/// struct C;
///
/// impl FromWorld for C {
///     fn from_world(_world: &mut World) -> Self {
///         Self
///     }
/// }
///
/// #[derive(FromWorld)]
/// struct D(A, B, C);
///
/// #[derive(FromWorld)]
/// enum E {
///     #[from_world]
///     F,
///     G
/// }
/// ```
pub trait FromWorld {
    fn from_world(world: &mut World) -> Self;
}

impl World {
    pub fn new() -> Self {
        todo!()
    }

    /// Creates a new [`UnsafeWorldCell`] view with complete read+write access.
    #[inline]
    pub fn as_unsafe_world_cell(&mut self) -> UnsafeWorldCell<'_> {
        UnsafeWorldCell::new_mutable(self)
    }

    /// Creates a new [`UnsafeWorldCell`] view with only read access to everything.
    #[inline]
    pub fn as_unsafe_world_cell_readonly(&self) -> UnsafeWorldCell<'_> {
        UnsafeWorldCell::new_readonly(self)
    }

    /// Prepares a [`ComponentsRegistrator`] for the world.
    #[inline]
    pub fn components_registrator(&mut self) -> ComponentsRegistrator<'_> {
        // SAFETY: These are from the same world.
        unsafe { ComponentsRegistrator::new(&mut self.components, &mut self.component_ids) }
    }

    /// Registers a new [`Component`] type and returns the [`ComponentId`] created for it.
    ///
    /// # Usage Notes
    /// In most cases, you don't need to call this method directly since component registration
    /// happens automatically during system initialization.
    #[doc(alias = "register_resource")]
    pub fn register_component<T: Component>(&mut self) -> ComponentId {
        self.components_registrator().register_component::<T>()
    }

    /// Returns the [`ComponentId`] of the given [`Component`] type `T`.
    ///
    /// The returned `ComponentId` is specific to the `World` instance
    /// it was retrieved from and should not be used with another `World` instance.
    ///
    /// Returns [`None`] if the `Component` type has not yet been initialized within
    /// the `World` using [`World::register_component`].
    ///
    /// ```
    /// use bevy_ecs::prelude::*;
    ///
    /// let mut world = World::new();
    ///
    /// #[derive(Component)]
    /// struct ComponentA;
    ///
    /// let component_a_id = world.register_component::<ComponentA>();
    ///
    /// assert_eq!(component_a_id, world.component_id::<ComponentA>().unwrap())
    /// ```
    ///
    /// # See also
    ///
    /// * [`ComponentIdFor`](crate::component::ComponentIdFor)
    /// * [`Components::component_id()`]
    /// * [`Components::get_id()`]
    #[inline]
    pub fn component_id<T: Component>(&self) -> Option<ComponentId> {
        self.components.component_id::<T>()
    }

    // pub fn entity<F: WorldEntity

    /// Registers all of the components in the given [`Bundle`] and returns both the component
    /// ids and the bundle id.
    ///
    /// This is largely equivalent to calling [`register_component`](Self::register_component) on each
    /// component in the bundle.
    #[inline]
    pub fn register_bundle<B: Bundle>(&mut self) -> &BundleInfo {
        let id = self.register_bundle_info::<B>();

        // SAFETY: We just initialized the bundle so its id should definitely be valid.
        unsafe { self.bundles.get(id).debug_checked_unwrap() }
    }

    pub(crate) fn register_bundle_info<B: Bundle>(&mut self) -> BundleId {
        let mut registrator =
            unsafe { ComponentsRegistrator::new(&mut self.components, &mut self.component_ids) };

        // SAFETY: `registrator`, `self.storages` and `self.bundles` all come from this world.
        unsafe {
            self.bundles
                .register_info::<B>(&mut registrator, &mut self.storages)
        }
    }
}

impl fmt::Debug for World {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // SAFETY: `UnsafeWorldCell` requires that this must only access metadata.
        // Accessing any data stored in the world would be unsound.
        f.debug_struct("World")
            .field("id", &self.id)
            .field("entity_count", &self.entities.count_spawned())
            .field("archetype_count", &self.archetypes.len())
            .field("component_count", &self.components.len())
            .finish()
    }
}
