use std::{
    fmt::{self, Debug},
    iter::FusedIterator,
};

use derive_more::From;
use fixedbitset::{Difference, FixedBitSet, Intersection, IntoOnes, Ones, Union};
use thiserror::Error;

use crate::ecs::{
    component::ComponentId, resource::IS_RESOURCE, world::unsafe_world_cell::UnsafeWorldCell,
};

/// Tracks read and write access to specific elements in a collection.
///
/// Used internally to ensure soundness during system initialization and execution.
/// See the [`is_compatible`](Access::is_compatible) and [`get_conflicts`](Access::get_conflicts) functions.
#[derive(Eq, PartialEq, Default, Hash, Debug)]
pub struct Access {
    /// All accessed components, or forbidden components if
    /// `Self::component_read_and_writes_inverted` is set.
    read_and_writes: ComponentIdSet,
    /// All exclusively-accessed components, or components that may not be
    /// exclusively accessed if `Self::component_writes_inverted` is set.
    writes: ComponentIdSet,
    /// Is `true` if this component can read all components *except* those
    /// present in `Self::read_and_writes`.
    read_and_writes_inverted: bool,
    /// Is `true` if this component can write to all components *except* those
    /// present in `Self::writes`.
    writes_inverted: bool,
    /// Components that are not accessed, but whose presence in an archetype affect query results.
    archetypal: ComponentIdSet,
}

// This is needed since `#[derive(Clone)]` does not generate optimized `clone_from`.
impl Clone for Access {
    fn clone(&self) -> Self {
        Self {
            read_and_writes: self.read_and_writes.clone(),
            writes: self.writes.clone(),
            read_and_writes_inverted: self.read_and_writes_inverted,
            writes_inverted: self.writes_inverted,
            archetypal: self.archetypal.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.read_and_writes.clone_from(&source.read_and_writes);
        self.writes.clone_from(&source.writes);
        self.read_and_writes_inverted = source.read_and_writes_inverted;
        self.writes_inverted = source.writes_inverted;
        self.archetypal.clone_from(&source.archetypal);
    }
}

impl Access {
    /// Creates an empty [`Access`] collection.
    pub const fn new() -> Self {
        Self {
            read_and_writes_inverted: false,
            writes_inverted: false,
            read_and_writes: ComponentIdSet::new(),
            writes: ComponentIdSet::new(),
            archetypal: ComponentIdSet::new(),
        }
    }

    /// Creates an [`Access`] with read access to all components.
    /// This is equivalent to calling `read_all()` on `Access::new()`,
    /// but is available in a `const` context.
    pub(crate) const fn new_read_all() -> Self {
        let mut access = Self::new();
        // Note that we cannot use `read_all()`
        // because `FixedBitSet::clear()` is not `const`.
        access.read_and_writes_inverted = true;
        access
    }

    /// Creates an [`Access`] with read and write access to all components.
    /// This is equivalent to calling `write_all()` on `Access::new()`,
    /// but is available in a `const` context.
    pub(crate) const fn new_write_all() -> Self {
        let mut access = Self::new();
        // Note that we cannot use `write_all()`
        // because `FixedBitSet::clear()` is not `const`.
        access.read_and_writes_inverted = true;
        access.writes_inverted = true;
        access
    }

    /// Adds access to the component given by `index`.
    pub fn add_read(&mut self, index: ComponentId) {
        if !self.read_and_writes_inverted {
            self.read_and_writes.insert(index);
        } else {
            self.read_and_writes.remove(index);
        }
    }

    /// Adds exclusive access to the component given by `index`.
    pub fn add_write(&mut self, index: ComponentId) {
        self.add_read(index);
        if !self.writes_inverted {
            self.writes.insert(index);
        } else {
            self.writes.remove(index);
        }
    }

    /// Removes both read and write access to the component given by `index`.
    ///
    /// Because this method corresponds to the set difference operator ∖, it can
    /// create complicated logical formulas that you should verify correctness
    /// of. For example, A ∪ (B ∖ A) isn't equivalent to (A ∪ B) ∖ A, so you
    /// can't replace a call to `remove_component_read` followed by a call to
    /// `extend` with a call to `extend` followed by a call to
    /// `remove_read`.
    pub fn remove_read(&mut self, index: ComponentId) {
        self.remove_write(index);
        if !self.read_and_writes_inverted {
            self.read_and_writes.remove(index);
        } else {
            self.read_and_writes.insert(index);
        }
    }

    /// Removes write access to the component given by `index`.
    ///
    /// Because this method corresponds to the set difference operator ∖, it can
    /// create complicated logical formulas that you should verify correctness
    /// of. For example, A ∪ (B ∖ A) isn't equivalent to (A ∪ B) ∖ A, so you
    /// can't replace a call to `remove_write` followed by a call to
    /// `extend` with a call to `extend` followed by a call to
    /// `remove_write`.
    pub fn remove_write(&mut self, index: ComponentId) {
        if !self.writes_inverted {
            self.writes.remove(index);
        } else {
            self.writes.insert(index);
        }
    }

