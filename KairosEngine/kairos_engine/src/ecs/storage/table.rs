use nonmax::NonMaxU32;

use crate::ecs::entity::Entity;

/// A type-erased contiguous container for data of a homogeneous type.
///
/// Conceptually, a `Column` is very similar to a type-erased `Box<[T]>`.
/// It also stores the change detection ticks for its components, kept in two separate
/// contiguous buffers internally. An element shares its data across these buffers by using the
/// same index (i.e. the entity at row 3 has it's data at index 3 and its change detection ticks at index 3).
///
/// Like many other low-level storage types, `Column` has a limited and highly unsafe
/// interface. It's highly advised to use higher level types and their safe abstractions
/// instead of working directly with `Column`.
///
/// For performance reasons, `Column` does not does not store it's capacity and length.
/// This type is used by [`Table`] and [`ComponentSparseSet`], where the corresponding capacity
/// and length can be found.
///
/// [`ComponentSparseSet`]: crate::storage::ComponentSparseSet
#[derive(Debug)]
pub struct Column {}

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
