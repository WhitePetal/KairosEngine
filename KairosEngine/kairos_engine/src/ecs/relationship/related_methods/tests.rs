// use crate::ecs::{entity::Entity, hierarchy::{ChildOf, Children}, world::World};

// #[derive(Component, Clone, Copy)]
// struct TestComponent;

// #[test]
// fn insert_and_remove_recursive() {
//     let mut world = World::new();

//     let a = world.spawn_empty().id();
//     let b = world.spawn(ChildOf(a)).id();
//     let c = world.spawn(ChildOf(a)).id();
//     let d = world.spawn(ChildOf(b)).id();

//     world
//         .entity_mut(a)
//         .insert_recursive::<Children>(TestComponent);

//     for entity in [a, b, c, d] {
//         assert!(world.entity(entity).contains::<TestComponent>());
//     }

//     world
//         .entity_mut(b)
//         .remove_recursive::<Children, TestComponent>();

//     // Parent
//     assert!(world.entity(a).contains::<TestComponent>());
//     // Target
//     assert!(!world.entity(b).contains::<TestComponent>());
//     // Sibling
//     assert!(world.entity(c).contains::<TestComponent>());
//     // Child
//     assert!(!world.entity(d).contains::<TestComponent>());

//     world
//         .entity_mut(a)
//         .remove_recursive::<Children, TestComponent>();

//     for entity in [a, b, c, d] {
//         assert!(!world.entity(entity).contains::<TestComponent>());
//     }
// }

// #[test]
// fn remove_all_related() {
//     let mut world = World::new();

//     let a = world.spawn_empty().id();
//     let b = world.spawn(ChildOf(a)).id();
//     let c = world.spawn(ChildOf(a)).id();

//     world.entity_mut(a).detach_all_related::<ChildOf>();

//     assert_eq!(world.entity(a).get::<Children>(), None);
//     assert_eq!(world.entity(b).get::<ChildOf>(), None);
//     assert_eq!(world.entity(c).get::<ChildOf>(), None);
// }

// #[test]
// fn replace_related_works() {
//     let mut world = World::new();
//     let child1 = world.spawn_empty().id();
//     let child2 = world.spawn_empty().id();
//     let child3 = world.spawn_empty().id();

//     let mut parent = world.spawn_empty();
//     parent.add_children(&[child1, child2]);
//     let child_value = ChildOf(parent.id());
//     let some_child = Some(&child_value);

//     parent.replace_children(&[child2, child3]);
//     let children = parent.get::<Children>().unwrap().collection();
//     assert_eq!(children, &[child2, child3]);
//     assert_eq!(parent.world().get::<ChildOf>(child1), None);
//     assert_eq!(parent.world().get::<ChildOf>(child2), some_child);
//     assert_eq!(parent.world().get::<ChildOf>(child3), some_child);

//     parent.replace_children_with_difference(&[child3], &[child1, child2], &[child1]);
//     let children = parent.get::<Children>().unwrap().collection();
//     assert_eq!(children, &[child1, child2]);
//     assert_eq!(parent.world().get::<ChildOf>(child1), some_child);
//     assert_eq!(parent.world().get::<ChildOf>(child2), some_child);
//     assert_eq!(parent.world().get::<ChildOf>(child3), None);
// }

// #[test]
// fn add_related_keeps_relationship_data() {
//     #[derive(Component, PartialEq, Debug)]
//     #[relationship(relationship_target = Parent)]
//     struct Child {
//         #[relationship]
//         parent: Entity,
//         data: u8,
//     }

//     #[derive(Component)]
//     #[relationship_target(relationship = Child)]
//     struct Parent(Vec<Entity>);

//     let mut world = World::new();
//     let parent1 = world.spawn_empty().id();
//     let parent2 = world.spawn_empty().id();
//     let child = world
//         .spawn(Child {
//             parent: parent1,
//             data: 42,
//         })
//         .id();

//     world.entity_mut(parent2).add_related::<Child>(&[child]);
//     assert_eq!(
//         world.get::<Child>(child),
//         Some(&Child {
//             parent: parent2,
//             data: 42
//         })
//     );
// }

