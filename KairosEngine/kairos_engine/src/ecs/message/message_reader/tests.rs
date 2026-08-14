use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

use crate::ecs::{message::PopulatedMessageReader, world::World};


// #[test]
// fn test_populated_message_reader() {
//     let system_ran = Arc::new(AtomicBool::new(false));

//     let mut world = World::new();
//     MessageRegistry::register_message::<TheMessage>(&mut world);

//     let mut schedule = Schedule::default();
//     schedule.add_systems({
//         let system_ran = system_ran.clone();
//         move |mut _reader: PopulatedMessageReader<TheMessage>| {
//             system_ran.store(true, Ordering::SeqCst);
//         }
//     });

//     schedule.run(&mut world);
//     assert!(
//         !system_ran.load(Ordering::SeqCst),
//         "system with PopulatedMessageReader should have been skipped"
//     );

//     world.write_message(TheMessage);
//     schedule.run(&mut world);
//     assert!(
//         system_ran.load(Ordering::SeqCst),
//         "system with PopulatedMessageReader should NOT have been skipped"
//     );

//     #[derive(Message)]
//     struct TheMessage;
// }