    /// Adds an archetypal (indirect) access to the component given by `index`.
    ///
    /// This is for components whose values are not accessed (and thus will never cause conflicts),
    /// but whose presence in an archetype may affect query results.
    ///
    /// Currently, this is only used for [`Has<T>`] and [`Allow<T>`].
    ///
    /// [`Has<T>`]: crate::query::Has
    /// [`Allow<T>`]: crate::query::filter::Allow
    pub fn add_archetypal(&mut self, index: ComponentId) {
        self.archetypal.insert(index);
    }

    /// Returns `true` if this can access the component given by `index`.
    pub fn has_read(&self, index: ComponentId) -> bool {
        self.read_and_writes_inverted ^ self.read_and_writes.contains(index)
    }

    /// Returns `true` if this can access any component.
    pub fn has_any_read(&self) -> bool {
        self.read_and_writes_inverted || !self.read_and_writes.is_clear()
    }

    /// Returns `true` if this can exclusively access the component given by `index`.
    pub fn has_write(&self, index: ComponentId) -> bool {
        self.writes_inverted ^ self.writes.contains(index)
    }

    /// Returns `true` if this accesses any component mutably.
    pub fn has_any_write(&self) -> bool {
        self.writes_inverted || !self.writes.is_clear()
    }

    /// Returns true if this has an archetypal (indirect) access to the component given by `index`.
    ///
    /// This is a component whose value is not accessed (and thus will never cause conflicts),
    /// but whose presence in an archetype may affect query results.
    ///
    /// Currently, this is only used for [`Has<T>`].
    ///
    /// [`Has<T>`]: crate::query::Has
    pub fn has_archetypal(&self, index: ComponentId) -> bool {
        self.archetypal.contains(index)
    }

    /// Sets this as having access to all components (i.e. `EntityRef` and `&World`).
    #[inline]
    pub fn read_all(&mut self) {
        self.read_and_writes_inverted = true;
        self.read_and_writes.clear();
    }

    /// Sets this as having mutable access to all components (i.e. `EntityMut` and `&mut World`).
    #[inline]
    pub fn write_all(&mut self) {
        self.read_all();
        self.writes_inverted = true;
        self.writes.clear();
    }

    /// Returns `true` if this has access to all components (i.e. `EntityRef` and `&World`).
    #[inline]
    pub fn has_read_all(&self) -> bool {
        self.read_and_writes_inverted && self.read_and_writes.is_clear()
    }

    /// Returns `true` if this has write access to all components (i.e. `EntityMut` and `&mut World`).
    #[inline]
    pub fn has_write_all(&self) -> bool {
        self.writes_inverted && self.writes.is_clear()
    }

    /// Removes all writes.
    pub fn clear_writes(&mut self) {
        self.writes_inverted = false;
        self.writes.clear();
    }

    /// Removes all accesses.
    pub fn clear(&mut self) {
        self.read_and_writes_inverted = false;
        self.writes_inverted = false;
        self.read_and_writes.clear();
        self.writes.clear();
    }

    /// Adds all access from `other`.
    pub fn extend(&mut self, other: &Access) {
        invertible_union_with(
            &mut self.read_and_writes,
            &mut self.read_and_writes_inverted,
            &other.read_and_writes,
            other.read_and_writes_inverted,
        );
        invertible_union_with(
            &mut self.writes,
            &mut self.writes_inverted,
            &other.writes,
            other.writes_inverted,
        );
        self.archetypal.union_with(&other.archetypal);
    }

    /// Removes any access from `self` that would conflict with `other`.
    /// This removes any reads and writes for any component written by `other`,
    /// and removes any writes for any component read by `other`.
    pub fn remove_conflicting_access(&mut self, other: &Access) {
        invertible_difference_with(
            &mut self.read_and_writes,
            &mut self.read_and_writes_inverted,
            &other.writes,
            other.writes_inverted,
        );
        invertible_difference_with(
            &mut self.writes,
            &mut self.writes_inverted,
            &other.read_and_writes,
            other.read_and_writes_inverted,
        );
    }

    /// Returns `true` if the access and `other` can be active at the same time.
    ///
    /// [`Access`] instances are incompatible if one can write
    /// an element that the other can read or write.
    pub fn is_compatible(&self, other: &Access) -> bool {
        // We have a conflict if we write and they read or write, or if they
        // write and we read or write.
        for (
            lhs_writes,
            rhs_reads_and_writes,
            lhs_writes_inverted,
            rhs_reads_and_writes_inverted,
        ) in [
            (
                &self.writes,
                &other.read_and_writes,
                self.writes_inverted,
                other.read_and_writes_inverted,
            ),
            (
                &other.writes,
                &self.read_and_writes,
                other.writes_inverted,
                self.read_and_writes_inverted,
            ),
        ] {
            match (lhs_writes_inverted, rhs_reads_and_writes_inverted) {
                (true, true) => return false,
                (true, false) => {
                    if !rhs_reads_and_writes.is_subset(lhs_writes) {
                        return false;
                    }
                }
                (false, true) => {
                    if !lhs_writes.is_subset(rhs_reads_and_writes) {
                        return false;
                    }
                }
                (false, false) => {
                    if !lhs_writes.is_disjoint(rhs_reads_and_writes) {
                        return false;
                    }
                }
            }
        }

        true
    }

