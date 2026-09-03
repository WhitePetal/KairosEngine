// use std::marker::PhantomData;

// use kairos_ecs_macros::Resource;

// use crate::ecs::{system::Local, world::World};

// #[test]
// fn test_exclusive_system_params() {
//     #[derive(Resource, Default)]
//     struct Res {
//         test_value: u32,
//     }

//     fn my_system(world: &mut World, mut local: Local<u32>, _phantom: PhantomData<Vec<u32>>) {
//         assert_eq!(world.resource::<Res>().test_value, *local);
//         *local += 1;
//         world.resource_mut::<Res>().test_value += 1;
//     }

//     let mut schedule = Schedule::default();
//     schedule.add_systems(my_system);

//     let mut world = World::default();
//     world.init_resource::<Res>();

//     schedule.run(&mut world);
//     schedule.run(&mut world);

//     assert_eq!(2, world.get_resource::<Res>().unwrap().test_value);
// }
