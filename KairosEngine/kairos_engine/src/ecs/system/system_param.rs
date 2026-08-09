use std::{borrow::Cow, fmt::Display};

use thiserror::Error;
use variadics_please::all_tuples_enumerated;

use crate::{
    debug::DebugName,
    ecs::{
        change_detection::{ComponentTicksRef, Res, Tick},
        component::ComponentId,
        query::{FilteredAccess, FilteredAccessSet},
        resource::{IS_RESOURCE, Resource},
        system::SystemMeta,
        world::{DeferredWorld, World, unsafe_world_cell::UnsafeWorldCell},
    },
    ptr::UnsafeCellDeref,
};

/// A parameter that can be used in a [`System`](super::System).
///
/// # Derive
///
/// This trait can be derived with the [`derive@super::SystemParam`] macro.
/// This macro only works if each field on the derived struct implements [`SystemParam`].
/// Note: There are additional requirements on the field types.
/// See the *Generic `SystemParam`s* section for details and workarounds of the probable
/// cause if this derive causes an error to be emitted.
///
/// Derived `SystemParam` structs may have two lifetimes: `'w` for data stored in the [`World`],
/// and `'s` for data stored in the parameter's state.
///
/// The following list shows the most common [`SystemParam`]s and which lifetime they require
///
/// ```
/// # use bevy_ecs::prelude::*;
/// # #[derive(Component)]
/// # struct SomeComponent;
/// # #[derive(Resource)]
/// # struct SomeResource;
/// # #[derive(Message)]
/// # struct SomeMessage;
/// # #[derive(Resource)]
/// # struct SomeOtherResource;
/// # use bevy_ecs::system::SystemParam;
/// # #[derive(SystemParam)]
/// # struct ParamsExample<'w, 's> {
/// #    query:
/// Query<'w, 's, Entity>,
/// #    query2:
/// Query<'w, 's, &'static SomeComponent>,
/// #    res:
/// Res<'w, SomeResource>,
/// #    res_mut:
/// ResMut<'w, SomeOtherResource>,
/// #    local:
/// Local<'s, u8>,
/// #    commands:
/// Commands<'w, 's>,
/// #    message_reader:
/// MessageReader<'w, 's, SomeMessage>,
/// #    message_writer:
/// MessageWriter<'w, SomeMessage>
/// # }
/// ```
/// ## `PhantomData`
///
/// [`PhantomData`] is a special type of `SystemParam` that does nothing.
/// This is useful for constraining generic types or lifetimes.
///
/// # Example
///
/// ```
/// # use bevy_ecs::prelude::*;
/// # #[derive(Resource)]
/// # struct SomeResource;
/// use std::marker::PhantomData;
/// use bevy_ecs::system::SystemParam;
///
/// #[derive(SystemParam)]
/// struct MyParam<'w, Marker: 'static> {
///     foo: Res<'w, SomeResource>,
///     marker: PhantomData<Marker>,
/// }
///
/// fn my_system<T: 'static>(param: MyParam<T>) {
///     // Access the resource through `param.foo`
/// }
///
/// # bevy_ecs::system::assert_is_system(my_system::<()>);
/// ```
///
/// # Generic `SystemParam`s
///
/// When using the derive macro, you may see an error in the form of:
///
/// ```text
/// expected ... [ParamType]
/// found associated type `<[ParamType] as SystemParam>::Item<'_, '_>`
/// ```
/// where `[ParamType]` is the type of one of your fields.
/// To solve this error, you can wrap the field of type `[ParamType]` with [`StaticSystemParam`]
/// (i.e. `StaticSystemParam<[ParamType]>`).
///
/// ## Details
///
/// The derive macro requires that the [`SystemParam`] implementation of
/// each field `F`'s [`Item`](`SystemParam::Item`)'s is itself `F`
/// (ignoring lifetimes for simplicity).
/// This assumption is due to type inference reasons, so that the derived [`SystemParam`] can be
/// used as an argument to a function system.
/// If the compiler cannot validate this property for `[ParamType]`, it will error in the form shown above.
///
/// This will most commonly occur when working with `SystemParam`s generically, as the requirement
/// has not been proven to the compiler.
///
/// ## Custom Validation Messages
///
/// When using the derive macro, any [`SystemParamValidationError`]s will be propagated from the sub-parameters.
/// If you want to override the error message, add a `#[system_param(validation_message = "New message")]` attribute to the parameter.
///
/// ```
/// # use bevy_ecs::prelude::*;
/// # #[derive(Resource)]
/// # struct SomeResource;
/// # use bevy_ecs::system::SystemParam;
/// #
/// #[derive(SystemParam)]
/// struct MyParam<'w> {
///     #[system_param(validation_message = "Custom Message")]
///     foo: Res<'w, SomeResource>,
/// }
///
/// let mut world = World::new();
/// let err = world.run_system_cached(|param: MyParam| {}).unwrap_err();
/// let expected = "Parameter `MyParam::foo` failed validation: Custom Message";
/// # #[cfg(feature="Trace")] // Without debug_utils/debug enabled MyParam::foo is stripped and breaks the assert
/// assert!(err.to_string().contains(expected));
/// ```
///
/// ## Builders
///
/// If you want to use a [`SystemParamBuilder`](crate::system::SystemParamBuilder) with a derived [`SystemParam`] implementation,
/// add a `#[system_param(builder)]` attribute to the struct.
/// This will generate a builder struct whose name is the param struct suffixed with `Builder`.
/// The builder will not be `pub`, so you may want to expose a method that returns an `impl SystemParamBuilder<T>`.
///
/// ```
/// mod custom_param {
/// #     use bevy_ecs::{
/// #         prelude::*,
/// #         system::{LocalBuilder, QueryParamBuilder, SystemParam},
/// #     };
/// #
///     #[derive(SystemParam)]
///     #[system_param(builder)]
///     pub struct CustomParam<'w, 's> {
///         query: Query<'w, 's, ()>,
///         local: Local<'s, usize>,
///     }
///
///     impl<'w, 's> CustomParam<'w, 's> {
///         pub fn builder(
///             local: usize,
///             query: impl FnOnce(&mut QueryBuilder<()>),
///         ) -> impl SystemParamBuilder<Self> {
///             CustomParamBuilder {
///                 local: LocalBuilder(local),
///                 query: QueryParamBuilder::new(query),
///             }
///         }
///     }
/// }
///
/// use custom_param::CustomParam;
///
/// # use bevy_ecs::prelude::*;
/// # #[derive(Component)]
/// # struct A;
/// #
/// # let mut world = World::new();
/// #
/// let system = (CustomParam::builder(100, |builder| {
///     builder.with::<A>();
/// }),)
///     .build_state(&mut world)
///     .build_system(|param: CustomParam| {});
/// ```
///
/// # Safety
///
/// The implementor must ensure the following is true.
/// - [`SystemParam::init_access`] correctly registers all [`World`] accesses used
///   by [`SystemParam::get_param`] with the provided [`system_meta`](SystemMeta).
/// - None of the world accesses may conflict with any prior accesses registered
///   on `system_meta`.
pub unsafe trait SystemParam: Sized {
    /// Used to store data which persists across invocations of a system.
    type State: Send + Sync + 'static;