    /// Returns `true` if the set is a subset of another, i.e. `other` contains
    /// at least all the values in `self`.
    pub fn is_subset(&self, other: &Access) -> bool {
        for (
            our_components,
            their_components,
            our_components_inverted,
            their_components_inverted,
        ) in [
            (
                &self.read_and_writes,
                &other.read_and_writes,
                self.read_and_writes_inverted,
                other.read_and_writes_inverted,
            ),
            (
                &self.writes,
                &other.writes,
                self.writes_inverted,
                other.writes_inverted,
            ),
        ] {
            match (our_components_inverted, their_components_inverted) {
                (true, true) => {
                    if !their_components.is_subset(our_components) {
                        return false;
                    }
                }
                (true, false) => {
                    return false;
                }
                (false, true) => {
                    if !our_components.is_disjoint(their_components) {
                        return false;
                    }
                }
                (false, false) => {
                    if !our_components.is_subset(their_components) {
                        return false;
                    }
                }
            }
        }

        true
    }

    /// Returns a vector of elements that the access and `other` cannot access at the same time.
    #[inline]
    pub fn get_conflicts(&self, other: &Access) -> AccessConflicts {
        let mut conflicts = ComponentIdSet::new();

        // We have a conflict if we write and they read or write, or if they
        // write and we read or write.
        for (lhs_writes, rhs_reads_and_writes, lhs_writes_inverted, rhs_read_and_writes_inverted) in [
            (
                &self.writes,
                &other.read_and_writes,
                self.writes_inverted,
                other.read_and_writes_inverted,
            ),
            (
                &other.writes,
                &self.read_and_writes,
                other.writes_inverted,
                self.read_and_writes_inverted,
            ),
        ] {
            // There's no way that I can see to do this without a temporary.
            // Neither CNF nor DNF allows us to avoid one.
            let temp_conflicts: ComponentIdSet =
                match (lhs_writes_inverted, rhs_read_and_writes_inverted) {
                    (true, true) => return AccessConflicts::All,
                    (true, false) => rhs_reads_and_writes.difference(lhs_writes).collect(),
                    (false, true) => lhs_writes.difference(rhs_reads_and_writes).collect(),
                    (false, false) => lhs_writes.intersection(rhs_reads_and_writes).collect(),
                };
            conflicts.union_with(&temp_conflicts);
        }

        AccessConflicts::Individual(conflicts)
    }

    /// Returns the indices of the components that this has an archetypal access to.
    ///
    /// These are components whose values are not accessed (and thus will never cause conflicts),
    /// but whose presence in an archetype may affect query results.
    ///
    /// Currently, this is only used for [`Has<T>`].
    ///
    /// [`Has<T>`]: crate::query::Has
    pub fn archetypal(&self) -> &ComponentIdSet {
        &self.archetypal
    }

    /// Returns the set of components with read or write access,
    /// or an error if the access is unbounded.
    pub fn try_reads_and_writes(&self) -> Result<&ComponentIdSet, UnboundedAccessError> {
        // writes_inverted is only ever true when read_and_writes_inverted is
        // also true. Therefore it is sufficient to check just read_and_writes_inverted.
        if self.read_and_writes_inverted {
            return Err(UnboundedAccessError {
                writes_inverted: self.writes_inverted,
                read_and_writres_inverted: self.read_and_writes_inverted,
            });
        }
        Ok(&self.read_and_writes)
    }

    /// Returns the set of components with write access,
    /// or an error if the access is unbounded.
    pub fn try_writes(&self) -> Result<&ComponentIdSet, UnboundedAccessError> {
        if self.writes_inverted {
            return Err(UnboundedAccessError {
                writes_inverted: self.writes_inverted,
                read_and_writres_inverted: self.read_and_writes_inverted,
            });
        }
        Ok(&self.writes)
    }

    /// Returns an iterator over the component IDs and their [`ComponentAccessKind`].
    ///
    /// Returns `Err(UnboundedAccess)` if the access is unbounded.
    /// This typically occurs when an [`Access`] is marked as accessing all
    /// components, and then adding exceptions.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use bevy_ecs::query::{Access, ComponentAccessKind};
    /// # use bevy_ecs::component::ComponentId;
    /// let mut access = Access::default();
    ///
    /// access.add_read(ComponentId::new(1));
    /// access.add_write(ComponentId::new(2));
    /// access.add_archetypal(ComponentId::new(3));
    ///
    /// let result = access
    ///     .try_iter_access()
    ///     .map(Iterator::collect::<Vec<_>>);
    ///
    /// assert_eq!(
    ///     result,
    ///     Ok(vec![
    ///         ComponentAccessKind::Shared(ComponentId::new(1)),
    ///         ComponentAccessKind::Exclusive(ComponentId::new(2)),
    ///         ComponentAccessKind::Archetypal(ComponentId::new(3)),
    ///     ]),
    /// );
    /// ```
    pub fn try_iter_access(
        &self,
    ) -> Result<impl Iterator<Item = ComponentAccessKind> + '_, UnboundedAccessError> {
        let reads_and_writes = self.try_reads_and_writes()?.iter().map(|index| {
            if self.writes.contains(index) {
                ComponentAccessKind::Exclusive(index)
            } else {
                ComponentAccessKind::Shared(index)
            }
        });

        let archetypal = self
            .archetypal
            .difference(&self.read_and_writes)
            .map(ComponentAccessKind::Archetypal);

        Ok(reads_and_writes.chain(archetypal))
    }
}

