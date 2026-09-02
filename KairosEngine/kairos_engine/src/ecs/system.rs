//! Tools for controlling behavior in an ECS application.
//!
//! Systems define how an ECS based application behaves.
//! Systems are added to a [`Schedule`](crate::schedule::Schedule), which is then run.
//! A system is usually written as a normal function, which is automatically converted into a system.
//!
//! System functions can have parameters, through which one can query and mutate Bevy ECS state.
//! Only types that implement [`SystemParam`] can be used, automatically fetching data from
//! the [`World`].
//!
//! System functions often look like this:
//!
//! ```
//! # use bevy_ecs::prelude::*;
//! #
//! # #[derive(Component)]
//! # struct Player { alive: bool }
//! # #[derive(Component)]
//! # struct Score(u32);
//! # #[derive(Resource)]
//! # struct Round(u32);
//! #
//! fn update_score_system(
//!     mut query: Query<(&Player, &mut Score)>,
//!     mut round: ResMut<Round>,
//! ) {
//!     for (player, mut score) in &mut query {
//!         if player.alive {
//!             score.0 += round.0;
//!         }
//!     }
//!     round.0 += 1;
//! }
//! # bevy_ecs::system::assert_is_system(update_score_system);
//! ```
//!
//! # System ordering
//!
//! By default, the execution of systems is parallel and not deterministic.
//! Not all systems can run together: if a system mutably accesses data,
//! no other system that reads or writes that data can be run at the same time.
//! These systems are said to be **incompatible**.
//!
//! The relative order in which incompatible systems are run matters.
//! When this is not specified, a **system order ambiguity** exists in your schedule.
//! You can **explicitly order** systems:
//!
//! - by calling the `.before(this_system)` or `.after(that_system)` methods when adding them to your schedule
//! - by adding them to a [`SystemSet`], and then using `.configure_sets(ThisSet.before(ThatSet))` syntax to configure many systems at once
//! - through the use of `.add_systems((system_a, system_b, system_c).chain())`
//!
//! [`SystemSet`]: crate::schedule::SystemSet
//!
//! ## Example
//!
//! ```
//! # use bevy_ecs::prelude::*;
//! # let mut schedule = Schedule::default();
//! # let mut world = World::new();
//! // Configure these systems to run in order using `chain()`.
//! schedule.add_systems((print_first, print_last).chain());
//! // Prints "HelloWorld!"
//! schedule.run(&mut world);
//!
//! // Configure this system to run in between the other two systems
//! // using explicit dependencies.
//! schedule.add_systems(print_mid.after(print_first).before(print_last));
//! // Prints "Hello, World!"
//! schedule.run(&mut world);
//!
//! fn print_first() {
//!     print!("Hello");
//! }
//! fn print_mid() {
//!     print!(", ");
//! }
//! fn print_last() {
//!     println!("World!");
//! }
//! ```
//!
//! # System return type
//!
//! Systems added to a schedule through [`add_systems`](crate::schedule::Schedule) may either return
//! empty `()` or a [`Result`](crate::error::Result). Other contexts (like one shot systems) allow
//! systems to return arbitrary values.
//!
//! # System parameter list
//! Following is the complete list of accepted types as system parameters:
//!
//! - [`Query`]
//! - [`Res`] and `Option<Res>`
//! - [`ResMut`] and `Option<ResMut>`
//! - [`Commands`]
//! - [`Local`]
//! - [`MessageReader`](crate::message::MessageReader)
//! - [`MessageWriter`](crate::message::MessageWriter)
//! - [`NonSend`] and `Option<NonSend>`
//! - [`NonSendMut`] and `Option<NonSendMut>`
//! - [`RemovedComponents`](crate::lifecycle::RemovedComponents)
//! - [`SystemName`]
//! - [`SystemChangeTick`]
//! - [`Archetypes`](crate::archetype::Archetypes) (Provides Archetype metadata)
//! - [`Bundles`](crate::bundle::Bundles) (Provides Bundles metadata)
//! - [`Components`](crate::component::Components) (Provides Components metadata)
//! - [`Entities`](crate::entity::Entities) (Provides Entities metadata)
//! - All tuples between 1 to 16 elements where each element implements [`SystemParam`]
//! - [`ParamSet`]
//! - [`()` (unit primitive type)](https://doc.rust-lang.org/stable/std/primitive.unit.html)
//!
//! In addition, the following parameters can be used when constructing a dynamic system with [`SystemParamBuilder`],
//! but will only provide an empty value when used with an ordinary system:
//!
//! - [`FilteredResources`](crate::world::FilteredResources)
//! - [`FilteredResourcesMut`](crate::world::FilteredResourcesMut)
//! - [`DynSystemParam`]
//! - [`Vec<P>`] and [`SmallVec<[P, N]>`](smallvec::SmallVec) where `P: SystemParam`
//! - [`ParamSet<Vec<P>>`] where `P: SystemParam`
//!
//! [`Vec<P>`]: alloc::vec::Vec

mod adapter_system;
mod builder;
mod combinator;
mod commands;
mod function_system;
mod input;
mod observer_system;
mod query;
mod schedule_system;
mod system;
mod system_param;
mod system_registry;

