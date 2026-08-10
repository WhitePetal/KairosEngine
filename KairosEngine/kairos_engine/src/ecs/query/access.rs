use derive_more::From;

use crate::ecs::{component::ComponentId, world::unsafe_world_cell::UnsafeWorldCell};

/// Tracks read and write access to specific elements in a collection.
///
/// Used internally to ensure soundness during system initialization and execution.
/// See the [`is_compatible`](Access::is_compatible) and [`get_conflicts`](Access::get_conflicts) functions.
#[derive(Eq, PartialEq, Default, Hash, Debug)]
pub struct Access {}

impl Access {
    /// Creates an empty [`Access`] collection.
    pub const fn new() -> Self {
        todo!()
    }
    /// Returns `true` if this can access any component.
    pub fn has_any_read(&self) -> bool {
        todo!()
    }

    /// Returns `true` if this can access the component given by `index`.
    pub fn has_read(&self, index: ComponentId) -> bool {
        todo!()
    }

    /// Returns `true` if this can exclusively access the component given by `index`.
    pub fn has_write(&self, index: ComponentId) -> bool {
        todo!()
    }

    /// Adds all access from `other`.
    pub fn extend(&mut self, other: &Access) {
        todo!()
    }

    /// Returns a vector of elements that the access and `other` cannot access at the same time.
    #[inline]
    pub fn get_conflicts(&self, other: &Access) -> AccessConflicts {
        todo!()
    }
}

/// A collection of [`FilteredAccess`] instances.
///
/// Used internally to statically check if systems have conflicting access.
///
/// It stores multiple sets of accesses.
/// - A "combined" set, which is the access of all filters in this set combined.
/// - The set of access of each individual filters in this set.
#[derive(Debug, PartialEq, Eq, Default)]
pub struct FilteredAccessSet {}

// This is needed since `#[derive(Clone)]` does not generate optimized `clone_from`.
impl Clone for FilteredAccessSet {
    fn clone(&self) -> Self {
        todo!()
    }

    fn clone_from(&mut self, source: &Self) {
        todo!()
    }
}

impl FilteredAccessSet {
    /// Creates an empty [`FilteredAccessSet`].
    pub const fn new() -> Self {
        todo!()
    }

    /// Returns a reference to the unfiltered access of the entire set.
    #[inline]
    pub fn combined_access(&self) -> &Access {
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

    /// Marks the set as writing all T.
    pub fn write_all(&mut self) {
        let mut filter = FilteredAccess::matches_everything();
        filter.write_all();
        self.add(filter);
    }

    /// Adds a read access to a component to the set.
    pub(crate) fn add_unfiltered_component_read(&mut self, index: ComponentId) {
        let mut filter = FilteredAccess::default();
        filter.add_read(index);
        self.add(filter);
    }

    /// Adds a write access to a resource to the set.
    pub(crate) fn add_unfiltered_component_write(&mut self, index: ComponentId) {
        let mut filter = FilteredAccess::default();
        filter.add_write(index);
        self.add(filter);
    }

    /// Adds all of the accesses from the passed set to `self`.
    pub fn extend(&mut self, filtered_access_set: FilteredAccessSet) {
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
    /// Returns a `FilteredAccess` which has no access and matches everything.
    /// This is the equivalent of a `TRUE` logic atom.
    pub fn matches_everything() -> Self {
        todo!()
    }
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

    /// Sets the underlying unfiltered access as having mutable access to all components.
    pub fn write_all(&mut self) {
        todo!()
    }

    /// Returns a mutable reference to the underlying unfiltered access.
    #[inline]
    pub fn access_mut(&mut self) -> &mut Access {
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
