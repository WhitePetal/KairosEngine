// use kairos_ecs_macros::Component;

// use crate::ecs::{entity::Entity, world::World};

// #[derive(Component, Debug, PartialEq, PartialOrd, Clone, Copy)]
// struct A(f32);
// #[derive(Component, Debug, Eq, PartialEq, Clone, Copy)]
// #[component(storage = "SparseSet")]
// struct Sparse(usize);

// #[derive(Component)]
// struct Marker;

// #[test]
// #[cfg_attr(miri, ignore = "This test takes ~70s on CI")]
// fn query_iter_sorts() {
//     let mut world = World::new();
//     for i in 0..100 {
//         world.spawn((A(i as f32), Marker));
//         world.spawn((A(i as f32), Sparse(i), Marker));
//         world.spawn((Sparse(i), Marker));
//     }

//     let mut query = world.query_filtered::<Entity, With<Marker>>();

//     let sort = query.iter(&world).sort::<Entity>().collect::<Vec<_>>();
//     assert_eq!(sort.len(), 300);

//     let sort_unstable = query
//         .iter(&world)
//         .sort_unstable::<Entity>()
//         .collect::<Vec<_>>();

//     let sort_by = query
//         .iter(&world)
//         .sort_by::<Entity>(Ord::cmp)
//         .collect::<Vec<_>>();

//     let sort_unstable_by = query
//         .iter(&world)
//         .sort_unstable_by::<Entity>(Ord::cmp)
//         .collect::<Vec<_>>();

//     let sort_by_key = query
//         .iter(&world)
//         .sort_by_key::<Entity, _>(|&e| e)
//         .collect::<Vec<_>>();

//     let sort_unstable_by_key = query
//         .iter(&world)
//         .sort_unstable_by_key::<Entity, _>(|&e| e)
//         .collect::<Vec<_>>();

//     let sort_by_cached_key = query
//         .iter(&world)
//         .sort_by_cached_key::<Entity, _>(|&e| e)
//         .collect::<Vec<_>>();

//     let mut sort_v2 = query.iter(&world).collect::<Vec<_>>();
//     sort_v2.sort();

//     let mut sort_unstable_v2 = query.iter(&world).collect::<Vec<_>>();
//     sort_unstable_v2.sort_unstable();

//     let mut sort_by_v2 = query.iter(&world).collect::<Vec<_>>();
//     sort_by_v2.sort_by(Ord::cmp);

//     let mut sort_unstable_by_v2 = query.iter(&world).collect::<Vec<_>>();
//     sort_unstable_by_v2.sort_unstable_by(Ord::cmp);

//     let mut sort_by_key_v2 = query.iter(&world).collect::<Vec<_>>();
//     sort_by_key_v2.sort_by_key(|&e| e);

//     let mut sort_unstable_by_key_v2 = query.iter(&world).collect::<Vec<_>>();
//     sort_unstable_by_key_v2.sort_unstable_by_key(|&e| e);

//     let mut sort_by_cached_key_v2 = query.iter(&world).collect::<Vec<_>>();
//     sort_by_cached_key_v2.sort_by_cached_key(|&e| e);

//     assert_eq!(sort, sort_v2);
//     assert_eq!(sort_unstable, sort_unstable_v2);
//     assert_eq!(sort_by, sort_by_v2);
//     assert_eq!(sort_unstable_by, sort_unstable_by_v2);
//     assert_eq!(sort_by_key, sort_by_key_v2);
//     assert_eq!(sort_unstable_by_key, sort_unstable_by_key_v2);
//     assert_eq!(sort_by_cached_key, sort_by_cached_key_v2);
// }

// #[test]
// #[should_panic]
// fn query_iter_sort_after_next() {
//     let mut world = World::new();
//     world.spawn((A(0.),));
//     world.spawn((A(1.1),));
//     world.spawn((A(2.22),));

//     {
//         let mut query = world.query::<&A>();
//         let mut iter = query.iter(&world);
//         println!(
//             "archetype_entities: {} table_entities: {} current_len: {} current_row: {}",
//             iter.cursor.archetype_entities.len(),
//             iter.cursor.table_entities.len(),
//             iter.cursor.current_len,
//             iter.cursor.current_row
//         );
//         _ = iter.next();
//         println!(
//             "archetype_entities: {} table_entities: {} current_len: {} current_row: {}",
//             iter.cursor.archetype_entities.len(),
//             iter.cursor.table_entities.len(),
//             iter.cursor.current_len,
//             iter.cursor.current_row
//         );
//         println!("{}", iter.sort::<Entity>().len());
//     }
// }

