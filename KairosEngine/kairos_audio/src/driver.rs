//! The per-frame audio driver, mounted into the application's `Update` stage by
//! [`install`](crate::install).

use kairos_ecs::world::World;
use kairos_time::Time;

use crate::AudioEngine;

/// Advances the [`AudioEngine`] by one frame from the application's `Update`
/// stage.
///
/// An exclusive system because the engine needs `&mut World` twice over at once:
/// as the resource being advanced, and as the entity/asset store the advance
/// reads. `World::resource_scope` splits those borrows for the duration of the
/// update; a `SystemParam` cannot express the split (`&mut World` is not a
/// `SystemParam`), which is why this is not a regular system.
///
/// The frame's `delta_time` comes from the [`Time`] resource, which the `First`
/// stage advanced earlier in the same frame — mounting this in `Update` is what
/// makes that ordering a property of the schedule rather than of a per-frame
/// hand call.
pub(crate) fn audio_update_system(world: &mut World) {
    let delta_time = world.resource::<Time>().delta_time().as_secs_f32();
    world.resource_scope::<AudioEngine, _>(|world, mut audio| {
        audio.update(world, delta_time);
    });
}
