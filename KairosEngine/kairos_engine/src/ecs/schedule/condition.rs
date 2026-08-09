use crate::ecs::system::{IntoSystem, ReadOnlySystem, SystemInput};

/// A type-erased run condition stored in a [`Box`].
pub type BoxedCondition<In = ()> = Box<dyn ReadOnlySystem<In = In, Out = bool>>;

/// A system that determines if one or more scheduled systems should run.
///
/// Implemented for functions and closures that convert into [`System<Out=bool>`](System)
/// with [read-only](crate::system::ReadOnlySystemParam) parameters.
///
/// # Marker type parameter
///
/// `SystemCondition` trait has `Marker` type parameter, which has no special meaning,
/// but exists to work around the limitation of Rust's trait system.
///
/// Type parameter in return type can be set to `<()>` by calling [`IntoSystem::into_system`],
/// but usually have to be specified when passing a condition to a function.
///
/// ```
/// # use bevy_ecs::schedule::SystemCondition;
/// # use bevy_ecs::system::IntoSystem;
/// fn not_condition<Marker>(a: impl SystemCondition<Marker>) -> impl SystemCondition<()> {
///    IntoSystem::into_system(a.map(|x| !x))
/// }
/// ```
///
/// # Examples
/// A condition that returns true every other time it's called.
/// ```
/// # use bevy_ecs::prelude::*;
/// fn every_other_time() -> impl SystemCondition<()> {
///     IntoSystem::into_system(|mut flag: Local<bool>| {
///         *flag = !*flag;
///         *flag
///     })
/// }
///
/// # #[derive(Resource)] struct DidRun(bool);
/// # fn my_system(mut did_run: ResMut<DidRun>) { did_run.0 = true; }
/// # let mut schedule = Schedule::default();
/// schedule.add_systems(my_system.run_if(every_other_time()));
/// # let mut world = World::new();
/// # world.insert_resource(DidRun(false));
/// # schedule.run(&mut world);
/// # assert!(world.resource::<DidRun>().0);
/// # world.insert_resource(DidRun(false));
/// # schedule.run(&mut world);
/// # assert!(!world.resource::<DidRun>().0);
/// ```
///
/// A condition that takes a bool as an input and returns it unchanged.
///
/// ```
/// # use bevy_ecs::prelude::*;
/// fn identity() -> impl SystemCondition<(), In<bool>> {
///     IntoSystem::into_system(|In(x): In<bool>| x)
/// }
///
/// # fn always_true() -> bool { true }
/// # let mut app = Schedule::default();
/// # #[derive(Resource)] struct DidRun(bool);
/// # fn my_system(mut did_run: ResMut<DidRun>) { did_run.0 = true; }
/// app.add_systems(my_system.run_if(always_true.pipe(identity())));
/// # let mut world = World::new();
/// # world.insert_resource(DidRun(false));
/// # app.run(&mut world);
/// # assert!(world.resource::<DidRun>().0);
pub trait SystemCondition<Marker, In: SystemInput = ()>:
    IntoSystem<In, bool, Marker, System: ReadOnlySystem>
{
}
