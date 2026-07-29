use std::sync::RwLock;

use crate::{collections::TypeIdMap, ecs::storage::SparseSetIndex};

/// A value which uniquely identifies the type of a [`Component`] or [`Resource`] within a
/// [`World`](crate::world::World).
///
/// Each time a new `Component` type is registered within a `World` using
/// e.g. [`World::register_component`](crate::world::World::register_component) or
/// [`World::register_component_with_descriptor`](crate::world::World::register_component_with_descriptor)
/// or a Resource with e.g. [`World::init_resource`](crate::world::World::init_resource),
/// a corresponding `ComponentId` is created to track it.
///
/// While the distinction between `ComponentId` and [`TypeId`] may seem superficial, breaking them
/// into two separate but related concepts allows components to exist outside of Rust's type system.
/// Each Rust type registered as a `Component` will have a corresponding `ComponentId`, but additional
/// `ComponentId`s may exist in a `World` to track components which cannot be
/// represented as Rust types for scripting or other advanced use-cases.
///
/// A `ComponentId` is tightly coupled to its parent `World`. Attempting to use a `ComponentId` from
/// one `World` to access the metadata of a `Component` in a different `World` is undefined behavior
/// and must not be attempted.
///
/// Given a type `T` which implements [`Component`] (including [`Resource`]), the `ComponentId` for `T` can be retrieved
/// from a `World` using [`World::component_id()`](crate::world::World::component_id) or via [`Components::component_id()`].
#[derive(Debug, Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct ComponentId(pub(super) usize);

impl ComponentId {
    /// Creates a new [`ComponentId`].
    ///
    /// The `index` is a unique value associated with each type of component in a given world.
    /// Usually, this value is taken from a counter incremented for each type of component registered with the world.
    #[inline]
    pub const fn new(index: usize) -> ComponentId {
        ComponentId(index)
    }

    /// Returns the index of the current component.
    #[inline]
    pub fn index(self) -> usize {
        self.0
    }
}

pub struct ComponentInfo {
    pub(super) id: ComponentId,
}

impl SparseSetIndex for ComponentId {
    #[inline]
    fn sparse_set_index(&self) -> usize {
        self.index()
    }

    #[inline]
    fn get_sparse_set_index(value: usize) -> Self {
        Self(value)
    }
}

/// Stores metadata associated with each kind of [`Component`] in a given [`World`](crate::world::World).
#[derive(Debug, Default)]
pub struct Components {
    pub(super) components: Vec<Option<ComponentInfo>>,
    pub(super) indices: TypeIdMap<ComponentId>,
    // This is kept internal and local to verify that no deadlocks can occur.
    pub(super) queued: RwLock<QueuedCommponents>,
}
