use derive_more::From;

use crate::ecs::{component::ComponentId, world::unsafe_world_cell::UnsafeWorldCell};

/// A collection of [`FilteredAccess`] instances.
///
/// Used internally to statically check if systems have conflicting access.
///
/// It stores multiple sets of accesses.
/// - A "combined" set, which is the access of all filters in this set combined.
/// - The set of access of each individual filters in this set.
#[derive(Debug, PartialEq, Eq, Default)]
pub struct FilteredAccessSet {}

impl FilteredAccessSet {
    /// Creates an empty [`FilteredAccessSet`].
    pub const fn new() -> Self {
        todo!()
    }

    /// Returns a vector of elements that this set and `other` cannot access at the same time.
    pub fn get_conflicts_single(&self, filtered_acces: &FilteredAccess) -> AccessConflicts {
        todo!()
    }

    /// Adds the filtered access to the set.
    pub fn add(&mut self, filtered_acces: FilteredAccess) {
        todo!()
    }
}

/// An [`Access`] that has been filtered to include and exclude certain combinations of elements.
///
/// Used internally to statically check if queries are disjoint.
///
/// Subtle: a `read` or `write` in `access` should not be considered to imply a
/// `with` access.
///
/// For example consider `Query<Option<&T>>` this only has a `read` of `T` as doing
/// otherwise would allow for queries to be considered disjoint when they shouldn't:
/// - `Query<(&mut T, Option<&U>)>` read/write `T`, read `U`, with `U`
/// - `Query<&mut T, Without<U>>` read/write `T`, without `U`
///   from this we could reasonably conclude that the queries are disjoint but they aren't.
///
/// In order to solve this the actual access that `Query<(&mut T, Option<&U>)>` has
/// is read/write `T`, read `U`. It must still have a read `U` access otherwise the following
/// queries would be incorrectly considered disjoint:
/// - `Query<&mut T>`  read/write `T`
/// - `Query<Option<&T>>` accesses nothing
///
/// See comments the [`WorldQuery`](super::WorldQuery) impls of [`AnyOf`](super::AnyOf)/`Option`/[`Or`](super::Or) for more information.
#[derive(Debug, Eq, PartialEq)]
pub struct FilteredAccess {}

impl FilteredAccess {
    // Adds access to the component given by `index`.
    pub fn add_read(&mut self, index: ComponentId) {
        todo!()
    }

    /// Adds exclusive access to the component given by `index`.
    pub fn add_write(&mut self, index: ComponentId) {
        todo!()
    }

    /// Adds a `With` filter: corresponds to a conjunction (AND) operation.
    ///
    /// Suppose we begin with `Or<(With<A>, With<B>)>`, which is represented by an array of two `AccessFilter` instances.
    /// Adding `AND With<C>` via this method transforms it into the equivalent of  `Or<((With<A>, With<C>), (With<B>, With<C>))>`.
    pub fn and_with(&mut self, index: ComponentId) {
        todo!()
    }

    /// Sets the underlying unfiltered access as having access to all components.
    pub fn read_all(&mut self) {
        todo!()
    }
}

impl Default for FilteredAccess {
    fn default() -> Self {
        todo!()
    }
}

/// Records how two accesses conflict with each other
#[derive(Debug, PartialEq, From)]
pub enum AccessConflicts {
    /// Conflict is for all indices
    All,
    /// There is a conflict for a subset of indices
    Individual,
}

impl AccessConflicts {
    /// Returns true if there are no conflicts present
    pub fn is_empty(&self) -> bool {
        todo!()
    }

    pub(crate) fn format_conflict_list(&self, world: UnsafeWorldCell) -> String {
        todo!()
    }
}