    /// The item type returned when constructing this system param.
    /// The value of this associated type should be `Self`, instantiated with new lifetimes.
    ///
    /// You could think of [`SystemParam::Item<'w, 's>`] as being an *operation* that changes the lifetimes bound to `Self`.
    type Item<'world, 'state>: SystemParam<State = Self::State>;

    /// Creates a new instance of this param's [`State`](SystemParam::State).
    fn init_state(world: &mut World) -> Self::State;

    /// Registers any [`World`] access used by this [`SystemParam`].
    ///
    /// This method must panic if the access would conflict with any existing access in the [`FilteredAccessSet`].
    fn init_access(
        state: &Self::State,
        system_meta: &mut SystemMeta,
        component_access_set: &mut FilteredAccessSet,
        world: &mut World,
    );

    /// Applies any deferred mutations stored in this [`SystemParam`]'s state.
    /// This is used to apply [`Commands`] during [`ApplyDeferred`](crate::prelude::ApplyDeferred).
    ///
    /// [`Commands`]: crate::prelude::Commands
    #[inline]
    #[expect(
        unused_variables,
        reason = "The parameters here are intentionally unused by the default implementation; however, putting underscores here will result in the underscores being copied by rust-analyzer's tab completion."
    )]
    fn apply(state: &mut Self::State, system_meta: &SystemMeta, world: &mut World) {}

    /// Queues any deferred mutations to be applied at the next [`ApplyDeferred`](crate::prelude::ApplyDeferred).
    #[inline]
    #[expect(
        unused_variables,
        reason = "The parameters here are intentionally unused by the default implementation; however, putting underscores here will result in the underscores being copied by rust-analyzer's tab completion."
    )]
    fn queue(state: &mut Self::State, system_meta: &SystemMeta, world: DeferredWorld) {}

    /// Creates a parameter to be passed into a [`SystemParamFunction`](super::SystemParamFunction).
    ///
    /// This method also validates that the param can be acquired. If validation fails,
    /// an appropriate [`SystemParamValidationError`] should be returned.
    /// Systems will convert this to a [`RunSystemError`](super::RunSystemError),
    /// and the built-in executors will ignore any "skipped" validation results,
    /// but pass any "invalid" results to the fallback error handler defined in [`bevy_ecs::error`].
    ///
    /// For nested [`SystemParam`]s validation will fail if any
    /// delegated validation fails.
    ///
    /// # Safety
    ///
    /// - The passed [`UnsafeWorldCell`] must have access to any world data registered
    ///   in [`init_access`](SystemParam::init_access).
    /// - [`SystemParam::init_access`] must not request conflicting access.
    ///   If `Self` is `ReadOnlySystemParam`, the access is read-only and can never conflict.
    ///   Otherwise, [`SystemParam::init_access`] must be called to ensure it does not panic.
    /// - `world` must be the same [`World`] that was used to initialize [`state`](SystemParam::init_state).
    unsafe fn get_param<'world, 'state>(
        state: &'state mut Self::State,
        system_meta: &SystemMeta,
        world: UnsafeWorldCell<'world>,
        change_tick: Tick,
    ) -> Result<Self::Item<'world, 'state>, SystemParamValidationError>;
}

