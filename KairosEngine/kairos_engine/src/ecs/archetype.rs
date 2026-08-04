//! Types for defining [`Archetype`]s, collections of entities that have the same set of
//! components.
//!
//! An archetype uniquely describes a group of entities that share the same components:
//! a world only has one archetype for each unique combination of components, and all
//! entities that have those components and only those components belong to that
//! archetype.
//!
//! Archetypes are not to be confused with [`Table`]s. Each archetype stores its table
//! components in one table, and each archetype uniquely points to one table, but multiple
//! archetypes may store their table components in the same table. These archetypes
//! differ only by the [`SparseSet`] components.
//!
//! Like tables, archetypes can be created but are never cleaned up. Empty archetypes are
//! not removed, and persist until the world is dropped.
//!
//! Archetypes can be fetched from [`Archetypes`], which is accessible via [`World::archetypes`].
//!
//! [`Table`]: crate::storage::Table
//! [`World::archetypes`]: crate::world::World::archetypes

use nonmax::NonMaxU32;

/// An opaque location within a [`Archetype`].
///
/// This can be used in conjunction with [`ArchetypeId`] to find the exact location
/// of an [`Entity`] within a [`World`]. An entity's archetype and index can be
/// retrieved via [`Entities::get`].
///
/// [`World`]: crate::world::World
/// [`Entities::get`]: crate::entity::Entities
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
// SAFETY: Must be repr(transparent) due to the safety requirements on EntityLocation
#[repr(transparent)]
pub struct ArchetypeRow(NonMaxU32);

/// An opaque unique ID for a single [`Archetype`] within a [`World`].
///
/// Archetype IDs are only valid for a given World, and are not globally unique.
/// Attempting to use an archetype ID on a world that it wasn't sourced from will
/// not return the archetype with the same components. The only exception to this is
/// [`EMPTY`] which is guaranteed to be identical for all Worlds.
///
/// [`World`]: crate::world::World
/// [`EMPTY`]: ArchetypeId::EMPTY
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, PartialOrd, Ord)]
// SAFETY: Must be repr(transparent) due to the safety requirements on EntityLocation
#[repr(transparent)]
pub struct ArchetypeId(u32);

bitflags::bitflags! {
    /// Flags used to keep track of metadata about the component in this [`Archetype`]
    ///
    /// Used primarily to early-out when there are no [`ComponentHook`] registered for any contained components.
    #[derive(Clone, Copy)]
    pub(crate) struct ArchetypeFlags: u32 {
        const ON_ADD_HOOK = (1 << 0);
        const ON_INSERT_HOOK = (1 << 1);
        const ON_DISCARD_HOOK = (1 << 2);
        const ON_REMOVE_HOOK = (1 << 3);
        const ON_DESPAWN_HOOK = (1 << 4);
        const ON_ADD_OBSERVER = (1 << 5);
        const ON_INSERT_OBSERVER = (1 << 6);
        const ON_DISCARD_OBSERVER = (1 << 7);
        const ON_REMOVE_OBSERVER = (1 << 8);
        const ON_DESPAWN_OBSERVER = (1 << 9);
    }
}

/// Used in [`ArchetypeAfterBundleInsert`] to track whether components in the bundle are newly
/// added or already existed in the entity's archetype.
#[derive(Copy, Clone, Eq, PartialEq)]
pub(crate) enum ComponentStatus {
    Added,
    Existing,
}

pub(crate) trait BundleComponentStatus {
    unsafe fn get_status(&self, index: usize) -> ComponentStatus;
}

// TODO!