/// Performs an in-place union of `other` into `self`, where either set may be inverted.
///
/// Each set corresponds to a `FixedBitSet` if `inverted` is `false`,
/// or to the infinite (co-finite) complement of the `FixedBitSet` if `inverted` is `true`.
///
/// This updates the `self` set to include any elements in the `other` set.
/// Note that this may change `self_inverted` to `true` if we add an infinite
/// set to a finite one, resulting in a new infinite set.
fn invertible_union_with(
    self_set: &mut ComponentIdSet,
    self_inverted: &mut bool,
    other_set: &ComponentIdSet,
    other_inverted: bool,
) {
    match (*self_inverted, other_inverted) {
        (true, true) => self_set.intersect_with(other_set),
        (true, false) => self_set.difference_with(other_set),
        (false, true) => {
            *self_inverted = true;
            self_set.difference_from(other_set);
        }
        (false, false) => self_set.union_with(other_set),
    }
}

/// Performs an in-place set difference of `other` from `self`, where either set may be inverted.
///
/// Each set corresponds to a `FixedBitSet` if `inverted` is `false`,
/// or to the infinite (co-finite) complement of the `FixedBitSet` if `inverted` is `true`.
///
/// This updates the `self` set to remove any elements in the `other` set.
/// Note that this may change `self_inverted` to `false` if we remove an
/// infinite set from another infinite one, resulting in a finite difference.
fn invertible_difference_with(
    self_set: &mut ComponentIdSet,
    self_inverted: &mut bool,
    other_set: &ComponentIdSet,
    other_inverted: bool,
) {
    // We can share the implementation of `invertible_union_with` with some algebra:
    // A - B = A & !B = !(!A | B)
    *self_inverted = !*self_inverted;
    invertible_union_with(self_set, self_inverted, other_set, other_inverted);
    *self_inverted = !*self_inverted;
}

/// Error returned when attempting to iterate over items included in an [`Access`]
/// if the access excludes items rather than including them.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Error)]
#[error("Access is unbounded")]
pub struct UnboundedAccessError {
    /// [`Access`] is defined in terms of _excluding_ [exclusive](ComponentAccessKind::Exclusive)
    /// access.
    pub writes_inverted: bool,
    /// [`Access`] is defined in terms of _excluding_ [shared](ComponentAccessKind::Shared) and
    /// [exclusive](ComponentAccessKind::Exclusive) access.
    pub read_and_writres_inverted: bool,
}

/// Describes the level of access for a particular component as defined in an [`Access`].
#[derive(PartialEq, Eq, Hash, Debug, Clone, Copy)]
pub enum ComponentAccessKind {
    /// Archetypical access, such as `Has<Foo>`.
    Archetypal(ComponentId),
    /// Shared access, such as `&Foo`.
    Shared(ComponentId),
    /// Exclusive access, such as `&mut Foo`.
    Exclusive(ComponentId),
}