/// A [`SystemParam`] that only reads a given [`World`].
///
/// # Safety
/// This must only be implemented for [`SystemParam`] impls that exclusively read the World passed in to [`SystemParam::get_param`]
pub unsafe trait ReadOnlySystemParam: SystemParam {}

/// Shorthand way of accessing the associated type [`SystemParam::Item`] for a given [`SystemParam`].
pub type SystemParamItem<'w, 's, P> = <P as SystemParam>::Item<'w, 's>;

/// An error that occurs when a system parameter is not valid,
/// used by system executors to determine what to do with a system.
///
/// Returned as an error from [`SystemParam::get_param`],
/// and handled using the unified error handling mechanisms defined in [`bevy_ecs::error`].
#[derive(Debug, PartialEq, Eq, Clone, Error)]
pub struct SystemParamValidationError {
    /// Whether the system should be skipped.
    ///
    /// If `false`, the error should be handled.
    /// By default, this will result in a panic. See [`error`](`crate::error`) for more information.
    ///
    /// This is the default behavior, and is suitable for system params that should *always* be valid,
    /// either because sensible fallback behavior exists (like [`Query`]) or because
    /// failures in validation should be considered a bug in the user's logic that must be immediately addressed (like [`Res`]).
    ///
    /// If `true`, the system should be skipped.
    /// This is set by wrapping the system param in [`If`],
    /// and indicates that the system is intended to only operate in certain application states.
    pub skipped: bool,

    /// A message describing the validation error.
    pub message: Cow<'static, str>,

    /// A string identifying the invalid parameter.
    /// This is usually the type name of the parameter.
    pub param: DebugName,

    /// A string identifying the field within a parameter using `#[derive(SystemParam)]`.
    /// This will be an empty string for other parameters.
    ///
    /// This will be printed after `param` in the `Display` impl, and should include a `::` prefix if non-empty.
    pub field: Cow<'static, str>,
}

impl SystemParamValidationError {
    /// Constructs a `SystemParamValidationError` that skips the system.
    /// The parameter name is initialized to the type name of `T`, so a `SystemParam` should usually pass `Self`.
    pub fn skipped<T>(message: impl Into<Cow<'static, str>>) -> Self {
        Self::new::<T>(true, message, Cow::Borrowed(""))
    }

