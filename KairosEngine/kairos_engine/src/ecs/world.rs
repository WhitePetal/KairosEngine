mod identifier;

pub use identifier::WorldId;

use crate::ecs::{
    component::{Component, ComponentId, ComponentIds, Components, ComponentsRegistrator},
    entity::{Entities, EntityAllocator},
    storage::Storages,
    world::unsafe_world_cell::UnsafeWorldCell,
};

pub mod unsafe_world_cell;

mod deferred_world;
mod entity_access;

pub use deferred_world::DeferredWorld;
pub use entity_access::EntityWorldMut;

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
    pub(crate) storages: Storages,
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
    /// Creates a new [`UnsafeWorldCell`] view with complete read+write access.
    #[inline]
    pub fn as_unsafe_world_cell(&mut self) -> UnsafeWorldCell<'_> {
        UnsafeWorldCell::new_mutable(self)
    }

    /// Prepares a [`ComponentsRegistrator`] for the world.
    #[inline]
    pub fn components_registrator(&mut self) -> ComponentsRegistrator<'_> {
        // SAFETY: These are from the same world.
        unsafe { ComponentsRegistrator::new(&mut self.components, &mut self.component_ids) }
    }

    pub fn register_component<T: Component>(&mut self) -> ComponentId {
        todo!()
    }
}