impl ComponentAccessKind {
    /// Gets the index of this `ComponentAccessKind`.
    pub fn index(&self) -> &ComponentId {
        let (Self::Archetypal(value) | Self::Shared(value) | Self::Exclusive(value)) = self;
        value
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
pub struct FilteredAccess {
    pub(crate) access: Access,
    pub(crate) required: ComponentIdSet,
    // An array of filter sets to express `With` or `Without` clauses in disjunctive normal form, for example: `Or<(With<A>, With<B>)>`.
    // Filters like `(With<A>, Or<(With<B>, Without<C>)>` are expanded into `Or<((With<A>, With<B>), (With<A>, Without<C>))>`.
    pub(crate) filter_sets: Vec<AccessFilters>,
}

// This is needed since `#[derive(Clone)]` does not generate optimized `clone_from`.
impl Clone for FilteredAccess {
    fn clone(&self) -> Self {
        Self {
            access: self.access.clone(),
            required: self.required.clone(),
            filter_sets: self.filter_sets.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.access.clone_from(&source.access);
        self.required.clone_from(&source.required);
        self.filter_sets.clone_from(&source.filter_sets);
    }
}

impl Default for FilteredAccess {
    fn default() -> Self {
        Self::matches_everything()
    }
}

impl From<FilteredAccess> for FilteredAccessSet {
    fn from(filtered_access: FilteredAccess) -> Self {
        let mut base = FilteredAccessSet::default();
        base.add(filtered_access);
        base
    }
}

/// Records how two accesses conflict with each other
#[derive(Debug, PartialEq, From)]
pub enum AccessConflicts {
    /// Conflict is for all indices
    All,
    /// There is a conflict for a subset of indices
    Individual(ComponentIdSet),
}

impl AccessConflicts {
    fn add(&mut self, other: &Self) {
        match (self, other) {
            (s, AccessConflicts::All) => {
                *s = AccessConflicts::All;
            }
            (AccessConflicts::Individual(this), AccessConflicts::Individual(other)) => {
                this.extend(other);
            }
            _ => {}
        }
    }

    /// Returns true if there are no conflicts present
    pub fn is_empty(&self) -> bool {
        match self {
            AccessConflicts::All => false,
            AccessConflicts::Individual(set) => set.is_clear(),
        }
    }

    pub(crate) fn format_conflict_list(&self, world: UnsafeWorldCell) -> String {
        match self {
            AccessConflicts::All => String::new(),
            AccessConflicts::Individual(indices) => indices
                .iter()
                .map(|index| {
                    format!(
                        "{}",
                        world.components().get_name(index).unwrap().shortname()
                    )
                })
                .collect::<Vec<_>>()
                .join(", "),
        }
    }

    /// An [`AccessConflicts`] which represents the absence of any conflict
    pub(crate) fn empty() -> Self {
        Self::Individual(ComponentIdSet::new())
    }
}

impl From<Vec<ComponentId>> for AccessConflicts {
    fn from(value: Vec<ComponentId>) -> Self {
        Self::Individual(value.into_iter().collect())
    }
}

impl FilteredAccess {
    /// Returns a `FilteredAccess` which has no access and matches everything.
    /// This is the equivalent of a `TRUE` logic atom.
    pub fn matches_everything() -> Self {
        Self {
            access: Access::default(),
            required: ComponentIdSet::default(),
            filter_sets: vec![AccessFilters::default()],
        }
    }

    /// Returns a `FilteredAccess` which has no access and matches nothing.
    /// This is the equivalent of a `FALSE` logic atom.
    pub fn matches_nothing() -> Self {
        Self {
            access: Access::default(),
            required: ComponentIdSet::default(),
            filter_sets: Vec::new(),
        }
    }

    /// Returns a reference to the underlying unfiltered access.
    #[inline]
    pub fn access(&self) -> &Access {
        &self.access
    }

    /// Returns a mutable reference to the underlying unfiltered access.
    #[inline]
    pub fn access_mut(&mut self) -> &mut Access {
        &mut self.access
    }

    /// Adds access to the component given by `index`.
    pub fn add_read(&mut self, index: ComponentId) {
        self.access.add_read(index);
        self.add_required(index);
        self.and_with(index);
    }

    /// Adds exclusive access to the component given by `index`.
    pub fn add_write(&mut self, index: ComponentId) {
        self.access.add_write(index);
        self.add_required(index);
        self.and_with(index);
    }

    fn add_required(&mut self, index: ComponentId) {
        self.required.insert(index);
    }

    /// Adds a `With` filter: corresponds to a conjunction (AND) operation.
    ///
    /// Suppose we begin with `Or<(With<A>, With<B>)>`, which is represented by an array of two `AccessFilter` instances.
    /// Adding `AND With<C>` via this method transforms it into the equivalent of  `Or<((With<A>, With<C>), (With<B>, With<C>))>`.
    pub fn and_with(&mut self, index: ComponentId) {
        for filter in &mut self.filter_sets {
            filter.with.insert(index);
        }
    }

    /// Adds a `Without` filter: corresponds to a conjunction (AND) operation.
    ///
    /// Suppose we begin with `Or<(With<A>, With<B>)>`, which is represented by an array of two `AccessFilter` instances.
    /// Adding `AND Without<C>` via this method transforms it into the equivalent of  `Or<((With<A>, Without<C>), (With<B>, Without<C>))>`.
    pub fn and_without(&mut self, index: ComponentId) {
        for filter in &mut self.filter_sets {
            filter.without.insert(index);
        }
    }

    /// Appends an array of filters: corresponds to a disjunction (OR) operation.
    ///
    /// As the underlying array of filters represents a disjunction,
    /// where each element (`AccessFilters`) represents a conjunction,
    /// we can simply append to the array.
    pub fn append_or(&mut self, other: &FilteredAccess) {
        self.filter_sets.append(&mut other.filter_sets.clone());
    }

    /// Adds all of the accesses from `other` to `self`.
    pub fn extend_access(&mut self, other: &FilteredAccess) {
        self.access.extend(&other.access);
    }

    /// Returns `true` if this and `other` can be active at the same time.
    pub fn is_compatible(&self, other: &FilteredAccess) -> bool {
        if self.access.is_compatible(&other.access) {
            return true;
        }

        // If the access instances are incompatible, we want to check that whether filters can
        // guarantee that queries are disjoint.
        // Since the `filter_sets` array represents a Disjunctive Normal Form formula ("ORs of ANDs"),
        // we need to make sure that each filter set (ANDs) rule out every filter set from the `other` instance.
        //
        // For example, `Query<&mut C, Or<(With<A>, Without<B>)>>` is compatible `Query<&mut C, (With<B>, Without<A>)>`,
        // but `Query<&mut C, Or<(Without<A>, Without<B>)>>` isn't compatible with `Query<&mut C, Or<(With<A>, With<B>)>>`.
        self.filter_sets.iter().all(|filter| {
            other
                .filter_sets
                .iter()
                .all(|other_filter| filter.is_ruled_out_by(other_filter))
        })
    }

    /// Returns a vector of elements that this and `other` cannot access at the same time.
    pub fn get_conflicts(&self, other: &FilteredAccess) -> AccessConflicts {
        if !self.is_compatible(other) {
            // filters are disjoint, so we can just look at the unfiltered intersection
            return self.access.get_conflicts(&other.access);
        }
        AccessConflicts::empty()
    }

    /// Adds all access and filters from `other`.
    ///
    /// Corresponds to a conjunction operation (AND) for filters.
    ///
    /// Extending `Or<(With<A>, Without<B>)>` with `Or<(With<C>, Without<D>)>` will result in
    /// `Or<((With<A>, With<C>), (With<A>, Without<D>), (Without<B>, With<C>), (Without<B>, Without<D>))>`.
    pub fn extend(&mut self, other: &FilteredAccess) {
        self.access.extend(&other.access);
        self.required.union_with(&other.required);

        // We can avoid allocating a new array of bitsets if `other` contains just a single set of filters:
        // in this case we can short-circuit by performing an in-place union for each bitset.
        if other.filter_sets.len() == 1 {
            for filter in &mut self.filter_sets {
                filter.with.union_with(&other.filter_sets[0].with);
                filter.without.union_with(&other.filter_sets[0].without);
            }
            return;
        }

        let mut new_filters = Vec::with_capacity(self.filter_sets.len() * other.filter_sets.len());
        for filter in &self.filter_sets {
            for other_filter in &other.filter_sets {
                let mut new_filter = filter.clone();
                new_filter.with.union_with(&other_filter.with);
                new_filter.without.union_with(&other_filter.without);
                new_filters.push(new_filter);
            }
        }
        self.filter_sets = new_filters
    }

    /// Sets the underlying unfiltered access as having access to all components.
    pub fn read_all(&mut self) {
        self.access.read_all();
    }

    /// Sets the underlying unfiltered access as having mutable access to all components.
    pub fn write_all(&mut self) {
        self.access.write_all();
    }

    /// Returns `true` if the set is a subset of another, i.e. `other` contains
    /// at least all the values in `self`.
    pub fn is_subset(&self, other: &FilteredAccess) -> bool {
        self.required.is_subset(&other.required) && self.access().is_subset(other.access())
    }

    /// Returns the set of components that must be present for this access to match.
    /// These components will also be included in the [`AccessFilters::with`] collection
    /// for every filter in [`Self::filter_sets`].
    ///
    /// This is used by [query transmutes](crate::system::Query::transmute_lens) to ensure that
    /// components read by the query are present.
    /// This will include components from query types like `&C`,
    /// but not from filters like [`With<C>`](super::With),
    /// and not from optional data like `Option<&C>`.
    pub fn required(&self) -> &ComponentIdSet {
        &self.required
    }

    /// The list of filters, expressed in disjunctive normal form.
    ///
    /// This [`FilteredAccess`] will match an entity if
    /// *any* of the [`AccessFilters`] matches the entity.
    pub fn filter_sets(&self) -> &[AccessFilters] {
        &self.filter_sets
    }

    /// Returns the indices of the elements that this access filters for.
    pub fn with_filters(&self) -> impl Iterator<Item = ComponentId> + '_ {
        self.filter_sets.iter().flat_map(|f| f.with.iter())
    }

    /// Returns the indices of the elements that this access filters out.
    pub fn without_filters(&self) -> impl Iterator<Item = ComponentId> + '_ {
        self.filter_sets.iter().flat_map(|f| f.without.iter())
    }

    /// Returns true if the index is used by this `FilteredAccess` in filters or archetypal access.
    /// This includes most ways to access a component, but notably excludes `EntityRef` and `EntityMut`
    /// along with anything inside `Option<T>`.
    pub fn contains(&self, index: ComponentId) -> bool {
        self.access().has_archetypal(index)
            || self
                .filter_sets
                .iter()
                .any(|f| f.with.contains(index) || f.without.contains(index))
    }
}

/// A clause in disjunctive normal form that filters entities by their components.
/// An [`AccessFilters`] matches entities that have *all* the components in the
/// `with` filters and *none* of the components in the `without` filters.
#[derive(Eq, PartialEq, Default, Debug)]
pub struct AccessFilters {
    pub(crate) with: ComponentIdSet,
    pub(crate) without: ComponentIdSet,
}

// This is needed since `#[derive(Clone)]` does not generate optimized `clone_from`.
impl Clone for AccessFilters {
    fn clone(&self) -> Self {
        Self {
            with: self.with.clone(),
            without: self.without.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.with.clone_from(&source.with);
        self.without.clone_from(&source.without);
    }
}

impl AccessFilters {
    /// The set of components that must all be present for this [`AccessFilters`] to match.
    pub fn with(&self) -> &ComponentIdSet {
        &self.with
    }

