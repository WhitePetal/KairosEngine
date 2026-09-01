// use smallvec::SmallVec;

// use crate::ecs::{entity::Entity, world::World};

// #[test]
// fn vec_relationship_source_collection() {
//     #[derive(Component)]
//     #[relationship(relationship_target = RelTarget)]
//     struct Rel(Entity);

//     #[derive(Component)]
//     #[relationship_target(relationship = Rel, linked_spawn)]
//     struct RelTarget(Vec<Entity>);

//     let mut world = World::new();
//     let a = world.spawn_empty().id();
//     let b = world.spawn_empty().id();

//     world.entity_mut(a).insert(Rel(b));

//     let rel_target = world.get::<RelTarget>(b).unwrap();
//     let collection = rel_target.collection();
//     assert_eq!(collection, &vec!(a));
// }

// #[test]
// fn smallvec_relationship_source_collection() {
//     #[derive(Component)]
//     #[relationship(relationship_target = RelTarget)]
//     struct Rel(Entity);

//     #[derive(Component)]
//     #[relationship_target(relationship = Rel, linked_spawn)]
//     struct RelTarget(SmallVec<[Entity; 4]>);

//     let mut world = World::new();
//     let a = world.spawn_empty().id();
//     let b = world.spawn_empty().id();

//     world.entity_mut(a).insert(Rel(b));

//     let rel_target = world.get::<RelTarget>(b).unwrap();
//     let collection = rel_target.collection();
//     assert_eq!(collection, &SmallVec::from_buf([a]));
// }

// #[test]
// fn entity_relationship_source_collection() {
//     #[derive(Component)]
//     #[relationship(relationship_target = RelTarget)]
//     struct Rel(Entity);

//     #[derive(Component)]
//     #[relationship_target(relationship = Rel)]
//     struct RelTarget(Entity);

//     let mut world = World::new();
//     let a = world.spawn_empty().id();
//     let b = world.spawn_empty().id();

//     world.entity_mut(a).insert(Rel(b));

//     let rel_target = world.get::<RelTarget>(b).unwrap();
//     let collection = rel_target.collection();
//     assert_eq!(collection, &a);
// }

// #[test]
// fn one_to_one_relationships() {
//     #[derive(Component)]
//     #[relationship(relationship_target = Below)]
//     struct Above(Entity);

//     #[derive(Component)]
//     #[relationship_target(relationship = Above)]
//     struct Below(Entity);

//     let mut world = World::new();
//     let a = world.spawn_empty().id();
//     let b = world.spawn_empty().id();

//     world.entity_mut(a).insert(Above(b));
//     assert_eq!(a, world.get::<Below>(b).unwrap().0);

//     // Verify removing target removes relationship
//     world.entity_mut(b).remove::<Below>();
//     assert!(world.get::<Above>(a).is_none());

//     // Verify removing relationship removes target
//     world.entity_mut(a).insert(Above(b));
//     world.entity_mut(a).remove::<Above>();
//     assert!(world.get::<Below>(b).is_none());

//     // Actually - a is above c now! Verify relationship was updated correctly
//     let c = world.spawn_empty().id();
//     world.entity_mut(a).insert(Above(c));
//     assert!(world.get::<Below>(b).is_none());
//     assert_eq!(a, world.get::<Below>(c).unwrap().0);
// }

// #[test]
// fn entity_index_map() {
//     for add_before in [false, true] {
//         #[derive(Component)]
//         #[relationship(relationship_target = RelTarget)]
//         struct Rel(Entity);

//         #[derive(Component)]
//         #[relationship_target(relationship = Rel, linked_spawn)]
//         struct RelTarget(Vec<Entity>);

//         let mut world = World::new();
//         if add_before {
//             let _ = world.spawn_empty().id();
//         }
//         let a = world.spawn_empty().id();
//         let b = world.spawn_empty().id();
//         let c = world.spawn_empty().id();
//         let d = world.spawn_empty().id();

//         world.entity_mut(a).add_related::<Rel>(&[b, c, d]);

//         let rel_target = world.get::<RelTarget>(a).unwrap();
//         let collection = rel_target.collection();

//         // Insertions should maintain ordering
//         assert!(collection.iter().eq([b, c, d]));

//         world.entity_mut(c).despawn();

//         let rel_target = world.get::<RelTarget>(a).unwrap();
//         let collection = rel_target.collection();

//         // Removals should maintain ordering
//         assert!(collection.iter().eq([b, d]));
//     }
// }

// #[test]
// fn one_to_one_relationship_shared_target() {
//     #[derive(Component)]
//     #[relationship(relationship_target = Below)]
//     struct Above(Entity);

//     #[derive(Component)]
//     #[relationship_target(relationship = Above)]
//     struct Below(Entity);
//     let mut world = World::new();
//     let a = world.spawn_empty().id();
//     let b = world.spawn_empty().id();
//     let c = world.spawn_empty().id();

//     world.entity_mut(a).insert(Above(c));
//     world.entity_mut(b).insert(Above(c));

//     // The original relationship (a -> c) should be removed and the new relationship (b -> c) should be established
//     assert!(
//         world.get::<Above>(a).is_none(),
//         "Original relationship should be removed"
//     );
//     assert_eq!(
//         world.get::<Above>(b).unwrap().0,
//         c,
//         "New relationship should be established"
//     );
//     assert_eq!(
//         world.get::<Below>(c).unwrap().0,
//         b,
//         "Target should point to new source"
//     );
// }

// #[test]
// fn one_to_one_relationship_reinsert() {
//     #[derive(Component)]
//     #[relationship(relationship_target = Below)]
//     struct Above(Entity);

//     #[derive(Component)]
//     #[relationship_target(relationship = Above)]
//     struct Below(Entity);

//     let mut world = World::new();
//     let a = world.spawn_empty().id();
//     let b = world.spawn_empty().id();

//     world.entity_mut(a).insert(Above(b));
//     world.entity_mut(a).insert(Above(b));
// }
