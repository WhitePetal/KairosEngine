// use std::cell::Cell;

// use kairos_ecs_macros::Event;

// use crate::ecs::{change_detection::{Res, ResMut}, error::Result, system::{Commands, In, InMut, InRef, IntoSystem, Local, Query, RegisteredSystemError, SystemId}, world::World};

// #[derive(Resource, Default, PartialEq, Debug)]
// struct Counter(u8);

// #[test]
// fn change_detection() {
//     #[derive(Resource, Default)]
//     struct ChangeDetector;

//     fn count_up_iff_changed(
//         mut counter: ResMut<Counter>,
//         change_detector: ResMut<ChangeDetector>,
//     ) {
//         if change_detector.is_changed() {
//             counter.0 += 1;
//         }
//     }

//     let mut world = World::new();
//     world.init_resource::<ChangeDetector>();
//     world.init_resource::<Counter>();
//     assert_eq!(*world.resource::<Counter>(), Counter(0));
//     // Resources are changed when they are first added.
//     let id = world.register_system(count_up_iff_changed);
//     world.run_system(id).expect("system runs successfully");
//     assert_eq!(*world.resource::<Counter>(), Counter(1));
//     // Nothing changed
//     world.run_system(id).expect("system runs successfully");
//     assert_eq!(*world.resource::<Counter>(), Counter(1));
//     // Making a change
//     world.resource_mut::<ChangeDetector>().set_changed();
//     world.run_system(id).expect("system runs successfully");
//     assert_eq!(*world.resource::<Counter>(), Counter(2));
// }

// #[test]
// fn local_variables() {
//     // The `Local` begins at the default value of 0
//     fn doubling(last_counter: Local<Counter>, mut counter: ResMut<Counter>) {
//         counter.0 += last_counter.0 .0;
//         last_counter.0 .0 = counter.0;
//     }

//     let mut world = World::new();
//     world.insert_resource(Counter(1));
//     assert_eq!(*world.resource::<Counter>(), Counter(1));
//     let id = world.register_system(doubling);
//     world.run_system(id).expect("system runs successfully");
//     assert_eq!(*world.resource::<Counter>(), Counter(1));
//     world.run_system(id).expect("system runs successfully");
//     assert_eq!(*world.resource::<Counter>(), Counter(2));
//     world.run_system(id).expect("system runs successfully");
//     assert_eq!(*world.resource::<Counter>(), Counter(4));
//     world.run_system(id).expect("system runs successfully");
//     assert_eq!(*world.resource::<Counter>(), Counter(8));
// }

// #[test]
// fn input_values() {
//     // Verify that a non-Copy, non-Clone type can be passed in.
//     struct NonCopy(u8);

//     fn increment_sys(In(NonCopy(increment_by)): In<NonCopy>, mut counter: ResMut<Counter>) {
//         counter.0 += increment_by;
//     }

//     let mut world = World::new();

//     let id = world.register_system(increment_sys);

//     // Insert the resource after registering the system.
//     world.insert_resource(Counter(1));
//     assert_eq!(*world.resource::<Counter>(), Counter(1));

//     world
//         .run_system_with(id, NonCopy(1))
//         .expect("system runs successfully");
//     assert_eq!(*world.resource::<Counter>(), Counter(2));

//     world
//         .run_system_with(id, NonCopy(1))
//         .expect("system runs successfully");
//     assert_eq!(*world.resource::<Counter>(), Counter(3));

//     world
//         .run_system_with(id, NonCopy(20))
//         .expect("system runs successfully");
//     assert_eq!(*world.resource::<Counter>(), Counter(23));

//     world
//         .run_system_with(id, NonCopy(1))
//         .expect("system runs successfully");
//     assert_eq!(*world.resource::<Counter>(), Counter(24));
// }

// #[test]
// fn output_values() {
//     // Verify that a non-Copy, non-Clone type can be returned.
//     #[derive(Eq, PartialEq, Debug)]
//     struct NonCopy(u8);

//     fn increment_sys(mut counter: ResMut<Counter>) -> NonCopy {
//         counter.0 += 1;
//         NonCopy(counter.0)
//     }

//     let mut world = World::new();

//     let id = world.register_system(increment_sys);

