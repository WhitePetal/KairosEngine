use std::marker::PhantomData;

use crate::ecs::{
    system::{SystemParam, SystemParamFunction, SystemState},
    world::World,
};

/// A builder that can create a [`SystemParam`].
///
/// ```
/// # use bevy_ecs::{
/// #     prelude::*,
/// #     system::{SystemParam, ParamBuilder},
/// # };
/// # #[derive(Resource)]
/// # struct R;
/// #
/// # #[derive(SystemParam)]
/// # struct MyParam;
/// #
/// fn some_system(param: MyParam) {}
///
/// fn build_system(builder: impl SystemParamBuilder<MyParam> + 'static) {
///     // To build a system, create a tuple of `SystemParamBuilder`s
///     // with a builder for each parameter.
///     // Note that the builder for a system must be a tuple,
///     // even if there is only one parameter.
/// #   let _system: bevy_ecs::system::IntoBuilderSystem<fn(MyParam), (), (), _, _> =
///     (builder,)
///         .build_system(some_system);
/// }
///
/// fn build_system_direct(builder: impl SystemParamBuilder<MyParam>) {
///     let mut world = World::new();
///     // You can also construct a system in two steps, first by
///     // constructing a [`SystemState`] with `build_state` and
///     // second by constructing the final system with `build_system`.
///     // This can be useful in cases that require type inference
///     // for function parameters (like closures!), since normal
///     // `build_system` requires explicitly specifying all parameter
///     // types. See `build_closure_system_infer/explicit` below for more
///     // info.
///     (builder,)
///         .build_state(&mut world)
///         .build_system(some_system);
/// }
///
/// fn build_closure_system_infer(builder: impl SystemParamBuilder<MyParam>) {
///     let mut world = World::new();
///     // Closures can be used in addition to named functions.
///     // If a closure is used, the parameter types must all be inferred
///     // from the builders, so you cannot use plain `ParamBuilder`.
///     (builder, ParamBuilder::resource())
///         .build_state(&mut world)
///         .build_system(|param, res| {
///             let param: MyParam = param;
///             let res: Res<R> = res;
///         });
/// }
///
/// fn build_closure_system_explicit(builder: impl SystemParamBuilder<MyParam>) {
///     let mut world = World::new();
///     // Alternately, you can provide all types in the closure
///     // parameter list and call `build_system()` normally.
///     (builder, ParamBuilder::resource())
///         .build_state(&mut world) // this line can be optionally omitted, since all the parameter types are explicit!
///         .build_system(|param: MyParam, res: Res<R>| {});
/// }
/// ```
///
/// See the documentation for individual builders for more examples.
///
/// # List of Builders
///
/// [`ParamBuilder`] can be used for parameters that don't require any special building.
/// Using a `ParamBuilder` will build the system parameter the same way it would be initialized in an ordinary system.
///
/// `ParamBuilder` also provides factory methods that return a `ParamBuilder` typed as `impl SystemParamBuilder<P>`
/// for common system parameters that can be used to guide closure parameter inference.
///
/// [`QueryParamBuilder`] can build a [`Query`] to add additional filters,
/// or to configure the components available to [`FilteredEntityRef`](crate::world::FilteredEntityRef) or [`FilteredEntityMut`](crate::world::FilteredEntityMut).
/// You can also use a [`QueryState`] to build a [`Query`].
///
/// [`LocalBuilder`] can build a [`Local`] to supply the initial value for the `Local`.
///
/// [`FilteredResourcesParamBuilder`] can build a [`FilteredResources`],
/// and [`FilteredResourcesMutParamBuilder`] can build a [`FilteredResourcesMut`],
/// to configure the resources that can be accessed.
///
/// [`DynParamBuilder`] can build a [`DynSystemParam`] to determine the type of the inner parameter,
/// and to supply any `SystemParamBuilder` it needs.
///
/// Tuples of builders can build tuples of parameters, one builder for each element.
/// Note that since systems require a tuple as a parameter, the outer builder for a system will always be a tuple.
///
/// A [`Vec`] of builders can build a `Vec` of parameters, one builder for each element.
///
/// A [`ParamSetBuilder`] can build a [`ParamSet`].
/// This can wrap either a tuple or a `Vec`, one builder for each element.
///
/// A custom system param created with `#[derive(SystemParam)]` can be buildable if it includes a `#[system_param(builder)]` attribute.
/// See [the documentation for `SystemParam` derives](SystemParam#builders).
///
/// # Safety
///
/// The implementor must ensure that the state returned
/// from [`SystemParamBuilder::build`] is valid for `P`.
/// Note that the exact safety requirements depend on the implementation of [`SystemParam`],
/// so if `Self` is not a local type then you must call [`SystemParam::init_state`]
/// or another [`SystemParamBuilder::build`].
pub unsafe trait SystemParamBuilder<P: SystemParam>: Sized {
    /// Registers any [`World`] access used by this [`SystemParam`]
    /// and creates a new instance of this param's [`State`](SystemParam::State).
    fn build(self, world: &mut World) -> P::State;

