use std::{borrow::Cow, marker::PhantomData};

#[cfg(feature = "trace")]
use tracing::{Span, info_span};
use variadics_please::all_tuples;

use crate::{
    debug::DebugName,
    ecs::{
        change_detection::Tick,
        error::BevyError,
        never::Never,
        query::FilteredAccessSet,
        system::{
            FromInput, ReadOnlySystemParam, RunSystemError, SystemInput, SystemParam,
            SystemParamBuilder, SystemParamItem, SystemParamValidationError, SystemStateFlags,
        },
        world::{FromWorld, World, WorldId, unsafe_world_cell::UnsafeWorldCell},
    },
};

/// The metadata of a [`System`].
#[derive(Clone)]
pub struct SystemMeta {
    pub(crate) name: DebugName,
    // NOTE: this must be kept private. making a SystemMeta non-send is irreversible to prevent
    // SystemParams from overriding each other
    flags: SystemStateFlags,
    pub(crate) last_run: Tick,
    #[cfg(feature = "trace")]
    pub(crate) system_span: Span,
    #[cfg(feature = "trace")]
    pub(crate) commands_span: Span,
}

impl SystemMeta {
    pub(crate) fn new<T>() -> Self {
        let name = DebugName::type_name::<T>();
        Self {
            // These spans are initialized during plugin build, so we set the parent to `None` to prevent
            // them from being children of the span that is measuring the plugin build time.
            #[cfg(feature = "trace")]
            system_span: info_span!(parent: None, "system", name = name.clone().to_string()),
            #[cfg(feature = "trace")]
            commands_span: info_span!(parent: None, "system_commands", name = name.clone().to_string()),
            name,
            flags: SystemStateFlags::empty(),
            last_run: Tick::new(0),
        }
    }

    /// Returns the system's name
    #[inline]
    pub fn name(&self) -> &DebugName {
        &self.name
    }

    /// Returns the system's state flags
    pub fn flags(&self) -> SystemStateFlags {
        self.flags
    }

    /// Sets the name of this system.
    ///
    /// Useful to give closure systems more readable and unique names for debugging and tracing.
    #[inline]
    pub fn set_name(&mut self, new_name: impl Into<Cow<'static, str>>) {
        let new_name: Cow<'static, str> = new_name.into();
        #[cfg(feature = "trace")]
        {
            let name = new_name.as_ref();
            self.system_span = info_span!(parent: None, "system", name = name);
            self.commands_span = info_span!(parent: None, "system_commands", name = name);
        }
        self.name = new_name.into();
    }

    /// Gets the last time this system was run.
    #[inline]
    pub fn get_last_run(&self) -> Tick {
        self.last_run
    }

    /// Sets the last time this system was run.
    #[inline]
    pub fn set_last_run(&mut self, last_run: Tick) {
        self.last_run = last_run
    }

    /// Returns true if the system is [`Send`].
    #[inline]
    pub fn is_send(&self) -> bool {
        !self.flags.intersects(SystemStateFlags::NON_SEND)
    }

    /// Sets the system to be not [`Send`].
    ///
    /// This is irreversible.
    #[inline]
    pub fn set_non_send(&mut self) {
        self.flags |= SystemStateFlags::NON_SEND;
    }

    /// Returns true if the system has deferred [`SystemParam`]'s
    #[inline]
    pub fn has_deferred(&self) -> bool {
        self.flags.intersects(SystemStateFlags::DEFERRED)
    }

    /// Marks the system as having deferred buffers like [`Commands`](`super::Commands`)
    /// This lets the scheduler insert [`ApplyDeferred`](`crate::prelude::ApplyDeferred`) systems automatically.
    #[inline]
    pub fn set_has_deferred(&mut self) {
        self.flags |= SystemStateFlags::DEFERRED;
    }

    /// Mark the system to run exclusively. i.e. no other systems will run at the same time.
    pub fn set_exclusive(&mut self) {
        self.flags |= SystemStateFlags::EXCLUSIVE;
    }
}