//     // Insert the resource after registering the system.
//     world.insert_resource(Counter(1));
//     assert_eq!(*world.resource::<Counter>(), Counter(1));

//     let output = world.run_system(id).expect("system runs successfully");
//     assert_eq!(*world.resource::<Counter>(), Counter(2));
//     assert_eq!(output, NonCopy(2));

//     let output = world.run_system(id).expect("system runs successfully");
//     assert_eq!(*world.resource::<Counter>(), Counter(3));
//     assert_eq!(output, NonCopy(3));
// }

// #[test]
// fn fallible_system() {
//     fn sys() -> Result<()> {
//         Err("error")?;
//         Ok(())
//     }

//     let mut world = World::new();
//     let fallible_system_id = world.register_system(sys);
//     let output = world.run_system(fallible_system_id);
//     assert!(matches!(output, Ok(Err(_))));
// }

// #[test]
// fn exclusive_system() {
//     let mut world = World::new();
//     let exclusive_system_id = world.register_system(|world: &mut World| {
//         world.spawn_empty();
//     });
//     let entity_count = world.entities.count_spawned();
//     let _ = world.run_system(exclusive_system_id);
//     assert_eq!(world.entities.count_spawned(), entity_count + 1);
// }

// #[test]
// fn nested_systems() {
//     use crate::ecs::system::SystemId;

//     #[derive(Component)]
//     struct Callback(SystemId);

//     fn nested(query: Query<&Callback>, mut commands: Commands) {
//         for callback in query.iter() {
//             commands.run_system(callback.0);
//         }
//     }

//     let mut world = World::new();
//     world.insert_resource(Counter(0));

//     let increment_two = world.register_system(|mut counter: ResMut<Counter>| {
//         counter.0 += 2;
//     });
//     let increment_three = world.register_system(|mut counter: ResMut<Counter>| {
//         counter.0 += 3;
//     });
//     let nested_id = world.register_system(nested);

//     world.spawn(Callback(increment_two));
//     world.spawn(Callback(increment_three));
//     let _ = world.run_system(nested_id);
//     assert_eq!(*world.resource::<Counter>(), Counter(5));
// }

// #[test]
// fn nested_systems_with_inputs() {
//     use crate::ecs::system::SystemId;

//     #[derive(Component)]
//     struct Callback(SystemId<In<u8>>, u8);

//     fn nested(query: Query<&Callback>, mut commands: Commands) {
//         for callback in query.iter() {
//             commands.run_system_with(callback.0, callback.1);
//         }
//     }

//     let mut world = World::new();
//     world.insert_resource(Counter(0));

//     let increment_by =
//         world.register_system(|In(amt): In<u8>, mut counter: ResMut<Counter>| {
//             counter.0 += amt;
//         });
//     let nested_id = world.register_system(nested);

//     world.spawn(Callback(increment_by, 2));
//     world.spawn(Callback(increment_by, 3));
//     let _ = world.run_system(nested_id);
//     assert_eq!(*world.resource::<Counter>(), Counter(5));
// }

// #[test]
// fn cached_system() {
//     use crate::ecs::system::RegisteredSystemError;

//     fn four() -> i32 {
//         4
//     }

//     let mut world = World::new();
//     let old = world.register_system_cached(four);
//     let new = world.register_system_cached(four);
//     assert_eq!(old, new);

//     let result = world.unregister_system_cached(four);
//     assert!(result.is_ok());
//     let new = world.register_system_cached(four);
//     assert_ne!(old, new);

//     let output = world.run_system(old);
//     assert!(matches!(
//         output,
//         Err(RegisteredSystemError::SystemIdNotRegistered(x)) if x == old,
//     ));
//     let output = world.run_system(new);
//     assert!(matches!(output, Ok(x) if x == four()));
//     let output = world.run_system_cached(four);
//     assert!(matches!(output, Ok(x) if x == four()));
//     let output = world.run_system_cached_with(four, ());
//     assert!(matches!(output, Ok(x) if x == four()));
// }

// #[test]
// fn cached_fallible_system() {
//     fn sys() -> Result<()> {
//         Err("error")?;
//         Ok(())
//     }

