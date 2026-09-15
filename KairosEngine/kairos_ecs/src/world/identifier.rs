use std::sync::atomic::{AtomicUsize, Ordering};

use crate::{change_detection::Tick, query::FilteredAccessSet, storage::SparseSetIndex, system::{ExclusiveSystemParam, ReadOnlySystemParam, SystemMeta, SystemParam, SystemParamValidationError}, world::{FromWorld, World, unsafe_world_cell::UnsafeWorldCell}};

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
// We use usize here because that is the largest `Atomic` we want to require
/// A unique identifier for a [`World`].
///
/// The trait [`FromWorld`] is implemented for this type, which returns the
/// ID of the world passed to [`FromWorld::from_world`].
pub struct WorldId(usize);

/// The next [`WorldId`].
static MAX_WORLD_ID: AtomicUsize = AtomicUsize::new(0);

impl WorldId {
    /// Create a new, unique [`WorldId`]. Returns [`None`] if the supply of unique
    /// [`WorldId`]s has been exhausted
    ///
    /// Please note that the [`WorldId`]s created from this method are unique across
    /// time - if a given [`WorldId`] is [`Drop`]ped its value still cannot be reused
    pub fn new() -> Option<Self> {
        MAX_WORLD_ID
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |val| {
                val.checked_add(1)
            })
            .map(WorldId)
            .ok()
    }
}

impl FromWorld for WorldId {
    #[inline]
    fn from_world(world: &mut super::World) -> Self {
        world.id
    }
}

// SAFETY: No world data is accessed.
unsafe impl ReadOnlySystemParam for WorldId {}

// SAFETY: No world data is accessed.
unsafe impl SystemParam for WorldId {
    type State = ();

    type Item<'world, 'state> = WorldId;

    fn init_state(_: &mut World) -> Self::State {}

    fn init_access(
        _state: &Self::State,
        _system_meta: &mut SystemMeta,
        _component_access_set: &mut FilteredAccessSet,
        _world: &mut World,
    ) {
    }

    #[inline]
    unsafe fn get_param<'world, 'state>(
        _: &'state mut Self::State,
        _: &SystemMeta,
        world: UnsafeWorldCell<'world>,
        _: Tick,
    ) -> Result<Self::Item<'world, 'state>, SystemParamValidationError> {
        Ok(world.id())
    }
}

impl ExclusiveSystemParam for WorldId {
    type State = WorldId;
    type Item<'s> = WorldId;

    fn init(world: &mut World, _system_meta: &mut SystemMeta) -> Self::State {
        world.id()
    }

    fn get_param<'s>(
        state: &'s mut Self::State,
        _system_meta: &SystemMeta,
    ) -> Result<Self::Item<'s>, SystemParamValidationError> {
        Ok(*state)
    }
}

impl SparseSetIndex for WorldId {
    #[inline]
    fn sparse_set_index(&self) -> usize {
        self.0
    }

    #[inline]
    fn get_sparse_set_index(value: usize) -> Self {
        Self(value)
    }
}
