use nonmax::NonMaxU32;

use crate::ecs::entity::Entity;

mod column;

pub use column::*;

/// An opaque unique ID for a [`Table`] within a [`World`].
///
/// Can be used with [`Tables::get`] to fetch the corresponding
/// table.
///
/// Each [`Archetype`] always points to a table via [`Archetype::table_id`].
/// Multiple archetypes can point to the same table so long as the components
/// stored in the table are identical, but do not share the same sparse set
/// components.
///
/// [`World`]: crate::world::World
/// [`Archetype`]: crate::archetype::Archetype
/// [`Archetype::table_id`]: crate::archetype::Archetype::table_id
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableId(u32);

impl TableId {
    /// Creates a new [`TableId`].
    ///
    /// `index` *must* be retrieved from calling [`TableId::as_u32`] on a `TableId` you got
    /// from a table of a given [`World`] or the created ID may be invalid.
    ///
    /// [`World`]: crate::world::World
    #[inline]
    pub const fn from_u32(index: u32) -> Self {
        Self(index)
    }

    /// Creates a new [`TableId`].
    ///
    /// `index` *must* be retrieved from calling [`TableId::as_usize`] on a `TableId` you got
    /// from a table of a given [`World`] or the created ID may be invalid.
    ///
    /// [`World`]: crate::world::World
    ///
    /// # Panics
    ///
    /// Will panic if the provided value does not fit within a [`u32`].
    #[inline]
    pub const fn from_usize(index: usize) -> Self {
        debug_assert!(index as u32 as usize == index);
        Self(index as u32)
    }

    /// Gets the underlying table index from the ID.
    #[inline]
    pub const fn as_u32(self) -> u32 {
        self.0
    }

    /// Gets the underlying table index from the ID.
    #[inline]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }

    /// The [`TableId`] of the [`Table`] without any components.
    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }
}

/// An opaque newtype for rows in [`Table`]s. Specifies a single row in a specific table.
///
/// Values of this type are retrievable from [`Archetype::entity_table_row`] and can be
/// used alongside [`Archetype::table_id`] to fetch the exact table and row where an
/// [`Entity`]'s components are stored.
///
/// Values of this type are only valid so long as entities have not moved around.
/// Adding and removing components from an entity, or despawning it will invalidate
/// potentially any table row in the table the entity was previously stored in. Users
/// should *always* fetch the appropriate row from the entity's [`Archetype`] before
/// fetching the entity's components.
///
/// [`Archetype`]: crate::archetype::Archetype
/// [`Archetype::entity_table_row`]: crate::archetype::Archetype::entity_table_row
/// [`Archetype::table_id`]: crate::archetype::Archetype::table_id
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct TableRow(NonMaxU32);

impl TableRow {
    /// Creates a [`TableRow`].
    #[inline]
    pub const fn new(index: NonMaxU32) -> Self {
        Self(index)
    }

    /// Gets the index of the row as a [`usize`].
    #[inline]
    pub const fn index(self) -> usize {
        // usize is at least u32 in Bevy
        self.0.get() as usize
    }

    /// Gets the index of the row as a [`usize`].
    #[inline]
    pub const fn index_u32(self) -> u32 {
        self.0.get()
    }
}

/// A column-oriented [structure-of-arrays] based storage for [`Component`]s of entities
/// in a [`World`].
///
/// Conceptually, a `Table` can be thought of as a `HashMap<ComponentId, Column>`, where
/// each [`Column`] is a type-erased `Vec<T: Component>`. Each row corresponds to a single entity
/// (i.e. index 3 in Column A and index 3 in Column B point to different components on the same
/// entity). Fetching components from a table involves fetching the associated column for a
/// component type (via its [`ComponentId`]), then fetching the entity's row within that column.
///
/// [structure-of-arrays]: https://en.wikipedia.org/wiki/AoS_and_SoA#Structure_of_arrays
/// [`Component`]: crate::component::Component
/// [`World`]: crate::world::World
//
// # Safety
// The capacity of all columns is determined by that of the `entities` Vec. This means that
// it must be the correct capacity to allocate, reallocate, and deallocate all columns. This
// means the safety invariant must be enforced even in `TableBuilder`.
pub struct Table {
    entities: Vec<Entity>,
}

// TODO!