// TODO: Actually use this in FunctionSystem. We should probably only do this once Systems are constructed using a World reference
// (to avoid the need for unwrapping to retrieve SystemMeta)
/// Holds on to persistent state required to drive [`SystemParam`] for a [`System`].
///
/// This is a powerful and convenient tool for working with exclusive world access,
/// allowing you to fetch data from the [`World`] as if you were running a [`System`].
/// However, simply calling `world::run_system(my_system)` using a [`World::run_system`](World::run_system)
/// can be significantly simpler and ensures that change detection and command flushing work as expected.
///
/// Borrow-checking is handled for you, allowing you to mutably access multiple compatible system parameters at once,
/// and arbitrary system parameters (like [`MessageWriter`](crate::message::MessageWriter)) can be conveniently fetched.
///
/// For an alternative approach to split mutable access to the world, see [`World::resource_scope`].
///
/// # Warning
///
/// [`SystemState`] values created can be cached to improve performance,
/// and *must* be cached and reused in order for system parameters that rely on local state to work correctly.
/// These include:
/// - [`Added`](crate::query::Added), [`Changed`](crate::query::Changed) and [`Spawned`](crate::query::Spawned) query filters
/// - [`Local`](crate::system::Local) variables that hold state
/// - [`MessageReader`](crate::message::MessageReader) system parameters, which rely on a [`Local`](crate::system::Local) to track which messages have been seen
///
/// Note that this is automatically handled for you when using a [`World::run_system`](World::run_system).
///
/// # Example
///
/// Basic usage:
/// ```
/// # use bevy_ecs::prelude::*;
/// # use bevy_ecs::system::SystemState;
/// #
/// # #[derive(Message)]
/// # struct MyMessage;
/// # #[derive(Resource)]
/// # struct MyResource(u32);
/// #
/// # #[derive(Component)]
/// # struct MyComponent;
/// #
/// // Work directly on the `World`
/// let mut world = World::new();
/// world.init_resource::<Messages<MyMessage>>();
///
/// // Construct a `SystemState` struct, passing in a tuple of `SystemParam`
/// // as if you were writing an ordinary system.
/// let mut system_state: SystemState<(
///     MessageWriter<MyMessage>,
///     Option<ResMut<MyResource>>,
///     Query<&MyComponent>,
/// )> = SystemState::new(&mut world);
///
/// // Use system_state.get_mut(&mut world) and unpack your system parameters into variables!
/// // system_state.get(&world) provides read-only versions of your system parameters instead.
/// let (message_writer, maybe_resource, query) = system_state.get_mut(&mut world).unwrap();
///
/// // If you are using `Commands`, you can choose when you want to apply them to the world.
/// // You need to manually call `.apply(world)` on the `SystemState` to apply them.
/// ```
/// Caching:
/// ```
/// # use bevy_ecs::prelude::*;
/// # use bevy_ecs::system::SystemState;
/// # use bevy_ecs::message::Messages;
/// #
/// # #[derive(Message)]
/// # struct MyMessage;
/// #[derive(Resource)]
/// struct CachedSystemState {
///     message_state: SystemState<MessageReader<'static, 'static, MyMessage>>,
/// }
///
/// // Create and store a system state once
/// let mut world = World::new();
/// world.init_resource::<Messages<MyMessage>>();
/// let initial_state: SystemState<MessageReader<MyMessage>> = SystemState::new(&mut world);
///
/// // The system state is cached in a resource
/// world.insert_resource(CachedSystemState {
///     message_state: initial_state,
/// });
///
/// // Later, fetch the cached system state, saving on overhead
/// world.resource_scope(|world, mut cached_state: Mut<CachedSystemState>| {
///     let mut message_reader = cached_state.message_state.get_mut(world).unwrap();
///
///     for message in message_reader.read() {
///         println!("Hello World!");
///     }
/// });
/// ```
/// Exclusive System:
/// ```
/// # use bevy_ecs::prelude::*;
/// # use bevy_ecs::system::SystemState;
/// #
/// # #[derive(Message)]
/// # struct MyMessage;
/// #
/// fn exclusive_system(world: &mut World, system_state: &mut SystemState<MessageReader<MyMessage>>) {
///     let mut message_reader = system_state.get_mut(world).unwrap();
///
///     for message in message_reader.read() {
///         println!("Hello World!");
///     }
/// }
/// ```
pub struct SystemState<Param: SystemParam + 'static> {
    meta: SystemMeta,
    param_state: Param::State,
    world_id: WorldId,
}

