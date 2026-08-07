use std::ops::Deref;

use crate::{
    debug::MaybeLocation,
    ecs::{
        event::{Event, EventKey},
        world::{World, unsafe_world_cell::UnsafeWorldCell},
    },
};

/// A [`World`] reference that disallows structural ECS changes.
/// This includes initializing resources, registering components or spawning entities.
///
/// This means that in order to add entities, for example, you will need to use commands instead of the world directly.
pub struct DeferredWorld<'w> {
    // SAFETY: Implementers must not use this reference to make structural changes
    world: UnsafeWorldCell<'w>,
}

impl<'w> Deref for DeferredWorld<'w> {
    type Target = World;

    fn deref(&self) -> &Self::Target {
        // SAFETY: Structural changes cannot be made through &World
        unsafe { self.world.world() }
    }
}

impl<'w> DeferredWorld<'w> {
    /// Sends a global [`Event`] without any targets.
    ///
    /// This will run any [`Observer`] of the given [`Event`] that isn't scoped to specific targets.
    ///
    /// [`Observer`]: crate::observer::Observer
    pub fn trigger<'a>(&mut self, event: impl Event<Trigger<'a>: Default>) {
        todo!()
    }

    /// Triggers all `event` observers for the given `targets`
    ///
    /// # Safety
    /// - Caller must ensure `E` is accessible as the type represented by `event_key`
    #[inline]
    pub unsafe fn trigger_raw<'a, E: Event>(
        &mut self,
        event_key: EventKey,
        event: &mut E,
        trigger: &mut E::Trigger<'a>,
        caller: MaybeLocation,
    ) {
        todo!()
    }
}

impl<'w> UnsafeWorldCell<'w> {
    /// Turn self into a [`DeferredWorld`]
    ///
    /// # Safety
    /// Caller must ensure there are no outstanding mutable references to world and no
    /// outstanding references to the world's command queue, resource or component data
    #[inline]
    pub unsafe fn into_deferred(self) -> DeferredWorld<'w> {
        DeferredWorld { world: self }
    }
}

impl<'w> From<&'w mut World> for DeferredWorld<'w> {
    fn from(world: &'w mut World) -> DeferredWorld<'w> {
        DeferredWorld {
            world: world.as_unsafe_world_cell(),
        }
    }
}

// TODO!
