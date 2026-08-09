// TODO!
// #[derive(Event)]
// struct TriggerEvent;

// #[test]
// #[should_panic(expected = "I failed!")]
// fn test_fallible_observer() {
//     fn system(_: On<TriggerEvent>) -> Result {
//         Err("I failed!".into())
//     }

//     let mut world = World::default();
//     world.add_observer(system);
//     Schedule::default().run(&mut world);
//     world.trigger(TriggerEvent);
// }

// #[test]
// fn test_fallible_observer_ignored_errors() {
//     #[derive(Resource, Default)]
//     struct Ran(bool);

//     fn system(_: On<TriggerEvent>, mut ran: ResMut<Ran>) -> Result {
//         ran.0 = true;
//         Err("I failed!".into())
//     }

//     // Using observer error handler
//     let mut world = World::default();
//     world.init_resource::<Ran>();
//     world.spawn(Observer::new(system).with_error_handler(ignore));
//     world.trigger(TriggerEvent);
//     assert!(world.resource::<Ran>().0);

//     // Using world error handler
//     let mut world = World::default();
//     world.init_resource::<Ran>();
//     world.spawn(Observer::new(system));
//     // Test that the correct handler is used when the observer was added
//     // before the fallback handler
//     world.insert_resource(FallbackErrorHandler(ignore));
//     world.trigger(TriggerEvent);
//     assert!(world.resource::<Ran>().0);
// }

// #[test]
// #[should_panic]
// fn exclusive_system_cannot_be_observer() {
//     fn system(_: On<TriggerEvent>, _world: &mut World) {}
//     let mut world = World::default();
//     world.add_observer(system);
// }