impl<Param: SystemParam> SystemState<Param> {
    /// Creates a new [`SystemState`] with default state.
    #[track_caller]
    pub fn new(world: &mut World) -> Self {
        let mut meta = SystemMeta::new::<Param>();
        meta.last_run = world.change_tick().relative_to(Tick::MAX);
        let param_state = Param::init_state(world);
        let mut component_access_set = FilteredAccessSet::new();

        Param::init_access(&param_state, &mut meta, &mut component_access_set, world);
        Self {
            meta,
            param_state,
            world_id: world.id(),
        }
    }

    /// Create a [`SystemState`] from a [`SystemParamBuilder`]
    pub(crate) fn from_builder(world: &mut World, builder: impl SystemParamBuilder<Param>) -> Self {
        let mut meta = SystemMeta::new::<Param>();
        meta.last_run = world.change_tick().relative_to(Tick::MAX);
        let param_state = builder.build(world);
        let mut component_access_set = FilteredAccessSet::new();
        // We need to call `init_access` to ensure there are no panics from conflicts within `Param`,
        // even though we don't use the calculated access.
        Param::init_access(&param_state, &mut meta, &mut component_access_set, world);
        Self {
            meta,
            param_state,
            world_id: world.id(),
        }
    }

    /// Create a [`FunctionSystem`] from a [`SystemState`].
    /// This method signature allows any system function, but the compiler will not perform type inference on closure parameters.
    /// You can use [`SystemState::build_system()`] or [`SystemState::build_system_with_input()`] to get type inference on parameters.
    #[inline]
    pub fn build_any_system<Marker, In, Out, F>(self, func: F) -> FunctionSystem<Marker, In, Out, F>
    where
        In: SystemInput,
        F: SystemParamFunction<Marker, In: FromInput<In>, Out: IntoResult<Out>, Param = Param>,
    {
        FunctionSystem::new(
            func,
            self.meta,
            Some(FunctionSystemState {
                param: self.param_state,
                world_id: self.world_id,
            }),
        )
    }

    /// Gets the metadata for this instance.
    #[inline]
    pub fn meta(&self) -> &SystemMeta {
        &self.meta
    }

    /// Gets the metadata for this instance.
    #[inline]
    pub fn meta_mut(&mut self) -> &mut SystemMeta {
        &mut self.meta
    }

    /// Retrieve the [`SystemParam`] values. This can only be called when all parameters are read-only.
    ///
    /// Returns an error if system parameter validation fails.
    #[inline]
    pub fn get<'w, 's>(
        &'s mut self,
        world: &'w World,
    ) -> Result<SystemParamItem<'w, 's, Param>, SystemParamValidationError>
    where
        Param: ReadOnlySystemParam,
    {
        self.validate_world(world.id());
        // SAFETY: Param is read-only and doesn't allow mutable access to World.
        // It also matches the World this SystemState was created with.
        unsafe { self.get_unchecked(world.as_unsafe_world_cell_readonly()) }
    }

    /// Retrieve the mutable [`SystemParam`] values.
    ///
    /// Returns an error if system parameter validation fails.
    #[inline]
    #[track_caller]
    pub fn get_mut<'w, 's>(
        &'s mut self,
        world: &'w mut World,
    ) -> Result<SystemParamItem<'w, 's, Param>, SystemParamValidationError> {
        self.validate_world(world.id());
        // SAFETY: World is uniquely borrowed and matches the World this SystemState was created with.
        unsafe { self.get_unchecked(world.as_unsafe_world_cell()) }
    }

    /// Applies all state queued up for [`SystemParam`] values. For example, this will apply commands queued up
    /// by a [`Commands`](`super::Commands`) parameter to the given [`World`].
    /// This function should be called manually after the values returned by [`SystemState::get`] and [`SystemState::get_mut`]
    /// are finished being used.
    pub fn apply(&mut self, world: &mut World) {
        Param::apply(&mut self.param_state, &self.meta, world);
    }

    /// Returns `true` if `world_id` matches the [`World`] that was used to call [`SystemState::new`].
    /// Otherwise, this returns false.
    #[inline]
    pub fn matches_world(&self, world_id: WorldId) -> bool {
        self.world_id == world_id
    }

    /// Asserts that the [`SystemState`] matches the provided world.
    #[inline]
    #[track_caller]
    fn validate_world(&self, world_id: WorldId) {
        #[inline(never)]
        #[track_caller]
        #[cold]
        fn painc_mismatched(this: WorldId, other: WorldId) -> ! {
            panic!(
                "Encountered a mismatched World. This SystemState was created from {this:?}, but a method was called using {other:?}."
            );
        }

        if !self.matches_world(world_id) {
            painc_mismatched(self.world_id, world_id)
        }
    }