//     let mut world = World::new();
//     let fallible_system_id = world.register_system_cached(sys);
//     let output = world.run_system(fallible_system_id);
//     assert!(matches!(output, Ok(Err(_))));
//     let output = world.run_system_cached(sys);
//     assert!(matches!(output, Ok(Err(_))));
//     let output = world.run_system_cached_with(sys, ());
//     assert!(matches!(output, Ok(Err(_))));
// }

// #[test]
// fn cached_system_commands() {
//     fn sys(mut counter: ResMut<Counter>) {
//         counter.0 += 1;
//     }

//     let mut world = World::new();
//     world.insert_resource(Counter(0));
//     world.commands().run_system_cached(sys);
//     world.flush_commands();
//     assert_eq!(world.resource::<Counter>().0, 1);
//     world.commands().run_system_cached_with(sys, ());
//     world.flush_commands();
//     assert_eq!(world.resource::<Counter>().0, 2);
// }

// #[test]
// fn cached_fallible_system_commands() {
//     fn sys(mut counter: ResMut<Counter>) -> Result {
//         counter.0 += 1;
//         Ok(())
//     }

//     let mut world = World::new();
//     world.insert_resource(Counter(0));
//     world.commands().run_system_cached(sys);
//     world.flush_commands();
//     assert_eq!(world.resource::<Counter>().0, 1);
//     world.commands().run_system_cached_with(sys, ());
//     world.flush_commands();
//     assert_eq!(world.resource::<Counter>().0, 2);
// }

// #[test]
// #[should_panic(expected = "This system always fails")]
// fn cached_fallible_system_commands_can_fail() {
//     use crate::ecs::system::command;
//     fn sys() -> Result {
//         Err("This system always fails".into())
//     }

//     let mut world = World::new();
//     world.commands().queue(command::run_system_cached(sys));
//     world.flush_commands();
// }

// #[test]
// fn cached_system_adapters() {
//     fn four() -> i32 {
//         4
//     }

//     fn double(In(i): In<i32>) -> i32 {
//         i * 2
//     }

//     let mut world = World::new();

//     let output = world.run_system_cached(four.pipe(double));
//     assert!(matches!(output, Ok(8)));

//     let output = world.run_system_cached(four.map(|i| i * 2));
//     assert!(matches!(output, Ok(8)));
// }

// #[test]
// fn cached_system_into_same_system_type() {
//     struct Foo;
//     impl IntoSystem<(), (), ()> for Foo {
//         type System = ApplyDeferred;
//         fn into_system(_: Self) -> Self::System {
//             ApplyDeferred
//         }
//     }

//     struct Bar;
//     impl IntoSystem<(), (), ()> for Bar {
//         type System = ApplyDeferred;
//         fn into_system(_: Self) -> Self::System {
//             ApplyDeferred
//         }
//     }

//     let mut world = World::new();
//     let foo1 = world.register_system_cached(Foo);
//     let foo2 = world.register_system_cached(Foo);
//     let bar1 = world.register_system_cached(Bar);
//     let bar2 = world.register_system_cached(Bar);

//     // The `S: IntoSystem` types are different, so they should be cached
//     // as separate systems, even though the `<S as IntoSystem>::System`
//     // types / values are the same (`ApplyDeferred`).
//     assert_ne!(foo1, bar1);

//     // But if the `S: IntoSystem` types are the same, they'll be cached
//     // as the same system.
//     assert_eq!(foo1, foo2);
//     assert_eq!(bar1, bar2);
// }

// #[test]
// fn system_with_input_ref() {
//     fn with_ref(InRef(input): InRef<u8>, mut counter: ResMut<Counter>) {
//         counter.0 += *input;
//     }

//     let mut world = World::new();
//     world.insert_resource(Counter(0));

//     let id = world.register_system(with_ref);
//     world.run_system_with(id, &2).unwrap();
//     assert_eq!(*world.resource::<Counter>(), Counter(2));
// }

// #[test]
// fn system_with_input_mut() {
//     #[derive(Event)]
//     struct MyEvent {
//         cancelled: bool,
//     }

//     fn post(InMut(event): InMut<MyEvent>, counter: ResMut<Counter>) {
//         if counter.0 > 0 {
//             event.cancelled = true;
//         }
//     }

//     let mut world = World::new();
//     world.insert_resource(Counter(0));
//     let post_system = world.register_system(post);