pub use adapter_system::*;
pub use builder::*;
pub use combinator::*;
pub use commands::*;
pub use function_system::*;
pub use input::*;
pub use observer_system::*;
pub use query::*;
pub use schedule_system::*;
pub use system::*;
pub use system_param::*;
pub use system_registry::*;

use crate::ecs::world::World;

/// Conversion trait to turn something into a [`System`].
///
/// Use this to get a system from a function. Also note that every system implements this trait as
/// well.
///
/// # Usage notes
///
/// This trait should only be used as a bound for trait implementations or as an
/// argument to a function. If a system needs to be returned from a function or
/// stored somewhere, use [`System`] instead of this trait.
///
/// # Examples
///
/// ```
/// use bevy_ecs::prelude::*;
///
/// fn my_system_function(a_usize_local: Local<usize>) {}
///
/// let system = IntoSystem::into_system(my_system_function);
/// ```
// This trait has to be generic because we have potentially overlapping impls, in particular
// because Rust thinks a type could impl multiple different `FnMut` combinations
// even though none can currently
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a valid system with input `{In}` and output `{Out}`",
    label = "invalid system"
)]
pub trait IntoSystem<In: SystemInput, Out, Marker>: Sized {
    /// The type of [`System`] that this instance converts into.
    type System: System<In = In, Out = Out>;

    /// Turns this value into its corresponding [`System`].
    fn into_system(this: Self) -> Self::System;

    /// Pass the output of this system `A` into a second system `B`, creating a new compound system.
    ///
    /// The second system must have [`In<T>`](crate::system::In) as its first parameter,
    /// where `T` is the return type of the first system.
    fn pipe<B, BIn, BOut, MarkerB>(self, system: B) -> IntoPipeSystem<Self, B>
    where
        Out: 'static,
        B: IntoSystem<BIn, BOut, MarkerB>,
        for<'a> BIn: SystemInput<Inner<'a> = Out>,
    {
        IntoPipeSystem::new(self, system)
    }

    /// Pass the output of this system into the passed function `f`, creating a new system that
    /// outputs the value returned from the function.
    ///
    /// ```
    /// # use bevy_ecs::prelude::*;
    /// # let mut schedule = Schedule::default();
    /// // Ignores the output of a system that may fail.
    /// schedule.add_systems(my_system.map(drop));
    /// # let mut world = World::new();
    /// # world.insert_resource(T);
    /// # schedule.run(&mut world);
    ///
    /// # #[derive(Resource)] struct T;
    /// # type Err = ();
    /// fn my_system(res: Res<T>) -> Result<(), Err> {
    ///     // ...
    ///     # Err(())
    /// }
    /// ```
    fn map<T, F>(self, f: F) -> IntoAdapterSystem<F, Self>
    where
        F: Send + Sync + 'static + FnMut(Out) -> T,
    {
        IntoAdapterSystem::new(f, self)
    }

    /// Passes a mutable reference to `value` as input to the system each run,
    /// turning it into a system that takes no input.
    ///
    /// `Self` can have any [`SystemInput`] type that takes a mutable reference
    /// to `T`, such as [`InMut`].
    ///
    /// # Example
    ///
    /// ```
    /// # use bevy_ecs::prelude::*;
    /// #
    /// fn my_system(InMut(value): InMut<usize>) {
    ///     *value += 1;
    ///     if *value > 10 {
    ///        println!("Value is greater than 10!");
    ///     }
    /// }
    ///
    /// # let mut schedule = Schedule::default();
    /// schedule.add_systems(my_system.with_input(0));
    /// # bevy_ecs::system::assert_is_system(my_system.with_input(0));
    /// ```
    fn with_input<T>(self, value: T) -> WithInputWrapper<Self::System, T>
    where
        for<'i> In: SystemInput<Inner<'i> = &'i mut T>,
        T: Send + Sync + 'static,
    {
        WithInputWrapper::new(self, value)
    }
}

// All systems implicitly implement IntoSystem.
impl<T: System> IntoSystem<T::In, T::Out, ()> for T {
    type System = T;
    fn into_system(this: Self) -> Self {
        this
    }
}

/// Ensure that a given function is a [system](System).
///
/// This should be used when writing doc examples,
/// to confirm that systems used in an example are
/// valid systems.
///
/// # Examples
///
/// The following example will panic when run since the
/// system's parameters mutably access the same component
/// multiple times.
///
/// ```should_panic
/// # use bevy_ecs::{prelude::*, system::assert_is_system};
/// #
/// # #[derive(Component)]
/// # struct Transform;
/// #
/// fn my_system(query1: Query<&mut Transform>, query2: Query<&mut Transform>) {
///     // ...
/// }
///
/// assert_is_system(my_system);
/// ```
pub fn assert_is_system<In: SystemInput, Out: 'static, Marker>(
    system: impl IntoSystem<In, Out, Marker>,
) {
    let mut system = IntoSystem::into_system(system);

    // Initialize the system, which will panic if the system has access conflicts.
    let mut world = World::new();
    system.initialize(&mut world);
}

// TODO!
