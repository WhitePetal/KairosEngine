// use std::{panic::AssertUnwindSafe, sync::{Arc, atomic::{AtomicU32, Ordering}}};

// #[cfg(miri)]
// use alloc::format;

// use crate::ecs::{system::Command, world::{World, command_queue::CommandQueue}};

// struct DropCheck(Arc<AtomicU32>);

// impl DropCheck {
//     fn new() -> (Self, Arc<AtomicU32>) {
//         let drops = Arc::new(AtomicU32::new(0));
//         (Self(drops.clone()), drops)
//     }
// }

// impl Drop for DropCheck {
//     fn drop(&mut self) {
//         self.0.fetch_add(1, Ordering::Relaxed);
//     }
// }

// impl Command for DropCheck {
//     type Out = ();

//     fn apply(self, _: &mut World) {}
// }

// #[test]
// fn test_command_queue_inner_drop() {
//     let mut queue = CommandQueue::default();

//     let (dropcheck_a, drops_a) = DropCheck::new();
//     let (dropcheck_b, drops_b) = DropCheck::new();

//     queue.push(dropcheck_a);
//     queue.push(dropcheck_b);

//     assert_eq!(drops_a.load(Ordering::Relaxed), 0);
//     assert_eq!(drops_b.load(Ordering::Relaxed), 0);

//     let mut world = World::new();
//     queue.apply(&mut world);

//     assert_eq!(drops_a.load(Ordering::Relaxed), 1);
//     assert_eq!(drops_b.load(Ordering::Relaxed), 1);
// }

// /// Asserts that inner [commands](`Command`) are dropped on early drop of [`CommandQueue`].
// /// Originally identified as an issue in [#10676](https://github.com/bevyengine/bevy/issues/10676)
// #[test]
// fn test_command_queue_inner_drop_early() {
//     let mut queue = CommandQueue::default();

//     let (dropcheck_a, drops_a) = DropCheck::new();
//     let (dropcheck_b, drops_b) = DropCheck::new();

//     queue.push(dropcheck_a);
//     queue.push(dropcheck_b);

//     assert_eq!(drops_a.load(Ordering::Relaxed), 0);
//     assert_eq!(drops_b.load(Ordering::Relaxed), 0);

//     drop(queue);

//     assert_eq!(drops_a.load(Ordering::Relaxed), 1);
//     assert_eq!(drops_b.load(Ordering::Relaxed), 1);
// }

// #[derive(Component)]
// struct A;

// struct SpawnCommand;

// impl Command for SpawnCommand {
//     type Out = ();

//     fn apply(self, world: &mut World) {
//         world.spawn(A);
//     }
// }

// #[test]
// fn test_command_queue_inner() {
//     let mut queue = CommandQueue::default();

//     queue.push(SpawnCommand);
//     queue.push(SpawnCommand);

//     let mut world = World::new();
//     queue.apply(&mut world);

//     assert_eq!(world.query::<&A>().query(&world).count(), 2);

//     // The previous call to `apply` cleared the queue.
//     // This call should do nothing.
//     queue.apply(&mut world);
//     assert_eq!(world.query::<&A>().query(&world).count(), 2);
// }

// #[expect(
//     dead_code,
//     reason = "The inner string is used to ensure that, when the PanicCommand gets pushed to the queue, some data is written to the `bytes` vector."
// )]
// struct PanicCommand(String);
// impl Command for PanicCommand {
//     type Out = ();

//     fn apply(self, _: &mut World) {
//         panic!("command is panicking");
//     }
// }

// #[test]
// fn test_command_queue_inner_panic_safe() {
//     std::panic::set_hook(Box::new(|_| {}));

//     let mut queue = CommandQueue::default();

//     queue.push(PanicCommand("I panic!".to_owned()));
//     queue.push(SpawnCommand);

//     let mut world = World::new();

//     let _ = std::panic::catch_unwind(AssertUnwindSafe(|| {
//         queue.apply(&mut world);
//     }));

//     // Even though the first command panicked, it's still ok to push
//     // more commands.
//     queue.push(SpawnCommand);
//     queue.push(SpawnCommand);
//     queue.apply(&mut world);
//     assert_eq!(world.query::<&A>().query(&world).count(), 3);
// }

// #[test]
// fn test_command_queue_inner_nested_panic_safe() {
//     std::panic::set_hook(Box::new(|_| {}));

//     #[derive(Resource, Default)]
//     struct Order(Vec<usize>);

//     let mut world = World::new();
//     world.init_resource::<Order>();

//     fn add_index(index: usize) -> impl Command {
//         move |world: &mut World| world.resource_mut::<Order>().0.push(index)
//     }
//     world.commands().queue(add_index(1));
//     world.commands().queue(|world: &mut World| {
//         world.commands().queue(add_index(2));
//         world.commands().queue(PanicCommand("I panic!".to_owned()));
//         world.commands().queue(add_index(3));
//         world.flush_commands();
//     });
//     world.commands().queue(add_index(4));

//     let _ = std::panic::catch_unwind(AssertUnwindSafe(|| {
//         world.flush_commands();
//     }));

//     world.commands().queue(add_index(5));
//     world.flush_commands();
//     assert_eq!(&world.resource::<Order>().0, &[1, 2, 3, 4, 5]);
// }

// // NOTE: `CommandQueue` is `Send` because `Command` is send.
// // If the `Command` trait gets reworked to be non-send, `CommandQueue`
// // should be reworked.
// // This test asserts that Command types are send.
// fn assert_is_send_impl(_: impl Send) {}
// fn assert_is_send(command: impl Command) {
//     assert_is_send_impl(command);
// }

// #[test]
// fn test_command_is_send() {
//     assert_is_send(SpawnCommand);
// }

// #[expect(
//     dead_code,
//     reason = "This struct is used to test how the CommandQueue reacts to padding added by rust's compiler."
// )]
// struct CommandWithPadding(u8, u16);
// impl Command for CommandWithPadding {
//     type Out = ();

//     fn apply(self, _: &mut World) {}
// }

// #[cfg(miri)]
// #[test]
// fn test_uninit_bytes() {
//     let mut queue = CommandQueue::default();
//     queue.push(CommandWithPadding(0, 0));
//     let _ = format!("{:?}", queue.bytes);
// }
