use crate::ecs::{bundle::Bundle, event::Event, observer::On, system::System};

/// Trait for types that can be used as input to [`System`]s.
///
/// Provided implementations are:
/// - `()`: No input
/// - [`In<T>`]: For values
/// - [`InRef<T>`]: For read-only references to values
/// - [`InMut<T>`]: For mutable references to values
/// - [`On<E, B>`]: For [`ObserverSystem`]s
/// - [`StaticSystemInput<I>`]: For arbitrary [`SystemInput`]s in generic contexts
/// - `Option<I>`: For optional inputs of some [`SystemInput`] `I`
/// - Tuples of [`SystemInput`]s up to 8 elements
///
/// For advanced usecases, you can implement this trait for your own types.
///
/// # Examples
///
/// ## Tuples of [`SystemInput`]s
///
/// ```
/// use bevy_ecs::prelude::*;
///
/// fn add((InMut(a), In(b)): (InMut<usize>, In<usize>)) {
///     *a += b;
/// }
/// # let mut world = World::new();
/// # let mut add = IntoSystem::into_system(add);
/// # add.initialize(&mut world);
/// # let mut a = 12;
/// # let b = 24;
/// # add.run((&mut a, b), &mut world);
/// # assert_eq!(a, 36);
/// ```
///
/// [`ObserverSystem`]: crate::system::ObserverSystem
pub trait SystemInput: Sized {
    /// The wrapper input type that is defined as the first argument to [`FunctionSystem`]s.
    ///
    /// [`FunctionSystem`]: crate::system::FunctionSystem
    type Param<'i>: SystemInput;
    /// The inner input type that is passed to functions that run systems,
    /// such as [`System::run`].
    ///
    /// [`System::run`]: crate::system::System::run
    type Inner<'i>;

    /// Converts a [`SystemInput::Inner`] into a [`SystemInput::Param`].
    fn wrap(this: Self::Inner<'_>) -> Self::Param<'_>;
}

/// Shorthand way to get the [`System::In`] for a [`System`] as a [`SystemInput::Inner`].
pub type SystemIn<'a, S> = <<S as System>::In as SystemInput>::Inner<'a>;

/// Used for [`ObserverSystem`]s.
///
/// [`ObserverSystem`]: crate::system::ObserverSystem
impl<E: Event, B: Bundle> SystemInput for On<'_, '_, E, B> {
    // Note: the fact that we must use a shared lifetime here is
    // a key piece of the complicated safety story documented above
    // the `&mut E::Trigger<'_>` cast in `observer_system_runner` and in
    // the `On` implementation.
    type Param<'i> = On<'i, 'i, E, B>;
    type Inner<'i> = On<'i, 'i, E, B>;

    fn wrap(this: Self::Inner<'_>) -> Self::Param<'_> {
        this
    }
}

// TODO!
