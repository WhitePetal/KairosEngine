use std::{any::TypeId, marker::PhantomData};

use thiserror::Error;

use crate::{
    debug::DebugName,
    ecs::{
        component::{Component, Mutable, StorageType},
        entity::Entity,
        error::KairosError,
        system::{BoxedSystem, IntoSystem, SystemInput, SystemParamValidationError},
        world::World,
    },
};

#[derive(Debug, Clone)]
struct TypeIdAndName {
    type_id: TypeId,
    name: DebugName,
}

impl TypeIdAndName {
    fn new<T: 'static>() -> Self {
        Self {
            type_id: TypeId::of::<T>(),
            name: DebugName::type_name::<T>(),
        }
    }
}

impl Default for TypeIdAndName {
    fn default() -> Self {
        Self {
            type_id: TypeId::of::<()>(),
            name: DebugName::type_name::<()>(),
        }
    }
}

/// Marker [`Component`](bevy_ecs::component::Component) for identifying [`SystemId`] [`Entity`]s.
// #[derive(Debug, Default, Clone, Component)]
#[derive(Debug, Default, Clone)]
pub struct SystemIdMarker {
    input_type_id: TypeIdAndName,
    output_type_id: TypeIdAndName,
}

// TODO: use derive
impl Component for SystemIdMarker {
    const STORAGE_TYPE: StorageType = StorageType::Table;

    type Mutability = Mutable;
}

impl SystemIdMarker {
    fn typed_system_id_marker<I: 'static, O: 'static>() -> Self {
        Self {
            input_type_id: TypeIdAndName::new::<I>(),
            output_type_id: TypeIdAndName::new::<O>(),
        }
    }
}

impl<I, O> RemovedSystem<I, O> {
    /// Is the system initialized?
    /// A system is initialized the first time it's ran.
    pub fn initialized(&self) -> bool {
        self.initialized
    }

    /// The system removed from the storage.
    pub fn system(self) -> BoxedSystem<I, O> {
        self.system
    }
}

/// A system that has been removed from the registry.
/// It contains the system and whether or not it has been initialized.
///
/// This struct is returned by [`World::unregister_system`].
pub struct RemovedSystem<I = (), O = ()> {
    initialized: bool,
    system: BoxedSystem<I, O>,
}

/// An identifier for a registered system.
///
/// These are opaque identifiers, keyed to a specific [`World`],
/// and are created via [`World::register_system`].
pub struct SystemId<I: SystemInput = (), O = ()> {
    pub(crate) entity: Entity,
    pub(crate) marker: PhantomData<fn(I) -> O>,
}

impl<I: SystemInput, O> std::fmt::Debug for SystemId<I, O> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SystemId").field(&self.entity).finish()
    }
}

impl World {
    /// Run stored systems by their [`SystemId`].
    /// Before running a system, it must first be registered.
    /// The method [`World::register_system`] stores a given system and returns a [`SystemId`].
    /// This is different from [`RunSystemOnce::run_system_once`](crate::system::RunSystemOnce::run_system_once),
    /// because it keeps local state between calls and change detection works correctly.
    ///
    /// Also runs any queued-up commands.
    ///
    /// In order to run a chained system with an input, use [`World::run_system_with`] instead.
    ///
    /// # Examples
    ///
    /// ## Running a system
    ///
    /// ```
    /// # use bevy_ecs::prelude::*;
    /// fn increment(mut counter: Local<u8>) {
    ///    *counter += 1;
    ///    println!("{}", *counter);
    /// }
    ///
    /// let mut world = World::default();
    /// let counter_one = world.register_system(increment);
    /// let counter_two = world.register_system(increment);
    /// world.run_system(counter_one); // -> 1
    /// world.run_system(counter_one); // -> 2
    /// world.run_system(counter_two); // -> 1
    /// ```
    ///
    /// ## Change detection
    ///
    /// ```
    /// # use bevy_ecs::prelude::*;
    /// #[derive(Resource, Default)]
    /// struct ChangeDetector;
    ///
    /// let mut world = World::default();
    /// world.init_resource::<ChangeDetector>();
    /// let detector = world.register_system(|change_detector: ResMut<ChangeDetector>| {
    ///     if change_detector.is_changed() {
    ///         println!("Something happened!");
    ///     } else {
    ///         println!("Nothing happened.");
    ///     }
    /// });
    ///
    /// // Resources are changed when they are first added
    /// let _ = world.run_system(detector); // -> Something happened!
    /// let _ = world.run_system(detector); // -> Nothing happened.
    /// world.resource_mut::<ChangeDetector>().set_changed();
    /// let _ = world.run_system(detector); // -> Something happened!
    /// ```
    ///
    /// ## Getting system output
    ///
    /// ```
    /// # use bevy_ecs::prelude::*;
    ///
    /// #[derive(Resource)]
    /// struct PlayerScore(i32);
    ///
    /// #[derive(Resource)]
    /// struct OpponentScore(i32);
    ///
    /// fn get_player_score(player_score: Res<PlayerScore>) -> i32 {
    ///   player_score.0
    /// }
    ///
    /// fn get_opponent_score(opponent_score: Res<OpponentScore>) -> i32 {
    ///   opponent_score.0
    /// }
    ///
    /// let mut world = World::default();
    /// world.insert_resource(PlayerScore(3));
    /// world.insert_resource(OpponentScore(2));
    ///
    /// let scoring_systems = [
    ///   ("player", world.register_system(get_player_score)),
    ///   ("opponent", world.register_system(get_opponent_score)),
    /// ];
    ///
    /// for (label, scoring_system) in scoring_systems {
    ///   println!("{label} has score {}", world.run_system(scoring_system).expect("system succeeded"));
    /// }
    /// ```
    pub fn run_system<O: 'static>(
        &mut self,
        id: impl Into<SystemId<(), O>>,
    ) -> Result<O, RegisteredSystemError<(), O>> {
        todo!()
    }

    /// Run a stored chained system by its [`SystemId`], providing an input value.
    /// Before running a system, it must first be registered.
    /// The method [`World::register_system`] stores a given system and returns a [`SystemId`].
    ///
    /// To use the supplied input, the system should have a [`SystemInput`] as the first parameter.
    /// Also runs any queued-up commands.
    ///
    /// # Examples
    ///
    /// ```
    /// # use bevy_ecs::prelude::*;
    /// fn increment(In(increment_by): In<u8>, mut counter: Local<u8>) -> u8 {
    ///   *counter += increment_by;
    ///   *counter
    /// }
    ///
    /// let mut world = World::default();
    /// let counter_one = world.register_system(increment);
    /// let counter_two = world.register_system(increment);
    /// assert_eq!(world.run_system_with(counter_one, 1).unwrap(), 1);
    /// assert_eq!(world.run_system_with(counter_one, 20).unwrap(), 21);
    /// assert_eq!(world.run_system_with(counter_two, 30).unwrap(), 30);
    /// ```
    ///
    /// See [`World::run_system`] for more examples.
    pub fn run_system_with<I, O>(
        &mut self,
        id: impl Into<SystemId<I, O>>,
        input: I::Inner<'_>,
    ) -> Result<O, RegisteredSystemError<I, O>>
    where
        I: SystemInput + 'static,
        O: 'static,
    {
        todo!()
    }

    /// Runs a cached system, registering it if necessary.
    ///
    /// See [`World::register_system_cached`] for more information.
    pub fn run_system_cached<O: 'static, M, S: IntoSystem<(), O, M> + 'static>(
        &mut self,
        system: S,
    ) -> Result<O, RegisteredSystemError<(), O>> {
        todo!()
    }

    /// Runs a cached system with an input, registering it if necessary.
    ///
    /// To use the supplied input, the system should have a [`SystemInput`] as the first parameter.
    /// See [`World::register_system_cached`] for more information.
    pub fn run_system_cached_with<I, O, M, S>(
        &mut self,
        system: S,
        input: I::Inner<'_>,
    ) -> Result<O, RegisteredSystemError<I, O>>
    where
        I: SystemInput + 'static,
        O: 'static,
        S: IntoSystem<I, O, M> + 'static,
    {
        todo!()
    }

    /// Removes a cached system and its [`CachedSystemId`] resource.
    ///
    /// See [`World::register_system_cached`] for more information.
    pub fn unregister_system_cached<I, O, M, S>(
        &mut self,
        _system: S,
    ) -> Result<RemovedSystem<I, O>, RegisteredSystemError<I, O>>
    where
        I: SystemInput + 'static,
        O: 'static,
        S: IntoSystem<I, O, M> + 'static,
    {
        todo!()
    }
}