// #[test]
// fn insert_related_keeps_relationship_data() {
//     #[derive(Component, PartialEq, Debug)]
//     #[relationship(relationship_target = Parent)]
//     struct Child {
//         #[relationship]
//         parent: Entity,
//         data: u8,
//     }

//     #[derive(Component)]
//     #[relationship_target(relationship = Child)]
//     struct Parent(Vec<Entity>);

//     let mut world = World::new();
//     let parent1 = world.spawn_empty().id();
//     let parent2 = world.spawn_empty().id();
//     let child = world
//         .spawn(Child {
//             parent: parent1,
//             data: 42,
//         })
//         .id();

//     world
//         .entity_mut(parent2)
//         .insert_related::<Child>(0, &[child]);
//     assert_eq!(
//         world.get::<Child>(child),
//         Some(&Child {
//             parent: parent2,
//             data: 42
//         })
//     );
// }

// #[test]
// fn replace_related_keeps_relationship_data() {
//     #[derive(Component, PartialEq, Debug)]
//     #[relationship(relationship_target = Parent)]
//     struct Child {
//         #[relationship]
//         parent: Entity,
//         data: u8,
//     }

//     #[derive(Component)]
//     #[relationship_target(relationship = Child)]
//     struct Parent(Vec<Entity>);

//     let mut world = World::new();
//     let parent1 = world.spawn_empty().id();
//     let parent2 = world.spawn_empty().id();
//     let child = world
//         .spawn(Child {
//             parent: parent1,
//             data: 42,
//         })
//         .id();

//     world
//         .entity_mut(parent2)
//         .replace_related_with_difference::<Child>(&[], &[child], &[child]);
//     assert_eq!(
//         world.get::<Child>(child),
//         Some(&Child {
//             parent: parent2,
//             data: 42
//         })
//     );

//     world.entity_mut(parent1).replace_related::<Child>(&[child]);
//     assert_eq!(
//         world.get::<Child>(child),
//         Some(&Child {
//             parent: parent1,
//             data: 42
//         })
//     );
// }

// #[test]
// fn replace_related_keeps_relationship_target_data() {
//     #[derive(Component)]
//     #[relationship(relationship_target = Parent)]
//     struct Child(Entity);

//     #[derive(Component)]
//     #[relationship_target(relationship = Child)]
//     struct Parent {
//         #[relationship]
//         children: Vec<Entity>,
//         data: u8,
//     }

//     let mut world = World::new();
//     let child1 = world.spawn_empty().id();
//     let child2 = world.spawn_empty().id();
//     let mut parent = world.spawn_empty();
//     parent.add_related::<Child>(&[child1]);
//     parent.get_mut::<Parent>().unwrap().data = 42;

//     parent.replace_related_with_difference::<Child>(&[child1], &[child2], &[child2]);
//     let data = parent.get::<Parent>().unwrap().data;
//     assert_eq!(data, 42);

//     parent.replace_related::<Child>(&[child1]);
//     let data = parent.get::<Parent>().unwrap().data;
//     assert_eq!(data, 42);
// }

// #[test]
// fn despawn_related_observers_can_access_relationship_data() {
//     use crate::lifecycle::Discard;
//     use crate::observer::On;
//     use crate::prelude::Has;
//     use crate::system::Query;

//     #[derive(Component)]
//     struct MyComponent;

//     #[derive(Component, Default)]
//     struct ObserverResult {
//         success: bool,
//     }

//     let mut world = World::new();
//     let result_entity = world.spawn(ObserverResult::default()).id();

//     world.add_observer(
//         move |replace: On<Discard, MyComponent>,
//               has_relationship: Query<Has<ChildOf>>,
//               mut results: Query<&mut ObserverResult>| {
//             if has_relationship.get(replace.entity).unwrap_or(false) {
//                 results.get_mut(result_entity).unwrap().success = true;
//             }
//         },
//     );

//     let parent = world.spawn_empty().id();
//     let _child = world.spawn((MyComponent, ChildOf(parent))).id();

//     world.entity_mut(parent).despawn_related::<Children>();

//     assert!(world.get::<ObserverResult>(result_entity).unwrap().success);
// }
