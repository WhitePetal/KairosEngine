use derive_more::{Display, Into};

use crate::{
    debug::DebugName,
    ecs::{
        system::{ExclusiveSystemParam, ReadOnlySystemParam, SystemMeta, SystemParam},
        world::World,
    },
};

#[cfg(test)]
mod tests;

/// [`SystemParam`] that returns the name of the system which it is used in.
///
/// This is not a reliable identifier, it is more so useful for debugging or logging.
///
/// # Examples
///
/// ```
/// # use bevy_ecs::system::SystemName;
/// # use bevy_ecs::system::SystemParam;
///
/// #[derive(SystemParam)]
/// struct Logger {
///     system_name: SystemName,
/// }
///
/// impl Logger {
///     fn log(&mut self, message: &str) {
///         eprintln!("{}: {}", self.system_name, message);
///     }
/// }
///
/// fn system1(mut logger: Logger) {
///     // Prints: "crate_name::mod_name::system1: Hello".
///     logger.log("Hello");
/// }
/// ```
#[derive(Debug, Into, Display)]
pub struct SystemName(DebugName);

impl SystemName {
    /// Gets the name of the system.
    pub fn name(&self) -> DebugName {
        self.0.clone()
    }
}

// SAFETY: no component value access
unsafe impl SystemParam for SystemName {
    type State = ();

    type Item<'w, 's> = SystemName;

    fn init_state(_world: &mut crate::ecs::world::World) -> Self::State {}

    fn init_access(
        _state: &Self::State,
        _system_meta: &mut super::SystemMeta,
        _component_access_set: &mut crate::ecs::query::FilteredAccessSet,
        _world: &mut crate::ecs::world::World,
    ) {
    }

    #[inline]
    unsafe fn get_param<'world, 'state>(
        _state: &'state mut Self::State,
        system_meta: &super::SystemMeta,
        _world: crate::ecs::world::unsafe_world_cell::UnsafeWorldCell<'world>,
        _change_tick: crate::ecs::change_detection::Tick,
    ) -> Result<Self::Item<'world, 'state>, super::SystemParamValidationError> {
        Ok(SystemName(system_meta.name.clone()))
    }
}

// SAFETY: Only reads internal system state
unsafe impl ReadOnlySystemParam for SystemName {}

impl ExclusiveSystemParam for SystemName {
    type State = ();

    type Item<'s> = SystemName;

    fn init(_world: &mut World, _system_meta: &mut SystemMeta) -> Self::State {}

    fn get_param<'s>(
        _state: &'s mut Self::State,
        system_meta: &super::SystemMeta,
    ) -> Result<Self::Item<'s>, super::SystemParamValidationError> {
        Ok(SystemName(system_meta.name.clone()))
    }
}