// #[test]
// #[should_panic]
// fn query_iter_sort_after_next_dense() {
//     let mut world = World::new();
//     world.spawn((Sparse(11),));
//     world.spawn((Sparse(22),));
//     world.spawn((Sparse(33),));

//     {
//         let mut query = world.query::<&Sparse>();
//         let mut iter = query.iter(&world);
//         println!(
//             "before_next_call: archetype_entities: {} table_entities: {} current_len: {} current_row: {}",
//             iter.cursor.archetype_entities.len(),
//             iter.cursor.table_entities.len(),
//             iter.cursor.current_len,
//             iter.cursor.current_row
//         );
//         _ = iter.next();
//         println!(
//             "after_next_call: archetype_entities: {} table_entities: {} current_len: {} current_row: {}",
//             iter.cursor.archetype_entities.len(),
//             iter.cursor.table_entities.len(),
//             iter.cursor.current_len,
//             iter.cursor.current_row
//         );
//         println!("{}", iter.sort::<Entity>().len());
//     }
// }

// #[test]
// fn empty_query_iter_sort_after_next_does_not_panic() {
//     let mut world = World::new();
//     {
//         let mut query = world.query::<(&A, &Sparse)>();
//         let mut iter = query.iter(&world);
//         println!(
//             "before_next_call: archetype_entities: {} table_entities: {} current_len: {} current_row: {}",
//             iter.cursor.archetype_entities.len(),
//             iter.cursor.table_entities.len(),
//             iter.cursor.current_len,
//             iter.cursor.current_row
//         );
//         _ = iter.next();
//         println!(
//             "after_next_call: archetype_entities: {} table_entities: {} current_len: {} current_row: {}",
//             iter.cursor.archetype_entities.len(),
//             iter.cursor.table_entities.len(),
//             iter.cursor.current_len,
//             iter.cursor.current_row
//         );
//         println!("{}", iter.sort::<Entity>().len());
//     }
// }

// #[test]
// fn query_iter_cursor_state_non_empty_after_next() {
//     let mut world = World::new();
//     world.spawn((A(0.), Sparse(11)));
//     world.spawn((A(1.1), Sparse(22)));
//     world.spawn((A(2.22), Sparse(33)));
//     {
//         let mut query = world.query::<(&A, &Sparse)>();
//         let mut iter = query.iter(&world);
//         println!(
//             "before_next_call: archetype_entities: {} table_entities: {} current_len: {} current_row: {}",
//             iter.cursor.archetype_entities.len(),
//             iter.cursor.table_entities.len(),
//             iter.cursor.current_len,
//             iter.cursor.current_row
//         );
//         assert!(iter.cursor.table_entities.len() | iter.cursor.archetype_entities.len() == 0);
//         _ = iter.next();
//         println!(
//             "after_next_call: archetype_entities: {} table_entities: {} current_len: {} current_row: {}",
//             iter.cursor.archetype_entities.len(),
//             iter.cursor.table_entities.len(),
//             iter.cursor.current_len,
//             iter.cursor.current_row
//         );
//         assert!(
//             (
//                 iter.cursor.table_entities.len(),
//                 iter.cursor.archetype_entities.len()
//             ) != (0, 0)
//         );
//     }
// }

// #[test]
// fn query_iter_many_sorts() {
//     let mut world = World::new();

//     let entity_list: &Vec<_> = &world
//         .spawn_batch([A(0.), A(1.), A(2.), A(3.), A(4.)])
//         .collect();

//     let mut query = world.query::<Entity>();

//     let sort = query
//         .iter_many(&world, entity_list)
//         .sort::<Entity>()
//         .collect::<Vec<_>>();

//     let sort_unstable = query
//         .iter_many(&world, entity_list)
//         .sort_unstable::<Entity>()
//         .collect::<Vec<_>>();

//     let sort_by = query
//         .iter_many(&world, entity_list)
//         .sort_by::<Entity>(Ord::cmp)
//         .collect::<Vec<_>>();

//     let sort_unstable_by = query
//         .iter_many(&world, entity_list)
//         .sort_unstable_by::<Entity>(Ord::cmp)
//         .collect::<Vec<_>>();

//     let sort_by_key = query
//         .iter_many(&world, entity_list)
//         .sort_by_key::<Entity, _>(|&e| e)
//         .collect::<Vec<_>>();

//     let sort_unstable_by_key = query
//         .iter_many(&world, entity_list)
//         .sort_unstable_by_key::<Entity, _>(|&e| e)
//         .collect::<Vec<_>>();

