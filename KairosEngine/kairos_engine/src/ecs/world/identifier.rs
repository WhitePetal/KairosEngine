use std::sync::atomic::{AtomicUsize, Ordering};

use crate::ecs::world::FromWorld;

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
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |val| {
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
