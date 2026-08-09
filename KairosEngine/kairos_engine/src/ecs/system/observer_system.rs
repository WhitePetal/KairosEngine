use crate::ecs::{bundle::Bundle, event::Event, observer::On, system::System};

pub trait ObserverSystem<E: Event, B: Bundle, Out = ()>:
    System<In = On<'static, 'static, E, B>, Out = Out> + Send + 'static
{
}

impl<E: Event, B: Bundle, Out, T> ObserverSystem<E, B, Out> for T where
    T: System<In = On<'static, 'static, E, B>, Out = Out> + Send + 'static
{
}

/// Implemented for systems that convert into [`ObserverSystem`].
///
/// # Usage notes
///
/// This trait should only be used as a bound for trait implementations or as an
/// argument to a function. If an observer system needs to be returned from a
/// function or stored somewhere, use [`ObserverSystem`] instead of this trait.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot become an `ObserverSystem`",
    label = "the trait `IntoObserverSystem` is not implemented",
    note = "for function `ObserverSystem`s, ensure the first argument is `On<T>` and any subsequent ones are `SystemParam`"
)]
pub trait IntoObserverSystem<E: Event, B: Bundle, M, Out = ()>: Send + 'static {
    /// The type of [`System`] that this instance converts into.
    type System: ObserverSystem<E, B, Out>;

    /// Turns this value into its corresponding [`System`].
    fn into_system(this: Self) -> Self::System;
}

// TODO!