//     let mut event = MyEvent { cancelled: false };
//     world.run_system_with(post_system, &mut event).unwrap();
//     assert!(!event.cancelled);

//     world.resource_mut::<Counter>().0 = 1;
//     world.run_system_with(post_system, &mut event).unwrap();
//     assert!(event.cancelled);
// }

// #[test]
// fn run_system_invalid_params() {
//     use crate::ecs::system::RegisteredSystemError;

//     #[derive(Resource)]
//     struct T;

//     fn system(_: Res<T>) {}

//     let mut world = World::new();
//     let id = world.register_system(system);
//     // This fails because `T` has not been added to the world yet.
//     let result = world.run_system(id);

//     assert!(matches!(result, Err(RegisteredSystemError::Failed { .. })));
//     let expected = "does not exist";
//     let actual = result.unwrap_err().to_string();

//     assert!(
//         actual.contains(expected),
//         "Expected error message to contain `{}` but got `{}`",
//         expected,
//         actual
//     );
// }

// #[test]
// fn run_system_recursive() {
//     std::thread_local! {
//         static INVOCATIONS_LEFT: Cell<i32> = const { Cell::new(3) };
//         static SYSTEM_ID: Cell<Option<SystemId>> = default();
//     }

//     fn system(mut commands: Commands) {
//         let count = INVOCATIONS_LEFT.get() - 1;
//         INVOCATIONS_LEFT.set(count);
//         if count > 0 {
//             commands.run_system(SYSTEM_ID.get().unwrap());
//         }
//     }

//     let mut world = World::new();
//     let id = world.register_system(system);
//     SYSTEM_ID.set(Some(id));
//     world.run_system(id).unwrap();

//     assert_eq!(INVOCATIONS_LEFT.get(), 0);
// }

// #[test]
// fn run_system_exclusive_adapters() {
//     let mut world = World::new();
//     fn system(_: &mut World) {}
//     world.run_system_cached(system).unwrap();
//     world.run_system_cached(system.pipe(system)).unwrap();
//     world.run_system_cached(system.map(|()| {})).unwrap();
// }

// #[test]
// fn wrong_system_type() {
//     fn test() -> Result<(), u8> {
//         Ok(())
//     }

//     let mut world = World::new();

//     let entity = world.register_system_cached(test).entity();

//     match world.run_system::<u8>(SystemId::from_entity(entity)) {
//         Ok(_) => panic!("Should fail since called `run_system` with wrong SystemId type."),
//         Err(RegisteredSystemError::IncorrectType(_, _)) => (),
//         Err(err) => panic!("Failed with wrong error. `{:?}`", err),
//     }
// }

// #[test]
// fn despawn_unused() {
//     let mut world = World::new();

//     fn system() {}

//     let handle = world.register_tracked_system(system);
//     let entity = handle.entity();
//     drop(handle);

//     assert!(world.get_entity(entity).is_ok());

//     world
//         .run_system_cached(despawn_unused_registered_systems)
//         .unwrap();

//     assert!(world.get_entity(entity).is_err());
// }

// #[test]
// fn system_handle_template() {
//     fn my_system() {}

//     let mut world = World::new();

//     {
//         let my_system_handle = world.register_tracked_system(my_system);
//         let system_handle = world
//             .spawn_empty()
//             .build_template(&SystemHandleTemplate::Handle(my_system_handle.clone()))
//             .unwrap();
//         assert_eq!(system_handle, my_system_handle);
//     }

//     {
//         let template = system_value(my_system);

//         let a = world.spawn_empty().build_template(&template).unwrap();
//         let b = world.spawn_empty().build_template(&template).unwrap();

//         assert!(matches!(a, SystemHandle::Strong(_)));
//         assert!(matches!(b, SystemHandle::Strong(_)));

//         assert_eq!(a, b);
//     }
// }

// #[test]
// fn run_system_with_owned_system_handle() {
//     fn increment(mut counter: ResMut<Counter>) {
//         counter.0 += 1;
//     }

//     let mut world = World::new();
//     world.insert_resource(Counter(0));

//     let handle = world.register_tracked_system(increment);
//     world.run_system(handle).expect("system runs successfully");

//     assert_eq!(*world.resource::<Counter>(), Counter(1));
// }
