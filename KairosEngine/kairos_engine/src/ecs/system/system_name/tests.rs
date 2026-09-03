// use crate::ecs::{
//     system::{IntoSystem, SystemName},
//     world::World,
// };

// #[test]
// fn test_system_name_regular_param() {
//     fn testing(name: SystemName) -> String {
//         name.name().as_string()
//     }

//     let mut world = World::default();
//     let id = world.register_system(testing);
//     let name = world.run_system(id).unwrap();
//     assert!(name.ends_with("testing"));
// }

// #[test]
// fn test_system_name_exclusive_param() {
//     fn testing(_world: &mut World, name: SystemName) -> String {
//         name.name().as_string()
//     }

//     let mut world = World::default();
//     let id = world.register_system(testing);
//     let name = world.run_system(id).unwrap();
//     assert!(name.ends_with("testing"));
// }

// #[test]
// fn test_closure_system_name_regular_param() {
//     let mut world = World::default();
//     let system =
//         IntoSystem::into_system(|name: SystemName| name.name().to_owned()).with_name("testing");
//     let name = world.run_system_once(system).unwrap().as_string();
//     assert_eq!(name, "testing");
// }

// #[test]
// fn test_exclusive_closure_system_name_regular_param() {
//     let mut world = World::default();
//     let system =
//         IntoSystem::into_system(|_world: &mut World, name: SystemName| name.name().to_owned())
//             .with_name("testing");
//     let name = world.run_system_once(system).unwrap().as_string();
//     assert_eq!(name, "testing");
// }