    /// Constructs a `SystemParamValidationError` for an invalid parameter that should be treated as an error.
    /// The parameter name is initialized to the type name of `T`, so a `SystemParam` should usually pass `Self`.
    pub fn invalid<T>(message: impl Into<Cow<'static, str>>) -> Self {
        Self::new::<T>(false, message, Cow::Borrowed(""))
    }

    /// Constructs a `SystemParamValidationError` for an invalid parameter.
    /// The parameter name is initialized to the type name of `T`, so a `SystemParam` should usually pass `Self`.
    pub fn new<T>(
        skipped: bool,
        message: impl Into<Cow<'static, str>>,
        field: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self {
            skipped,
            message: message.into(),
            param: DebugName::type_name::<T>(),
            field: field.into(),
        }
    }

    pub(crate) const EMPTY: Self = Self {
        skipped: false,
        message: Cow::Borrowed(""),
        param: DebugName::borrowed(""),
        field: Cow::Borrowed(""),
    };
}

impl Display for SystemParamValidationError {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
        write!(
            fmt,
            "Parameter `{}{}` failed validation: {}",
            self.param.shortname(),
            self.field,
            self.message
        )?;
        if !self.skipped {
            write!(
                fmt,
                "\nIf this is an expected state, wrap the parameter in `Option<T>` and handle `None` when it happens, or wrap the parameter in `If<T>` to skip the system when it happens."
            )?;
        }
        Ok(())
    }
}

/// A collection of potentially conflicting [`SystemParam`]s allowed by disjoint access.
///
/// Allows systems to safely access and interact with up to 8 mutually exclusive [`SystemParam`]s, such as
/// two queries that reference the same mutable data or an event reader and writer of the same type.
///
/// Each individual [`SystemParam`] can be accessed by using the functions `p0()`, `p1()`, ..., `p7()`,
/// according to the order they are defined in the `ParamSet`. This ensures that there's either
/// only one mutable reference to a parameter at a time or any number of immutable references.
///
/// # Examples
///
/// The following system mutably accesses the same component two times,
/// which is not allowed due to rust's mutability rules.
///
/// ```should_panic
/// # use bevy_ecs::prelude::*;
/// #
/// # #[derive(Component)]
/// # struct Health;
/// #
/// # #[derive(Component)]
/// # struct Enemy;
/// #
/// # #[derive(Component)]
/// # struct Ally;
/// #
/// // This will panic at runtime when the system gets initialized.
/// fn bad_system(
///     mut enemies: Query<&mut Health, With<Enemy>>,
///     mut allies: Query<&mut Health, With<Ally>>,
/// ) {
///     // ...
/// }
/// #
/// # let mut bad_system_system = IntoSystem::into_system(bad_system);
/// # let mut world = World::new();
/// # bad_system_system.initialize(&mut world);
/// # bad_system_system.run((), &mut world);
/// ```
///
/// Conflicting `SystemParam`s like these can be placed in a `ParamSet`,
/// which leverages the borrow checker to ensure that only one of the contained parameters are accessed at a given time.
///
/// ```
/// # use bevy_ecs::prelude::*;
/// #
/// # #[derive(Component)]
/// # struct Health;
/// #
/// # #[derive(Component)]
/// # struct Enemy;
/// #
/// # #[derive(Component)]
/// # struct Ally;
/// #
/// // Given the following system
/// fn fancy_system(
///     mut set: ParamSet<(
///         Query<&mut Health, With<Enemy>>,
///         Query<&mut Health, With<Ally>>,
///     )>
/// ) {
///     // This will access the first `SystemParam`.
///     for mut health in set.p0().iter_mut() {
///         // Do your fancy stuff here...
///     }
///
///     // The second `SystemParam`.
///     // This would fail to compile if the previous parameter was still borrowed.
///     for mut health in set.p1().iter_mut() {
///         // Do even fancier stuff here...
///     }
/// }
/// # bevy_ecs::system::assert_is_system(fancy_system);
/// ```
///
/// Of course, `ParamSet`s can be used with any kind of `SystemParam`, not just [queries](Query).
///
/// ```
/// # use bevy_ecs::prelude::*;
/// #
/// # #[derive(Message)]
/// # struct MyMessage;
/// # impl MyMessage {
/// #   pub fn new() -> Self { Self }
/// # }
/// fn message_system(
///     mut set: ParamSet<(
///         // PROBLEM: `MessageReader` and `MessageWriter` cannot be used together normally,
///         // because they both need access to the same message queue.
///         // SOLUTION: `ParamSet` allows these conflicting parameters to be used safely
///         // by ensuring only one is accessed at a time.
///         // Note that a better solution here is to use `MessageMutator`,
///         // which both reads and writes messages with a single parameter.
///         MessageReader<MyMessage>,
///         MessageWriter<MyMessage>,
///         // PROBLEM: `&World` needs read access to everything, which conflicts with
///         // any mutable access in the same system.
///         // SOLUTION: `ParamSet` ensures `&World` is only accessed when we're not
///         // using the other mutable parameters.
///         &World,
///     )>,
/// ) {
///     for message in set.p0().read() {
///         // ...
///         # let _message = message;
///     }
///     set.p1().write(MyMessage::new());
///
///     let entities = set.p2().entities();
///     // ...
///     # let _entities = entities;
/// }
/// # bevy_ecs::system::assert_is_system(message_system);
/// ```
pub struct ParamSet<'w, 's, T: SystemParam> {
    param_states: &'s mut T::State,
    world: UnsafeWorldCell<'w>,
    system_meta: SystemMeta,
    change_tick: Tick,
}

