// use kairos_ecs_macros::Event;

// use crate::ecs::{
//     observer::On,
//     system::{In, IntoSystem},
//     world::World,
// };

// #[derive(Event)]
// struct TriggerEvent;

// #[test]
// fn test_piped_observer_systems_no_input() {
//     fn a(_: On<TriggerEvent>) {}
//     fn b() {}

//     let mut world = World::new();
//     world.add_observer(a.pipe(b));
// }

// #[test]
// fn test_piped_observer_systems_with_inputs() {
//     fn a(_: On<TriggerEvent>) -> u32 {
//         3
//     }
//     fn b(_: In<u32>) {}

//     let mut world = World::new();
//     world.add_observer(a.pipe(b));
// }