/// An operation with stored systems failed.
#[derive(Error)]
pub enum RegisteredSystemError<I: SystemInput = (), O = ()> {
    /// A system was run by id, but no system with that id was found.
    ///
    /// Did you forget to register it?
    #[error("System {0:?} was not registered")]
    SystemIdNotRegistered(SystemId<I, O>),
    /// A cached system was removed by value, but no system with its type was found.
    ///
    /// Did you forget to register it?
    #[error("Cached system was not found")]
    SystemNotCached,
    /// The `RegisteredSystem` component is missing.
    #[error(
        "System {0:?} does not have a RegisteredSystem component. This only happens if app code removed the component."
    )]
    MissingRegisteredSystemComponent(SystemId<I, O>),
    /// A system tried to remove itself.
    #[error("System {0:?} tried to remove itself")]
    SelfRemove(SystemId<I, O>),
    /// System could not be run due to parameters that failed validation.
    /// This is not considered an error.
    #[error("System did not run due to failed parameter validation: {0}")]
    Skipped(SystemParamValidationError),
    /// System returned an error or failed required parameter validation.
    #[error("System returned error: {0}")]
    Failed(KairosError),
    /// [`SystemId`] had different input and/or output types than [`SystemIdMarker`]
    #[error("Could not get system from `{}`, entity was `SystemId<{}, {}>`", DebugName::type_name::<SystemId<I, O>>(), .1.input_type_id.name, .1.output_type_id.name)]
    IncorrectType(SystemId<I, O>, SystemIdMarker),
    /// System is not present in the `RegisteredSystem` component.
    // TODO: We should consider using catch_unwind to protect against the panic case.
    #[error(
        "The system is not present in the RegisteredSystem component. This can happen if the system was called recursively or if the system panicked on the last run."
    )]
    SystemMissing(SystemId<I, O>),
}

impl<I: SystemInput, O> std::fmt::Debug for RegisteredSystemError<I, O> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SystemIdNotRegistered(arg0) => {
                f.debug_tuple("SystemIdNotRegistered").field(arg0).finish()
            }
            Self::SystemNotCached => write!(f, "SystemNotCached"),
            Self::MissingRegisteredSystemComponent(arg0) => f
                .debug_tuple("MissingRegisteredSystemComponent")
                .field(arg0)
                .finish(),
            Self::SelfRemove(arg0) => f.debug_tuple("SelfRemove").field(arg0).finish(),
            Self::Skipped(arg0) => f.debug_tuple("Skipped").field(arg0).finish(),
            Self::Failed(arg0) => f.debug_tuple("Failed").field(arg0).finish(),
            Self::IncorrectType(arg0, arg1) => f
                .debug_tuple("IncorrectType")
                .field(arg0)
                .field(arg1)
                .finish(),
            Self::SystemMissing(arg0) => f.debug_tuple("SystemMissing").field(arg0).finish(),
        }
    }
}