//     let sort_by_cached_key = query
//         .iter_many(&world, entity_list)
//         .sort_by_cached_key::<Entity, _>(|&e| e)
//         .collect::<Vec<_>>();

//     let mut sort_v2 = query.iter_many(&world, entity_list).collect::<Vec<_>>();
//     sort_v2.sort();

//     let mut sort_unstable_v2 = query.iter_many(&world, entity_list).collect::<Vec<_>>();
//     sort_unstable_v2.sort_unstable();

//     let mut sort_by_v2 = query.iter_many(&world, entity_list).collect::<Vec<_>>();
//     sort_by_v2.sort_by(Ord::cmp);

//     let mut sort_unstable_by_v2 = query.iter_many(&world, entity_list).collect::<Vec<_>>();
//     sort_unstable_by_v2.sort_unstable_by(Ord::cmp);

//     let mut sort_by_key_v2 = query.iter_many(&world, entity_list).collect::<Vec<_>>();
//     sort_by_key_v2.sort_by_key(|&e| e);

//     let mut sort_unstable_by_key_v2 = query.iter_many(&world, entity_list).collect::<Vec<_>>();
//     sort_unstable_by_key_v2.sort_unstable_by_key(|&e| e);

//     let mut sort_by_cached_key_v2 = query.iter_many(&world, entity_list).collect::<Vec<_>>();
//     sort_by_cached_key_v2.sort_by_cached_key(|&e| e);

//     assert_eq!(sort, sort_v2);
//     assert_eq!(sort_unstable, sort_unstable_v2);
//     assert_eq!(sort_by, sort_by_v2);
//     assert_eq!(sort_unstable_by, sort_unstable_by_v2);
//     assert_eq!(sort_by_key, sort_by_key_v2);
//     assert_eq!(sort_unstable_by_key, sort_unstable_by_key_v2);
//     assert_eq!(sort_by_cached_key, sort_by_cached_key_v2);
// }

// #[test]
// fn query_iter_many_sort_doesnt_panic_after_next() {
//     let mut world = World::new();

//     let entity_list: &Vec<_> = &world
//         .spawn_batch([A(0.), A(1.), A(2.), A(3.), A(4.)])
//         .collect();

//     let mut query = world.query::<Entity>();
//     let mut iter = query.iter_many(&world, entity_list);

//     _ = iter.next();

//     iter.sort::<Entity>();

//     let mut query_2 = world.query::<&mut A>();
//     let mut iter_2 = query_2.iter_many_mut(&mut world, entity_list);

//     _ = iter_2.fetch_next();

//     iter_2.sort::<Entity>();
// }

// // This test should be run with miri to check for UB caused by aliasing.
// // The lens items created during the sort must not be live at the same time as the mutable references returned from the iterator.
// #[test]
// fn query_iter_many_sorts_duplicate_entities_no_ub() {
//     #[derive(Component, Ord, PartialOrd, Eq, PartialEq)]
//     struct C(usize);

//     let mut world = World::new();
//     let id = world.spawn(C(10)).id();
//     let mut query_state = world.query::<&mut C>();

//     {
//         let mut query = query_state.iter_many_mut(&mut world, [id, id]).sort::<&C>();
//         while query.fetch_next().is_some() {}
//     }
//     {
//         let mut query = query_state
//             .iter_many_mut(&mut world, [id, id])
//             .sort_unstable::<&C>();
//         while query.fetch_next().is_some() {}
//     }
//     {
//         let mut query = query_state
//             .iter_many_mut(&mut world, [id, id])
//             .sort_by::<&C>(|l, r| Ord::cmp(l, r));
//         while query.fetch_next().is_some() {}
//     }
//     {
//         let mut query = query_state
//             .iter_many_mut(&mut world, [id, id])
//             .sort_unstable_by::<&C>(|l, r| Ord::cmp(l, r));
//         while query.fetch_next().is_some() {}
//     }
//     {
//         let mut query = query_state
//             .iter_many_mut(&mut world, [id, id])
//             .sort_by_key::<&C, _>(|d| d.0);
//         while query.fetch_next().is_some() {}
//     }
//     {
//         let mut query = query_state
//             .iter_many_mut(&mut world, [id, id])
//             .sort_unstable_by_key::<&C, _>(|d| d.0);
//         while query.fetch_next().is_some() {}
//     }
//     {
//         let mut query = query_state
//             .iter_many_mut(&mut world, [id, id])
//             .sort_by_cached_key::<&C, _>(|d| d.0);
//         while query.fetch_next().is_some() {}
//     }
// }