    /// Retrieve the [`SystemParam`] values.
    ///
    /// Returns an error if system parameter validation fails.
    ///
    /// # Safety
    /// This call might access any of the input parameters in a way that violates Rust's mutability rules. Make sure the data
    /// access is safe in the context of global [`World`] access. The passed-in [`World`] _must_ be the [`World`] the [`SystemState`] was
    /// created with.
    #[inline]
    #[track_caller]
    pub unsafe fn get_unchecked<'w, 's>(
        &'s mut self,
        world: UnsafeWorldCell<'w>,
    ) -> Result<SystemParamItem<'w, 's, Param>, SystemParamValidationError> {
        let change_tick = world.increment_change_tick();
        // SAFETY: The invariants are upheld by the caller.
        unsafe { self.fetch(world, change_tick) }
    }

    /// # Safety
    /// This call might access any of the input parameters in a way that violates Rust's mutability rules. Make sure the data
    /// access is safe in the context of global [`World`] access. The passed-in [`World`] _must_ be the [`World`] the [`SystemState`] was
    /// created with.
    #[inline]
    #[track_caller]
    unsafe fn fetch<'w, 's>(
        &'s mut self,
        world: UnsafeWorldCell<'w>,
        change_tick: Tick,
    ) -> Result<SystemParamItem<'w, 's, Param>, SystemParamValidationError> {
        let param =
            unsafe { Param::get_param(&mut self.param_state, &self.meta, world, change_tick) }?;
        self.meta.last_run = change_tick;
        Ok(param)
    }

    /// Returns a reference to the current system param states.
    pub fn param_state(&self) -> &Param::State {
        &self.param_state
    }

    /// Returns a mutable reference to the current system param states.
    /// Marked as unsafe because modifying the system states may result in violation to certain
    /// assumptions made by the [`SystemParam`]. Use with care.
    ///
    /// # Safety
    /// Modifying the system param states may have unintended consequences.
    /// The param state is generally considered to be owned by the [`SystemParam`]. Modifications
    /// should respect any invariants as required by the [`SystemParam`].
    /// For example, modifying the system state of [`ResMut`](crate::system::ResMut) will obviously create issues.
    pub unsafe fn param_state_mut(&mut self) -> &mut Param::State {
        &mut self.param_state
    }
}

impl<Param: SystemParam> FromWorld for SystemState<Param> {
    fn from_world(world: &mut World) -> Self {
        Self::new(world)
    }
}

/// The [`System`] counter part of an ordinary function.
///
/// You get this by calling [`IntoSystem::into_system`]  on a function that only accepts
/// [`SystemParam`]s. The output of the system becomes the functions return type, while the input
/// becomes the functions first parameter or `()` if no such parameter exists.
///
/// [`FunctionSystem`] must be `.initialized` before they can be run.
///
/// The [`Clone`] implementation for [`FunctionSystem`] returns a new instance which
/// is NOT initialized. The cloned system must also be `.initialized` before it can be run.
pub struct FunctionSystem<Marker, In, Out, F>
where
    F: SystemParamFunction<Marker>,
{
    func: F,
    // #[cfg(feature = "hotpatching")]
    // current_ptr: subsecond::HotFnPtr,
    state: Option<FunctionSystemState<F::Param>>,
    system_meta: SystemMeta,
    // NOTE: PhantomData<fn()-> T> gives this safe Send/Sync impls
    marker: PhantomData<fn(In) -> (Marker, Out)>,
}

/// The state of a [`FunctionSystem`], which must be initialized with
/// [`System::initialize`] before the system can be run. A panic will occur if
/// the system is run without being initialized.
struct FunctionSystemState<P: SystemParam> {
    /// The cached state of the system's [`SystemParam`]s.
    param: P::State,
    /// The id of the [`World`] this system was initialized with. If the world
    /// passed to [`System::run_unsafe`] does not match
    /// this id, a panic will occur.
    world_id: WorldId,
}

