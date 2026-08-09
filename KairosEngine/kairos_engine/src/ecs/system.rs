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

mod commands;
mod input;
mod observer_system;
mod system;
mod system_param;

pub use commands::*;
pub use input::*;
pub use observer_system::*;
pub use system::*;
pub use system_param::*;

// TODO!
