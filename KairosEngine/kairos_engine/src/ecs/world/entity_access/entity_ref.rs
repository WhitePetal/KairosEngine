use std::any::TypeId;

use crate::{
    debug::MaybeLocation,
    ecs::{
        archetype::Archetype,
        change_detection::{ComponentTicks, Ref},
        component::{Component, ComponentId},
        entity::{Entity, EntityLocation},
        query::{
            QueryAccessError, ReadOnlyQueryData, ReleaseStateQueryData, SingleEntityQueryData,
        },
        world::unsafe_world_cell::UnsafeEntityCell,
    },
};

/// A read-only reference to a particular [`Entity`] and all of its components.
///
/// # Examples
///
/// Read-only access disjoint with mutable access.
///
/// ```
/// # use bevy_ecs::prelude::*;
/// # #[derive(Component)] pub struct A;
/// # #[derive(Component)] pub struct B;
/// fn disjoint_system(
///     query1: Query<&mut A>,
///     query2: Query<EntityRef, Without<A>>,
/// ) {
///     // ...
/// }
/// # bevy_ecs::system::assert_is_system(disjoint_system);
/// ```
#[derive(Copy, Clone)]
pub struct EntityRef<'w> {
    cell: UnsafeEntityCell<'w>,
}

impl<'w> EntityRef<'w> {
    /// # Safety
    /// - `cell` must have permission to read every component of the entity.
    /// - No mutable accesses to any of the entity's components may exist
    ///   at the same time as the returned [`EntityRef`].
    #[inline]
    pub(crate) unsafe fn new(cell: UnsafeEntityCell<'w>) -> Self {
        Self { cell }
    }

    /// Returns the [ID](Entity) of the current entity.
    #[inline]
    #[must_use = "Omit the .id() call if you do not need to store the `Entity` identifier."]
    pub fn id(&self) -> Entity {
        self.cell.id()
    }

    /// Gets metadata indicating the location where the current entity is stored.
    #[inline]
    pub fn location(&self) -> EntityLocation {
        self.cell.location()
    }

    /// Returns the archetype that the current entity belongs to.
    #[inline]
    pub fn archetype(&self) -> &Archetype {
        self.cell.archetype()
    }

    /// Returns `true` if the current entity has a component of type `T`.
    /// Otherwise, this returns `false`.
    ///
    /// ## Notes
    ///
    /// If you do not know the concrete type of a component, consider using
    /// [`Self::contains_id`] or [`Self::contains_type_id`].
    #[inline]
    pub fn contains<T: Component>(&self) -> bool {
        self.contains_type_id(TypeId::of::<T>())
    }

    /// Returns `true` if the current entity has a component identified by `component_id`.
    /// Otherwise, this returns false.
    ///
    /// ## Notes
    ///
    /// - If you know the concrete type of the component, you should prefer [`Self::contains`].
    /// - If you know the component's [`TypeId`] but not its [`ComponentId`], consider using
    ///   [`Self::contains_type_id`].
    #[inline]
    pub fn contains_id(&self, component_id: ComponentId) -> bool {
        self.cell.contains_id(component_id)
    }

    /// Returns `true` if the current entity has a component with the type identified by `type_id`.
    /// Otherwise, this returns false.
    ///
    /// ## Notes
    ///
    /// - If you know the concrete type of the component, you should prefer [`Self::contains`].
    /// - If you have a [`ComponentId`] instead of a [`TypeId`], consider using [`Self::contains_id`].
    #[inline]
    pub fn contains_type_id(&self, type_id: TypeId) -> bool {
        self.cell.contains_type_id(type_id)
    }

    /// Gets access to the component of type `T` for the current entity.
    /// Returns `None` if the entity does not have a component of type `T`.
    #[inline]
    pub fn get<T: Component>(&self) -> Option<&'w T> {
        // SAFETY: We have read-only access to all components of this entity.
        unsafe { self.cell.get::<T>() }
    }

    /// Gets access to the component of type `T` for the current entity,
    /// including change detection information as a [`Ref`].
    ///
    /// Returns `None` if the entity does not have a component of type `T`.
    #[inline]
    pub fn get_ref<T: Component>(&self) -> Option<Ref<'w, T>> {
        // SAFETY: We have read-only access to all components of this entity.
        unsafe { self.cell.get_ref::<T>() }
    }

    /// Retrieves the change ticks for the given component. This can be useful for implementing change
    /// detection in custom runtimes.
    #[inline]
    pub fn get_change_ticks<T: Component>(&self) -> Option<ComponentTicks> {
        // SAFETY: We have read-only access to all components of this entity.
        unsafe { self.cell.get_change_ticks::<T>() }
    }

    /// Get the [`MaybeLocation`] from where the given [`Component`] was last changed from.
    /// This contains information regarding the last place (in code) that changed this component and can be useful for debugging.
    /// For more information, see [`Location`](https://doc.rust-lang.org/nightly/core/panic/struct.Location.html), and enable the `track_location` feature.
    pub fn get_changed_by<T: Component>(&self) -> Option<MaybeLocation> {
        // SAFETY: We have read-only access to all components of this entity.
        unsafe { self.cell.get_changed_by::<T>() }
    }

    /// Retrieves the change ticks for the given [`ComponentId`]. This can be useful for implementing change
    /// detection in custom runtimes.
    ///
    /// **You should prefer to use the typed API [`EntityRef::get_change_ticks`] where possible and only
    /// use this in cases where the actual component types are not known at
    /// compile time.**
    #[inline]
    pub fn get_change_ticks_by_id(&self, component_id: ComponentId) -> Option<ComponentTicks> {
        // SAFETY: We have read-only access to all components of this entity.
        unsafe { self.cell.get_change_ticks_by_id(component_id) }
    }

    /// Returns read-only components for the current entity that match the query `Q`,
    /// or `None` if the entity does not have the components required by the query `Q`.
    pub fn get_components<Q: ReadOnlyQueryData + ReleaseStateQueryData + SingleEntityQueryData>(
        &self,
    ) -> Result<Q::Item<'w, 'static>, QueryAccessError> {
        // SAFETY:
        // - We have read-only access to all components of this entity.
        // - The query is read-only, and read-only references cannot have conflicts.
        unsafe { self.cell.get_components::<Q>() }
    }
}

// TODO!