    /// The set of components that must all be absent for this [`AccessFilters`] to match.
    pub fn without(&self) -> &ComponentIdSet {
        &self.without
    }

    fn is_ruled_out_by(&self, other: &Self) -> bool {
        // Although not technically complete, we don't consider the case when `AccessFilters`'s
        // `without` bitset contradicts its own `with` bitset (e.g. `(With<A>, Without<A>)`).
        // Such query would be considered compatible with any other query, but as it's almost
        // always an error, we ignore this case instead of treating such query as compatible
        // with others.
        !self.with.is_disjoint(&other.without) || !self.without.is_disjoint(&other.with)
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
pub struct FilteredAccessSet {
    combined_access: Access,
    filtered_accesses: Vec<FilteredAccess>,
}

// This is needed since `#[derive(Clone)]` does not generate optimized `clone_from`.
impl Clone for FilteredAccessSet {
    fn clone(&self) -> Self {
        Self {
            combined_access: self.combined_access.clone(),
            filtered_accesses: self.filtered_accesses.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.combined_access.clone_from(&source.combined_access);
        self.filtered_accesses.clone_from(&source.filtered_accesses);
    }
}

impl FilteredAccessSet {
    /// Creates an empty [`FilteredAccessSet`].
    pub const fn new() -> Self {
        Self {
            combined_access: Access::new(),
            filtered_accesses: Vec::new(),
        }
    }

    /// Returns a reference to the unfiltered access of the entire set.
    #[inline]
    pub fn combined_access(&self) -> &Access {
        &self.combined_access
    }

    /// Returns a reference to the filtered accesses of the set.
    #[inline]
    pub fn filtered_accesses(&self) -> &[FilteredAccess] {
        &self.filtered_accesses
    }

    /// Returns `true` if this and `other` can be active at the same time.
    ///
    /// Access conflict resolution happen in two steps:
    /// 1. A "coarse" check, if there is no mutual unfiltered conflict between
    ///    `self` and `other`, we already know that the two access sets are
    ///    compatible.
    /// 2. A "fine grained" check, it kicks in when the "coarse" check fails.
    ///    the two access sets might still be compatible if some of the accesses
    ///    are restricted with the [`With`](super::With) or [`Without`](super::Without) filters so that access is
    ///    mutually exclusive. The fine grained phase iterates over all filters in
    ///    the `self` set and compares it to all the filters in the `other` set,
    ///    making sure they are all mutually compatible.
    pub fn is_compatible(&self, other: &FilteredAccessSet) -> bool {
        if self.combined_access.is_compatible(other.combined_access()) {
            return true;
        }
        for filtered in &self.filtered_accesses {
            for other_filtered in &other.filtered_accesses {
                if !filtered.is_compatible(other_filtered) {
                    return false;
                }
            }
        }
        true
    }

    /// Returns a vector of elements that this set and `other` cannot access at the same time.
    pub fn get_conflicts(&self, other: &FilteredAccessSet) -> AccessConflicts {
        // if the unfiltered access is incompatible, must check each pair
        let mut conflicts = AccessConflicts::empty();
        if !self.combined_access.is_compatible(other.combined_access()) {
            for filtered in &self.filtered_accesses {
                for other_filtered in &other.filtered_accesses {
                    conflicts.add(&filtered.get_conflicts(other_filtered));
                }
            }
        }
        conflicts
    }

    /// Returns a vector of elements that this set and `other` cannot access at the same time.
    pub fn get_conflicts_single(&self, filtered_access: &FilteredAccess) -> AccessConflicts {
        // if the unfiltered access is incompatible, must check each pair
        let mut conflicts = AccessConflicts::empty();
        if !self.combined_access.is_compatible(filtered_access.access()) {
            for filtered in &self.filtered_accesses {
                conflicts.add(&filtered.get_conflicts(filtered_access));
            }
        }
        conflicts
    }

    /// Adds the filtered access to the set.
    pub fn add(&mut self, filtered_access: FilteredAccess) {
        self.combined_access.extend(&filtered_access.access);
        self.filtered_accesses.push(filtered_access);
    }

    /// Adds a read access to a resource to the set.
    pub fn add_resource_read(&mut self, index: ComponentId) {
        let mut filter = FilteredAccess::default();
        filter.add_read(index);
        filter.and_with(IS_RESOURCE);
        self.add(filter);
    }

    /// Adds a read access to a component to the set.
    pub(crate) fn add_unfiltered_component_read(&mut self, index: ComponentId) {
        let mut filter = FilteredAccess::default();
        filter.add_read(index);
        self.add(filter);
    }

    /// Adds a write access to a resource to the set.
    pub fn add_resource_write(&mut self, index: ComponentId) {
        let mut filter = FilteredAccess::default();
        filter.add_write(index);
        filter.and_with(IS_RESOURCE);
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
        self.combined_access
            .extend(&filtered_access_set.combined_access);
        self.filtered_accesses
            .extend(filtered_access_set.filtered_accesses);
    }

    /// Marks the set as reading all possible indices of type T.
    pub fn read_all(&mut self) {
        let mut filter = FilteredAccess::matches_everything();
        filter.read_all();
        self.add(filter);
    }

    /// Marks the set as writing all T.
    pub fn write_all(&mut self) {
        let mut filter = FilteredAccess::matches_everything();
        filter.write_all();
        self.add(filter);
    }

    /// Removes all accesses stored in this set.
    pub fn clear(&mut self) {
        self.combined_access.clear();
        self.filtered_accesses.clear();
    }
}

/// A set of [`ComponentId`]s.
#[derive(Default, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct ComponentIdSet(FixedBitSet);

impl ComponentIdSet {
    /// Create a new empty `ComponentIdSet`.
    #[inline]
    pub const fn new() -> Self {
        Self(FixedBitSet::new())
    }

    #[cfg(test)]
    pub(crate) fn from_bits(bits: FixedBitSet) -> Self {
        Self(bits)
    }

    /// Adds a [`ComponentId`] to the set.
    #[inline]
    pub fn insert(&mut self, index: ComponentId) {
        self.0.grow_and_insert(index.index());
    }

    /// Removes a [`ComponentId`] from the set.
    #[inline]
    pub fn remove(&mut self, index: ComponentId) {
        if index.index() < self.0.len() {
            self.0.remove(index.index());
        }
    }

    /// Removes all [`ComponentId`]s from the set.
    #[inline]
    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Returns `true` if the [`ComponentId`] is in the set.
    #[inline]
    pub fn contains(&self, index: ComponentId) -> bool {
        self.0.contains(index.index())
    }

    /// Returns `true` if `self` has no elements in common with `other`. This
    /// is equivalent to checking for an empty intersection.
    #[inline]
    pub fn is_disjoint(&self, other: &ComponentIdSet) -> bool {
        self.0.is_disjoint(&other.0)
    }

    /// Returns `true` if the set is a subset of another, i.e. `other` contains
    /// at least all the values in `self`.
    #[inline]
    pub fn is_subset(&self, other: &ComponentIdSet) -> bool {
        self.0.is_subset(&other.0)
    }

    /// Returns `true` if the set is empty.
    #[inline]
    pub fn is_clear(&self) -> bool {
        self.0.is_clear()
    }

    /// Iterates the [`ComponentId`]s in the set.
    #[inline]
    pub fn iter(&self) -> ComponentIdIter<Ones<'_>> {
        ComponentIdIter(self.0.ones())
    }

    /// Returns a lazy iterator over the union of two [`ComponentIdSet`]s.
    #[inline]
    pub fn union<'a>(&'a self, other: &'a ComponentIdSet) -> ComponentIdIter<Union<'a>> {
        ComponentIdIter(self.0.union(&other.0))
    }

    /// Returns a lazy iterator over the intersection of two [`ComponentIdSet`]s.
    #[inline]
    pub fn intersection<'a>(
        &'a self,
        other: &'a ComponentIdSet,
    ) -> ComponentIdIter<Intersection<'a>> {
        ComponentIdIter(self.0.intersection(&other.0))
    }

