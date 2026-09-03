// use crate::ecs::{
//     change_detection::{NonSendMut, Res, ResMut},
//     system::{Commands, In, RunSystemError},
//     world::World,
// };

// #[test]
// fn run_system_once() {
//     #[derive(Resource)]
//     struct T(usize);

//     fn system(In(n): In<usize>, mut commands: Commands) -> usize {
//         commands.insert_resource(T(n));
//         n + 1
//     }

//     let mut world = World::default();
//     let n = world.run_system_once_with(system, 1).unwrap();
//     assert_eq!(n, 2);
//     assert_eq!(world.resource::<T>().0, 1);
// }

// #[derive(Resource, Default, PartialEq, Debug)]
// struct Counter(u8);

// fn count_up(mut counter: ResMut<Counter>) {
//     counter.0 += 1;
// }

// #[test]
// fn run_two_systems() {
//     let mut world = World::new();
//     world.init_resource::<Counter>();
//     assert_eq!(*world.resource::<Counter>(), Counter(0));
//     world.run_system_once(count_up).unwrap();
//     assert_eq!(*world.resource::<Counter>(), Counter(1));
//     world.run_system_once(count_up).unwrap();
//     assert_eq!(*world.resource::<Counter>(), Counter(2));
// }

// #[derive(Component)]
// struct A;

// fn spawn_entity(mut commands: Commands) {
//     commands.spawn(A);
// }

// #[test]
// fn command_processing() {
//     let mut world = World::new();
//     assert_eq!(world.query::<&A>().query(&world).count(), 0);
//     world.run_system_once(spawn_entity).unwrap();
//     assert_eq!(world.query::<&A>().query(&world).count(), 1);
// }

// #[test]
// fn non_send() {
//     fn non_send_count_down(mut ns: NonSendMut<Counter>) {
//         ns.0 -= 1;
//     }

//     let mut world = World::new();
//     world.insert_non_send(Counter(10));
//     assert_eq!(*world.non_send::<Counter>(), Counter(10));
//     world.run_system_once(non_send_count_down).unwrap();
//     assert_eq!(*world.non_send::<Counter>(), Counter(9));
// }

// #[test]
// fn run_system_once_invalid_params() {
//     #[derive(Resource)]
//     struct T;

//     fn system(_: Res<T>) {}

//     let mut world = World::default();
//     // This fails because `T` has not been added to the world yet.
//     let result = world.run_system_once(system);

//     assert!(matches!(result, Err(RunSystemError::Failed { .. })));

//     let expected = "Resource does not exist";
//     let actual = result.unwrap_err().to_string();

//     assert!(
//         actual.contains(expected),
//         "Expected error message to contain `{}` but got `{}`",
//         expected,
//         actual
//     );
// }