impl<Marker, In, Out, F> FunctionSystem<Marker, In, Out, F>
where
    F: SystemParamFunction<Marker>,
{
    #[inline]
    fn new(func: F, system_meta: SystemMeta, state: Option<FunctionSystemState<F::Param>>) -> Self {
        Self {
            func,
            #[cfg(feature = "hotpatching")]
            current_ptr: subsecond::HotFn::current(<F as SystemParamFunction<Marker>>::run)
                .ptr_address(),
            state,
            system_meta,
            marker: PhantomData,
        }
    }

    /// Return this system with a new name.
    ///
    /// Useful to give closure systems more readable and unique names for debugging and tracing.
    pub fn with_name(mut self, new_name: impl Into<Cow<'static, str>>) -> Self {
        self.system_meta.set_name(new_name.into());
        self
    }
}

// Allow closure arguments to be inferred.
// For a closure to be used as a `SystemParamFunction`, it needs to be generic in any `'w` or `'s` lifetimes.
// Rust will only infer a closure to be generic over lifetimes if it's passed to a function with a Fn constraint.
// So, generate a function for each arity with an explicit `FnMut` constraint to enable higher-order lifetimes,
// along with a regular `SystemParamFunction` constraint to allow the system to be built.
macro_rules! impl_build_system {
    ($(#[$meta:meta])* $($param: ident),*) => {
        $(#[$meta])*
        impl<$($param: SystemParam),*> SystemState<($($param,)*)> {
            /// Create a [`FunctionSystem`] from a [`SystemState`].
            /// This method signature allows type inference of closure parameters for a system with no input.
            /// You can use [`SystemState::build_system_with_input()`] if you have input, or [`SystemState::build_any_system()`] if you don't need type inference.
            #[inline]
            pub fn build_system<
                InnerOut: IntoResult<Out>,
                Out,
                Marker,
                F: FnMut($(SystemParamItem<$param>), *) -> InnerOut
                    + SystemParamFunction<Marker, In = (), Out = InnerOut, Param = ($($param,)*)>
            >
            (
                self,
                func: F,
            ) -> FunctionSystem<Marker, (), Out, F>
            {
                self.build_any_system(func)
            }

            pub fn build_system_with_input<
                InnerIn: SystemInput + FromInput<In>,
                In: SystemInput,
                InnerOut: IntoResult<Out>,
                Out,
                Marker,
                F: FnMut(InnerIn, $(SystemParamItem<$param>),*) -> InnerOut
                    + SystemParamFunction<Marker, In = InnerIn, Out = InnerOut, Param = ($($param,)*)>
            >
            (
                self,
                func: F
            ) -> FunctionSystem<Marker, In, Out, F> {
                self.build_any_system(func)
            }
        }
    };
}

impl_build_system!();
#[cfg_attr(any(docsrs, docsrs_dep), doc(fake_variadic))]
#[cfg_attr(
    any(docsrs, docsrs_dep),
    doc = "This trait is implemented for tuples up to 16 items long."
)]
impl<P: SystemParam> SystemState<(P,)> {
    #[doc = r" Create a [`FunctionSystem`] from a [`SystemState`]."]
    #[doc = r" This method signature allows type inference of closure parameters for a system with no input."]
    #[doc = r" You can use [`SystemState::build_system_with_input()`] if you have input, or [`SystemState::build_any_system()`] if you don't need type inference."]
    #[inline]
    pub fn build_system<
        InnerOut: IntoResult<Out>,
        Out,
        Marker,
        F: FnMut(SystemParamItem<P>) -> InnerOut
            + SystemParamFunction<Marker, In = (), Out = InnerOut, Param = (P,)>,
    >(
        self,
        func: F,
    ) -> FunctionSystem<Marker, (), Out, F> {
        self.build_any_system(func)
    }
    pub fn build_system_with_input<
        InnerIn: SystemInput + FromInput<In>,
        In: SystemInput,
        InnerOut: IntoResult<Out>,
        Out,
        Marker,
        F: FnMut(InnerIn, SystemParamItem<P>) -> InnerOut
            + SystemParamFunction<Marker, In = InnerIn, Out = InnerOut, Param = (P,)>,
    >(
        self,
        func: F,
    ) -> FunctionSystem<Marker, In, Out, F> {
        self.build_any_system(func)
    }
}
#[cfg_attr(any(docsrs, docsrs_dep), doc(hidden))]
impl<P0: SystemParam, P1: SystemParam> SystemState<(P0, P1)> {
    #[doc = r" Create a [`FunctionSystem`] from a [`SystemState`]."]
    #[doc = r" This method signature allows type inference of closure parameters for a system with no input."]
    #[doc = r" You can use [`SystemState::build_system_with_input()`] if you have input, or [`SystemState::build_any_system()`] if you don't need type inference."]
    #[inline]
    pub fn build_system<
        InnerOut: IntoResult<Out>,
        Out,
        Marker,
        F: FnMut(SystemParamItem<P0>, SystemParamItem<P1>) -> InnerOut
            + SystemParamFunction<Marker, In = (), Out = InnerOut, Param = (P0, P1)>,
    >(
        self,
        func: F,
    ) -> FunctionSystem<Marker, (), Out, F> {
        self.build_any_system(func)
    }
    pub fn build_system_with_input<
        InnerIn: SystemInput + FromInput<In>,
        In: SystemInput,
        InnerOut: IntoResult<Out>,
        Out,
        Marker,
        F: FnMut(InnerIn, SystemParamItem<P0>, SystemParamItem<P1>) -> InnerOut
            + SystemParamFunction<Marker, In = InnerIn, Out = InnerOut, Param = (P0, P1)>,
    >(
        self,
        func: F,
    ) -> FunctionSystem<Marker, In, Out, F> {
        self.build_any_system(func)
    }
}
impl_build_system!(
    #[cfg_attr(any(docsrs, docsrs_dep), doc(hidden))]
    P0,
    P1,
    P2
);
impl_build_system!(
    #[cfg_attr(any(docsrs, docsrs_dep), doc(hidden))]
    P0,
    P1,
    P2,
    P3
);
impl_build_system!(
    #[cfg_attr(any(docsrs, docsrs_dep), doc(hidden))]
    P0,
    P1,
    P2,
    P3,
    P4
);
impl_build_system!(
    #[cfg_attr(any(docsrs, docsrs_dep), doc(hidden))]
    P0,
    P1,
    P2,
    P3,
    P4,
    P5
);
impl_build_system!(
    #[cfg_attr(any(docsrs, docsrs_dep), doc(hidden))]
    P0,
    P1,
    P2,
    P3,
    P4,
    P5,
    P6
);
impl_build_system!(
    #[cfg_attr(any(docsrs, docsrs_dep), doc(hidden))]
    P0,
    P1,
    P2,
    P3,
    P4,
    P5,
    P6,
    P7
);
impl_build_system!(
    #[cfg_attr(any(docsrs, docsrs_dep), doc(hidden))]
    P0,
    P1,
    P2,
    P3,
    P4,
    P5,
    P6,
    P7,
    P8
);
impl_build_system!(
    #[cfg_attr(any(docsrs, docsrs_dep), doc(hidden))]
    P0,
    P1,
    P2,
    P3,
    P4,
    P5,
    P6,
    P7,
    P8,
    P9
);
impl_build_system!(
    #[cfg_attr(any(docsrs, docsrs_dep), doc(hidden))]
    P0,
    P1,
    P2,
    P3,
    P4,
    P5,
    P6,
    P7,
    P8,
    P9,
    P10
);
impl_build_system!(
    #[cfg_attr(any(docsrs, docsrs_dep), doc(hidden))]
    P0,
    P1,
    P2,
    P3,
    P4,
    P5,
    P6,
    P7,
    P8,
    P9,
    P10,
    P11
);
impl_build_system!(
    #[cfg_attr(any(docsrs, docsrs_dep), doc(hidden))]
    P0,
    P1,
    P2,
    P3,
    P4,
    P5,
    P6,
    P7,
    P8,
    P9,
    P10,
    P11,
    P12
);
impl_build_system!(
    #[cfg_attr(any(docsrs, docsrs_dep), doc(hidden))]
    P0,
    P1,
    P2,
    P3,
    P4,
    P5,
    P6,
    P7,
    P8,
    P9,
    P10,
    P11,
    P12,
    P13
);
impl_build_system!(
    #[cfg_attr(any(docsrs, docsrs_dep), doc(hidden))]
    P0,
    P1,
    P2,
    P3,
    P4,
    P5,
    P6,
    P7,
    P8,
    P9,
    P10,
    P11,
    P12,
    P13,
    P14
);
impl_build_system!(
    #[cfg_attr(any(docsrs, docsrs_dep), doc(hidden))]
    P0,
    P1,
    P2,
    P3,
    P4,
    P5,
    P6,
    P7,
    P8,
    P9,
    P10,
    P11,
    P12,
    P13,
    P14,
    P15
);