    /// Create a [`SystemState`] from a [`SystemParamBuilder`].
    /// To create a system, call [`SystemState::build_system`] on the result.
    fn build_state(self, world: &mut World) -> SystemState<P> {
        SystemState::from_builder(world, self)
    }

    /// Create a [`System`] from a [`SystemParamBuilder`] directly.
    ///
    /// This method is useful in cases where type inference for
    /// closure parameters isn't necessary, or where it's not
    /// possible to call [`SystemState::build_system`] by passing
    /// in an `&mut World`. Rather than constructing the system's
    /// state immediately, this function returns a wrapper that
    /// initializes the system state during the first run.
    ///
    /// Caveats:
    /// - doesn't support parameter type inference.
    /// - only works for 'static system param builder types.
    ///
    /// In cases where  either of these are required, call
    /// [`SystemParamBuilder::build_state`] instead.
    fn build_system<Marker, In, Out, Func>(
        self,
        func: Func,
    ) -> IntoBuilderSystem<Marker, In, Out, Func, Self>
    where
        Self: 'static,
        Func: SystemParamFunction<Marker, Param = P>,
    {
        IntoBuilderSystem::new(self, func)
    }
}

/// A [`SystemParamBuilder`] for any [`SystemParam`] that uses its default initialization.
///
/// ## Example
///
/// ```
/// # use bevy_ecs::{
/// #     prelude::*,
/// #     system::{SystemParam, ParamBuilder},
/// # };
/// #
/// # #[derive(Component)]
/// # struct A;
/// #
/// # #[derive(Resource)]
/// # struct R;
/// #
/// # #[derive(SystemParam)]
/// # struct MyParam;
/// #
/// # let mut world = World::new();
/// # world.insert_resource(R);
/// #
/// fn my_system(res: Res<R>, param: MyParam, query: Query<&A>) {
///     // ...
/// }
///
/// let system = (
///     // A plain ParamBuilder can build any parameter type.
///     ParamBuilder,
///     // The `of::<P>()` method returns a `ParamBuilder`
///     // typed as `impl SystemParamBuilder<P>`.
///     ParamBuilder::of::<MyParam>(),
///     // The other factory methods return typed builders
///     // for common parameter types.
///     ParamBuilder::query::<&A>(),
/// )
///     .build_state(&mut world)
///     .build_system(my_system);
/// ```
#[derive(Default, Debug, Clone)]
pub struct ParamBuilder;

unsafe impl<P: SystemParam> SystemParamBuilder<P> for ParamBuilder {
    fn build(self, world: &mut World) -> <P as SystemParam>::State {
        P::init_state(world)
    }
}

/// An [`IntoSystem`] creating an instance of [`BuilderSystem`]
pub struct IntoBuilderSystem<Marker, In, Out, Func, Builder>
where
    Func: SystemParamFunction<Marker>,
    Builder: SystemParamBuilder<Func::Param>,
{
    builder: Builder,
    func: Func,
    _marker: PhantomData<fn(In) -> (Marker, Out)>,
}

impl<Marker, In, Out, Func, Builder> IntoBuilderSystem<Marker, In, Out, Func, Builder>
where
    Func: SystemParamFunction<Marker>,
    Builder: SystemParamBuilder<Func::Param>,
{
    // Returns a new [`IntoBuilderSystem`] given a system param builder and system function
    pub fn new(builder: Builder, func: Func) -> Self {
        Self {
            builder,
            func,
            _marker: PhantomData,
        }
    }
}
