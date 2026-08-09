use std::marker::PhantomData;

use crate::ecs::{
    system::{SystemParam, SystemParamFunction, SystemState},
    world::World,
};

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
