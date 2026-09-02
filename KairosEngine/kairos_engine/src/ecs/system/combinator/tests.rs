// use crate::{
//     debug::DebugName,
//     ecs::{
//         change_detection::ResMut,
//         error::FallbackErrorHandler,
//         system::{CombinatorSystem, In, IntoSystem},
//         world::World,
//     },
// };

// #[test]
// fn combinator_with_error_handler_access() {
//     fn my_system(_: ResMut<FallbackErrorHandler>) {}
//     fn a() -> bool {
//         true
//     }
//     fn b(_: ResMut<FallbackErrorHandler>) -> bool {
//         true
//     }
//     fn asdf(_: In<bool>) {}

//     let mut world = World::new();
//     world.insert_resource(FallbackErrorHandler::default());

//     let system = CombinatorSystem::<OrElseMarker, _, _>::new(
//         IntoSystem::into_system(a),
//         IntoSystem::into_system(b),
//         DebugName::borrowed("a OR b"),
//     );

//     // `system` should not conflict with itself by mutably accessing the error handler resource.
//     assert_system_does_not_conflict(system.clone());

//     let mut schedule = Schedule::default();
//     schedule.add_systems((my_system, system.pipe(asdf)));
//     schedule.initialize(&mut world).unwrap();

//     // `my_system` should conflict with the combinator system because the combinator reads the error handler resource.
//     assert!(!schedule.graph().conflicting_systems().is_empty());

//     schedule.run(&mut world);
// }

// #[test]
// fn exclusive_system_piping_is_possible() {
//     fn my_exclusive_system(_world: &mut World) -> u32 {
//         1
//     }

//     fn out_pipe(input: In<u32>) {
//         assert!(input.0 == 1);
//     }

//     let mut world = World::new();

//     let mut schedule = Schedule::default();
//     schedule.add_systems(my_exclusive_system.pipe(out_pipe));

//     schedule.run(&mut world);
// }
