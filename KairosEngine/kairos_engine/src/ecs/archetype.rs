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


// TODO!