/// A trait implemented for all functions that can be used as [`System`]s.
///
/// This trait can be useful for making your own systems which accept other systems,
/// sometimes called higher order systems.
///
/// This should be used in combination with [`ParamSet`] when calling other systems
/// within your system.
/// Using [`ParamSet`] in this case avoids [`SystemParam`] collisions.
///
/// # Example
///
/// To create something like [`PipeSystem`], but in entirely safe code.
///
/// ```
/// use std::num::ParseIntError;
///
/// use bevy_ecs::prelude::*;
/// use bevy_ecs::system::StaticSystemInput;
///
/// /// Pipe creates a new system which calls `a`, then calls `b` with the output of `a`
/// pub fn pipe<A, B, AMarker, BMarker>(
///     mut a: A,
///     mut b: B,
/// ) -> impl FnMut(StaticSystemInput<A::In>, ParamSet<(A::Param, B::Param)>) -> B::Out
/// where
///     // We need A and B to be systems, add those bounds
///     A: SystemParamFunction<AMarker>,
///     B: SystemParamFunction<BMarker>,
///     for<'a> B::In: SystemInput<Inner<'a> = A::Out>,
/// {
///     // The type of `params` is inferred based on the return of this function above
///     move |StaticSystemInput(a_in), mut params| {
///         let shared = a.run(a_in, params.p0());
///         b.run(shared, params.p1())
///     }
/// }
///
/// // Usage example for `pipe`:
/// fn main() {
///     let mut world = World::default();
///     world.insert_resource(Message("42".to_string()));
///
///     // pipe the `parse_message_system`'s output into the `filter_system`s input.
///     // Type annotations should only needed when using `StaticSystemInput` as input
///     // AND the input type isn't constrained by nearby code.
///     let mut piped_system = IntoSystem::<(), Option<usize>, _>::into_system(pipe(parse_message, filter));
///     piped_system.initialize(&mut world);
///     assert_eq!(piped_system.run((), &mut world).unwrap(), Some(42));
/// }
///
/// #[derive(Resource)]
/// struct Message(String);
///
/// fn parse_message(message: Res<Message>) -> Result<usize, ParseIntError> {
///     message.0.parse::<usize>()
/// }
///
/// fn filter(In(result): In<Result<usize, ParseIntError>>) -> Option<usize> {
///     result.ok().filter(|&n| n < 100)
/// }
/// ```
/// [`PipeSystem`]: crate::system::PipeSystem
/// [`ParamSet`]: crate::system::ParamSet
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a valid system",
    label = "invalid system"
)]
pub trait SystemParamFunction<Marker>: Send + Sync + 'static {
    /// The input type of this system. See [`System::In`].
    type In: SystemInput;
    /// The return type of this system. See [`System::Out`].
    type Out;

    /// The [`SystemParam`]/s used by this system to access the [`World`].
    type Param: SystemParam;

    /// Executes this system once. See [`System::run`] or [`System::run_unsafe`].
    fn run(
        &mut self,
        input: <Self::In as SystemInput>::Inner<'_>,
        param_value: SystemParamItem<Self::Param>,
    ) -> Self::Out;
}

/// A type that may be converted to the output of a [`System`].
/// This is used to allow systems to return either a plain value or a [`Result`].
pub trait IntoResult<Out>: Sized {
    fn into_result(self) -> Result<Out, RunSystemError>;
}

impl<T> IntoResult<T> for T {
    fn into_result(self) -> Result<T, RunSystemError> {
        Ok(self)
    }
}

impl<T> IntoResult<T> for Result<T, RunSystemError> {
    fn into_result(self) -> Result<T, RunSystemError> {
        self
    }
}

impl<T> IntoResult<T> for Result<T, BevyError> {
    fn into_result(self) -> Result<T, RunSystemError> {
        Ok(self?)
    }
}

// The `!` impl can't be generic in `Out`, since that would overlap with
// `impl<T> IntoResult<T> for T` when `T` = `!`.
// Use explicit impls for `()` and `bool` so diverging functions
// can be used for systems and conditions.
impl IntoResult<()> for Never {
    fn into_result(self) -> Result<(), RunSystemError> {
        self
    }
}

impl IntoResult<bool> for Never {
    fn into_result(self) -> Result<bool, RunSystemError> {
        self
    }
}
