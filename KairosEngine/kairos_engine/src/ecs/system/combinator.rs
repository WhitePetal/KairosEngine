use std::marker::PhantomData;

use crate::{
    debug::DebugName,
    ecs::{
        change_detection::{CheckChangeTicks, Tick},
        error::{ErrorContext, FallbackErrorHandler},
        query::FilteredAccessSet,
        system::{ReadOnlySystem, RunSystemError, System, SystemIn, SystemInput},
        world::{DeferredWorld, World, unsafe_world_cell::UnsafeWorldCell},
    },
};

/// Customizes the behavior of a [`CombinatorSystem`].
///
/// # Examples
///
/// ```
/// use bevy_ecs::prelude::*;
/// use bevy_ecs::system::{CombinatorSystem, Combine, RunSystemError};
///
/// // A system combinator that performs an exclusive-or (XOR)
/// // operation on the output of two systems.
/// pub type Xor<A, B> = CombinatorSystem<XorMarker, A, B>;
///
/// // This struct is used to customize the behavior of our combinator.
/// pub struct XorMarker;
///
/// impl<A, B> Combine<A, B> for XorMarker
/// where
///     A: System<In = (), Out = bool>,
///     B: System<In = (), Out = bool>,
/// {
///     type In = ();
///     type Out = bool;
///
///     fn combine<T>(
///         _input: Self::In,
///         data: &mut T,
///         a: impl FnOnce(A::In, &mut T) -> Result<A::Out, RunSystemError>,
///         b: impl FnOnce(B::In, &mut T) -> Result<B::Out, RunSystemError>,
///     ) -> Result<Self::Out, RunSystemError> {
///         Ok(a((), data).unwrap_or(false) ^ b((), data).unwrap_or(false))
///     }
/// }
///
/// # #[derive(Resource, PartialEq, Eq)] struct A(u32);
/// # #[derive(Resource, PartialEq, Eq)] struct B(u32);
/// # #[derive(Resource, Default)] struct RanFlag(bool);
/// # let mut world = World::new();
/// # world.init_resource::<RanFlag>();
/// #
/// # let mut app = Schedule::default();
/// app.add_systems(my_system.run_if(Xor::new(
///     IntoSystem::into_system(resource_equals(A(1))),
///     IntoSystem::into_system(resource_equals(B(1))),
///     // The name of the combined system.
///     "a ^ b".into(),
/// )));
/// # fn my_system(mut flag: ResMut<RanFlag>) { flag.0 = true; }
/// #
/// # world.insert_resource(A(0));
/// # world.insert_resource(B(0));
/// # app.run(&mut world);
/// # // Neither condition passes, so the system does not run.
/// # assert!(!world.resource::<RanFlag>().0);
/// #
/// # world.insert_resource(A(1));
/// # app.run(&mut world);
/// # // Only the first condition passes, so the system runs.
/// # assert!(world.resource::<RanFlag>().0);
/// # world.resource_mut::<RanFlag>().0 = false;
/// #
/// # world.insert_resource(B(1));
/// # app.run(&mut world);
/// # // Both conditions pass, so the system does not run.
/// # assert!(!world.resource::<RanFlag>().0);
/// #
/// # world.insert_resource(A(0));
/// # app.run(&mut world);
/// # // Only the second condition passes, so the system runs.
/// # assert!(world.resource::<RanFlag>().0);
/// # world.resource_mut::<RanFlag>().0 = false;
/// ```
#[diagnostic::on_unimplemented(
    message = "`{Self}` can not combine systems `{A}` and `{B}`",
    label = "invalid system combination",
    note = "the inputs and outputs of `{A}` and `{B}` are not compatible with this combiner"
)]
pub trait Combine<A: System, B: System> {
    /// The [input](System::In) type for a [`CombinatorSystem`].
    type In: SystemInput;

    /// The [output](System::Out) type for a [`CombinatorSystem`].
    type Out;

