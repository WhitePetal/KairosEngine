use crate::{
    ecs::{
        change_detection::MutUntyped,
        component::ComponentId,
        world::{error::EntityComponentError, unsafe_world_cell::UnsafeEntityCell},
    },
    ptr::Ptr,
};

/// Types that can be used to fetch components from an entity dynamically by
/// [`ComponentId`]s.
///
/// Provided implementations are:
/// - [`ComponentId`]: Returns a single untyped reference.
/// - `[ComponentId; N]` and `&[ComponentId; N]`: Returns a same-sized array of untyped references.
/// - `&[ComponentId]`: Returns a [`Vec`] of untyped references.
/// - [`&HashSet<ComponentId>`](HashSet): Returns a [`HashMap`] of IDs to untyped references.
///
/// # Performance
///
/// - The slice and array implementations perform an aliased mutability check in
///   [`DynamicComponentFetch::fetch_mut`] that is `O(N^2)`.
/// - The [`HashSet`] implementation performs no such check as the type itself
///   guarantees unique IDs.
/// - The single [`ComponentId`] implementation performs no such check as only
///   one reference is returned.
///
/// # Safety
///
/// Implementor must ensure that:
/// - No aliased mutability is caused by the returned references.
/// - [`DynamicComponentFetch::fetch_ref`] returns only read-only references.
pub unsafe trait DynamicComponentFetch {
    /// The read-only reference type returned by [`DynamicComponentFetch::fetch_ref`].
    type Ref<'w>;

    /// The mutable reference type returned by [`DynamicComponentFetch::fetch_mut`].
    type Mut<'w>;

    /// Returns untyped read-only reference(s) to the component(s) with the
    /// given [`ComponentId`]s, as determined by `self`.
    ///
    /// # Safety
    ///
    /// It is the caller's responsibility to ensure that:
    /// - The given [`UnsafeEntityCell`] has read-only access to the fetched components.
    /// - No other mutable references to the fetched components exist at the same time.
    ///
    /// # Errors
    ///
    /// - Returns [`EntityComponentError::MissingComponent`] if a component is missing from the entity.
    unsafe fn fetch_ref(
        self,
        cell: UnsafeEntityCell<'_>,
    ) -> Result<Self::Ref<'_>, EntityComponentError>;

    /// Returns untyped mutable reference(s) to the component(s) with the
    /// given [`ComponentId`]s, as determined by `self`.
    ///
    /// # Safety
    ///
    /// It is the caller's responsibility to ensure that:
    /// - The given [`UnsafeEntityCell`] has mutable access to the fetched components.
    /// - No other references to the fetched components exist at the same time.
    ///
    /// # Errors
    ///
    /// - Returns [`EntityComponentError::MissingComponent`] if a component is missing from the entity.
    /// - Returns [`EntityComponentError::AliasedMutability`] if a component is requested multiple times.
    unsafe fn fetch_mut(
        self,
        cell: UnsafeEntityCell<'_>,
    ) -> Result<Self::Mut<'_>, EntityComponentError>;

    /// Returns untyped mutable reference(s) to the component(s) with the
    /// given [`ComponentId`]s, as determined by `self`.
    /// Assumes all [`ComponentId`]s refer to mutable components.
    ///
    /// # Safety
    ///
    /// It is the caller's responsibility to ensure that:
    /// - The given [`UnsafeEntityCell`] has mutable access to the fetched components.
    /// - No other references to the fetched components exist at the same time.
    /// - The requested components are all mutable.
    ///
    /// # Errors
    ///
    /// - Returns [`EntityComponentError::MissingComponent`] if a component is missing from the entity.
    /// - Returns [`EntityComponentError::AliasedMutability`] if a component is requested multiple times.
    unsafe fn fetch_mut_assume_mutable(
        self,
        cell: UnsafeEntityCell<'_>,
    ) -> Result<Self::Mut<'_>, EntityComponentError>;
}

// SAFETY:
// - No aliased mutability is caused because a single reference is returned.
// - No mutable references are returned by `fetch_ref`.
unsafe impl DynamicComponentFetch for ComponentId {
    type Ref<'w> = Ptr<'w>;

    type Mut<'w> = MutUntyped<'w>;

    unsafe fn fetch_ref(
        self,
        cell: UnsafeEntityCell<'_>,
    ) -> Result<Self::Ref<'_>, EntityComponentError> {
        // SAFETY: caller ensures that the cell has read access to the component.
        unsafe {
            cell.get_by_id(self)
                .ok_or(EntityComponentError::MissingComponent(self))
        }
    }

    unsafe fn fetch_mut(
        self,
        cell: UnsafeEntityCell<'_>,
    ) -> Result<Self::Mut<'_>, EntityComponentError> {
        // SAFETY: caller ensures that the cell has mutable access to the component.
        unsafe { cell.get_mut_by_id(self) }
            .map_err(|_| EntityComponentError::MissingComponent(self))
    }

    unsafe fn fetch_mut_assume_mutable(
        self,
        cell: UnsafeEntityCell<'_>,
    ) -> Result<Self::Mut<'_>, EntityComponentError> {
        unsafe { cell.get_mut_assume_mutable_by_id(self) }
            .map_err(|_| EntityComponentError::MissingComponent(self))
    }
}

// TODO!