    /// Returns a lazy iterator over the difference of two [`ComponentIdSet`]s.
    #[inline]
    pub fn difference<'a>(&'a self, other: &'a ComponentIdSet) -> ComponentIdIter<Difference<'a>> {
        ComponentIdIter(self.0.difference(&other.0))
    }

    /// In-place union of two [`ComponentIdSet`]s.
    #[inline]
    pub fn union_with(&mut self, other: &ComponentIdSet) {
        self.0.union_with(&other.0);
    }

    /// In-place intersection of two [`ComponentIdSet`]s.
    #[inline]
    pub fn intersect_with(&mut self, other: &ComponentIdSet) {
        self.0.intersect_with(&other.0);
    }

    /// In-place difference of two [`ComponentIdSet`]s.
    #[inline]
    pub fn difference_with(&mut self, other: &ComponentIdSet) {
        self.0.difference_with(&other.0);
    }

    /// In-place reversed difference of two [`ComponentIdSet`]s.
    /// This sets `self` to be `other.difference(self)`.
    #[inline]
    pub fn difference_from(&mut self, other: &ComponentIdSet) {
        // Calculate `other - self` as `!self & other`
        // We have to grow here because the new bits are going to get flipped to 1.
        self.0.grow(other.0.len());
        self.0.toggle_range(..);
        self.0.intersect_with(&other.0);
    }
}

impl Debug for ComponentIdSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `FixedBitSet` normally has a `Debug` output like:
        // FixedBitSet { data: [ 160 ], length: 8 }
        // Instead, print the list of set values, like:
        // [ 5, 7 ]
        // Don't wrap in `ComponentId`, since that would just output:
        // [ ComponentId(5), ComponentId(7) ]
        f.debug_list().entries(self.0.ones()).finish()
    }
}

