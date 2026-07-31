use std::{alloc::Layout, any::TypeId, fmt::Debug, mem::needs_drop, sync::RwLock};

use crate::{
    collections::TypeIdMap,
    debug::DebugName,
    ecs::{
        component::{Component, ComponentCloneBehavior, StorageType}, relationship::MaybeRelationshipAccessor, storage::SparseSetIndex
    },
    ptr::OwningPtr,
};

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

/// A value describing a component or resource, which may or may not correspond to a Rust type.
#[derive(Clone)]
pub struct ComponentDescriptor {
    name: DebugName,
    // SAFETY: This must remain private. It must match the statically known StorageType of the
    // associated rust component type if one exists.
    storage_type: StorageType,
    // SAFETY: This must remain private. It must only be set to "true" if this component is
    // actually Send + Sync
    is_send_and_sync: bool,
    type_id: Option<TypeId>,
    // SAFETY: This must always have `size()` that is a multiple of `align()`.
    // `BlobArray` relies on that to calculate byte offsets as a multiple of `size()`.
    layout: Layout,
    // SAFETY: this function must be safe to call with pointers pointing to items of the type
    // this descriptor describes.
    // None if the underlying type doesn't need to be dropped
    drop: Option<for<'a> unsafe fn(OwningPtr<'a>)>,
    mutable: bool,
    clone_behavior: ComponentCloneBehavior,
    relationship_accessor: MaybeRelationshipAccessor,
}

// We need to ignore the `drop` field in our `Debug` impl
impl Debug for ComponentDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComponentDescriptor")
            .field("name", &self.name)
            .field("storage_type", &self.storage_type)
            .field("is_send_and_sync", &self.is_send_and_sync)
            .field("type_id", &self.type_id)
            .field("layout", &self.layout)
            .field("mutable", &self.mutable)
            .field("clone_behavior", &self.clone_behavior)
            .field("relationship_accessor", &self.relationship_accessor)
            .finish()
    }
}

impl ComponentDescriptor {
    /// # Safety
    ///
    /// `x` must point to a valid value of type `T`.
    unsafe fn drop_ptr<T>(x: OwningPtr<'_>) {
        // SAFETY: Contract is required to be upheld by the caller.
        unsafe {
            x.drop_as::<T>();
        }
    }

    pub fn new<T: Component>() -> Self {
        Self {
            name: DebugName::type_name::<T>(),
            storage_type: T::STORAGE_TYPE,
            is_send_and_sync: true,
            type_id: Some(TypeId::of::<T>()),
            // `T` is a rust type, so the layout will have `size()` as a multiple of `align()`
            layout: Layout::new::<T>(),
            drop: needs_drop::<T>().then_some(Self::drop_ptr::<T> as _),
            mutable: T::Mutability::MUTABLE,
            clone_behavior: T::clone_behavior(),
            relationship_accessor: T::relationship_accessor().map(|v| v.initializer).into(),
        }
    }

    // TODO!
}

pub struct ComponentInfo {
    pub(super) id: ComponentId,
    pub(super) descriptor: ComponentDescriptor,
}

/// Stores metadata associated with each kind of [`Component`] in a given [`World`](crate::world::World).
#[derive(Debug, Default)]
pub struct Components {
    pub(super) components: Vec<Option<ComponentInfo>>,
    pub(super) indices: TypeIdMap<ComponentId>,
    // This is kept internal and local to verify that no deadlocks can occur.
    pub(super) queued: RwLock<QueuedCommponents>,
}
