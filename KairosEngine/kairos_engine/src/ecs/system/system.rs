use std::{any::Any, fmt::Display};

use crate::{
    debug::DebugName,
    ecs::{
        change_detection::Tick,
        error::BevyError,
        query::FilteredAccessSet,
        system::{SystemIn, SystemInput, SystemParamValidationError},
        world::{DeferredWorld, World, unsafe_world_cell::UnsafeWorldCell},
    },
};

/// An ECS system that can be added to a [`Schedule`](crate::schedule::Schedule)
///
/// Systems are functions with all arguments implementing
/// [`SystemParam`](crate::system::SystemParam).
///
/// Systems are added to an application using `App::add_systems(Update, my_system)`
/// or similar methods, and will generally run once per pass of the main loop.
///
/// Systems are executed in parallel, in opportunistic order; data access is managed automatically.
/// It's possible to specify explicit execution order between specific systems,
/// see [`IntoScheduleConfigs`](crate::schedule::IntoScheduleConfigs).
#[diagnostic::on_unimplemented(message = "`{Self}` is not a system", label = "invalid system")]
pub trait System: Send + Sync + 'static {
    /// The system's input.
    type In: SystemInput;
    /// The system's output.
    type Out;

    /// Returns the system's name.
    fn name(&self) -> DebugName;

    /// Returns true if the system must be run exclusively.
    #[inline]
    fn is_exclusive(&self) -> bool {
        todo!()
    }

    /// Initialize the system.
    ///
    /// Returns a [`FilteredAccessSet`] with the access required to run the system.
    fn initialize(&mut self, _world: &mut World) -> FilteredAccessSet;

    /// Runs the system with the given input in the world. Unlike [`System::run`], this function
    /// can be called in parallel with other systems and may break Rust's aliasing rules
    /// if used incorrectly, making it unsafe to call.
    ///
    /// Unlike [`System::run`], this will not apply deferred parameters, which must be independently
    /// applied by calling [`System::apply_deferred`] at later point in time.
    ///
    /// # Safety
    ///
    /// - The caller must ensure that [`world`](UnsafeWorldCell) has permission to access any world data
    ///   registered in the access returned from [`System::initialize`]. There must be no conflicting
    ///   simultaneous accesses while the system is running.
    /// - If [`System::is_exclusive`] returns `true`, then it must be valid to call
    ///   [`UnsafeWorldCell::world_mut`] on `world`.
    unsafe fn run_unsafe(
        &mut self,
        input: SystemIn<'_, Self>,
        world: UnsafeWorldCell,
    ) -> Result<Self::Out, RunSystemError>;

    /// Gets the tick indicating the last time this system ran.
    fn get_last_run(&self) -> Tick;

    /// Enqueues any [`Deferred`](crate::system::Deferred) system parameters (or other system buffers)
    /// of this system into the world's command buffer.
    fn queue_deferred(&mut self, world: DeferredWorld);
}

/// Running system failed.
#[derive(Debug)]
pub enum RunSystemError {
    /// System could not be run due to parameters that failed validation.
    /// This is not considered an error.
    Skipped(SystemParamValidationError),
    /// System returned an error or failed required parameter validation.
    Failed(BevyError),
}

impl Display for RunSystemError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Skipped(err) => write!(
                f,
                "System did not run due to failed parameter validation: {err}"
            ),
            Self::Failed(err) => write!(f, "{err}"),
        }
    }
}

impl<E: Any> From<E> for RunSystemError
where
    BevyError: From<E>,
{
    fn from(mut value: E) -> Self {
        // Specialize the impl so that a skipped `SystemParamValidationError`
        // is converted to `Skipped` instead of `Failed`.
        // Note that the `downcast_mut` check is based on the static type,
        // and can be optimized out after monomorphization.
        let any: &mut dyn Any = &mut value;
        if let Some(err) = any.downcast_mut::<SystemParamValidationError>()
            && err.skipped
        {
            return Self::Skipped(std::mem::replace(err, SystemParamValidationError::EMPTY));
        }
        Self::Failed(From::from(value))
    }
}

// TODO!
