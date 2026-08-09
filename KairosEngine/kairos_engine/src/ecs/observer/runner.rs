//! Logic for evaluating observers, and storing functions inside of observers.

use std::any::Any;

use crate::{
    debug::DebugCheckedUnwrap,
    ecs::{
        bundle::Bundle,
        entity::Entity,
        error::ErrorContext,
        event::Event,
        observer::{Observer, On, TriggerContext},
        system::{ObserverSystem, RunSystemError},
        world::DeferredWorld,
    },
    ptr::PtrMut,
};

#[cfg(test)]
mod tests;

pub type ObserverRunner =
    unsafe fn(DeferredWorld, observer: Entity, &TriggerContext, event: PtrMut, trigger: PtrMut);

/// # Safety
///
/// - `world` must be the [`DeferredWorld`] that the `entity` is defined in
/// - `event_ptr` must match the `E` [`Event`] type.
/// - `trigger_ptr` must match the [`Event::Trigger`] type for `E`.
/// - `trigger_context`'s [`TriggerContext::event_key`] must match the `E` event type.
///
// NOTE: The way `Trigger` and `On` interact in this implementation is _subtle_ and _easily invalidated_
// from a soundness perspective. Please read and understand the safety comments before making any changes,
// either here or in `On`.
pub(super) unsafe fn observer_system_runner<E: Event, B: Bundle, S: ObserverSystem<E, B>>(
    mut world: DeferredWorld,
    observer: Entity,
    trigger_context: &TriggerContext,
    event_ptr: PtrMut,
    trigger_ptr: PtrMut,
) {
    let world = world.as_unsafe_world_cell();

    // SAFETY: Observer was triggered so must still exist in world
    let observer_cell = unsafe { world.get_entity(observer).debug_checked_unwrap() };
    // SAFETY: Observer was triggered so must have an `Observer`
    let mut state = unsafe { observer_cell.get_mut::<Observer>().debug_checked_unwrap() };

    let last_trigger = world.last_trigger_id();
    if state.last_trigger_id == last_trigger {
        return;
    }
    state.last_trigger_id = last_trigger;

    // SAFETY:
    // - Conditions are initialized during observer registration (hook_on_add)
    // - Conditions are ReadOnlySystem (enforced by SystemCondition trait)
    // - No aliasing: we hold &mut Observer, but conditions only read world state
    let mut should_run = true;
    for condition in state.conditions.iter_mut() {
        should_run &= unsafe {
            // SAFETY: See the safety comment above.
            condition.check(world)
        }
    }

    if !should_run {
        return;
    }

    // SAFETY: Caller ensures `trigger_ptr` is castable to `&mut E::Trigger<'_>`
    // The soundness story here is complicated: This casts to &'a mut E::Trigger<'a> which notably
    // casts the _arbitrary lifetimes_ of the passed in `trigger_ptr` (&'w E::Trigger<'t>, which are
    // 'w and 't on On<'w, 't>) as the _same_ lifetime 'a, which is _local to this function call_.
    // This becomes On<'a, 'a> in practice. This is why `On<'w, 't>` has the strict constraint that
    // the 'w lifetime can never be exposed. To do so would make it possible to introduce use-after-free bugs.
    // See this thread for more details: <https://github.com/bevyengine/bevy/pull/20731#discussion_r2311907935>
    let trigger: &mut E::Trigger<'_> = unsafe { trigger_ptr.deref_mut() };

    let on: On<E, B> = On::new(
        unsafe { event_ptr.deref_mut() },
        observer,
        trigger,
        trigger_context,
    );

    // SAFETY:
    // - observer was triggered so must have an `Observer` component.
    // - observer cannot be dropped or mutated until after the system pointer is already dropped.
    let system: *mut dyn ObserverSystem<E, B> = unsafe {
        let system: &mut dyn Any = state.system.as_mut();
        let system = system.downcast_mut::<S>().debug_checked_unwrap();
        &mut *system
    };

    // SAFETY:
    // - there are no outstanding references to world except a private component
    // - system is an `ObserverSystem` so won't mutate world beyond the access of a `DeferredWorld`
    //   and is never exclusive
    // - system is the same type erased system from above
    unsafe {
        // #[cfg(feature = "hotpatching")]
        // if world
        //     .get_resource_ref::<crate::HotPatchChanges>()
        //     .is_none_or(|r| r.is_changed_after((*system).get_last_run()))
        // {
        //     (*system).refresh_hotpatch();
        // };

        if let Err(RunSystemError::Failed(err)) = (*system).run_unsafe(on, world) {
            let handler = state
                .error_handler
                .unwrap_or_else(|| world.fallback_error_handler());
            handler(
                err,
                ErrorContext::Observer {
                    name: (*system).name(),
                    last_run: (*system).get_last_run(),
                },
            );
        };
        (*system).queue_deferred(world.into_deferred());
    }
}
// TODO!