macro_rules! impl_param_set {
    ($(($index: tt, $param: ident, $fn_name: ident)),*) => {
        // SAFETY: All parameters are constrained to ReadOnlySystemParam, so World is only read
        unsafe impl<'w, 's, $($param,)*> ReadOnlySystemParam for ParamSet<'w, 's, ($($param,)*)>
        where $($param: ReadOnlySystemParam,)*
        {}

        // SAFETY: Relevant parameter ComponentId access is applied to SystemMeta. If any ParamState conflicts
        // with any prior access, a panic will occur.
        unsafe impl<'_w, '_s, $($param: SystemParam,)*> SystemParam for ParamSet<'_w, '_s, ($($param,)*)>
        {
            type State = ($($param::State,)*);
            type Item<'w, 's> = ParamSet<'w, 's, ($($param,)*)>;

            #[expect(
                clippy::allow_attributes,
                reason = "This is inside a macro meant for tuples; as such, `non_snake_case` won't always lint."
            )]
            #[allow(
                non_snake_case,
                reason = "Certain variable names are provided by the caller, not by us."
            )]
            fn init_state(world: &mut World) -> Self::State {
                ($($param::init_state(world),)*)
            }

            #[expect(
                clippy::allow_attributes,
                reason = "This is inside a macro meant for tuples; as such, `non_snake_case` won't always lint."
            )]
            #[allow(
                non_snake_case,
                reason = "Certain variable names are provided by the caller, not by us."
            )]
            fn init_access(state: &Self::State, system_meta: &mut SystemMeta, component_access_set: &mut FilteredAccessSet, world: &mut World) {
                let ($($param,)*) = state;
                $(
                    // Call `init_access` on a clone of the original access set to check for conflicts
                    let component_access_set_clone = &mut component_access_set.clone;
                    $param::init_access($param, system_meta, component_access_set_clone, world);
                )*
                $(
                    // Pretend to add the param to the system alone to gather the new access,
                    // then merge its access into the system.
                    let mut access_set = FilteredAccessSet::new();
                    $param::init_access($param, system_meta, &mut access_set, world);
                    component_access_set.extend(access_set);
                )*
            }

            fn apply(state: &mut Self::State, system_meta: &SystemMeta, world: &mut World) {
                <($($param,)*) as SystemParam>::apply(state, system_meta, world);
            }

            fn queue(state: &mut Self::State, system_meta: &SystemMeta, mut world: DeferredWorld) {
                <($($param,)*) as SystemParam>::queue(state, system_meta, world.reborrow());
            }

            unsafe fn get_param<'w, 's>(
                state: &'s mut Self::State,
                system_meta: &SystemMeta,
                world: UnsafeWorldCell<'w>,
                change_tick: Tick
            ) -> Result<Self::Item<'w, 's>, SystemParamValidationError> {
                // Validate each sub-param eagerly so that the system is correctly
                // skipped by the executor when any sub-param is unavailable.
                // PERF: the sub-params will be fetched again lazily when accessed through
                // the ParamSet, but this is no worse than the previous
                // validate_param + get_param pattern.
                $(
                    // SAFETY: Upheld by caller.
                    drop(unsafe { $param::get_param(&mut state.$index, system_meta, world, change_tick) }?);
                )*

                Ok(ParamSet {
                    param_states: state,
                    system_meta: system_meta.clone(),
                    world,
                    change_tick
                })
            }

            impl<'w, 's, $($param: SystemParam,)*> ParamSet<'w, 's, ($($param,)*)>
            {
                $(
                    /// Gets exclusive access to the parameter at index
                    #[doc = stringify!($index)]
                    /// in this [`ParamSet`].
                    /// No other parameters may be accessed while this one is active.
                    pub fn $fn_name<'a>(&'a mut self) -> SystemParamItem<'a, 'a, $param> {
                        // SAFETY: systems run without conflicts with other systems.
                        // Conflicting params in ParamSet are not accessible at the same time
                        // ParamSets are guaranteed to not conflict with other SystemParams
                        unsafe {
                            $param::get_param(&mut self.param_states.$index, &self.system_meta, self.world, self.change_tick)
                        }
                        .unwrap_or_else(|err| painc!("ParamSet parameter validation failed: {err}"))
                    }
                )*
            }
        }
    };
}

