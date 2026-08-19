use std::any::TypeId;

use thiserror::Error;

use crate::ecs::{
    archetype::Archetype,
    component::{Component, ComponentId},
    entity::{Entity, EntityLocation},
    query::Access,
    world::{EntityRef, unsafe_world_cell::UnsafeEntityCell},
};

/// Provides read-only access to a single entity and some of its components defined by the contained [`Access`].
///
/// To define the access when used as a [`QueryData`](crate::query::QueryData),
/// use a [`QueryBuilder`](crate::query::QueryBuilder) or [`QueryParamBuilder`](crate::system::QueryParamBuilder).
/// The [`FilteredEntityRef`] must be the entire [`QueryData`](crate::query::QueryData), and not nested inside a tuple with other data.
///
/// ```
/// # use bevy_ecs::{prelude::*, world::FilteredEntityRef};
/// #
/// # #[derive(Component)]
/// # struct A;
/// #
/// # let mut world = World::new();
/// # world.spawn(A);
/// #
/// // This gives the `FilteredEntityRef` access to `&A`.
/// let mut query = QueryBuilder::<FilteredEntityRef>::new(&mut world)
///     .data::<&A>()
///     .build();
///
/// let filtered_entity: FilteredEntityRef = query.single(&mut world).unwrap();
/// let component: &A = filtered_entity.get().unwrap();
/// ```
#[derive(Clone, Copy)]
pub struct FilteredEntityRef<'w, 's> {
    entity: UnsafeEntityCell<'w>,
    access: &'s Access,
}

impl<'w, 's> FilteredEntityRef<'w, 's> {
    /// # Safety
    /// - No `&mut World` can exist from the underlying `UnsafeWorldCell`
    /// - If `access` takes read access to a component no mutable reference to that
    ///   component can exist at the same time as the returned [`FilteredEntityMut`]
    /// - If `access` takes any access for a component `entity` must have that component.
    #[inline]
    pub(crate) unsafe fn new(entity: UnsafeEntityCell<'w>, access: &'s Access) -> Self {
        Self { entity, access }
    }

    /// Consumes self and returns read-only access to the entity and *all* of
    /// its components, with the world `'w` lifetime. Returns an error if the
    /// access does not include read access to all components.
    ///
    /// # Errors
    ///
    /// - [`TryFromFilteredError::MissingReadAllAccess`] - if the access does not include read access to all components.
    #[inline]
    pub fn try_into_all(self) -> Result<EntityRef<'w>, TryFromFilteredError> {
        if !self.access.has_read_all() {
            Err(TryFromFilteredError::MissingReadAllAccess)
        } else {
            Ok(unsafe {
                // SAFETY: check above guarantees read-only access to all components of the entity.
                EntityRef::new(self.entity)
            })
        }
    }

    /// Returns the [ID](Entity) of the current entity.
    #[inline]
    #[must_use = "Omit the .id() call if you do not need to store the `Entity` identifier."]
    pub fn id(&self) -> Entity {
        self.entity.id()
    }

    /// Gets metadata indicating the location where the current entity is stored.
    #[inline]
    pub fn location(&self) -> EntityLocation {
        self.entity.location()
    }

    /// Returns the archetype that the current entity belongs to.
    #[inline]
    pub fn archetype(&self) -> &Archetype {
        self.entity.archetype()
    }

    /// Returns a reference to the underlying [`Access`].
    #[inline]
    pub fn access(&self) -> &Access {
        self.access
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
        self.entity.contains_id(component_id)
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
        self.entity.contains_type_id(type_id)
    }
}

/// Provides mutable access to a single entity and some of its components defined by the contained [`Access`].
///
/// To define the access when used as a [`QueryData`](crate::query::QueryData),
/// use a [`QueryBuilder`](crate::query::QueryBuilder) or [`QueryParamBuilder`](crate::system::QueryParamBuilder).
/// The `FilteredEntityMut` must be the entire `QueryData`, and not nested inside a tuple with other data.
///
/// ```
/// # use bevy_ecs::{prelude::*, world::FilteredEntityMut};
/// #
/// # #[derive(Component)]
/// # struct A;
/// #
/// # let mut world = World::new();
/// # world.spawn(A);
/// #
/// // This gives the `FilteredEntityMut` access to `&mut A`.
/// let mut query = QueryBuilder::<FilteredEntityMut>::new(&mut world)
///     .data::<&mut A>()
///     .build();
///
/// let mut filtered_entity: FilteredEntityMut = query.single_mut(&mut world).unwrap();
/// let component: Mut<A> = filtered_entity.get_mut().unwrap();
/// ```
///
/// Also see [`UnsafeFilteredEntityMut`] for a way to bypass borrow-checker restrictions.
pub struct FilteredEntityMut<'w, 's> {
    entity: UnsafeEntityCell<'w>,
    access: &'s Access,
}

impl<'w, 's> FilteredEntityMut<'w, 's> {
    /// # Safety
    /// - No `&mut World` can exist from the underlying `UnsafeWorldCell`
    /// - If `access` takes read access to a component no mutable reference to that
    ///   component can exist at the same time as the returned [`FilteredEntityMut`]
    /// - If `access` takes write access to a component, no reference to that component
    ///   may exist at the same time as the returned [`FilteredEntityMut`]
    /// - If `access` takes any access for a component `entity` must have that component.
    #[inline]
    pub(crate) unsafe fn new(entity: UnsafeEntityCell<'w>, access: &'s Access) -> Self {
        Self { entity, access }
    }
}

/// Error type returned by [`TryFrom`] conversions from filtered entity types
/// ([`FilteredEntityRef`]/[`FilteredEntityMut`]) to full-access entity types
/// ([`EntityRef`]/[`EntityMut`]).
#[derive(Error, Debug)]
pub enum TryFromFilteredError {
    /// Error indicating that the filtered entity does not have read access to
    /// all components.
    #[error("Conversion failed, filtered entity ref does not have read access to all components")]
    MissingReadAllAccess,
    /// Error indicating that the filtered entity does not have write access to
    /// all components.
    #[error("Conversion failed, filtered entity ref does not have write access to all components")]
    MissingWriteAllAccess,
}
