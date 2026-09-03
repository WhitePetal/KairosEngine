// use crate::ecs::{
//     hierarchy::{ChildOf, Children},
//     name::Name,
//     relationship::RelatedSpawner,
//     spawn::{Spawn, SpawnIter, SpawnWith, WithOneRelated, WithRelated},
//     world::World,
// };

// #[test]
// fn spawn() {
//     let mut world = World::new();

//     let parent = world
//         .spawn((
//             Name::new("Parent"),
//             Children::spawn(Spawn(Name::new("Child1"))),
//         ))
//         .id();

//     let children = world
//         .query::<&Children>()
//         .get(&world, parent)
//         .expect("An entity with Children should exist");

//     assert_eq!(children.iter().count(), 1);

//     for ChildOf(child) in world.query::<&ChildOf>().iter(&world) {
//         assert_eq!(child, &parent);
//     }
// }

// #[test]
// fn spawn_iter() {
//     let mut world = World::new();

//     let parent = world
//         .spawn((
//             Name::new("Parent"),
//             Children::spawn(SpawnIter(["Child1", "Child2"].into_iter().map(Name::new))),
//         ))
//         .id();

//     let children = world
//         .query::<&Children>()
//         .get(&world, parent)
//         .expect("An entity with Children should exist");

//     assert_eq!(children.iter().count(), 2);

//     for ChildOf(child) in world.query::<&ChildOf>().iter(&world) {
//         assert_eq!(child, &parent);
//     }
// }

// #[test]
// fn spawn_with() {
//     let mut world = World::new();

//     let parent = world
//         .spawn((
//             Name::new("Parent"),
//             Children::spawn(SpawnWith(|parent: &mut RelatedSpawner<ChildOf>| {
//                 parent.spawn(Name::new("Child1"));
//             })),
//         ))
//         .id();

//     let children = world
//         .query::<&Children>()
//         .get(&world, parent)
//         .expect("An entity with Children should exist");

//     assert_eq!(children.iter().count(), 1);

//     for ChildOf(child) in world.query::<&ChildOf>().iter(&world) {
//         assert_eq!(child, &parent);
//     }
// }

// #[test]
// fn with_related() {
//     let mut world = World::new();

//     let child1 = world.spawn(Name::new("Child1")).id();
//     let child2 = world.spawn(Name::new("Child2")).id();

//     let parent = world
//         .spawn((
//             Name::new("Parent"),
//             Children::spawn(WithRelated::new([child1, child2])),
//         ))
//         .id();

//     let children = world
//         .query::<&Children>()
//         .get(&world, parent)
//         .expect("An entity with Children should exist");

//     assert_eq!(children.iter().count(), 2);

//     assert_eq!(
//         world.entity(child1).get::<ChildOf>(),
//         Some(&ChildOf(parent))
//     );
//     assert_eq!(
//         world.entity(child2).get::<ChildOf>(),
//         Some(&ChildOf(parent))
//     );
// }

// #[test]
// fn with_one_related() {
//     let mut world = World::new();

//     let child1 = world.spawn(Name::new("Child1")).id();

//     let parent = world
//         .spawn((Name::new("Parent"), Children::spawn(WithOneRelated(child1))))
//         .id();

//     let children = world
//         .query::<&Children>()
//         .get(&world, parent)
//         .expect("An entity with Children should exist");

//     assert_eq!(children.iter().count(), 1);

//     assert_eq!(
//         world.entity(child1).get::<ChildOf>(),
//         Some(&ChildOf(parent))
//     );
// }