    /// When used in a [`CombinatorSystem`], this function customizes how
    /// the two composite systems are invoked and their outputs are combined.
    ///
    /// See the trait-level docs for [`Combine`] for an example implementation.
    fn combine<T>(
        input: <Self::In as SystemInput>::Inner<'_>,
        data: &mut T,
        a: impl FnOnce(SystemIn<'_, A>, &mut T) -> Result<A::Out, RunSystemError>,
        b: impl FnOnce(SystemIn<'_, B>, &mut T) -> Result<B::Out, RunSystemError>,
    ) -> Result<Self::Out, RunSystemError>;
}

/// A [`System`] defined by combining two other systems.
/// The behavior of this combinator is specified by implementing the [`Combine`] trait.
/// For a full usage example, see the docs for [`Combine`].
pub struct CombinatorSystem<Func, A, B> {
    _marker: PhantomData<fn() -> Func>,
    a: A,
    b: B,
    name: DebugName,
}

impl<Func, A, B> CombinatorSystem<Func, A, B> {
    /// Creates a new system that combines two inner systems.
    ///
    /// The returned system will only be usable if `Func` implements [`Combine<A, B>`].
    pub fn new(a: A, b: B, name: DebugName) -> Self {
        Self {
            _marker: PhantomData,
            a,
            b,
            name,
        }
    }
}

impl<A, B, Func> System for CombinatorSystem<Func, A, B>
where
    Func: Combine<A, B> + 'static,
    A: System,
    B: System,
{
    type In = Func::In;

    type Out = Func::Out;

    fn name(&self) -> DebugName {
        self.name.clone()
    }

    #[inline]
    fn flags(&self) -> super::SystemStateFlags {
        self.a.flags() | self.b.flags()
    }

    unsafe fn run_unsafe(
        &mut self,
        input: SystemIn<'_, Self>,
        world: crate::ecs::world::unsafe_world_cell::UnsafeWorldCell,
    ) -> Result<Self::Out, RunSystemError> {
        struct PrivateUnsafeWorldCell<'w>(UnsafeWorldCell<'w>);

        // Since control over handling system run errors is passed on to the
        // implementation of `Func::combine`, which may run the two closures
        // however it wants, errors must be intercepted here if they should be
        // handled by the world's error handler.
        unsafe fn run_system<S: System>(
            system: &mut S,
            input: SystemIn<S>,
            world: &mut PrivateUnsafeWorldCell,
        ) -> Result<S::Out, RunSystemError> {
            match unsafe { system.run_unsafe(input, world.0) } {
                // let the world's fallback error handler handle the error if `Failed(_)`
                Err(RunSystemError::Failed(err)) => {
                    // SAFETY: We registered access to FallbackErrorHandler in `initialize`.
                    (unsafe { world.0.fallback_error_handler() })(
                        err,
                        ErrorContext::System {
                            name: system.name(),
                            last_run: system.get_last_run(),
                        },
                    );

                    // Since the error handler takes the error by value, create a new error:
                    // The original error has already been handled, including
                    // the reason for the failure here isn't important.
                    Err(format!("System `{}` failed", system.name()).into())
                }
                // `Skipped(_)` and `Ok(_)` are passed through:
                // system skipping is not an error, and isn't passed to the
                // world's error handler by the executors.
                result @ (Ok(_) | Err(RunSystemError::Skipped(_))) => result,
            }
        }

        Func::combine(
            input,
            &mut PrivateUnsafeWorldCell(world),
            // SAFETY: The world accesses for both underlying systems have been registered,
            // so the caller will guarantee that no other systems will conflict with (`a` or `b`) and the `FallbackErrorHandler` resource.
            // If either system has `is_exclusive()`, then the combined system also has `is_exclusive`.
            // Since we require a `combine` to pass in a mutable reference to `world` and that's a private type
            // passed to a function as an unbound non-'static generic argument, they can never be called in parallel
            // or re-entrantly because that would require forging another instance of `PrivateUnsafeWorldCell`.
            // This means that the world accesses in the two closures will not conflict with each other.
            // The closure's access to the FallbackErrorHandler does not
            // conflict with any potential access to the FallbackErrorHandler by
            // the systems since the closures are not run in parallel.
            |input, world| unsafe { run_system(&mut self.a, input, world) },
            // SAFETY: See the comment above.
            |input, world| unsafe { run_system(&mut self.b, input, world) },
        )
    }

    #[cfg(feature = "hotpatching")]
    #[inline]
    fn refresh_hotpatch(&mut self) {
        self.a.refresh_hotpatch();
        self.b.refresh_hotpatch();
    }

    #[inline]
    fn apply_deferred(&mut self, world: &mut World) {
        self.a.apply_deferred(world);
        self.b.apply_deferred(world);
    }

    #[inline]
    fn queue_deferred(&mut self, mut world: DeferredWorld) {
        self.a.queue_deferred(world.reborrow());
        self.b.queue_deferred(world);
    }

    fn initialize(&mut self, world: &mut World) -> FilteredAccessSet {
        let mut a_access = self.a.initialize(world);
        let b_access = self.b.initialize(world);
        a_access.extend(b_access);

        // We might need to read the fallback error handler after the component
        // systems have run to report failures.
        let error_resource = world.register_component::<FallbackErrorHandler>();
        a_access.add_resource_read(error_resource);
        a_access
    }

    fn check_change_tick(&mut self, check: CheckChangeTicks) {
        self.a.check_change_tick(check);
        self.b.check_change_tick(check);
    }

    fn default_system_sets(&self) -> Vec<crate::ecs::schedule::InternedSystemSet> {
        let mut default_sets = self.a.default_system_sets();
        default_sets.append(&mut self.b.default_system_sets());
        default_sets
    }

    fn get_last_run(&self) -> Tick {
        self.a.get_last_run()
    }

    fn set_last_run(&mut self, last_run: Tick) {
        self.a.set_last_run(last_run);
        self.b.set_last_run(last_run);
    }
}

// SAFETY: Both systems are read-only, so any system created by combining them will only read from the world.
unsafe impl<Func, A, B> ReadOnlySystem for CombinatorSystem<Func, A, B>
where
    Func: Combine<A, B> + 'static,
    A: ReadOnlySystem,
    B: ReadOnlySystem,
{
}

impl<Func, A, B> Clone for CombinatorSystem<Func, A, B>
where
    A: Clone,
    B: Clone,
{
    /// Clone the combined system. The cloned instance must be `.initialize()`d before it can run.
    fn clone(&self) -> Self {
        CombinatorSystem::new(self.a.clone(), self.b.clone(), self.name.clone())
    }
}
