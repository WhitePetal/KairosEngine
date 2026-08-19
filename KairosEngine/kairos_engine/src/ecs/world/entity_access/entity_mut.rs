use std::any::TypeId;

use crate::ecs::{
    archetype::Archetype,
    change_detection::Mut,
    component::{Component, ComponentId, Mutable},
    entity::{Entity, EntityLocation},
    query::{Access, ReadOnlyQueryData, ReleaseStateQueryData, SingleEntityQueryData},
    world::{
        EntityRef, FilteredEntityMut, entity_access::DynamicComponentFetch,
        error::EntityComponentError, unsafe_world_cell::UnsafeEntityCell,
    },
};

/// Provides mutable access to a single entity and all of its components.
///
/// Contrast with [`EntityWorldMut`], which allows adding and removing components,
/// despawning the entity, and provides mutable access to the entire world.
/// Because of this, `EntityWorldMut` cannot coexist with any other world accesses.
///
/// # Examples
///
/// Disjoint mutable access.
///
/// ```
/// # use bevy_ecs::prelude::*;
/// # #[derive(Component)] pub struct A;
/// fn disjoint_system(
///     query1: Query<EntityMut, With<A>>,
///     query2: Query<EntityMut, Without<A>>,
/// ) {
///     // ...
/// }
/// # bevy_ecs::system::assert_is_system(disjoint_system);
/// ```
///
/// [`EntityWorldMut`]: crate::world::EntityWorldMut
pub struct EntityMut<'w> {
    cell: UnsafeEntityCell<'w>,
}

impl<'w> EntityMut<'w> {
    /// # Safety
    /// - `cell` must have permission to mutate every component of the entity.
    /// - No accesses to any of the entity's components may exist
    ///   at the same time as the returned [`EntityMut`].
    #[inline]
    pub(crate) unsafe fn new(cell: UnsafeEntityCell<'w>) -> Self {
        Self { cell }
    }

    /// Returns a new instance with a shorter lifetime.
    /// This is useful if you have `&mut EntityMut`, but you need `EntityMut`.
    #[inline]
    pub fn reborrow(&mut self) -> EntityMut<'_> {
        // SAFETY:
        // - We have exclusive access to the entire entity and its components.
        // - `&mut self` ensures there are no other accesses.
        unsafe { Self::new(self.cell) }
    }

    /// Consumes `self` and returns read-only access to all of the entity's
    /// components, with the world `'w` lifetime.
    #[inline]
    pub fn into_readonly(self) -> EntityRef<'w> {
        // SAFETY:
        // - We have exclusive access to the entire entity and its components.
        // - Consuming `self` ensures there are no other accesses.
        unsafe { EntityRef::new(self.cell) }
    }

    /// Gets read-only access to all of the entity's components.
    #[inline]
    pub fn as_readonly(&self) -> EntityRef<'_> {
        // SAFETY:
        // - We have exclusive access to the entire entity and its components.
        // - `&self` ensures there are no mutable accesses.
        unsafe { EntityRef::new(self.cell) }
    }

    /// Consumes `self` and returns a [`FilteredEntityMut`] which has mutable
    /// access to all of the entity's components, with the world `'w` lifetime.
    #[inline]
    pub fn into_filtered(self) -> FilteredEntityMut<'w, 'static> {
        // SAFETY:
        // - We have exclusive access to the entire entity and its components.
        // - Consuming `self` ensures there are no other accesses.
        unsafe { FilteredEntityMut::new(self.cell, const { &Access::new_write_all() }) }
    }

    /// Get access to the underlying [`UnsafeEntityCell`].
    #[inline]
    pub fn as_unsafe_entity_cell(&mut self) -> UnsafeEntityCell<'_> {
        self.cell
    }

    /// Gets mutable access to the component of type `T` for the current entity.
    /// Returns `None` if the entity does not have a component of type `T`.
    #[inline]
    pub fn get_mut<T: Component<Mutability = Mutable>>(&mut self) -> Option<Mut<'_, T>> {
        // SAFETY: &mut self implies exclusive access for duration of returned value
        unsafe { self.cell.get_mut() }
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
    pub fn get<T: Component>(&self) -> Option<&'_ T> {
        self.as_readonly().get()
    }

    /// Returns read-only components for the current entity that match the query `Q`.
    ///
    /// # Panics
    ///
    /// If the entity does not have the components required by the query `Q`.
    pub fn components<Q: ReadOnlyQueryData + ReleaseStateQueryData + SingleEntityQueryData>(
        &self,
    ) -> Q::Item<'_, 'static> {
        self.as_readonly().components::<Q>()
    }

    /// Consumes self and gets mutable access to the component of type `T`
    /// with the world `'w` lifetime for the current entity.
    /// Returns `None` if the entity does not have a component of type `T`.
    #[inline]
    pub fn into_mut<T: Component<Mutability = Mutable>>(self) -> Option<Mut<'w, T>> {
        // SAFETY: consuming `self` implies exclusive access
        unsafe { self.cell.get_mut() }
    }

    /// Consumes `self` and returns untyped mutable reference(s)
    /// to component(s) with lifetime `'w` for the current entity, based on the
    /// given [`ComponentId`]s.
    ///
    /// **You should prefer to use the typed API [`EntityMut::into_mut`] where
    /// possible and only use this in cases where the actual component types
    /// are not known at compile time.**
    ///
    /// Unlike [`EntityMut::into_mut`], this returns untyped reference(s) to
    /// component(s), and it's the job of the caller to ensure the correct
    /// type(s) are dereferenced (if necessary).
    ///
    /// # Errors
    ///
    /// - Returns [`EntityComponentError::MissingComponent`] if the entity does
    ///   not have a component.
    /// - Returns [`EntityComponentError::AliasedMutability`] if a component
    ///   is requested multiple times.
    ///
    /// # Examples
    ///
    /// For examples on how to use this method, see [`EntityMut::get_mut_by_id`].
    #[inline]
    pub fn into_mut_by_id<F: DynamicComponentFetch>(
        self,
        component_ids: F,
    ) -> Result<F::Mut<'w>, EntityComponentError> {
        // SAFETY:
        // - consuming `self` ensures that no references exist to this entity's components.
        // - We have exclusive access to all components of this entity.
        unsafe { component_ids.fetch_mut(self.cell) }
    }
}