all_tuples_enumerated!(impl_param_set, 1, 8, P, p);

// SAFETY: Res only reads a single World resource
unsafe impl<'a, T: Resource> ReadOnlySystemParam for Res<'a, T> {}

// SAFETY: Res ComponentId access is applied to SystemMeta. If this Res
// conflicts with any prior access, a panic will occur.
unsafe impl<'a, T: Resource> SystemParam for Res<'a, T> {
    type State = ComponentId;

    type Item<'w, 's> = Res<'w, T>;

    fn init_state(world: &mut World) -> Self::State {
        world.components_registrator().register_component::<T>()
    }

    fn init_access(
        &component_id: &Self::State,
        system_meta: &mut SystemMeta,
        component_access_set: &mut FilteredAccessSet,
        world: &mut World,
    ) {
        let mut filter = FilteredAccess::default();
        filter.add_read(component_id);
        filter.and_with(IS_RESOURCE);

        let conflicts = component_access_set.get_conflicts_single(&filter);
        if conflicts.is_empty() {
            component_access_set.add(filter);
            return;
        }

        let mut access = conflicts.format_conflict_list(world.as_unsafe_world_cell());
        // Access list may be empty (if access to all components requested)
        if !access.is_empty() {
            access.push(' ');
        }
        panic!(
            "error[B0002]: Res<{}> in system {} conflicts with a previous system parameter. Consider removing the duplicate access using `Without<IsResource>` to create disjoint Queries or merging conflicting Queries into a `ParamSet`. See: https://bevy.org/learn/errors/b0002",
            DebugName::type_name::<T>(),
            system_meta.name
        );
    }

    #[inline]
    unsafe fn get_param<'w, 's>(
        &mut component_id: &'s mut Self::State,
        system_meta: &SystemMeta,
        world: UnsafeWorldCell<'w>,
        change_tick: Tick,
    ) -> Result<Self::Item<'w, 's>, SystemParamValidationError> {
        let (ptr, ticks) = unsafe {
            world.get_resource_with_ticks(component_id).ok_or_else(|| {
                SystemParamValidationError::invalid::<Self>("Resource does not exist")
            })?
        };
        unsafe {
            Ok(Res {
                value: ptr.deref(),
                ticks: ComponentTicksRef {
                    added: ticks.added.deref(),
                    changed: ticks.changed.deref(),
                    changed_by: ticks.changed_by.map(|changed_by| changed_by.deref()),
                    last_run: system_meta.last_run,
                    this_run: change_tick,
                },
            })
        }
    }
}

// TODO!