impl Clone for ComponentIdSet {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }

    #[inline]
    fn clone_from(&mut self, source: &Self) {
        self.0.clone_from(&source.0);
    }
}

impl IntoIterator for ComponentIdSet {
    type Item = ComponentId;

    type IntoIter = ComponentIdIter<IntoOnes>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        ComponentIdIter(self.0.into_ones())
    }
}

impl<'a> IntoIterator for &'a ComponentIdSet {
    type Item = ComponentId;

    type IntoIter = ComponentIdIter<Ones<'a>>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl FromIterator<ComponentId> for ComponentIdSet {
    #[inline]
    fn from_iter<T: IntoIterator<Item = ComponentId>>(iter: T) -> Self {
        Self(FixedBitSet::from_iter(
            iter.into_iter().map(ComponentId::index),
        ))
    }
}

impl Extend<ComponentId> for ComponentIdSet {
    #[inline]
    fn extend<T: IntoIterator<Item = ComponentId>>(&mut self, iter: T) {
        self.0.extend(iter.into_iter().map(ComponentId::index));
    }
}

/// An iterator of [`ComponentId`]s.
///
/// This is equivalent to `map(ComponentId::new)`,
/// but is a named type to allow it to be used in associated types.
#[repr(transparent)]
pub struct ComponentIdIter<I>(I);

impl<I: Iterator<Item = usize>> Iterator for ComponentIdIter<I> {
    type Item = ComponentId;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(ComponentId::new)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<I: DoubleEndedIterator<Item = usize>> DoubleEndedIterator for ComponentIdIter<I> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(ComponentId::new)
    }
}

impl<I: FusedIterator<Item = usize>> FusedIterator for ComponentIdIter<I> {}
