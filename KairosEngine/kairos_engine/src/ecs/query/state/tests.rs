// use crate::ecs::{entity::Entity, world::World};

// #[test]
// #[should_panic]
// fn right_world_get() {
//     let mut world_1 = World::new();
//     let world_2 = World::new();

//     let mut query_state = world_1.query::<Entity>();
//     let _panics = query_state.get(&world_2, Entity::from_raw_u32(0).unwrap());
// }

// #[test]
// #[should_panic]
// fn right_world_get_many() {
//     let mut world_1 = World::new();
//     let world_2 = World::new();

//     let mut query_state = world_1.query::<Entity>();
//     let _panics = query_state.get_many(&world_2, []);
// }

// #[test]
// #[should_panic]
// fn right_world_get_many_mut() {
//     let mut world_1 = World::new();
//     let mut world_2 = World::new();

//     let mut query_state = world_1.query::<Entity>();
//     let _panics = query_state.get_many_mut(&mut world_2, []);
// }

// #[derive(Component, PartialEq, Debug)]
// struct A(usize);

// #[derive(Component, PartialEq, Debug)]
// struct B(usize);

// #[derive(Component, PartialEq, Debug)]
// struct C(usize);

// #[derive(Component)]
// struct D;

// #[test]
// fn can_transmute_to_more_general() {
//     let mut world = World::new();
//     world.spawn((A(1), B(0)));

//     let query_state = world.query::<(&A, &B)>();
//     let mut new_query_state = query_state.transmute::<&A>(&world);
//     assert_eq!(new_query_state.iter(&world).len(), 1);
//     let a = new_query_state.single(&world).unwrap();

//     assert_eq!(a.0, 1);
// }

// #[test]
// fn cannot_get_data_not_in_original_query() {
//     let mut world = World::new();
//     world.spawn((A(0), B(0)));
//     world.spawn((A(1), B(0), C(0)));

//     let query_state = world.query_filtered::<(&A, &B), Without<C>>();
//     let mut new_query_state = query_state.transmute::<&A>(&world);
//     // even though we change the query to not have Without<C>, we do not get the component with C.
//     let a = new_query_state.single(&world).unwrap();

//     assert_eq!(a.0, 0);
// }

// #[test]
// fn can_transmute_empty_tuple() {
//     let mut world = World::new();
//     world.register_component::<A>();
//     let entity = world.spawn(A(10)).id();

//     let q = world.query_filtered::<(), With<A>>();
//     let mut q = q.transmute::<Entity>(&world);
//     assert_eq!(q.single(&world).unwrap(), entity);
// }

// #[test]
// fn can_transmute_immut_fetch() {
//     let mut world = World::new();
//     world.spawn(A(10));

//     let q = world.query::<&A>();
//     let mut new_q = q.transmute::<Ref<A>>(&world);
//     assert!(new_q.single(&world).unwrap().is_added());

//     let q = world.query::<Ref<A>>();
//     let _ = q.transmute::<&A>(&world);
// }

// #[test]
// fn can_transmute_mut_fetch() {
//     let mut world = World::new();
//     world.spawn(A(0));

//     let q = world.query::<&mut A>();
//     let _ = q.transmute::<Ref<A>>(&world);
//     let _ = q.transmute::<&A>(&world);
// }

// #[test]
// fn can_transmute_entity_mut() {
//     let mut world = World::new();
//     world.spawn(A(0));

//     let q: QueryState<EntityMut<'_>> = world.query::<EntityMut>();
//     let _ = q.transmute::<EntityRef>(&world);
// }

// #[test]
// fn can_generalize_with_option() {
//     let mut world = World::new();
//     world.spawn((A(0), B(0)));

//     let query_state = world.query::<(Option<&A>, &B)>();
//     let _ = query_state.transmute::<Option<&A>>(&world);
//     let _ = query_state.transmute::<&B>(&world);
// }

// #[test]
// #[should_panic]
// fn cannot_transmute_to_include_data_not_in_original_query() {
//     let mut world = World::new();
//     world.register_component::<A>();
//     world.register_component::<B>();
//     world.spawn(A(0));

//     let query_state = world.query::<&A>();
//     let mut _new_query_state = query_state.transmute::<(&A, &B)>(&world);
// }

// #[test]
// #[should_panic]
// fn cannot_transmute_immut_to_mut() {
//     let mut world = World::new();
//     world.spawn(A(0));

//     let query_state = world.query::<&A>();
//     let mut _new_query_state = query_state.transmute::<&mut A>(&world);
// }

// #[test]
// #[should_panic]
// fn cannot_transmute_option_to_immut() {
//     let mut world = World::new();
//     world.spawn(C(0));

//     let query_state = world.query::<Option<&A>>();
//     let mut new_query_state = query_state.transmute::<&A>(&world);
//     let x = new_query_state.single(&world).unwrap();
//     assert_eq!(x.0, 1234);
// }

// #[test]
// #[should_panic]
// fn cannot_transmute_entity_ref() {
//     let mut world = World::new();
//     world.register_component::<A>();

//     let q = world.query::<EntityRef>();
//     let _ = q.transmute::<&A>(&world);
// }

// #[test]
// fn can_transmute_filtered_entity() {
//     let mut world = World::new();
//     let entity = world.spawn((A(0), B(1))).id();
//     let query = QueryState::<(Entity, &A, &B)>::new(&mut world)
//         .transmute::<(Entity, FilteredEntityRef)>(&world);

//     let mut query = query;
//     // Our result is completely untyped
//     let (_entity, entity_ref) = query.single(&world).unwrap();

//     assert_eq!(entity, entity_ref.id());
//     assert_eq!(0, entity_ref.get::<A>().unwrap().0);
//     assert_eq!(1, entity_ref.get::<B>().unwrap().0);
// }

// #[test]
// fn can_transmute_added() {
//     let mut world = World::new();
//     let entity_a = world.spawn(A(0)).id();

//     let mut query = QueryState::<(Entity, &A, Has<B>)>::new(&mut world)
//         .transmute_filtered::<(Entity, Has<B>), Added<A>>(&world);

//     assert_eq!((entity_a, false), query.single(&world).unwrap());

//     world.clear_trackers();

//     let entity_b = world.spawn((A(0), B(0))).id();
//     assert_eq!((entity_b, true), query.single(&world).unwrap());

//     world.clear_trackers();

//     assert!(query.single(&world).is_err());
// }

// #[test]
// fn can_transmute_changed() {
//     let mut world = World::new();
//     let entity_a = world.spawn(A(0)).id();

//     let mut detection_query = QueryState::<(Entity, &A)>::new(&mut world)
//         .transmute_filtered::<Entity, Changed<A>>(&world);

//     let mut change_query = QueryState::<&mut A>::new(&mut world);
//     assert_eq!(entity_a, detection_query.single(&world).unwrap());

//     world.clear_trackers();

//     assert!(detection_query.single(&world).is_err());

//     change_query.single_mut(&mut world).unwrap().0 = 1;

//     assert_eq!(entity_a, detection_query.single(&world).unwrap());
// }

// #[test]
// #[should_panic]
// fn cannot_transmute_changed_without_access() {
//     let mut world = World::new();
//     world.register_component::<A>();
//     world.register_component::<B>();
//     let query = QueryState::<&A>::new(&mut world);
//     let _new_query = query.transmute_filtered::<Entity, Changed<B>>(&world);
// }

// #[test]
// #[should_panic]
// fn cannot_transmute_mutable_after_readonly() {
//     let mut world = World::new();
//     // Calling this method would mean we had aliasing queries.
//     fn bad(_: Query<&mut A>, _: Query<&A>) {}
//     world
//         .run_system_once(|query: Query<&mut A>| {
//             let mut readonly = query.as_readonly();
//             let mut lens: QueryLens<&mut A> = readonly.transmute_lens();
//             bad(lens.query(), query.as_readonly());
//         })
//         .unwrap();
// }

// // Regression test for #14629
// #[test]
// #[should_panic]
// fn transmute_with_different_world() {
//     let mut world = World::new();
//     world.spawn((A(1), B(2)));

//     let mut world2 = World::new();
//     world2.register_component::<B>();

//     world.query::<(&A, &B)>().transmute::<&B>(&world2);
// }

// /// Regression test for issue #14528
// #[test]
// fn transmute_from_sparse_to_dense() {
//     #[derive(Component)]
//     struct Dense;

//     #[derive(Component)]
//     #[component(storage = "SparseSet")]
//     struct Sparse;

//     let mut world = World::new();

//     world.spawn(Dense);
//     world.spawn((Dense, Sparse));

//     let mut query = world
//         .query_filtered::<&Dense, With<Sparse>>()
//         .transmute::<&Dense>(&world);

//     let matched = query.iter(&world).count();
//     assert_eq!(matched, 1);
// }
// #[test]
// fn transmute_from_dense_to_sparse() {
//     #[derive(Component)]
//     struct Dense;

//     #[derive(Component)]
//     #[component(storage = "SparseSet")]
//     struct Sparse;

//     let mut world = World::new();

//     world.spawn(Dense);
//     world.spawn((Dense, Sparse));

//     let mut query = world
//         .query::<&Dense>()
//         .transmute_filtered::<&Dense, With<Sparse>>(&world);

//     // Note: `transmute_filtered` is supposed to keep the same matched tables/archetypes,
//     // so it doesn't actually filter out those entities without `Sparse` and the iteration
//     // remains dense.
//     let matched = query.iter(&world).count();
//     assert_eq!(matched, 2);
// }

// #[test]
// fn transmute_to_or_filter() {
//     let mut world = World::new();
//     world.spawn(D);
//     world.spawn((A(0), D));

//     let mut query = world
//         .query::<(&D, Option<&A>)>()
//         .transmute_filtered::<Entity, Or<(With<A>,)>>(&world);
//     let iter = query.iter(&world);
//     let len = iter.len();
//     let count = iter.count();
//     // `transmute_filtered` keeps the same matched tables, so it should match both entities
//     // More importantly, `count()` and `len()` should return the same result!
//     assert_eq!(len, 2);
//     assert_eq!(count, len);

//     let mut query = world
//         .query::<(&D, Option<&A>)>()
//         .transmute_filtered::<Entity, Or<(Changed<A>,)>>(&world);
//     let iter = query.iter(&world);
//     let count = iter.count();
//     // The behavior of a non-archetypal filter like `Changed` should be the same as an archetypal one like `With`.
//     assert_eq!(count, 2);
// }

// #[test]
// fn dense_query_over_option_is_buggy() {
//     #[derive(Component)]
//     #[component(storage = "SparseSet")]
//     struct Sparse;

//     let mut world = World::new();
//     world.spawn(Sparse);

//     let mut query =
//         QueryState::<EntityRef>::new(&mut world).transmute::<Option<&Sparse>>(&world);
//     // EntityRef always performs dense iteration
//     // But `Option<&Sparse>` will incorrectly report a component as never being present when doing dense iteration
//     // See https://github.com/bevyengine/bevy/issues/16397
//     assert!(query.is_dense);
//     let matched = query.iter(&world).filter(Option::is_some).count();
//     assert_eq!(matched, 0);

//     let mut query = QueryState::<EntityRef>::new(&mut world).transmute::<Has<Sparse>>(&world);
//     // EntityRef always performs dense iteration
//     // But `Has<Sparse>` will incorrectly report a component as never being present when doing dense iteration
//     // See https://github.com/bevyengine/bevy/issues/16397
//     assert!(query.is_dense);
//     let matched = query.iter(&world).filter(|&has| has).count();
//     assert_eq!(matched, 0);
// }

// #[test]
// fn join() {
//     let mut world = World::new();
//     world.spawn(A(0));
//     world.spawn(B(1));
//     let entity_ab = world.spawn((A(2), B(3))).id();
//     world.spawn((A(4), B(5), C(6)));

//     let query_1 = QueryState::<&A, Without<C>>::new(&mut world);
//     let query_2 = QueryState::<&B, Without<C>>::new(&mut world);
//     let mut new_query: QueryState<Entity, ()> = query_1.join_filtered(&world, &query_2);

//     assert_eq!(new_query.single(&world).unwrap(), entity_ab);
// }

// #[test]
// fn join_with_get() {
//     let mut world = World::new();
//     world.spawn(A(0));
//     world.spawn(B(1));
//     let entity_ab = world.spawn((A(2), B(3))).id();
//     let entity_abc = world.spawn((A(4), B(5), C(6))).id();

//     let query_1 = QueryState::<&A>::new(&mut world);
//     let query_2 = QueryState::<&B, Without<C>>::new(&mut world);
//     let mut new_query: QueryState<Entity, ()> = query_1.join_filtered(&world, &query_2);

//     assert!(new_query.get(&world, entity_ab).is_ok());
//     // should not be able to get entity with c.
//     assert!(new_query.get(&world, entity_abc).is_err());
// }

// #[test]
// #[should_panic]
// fn cannot_join_wrong_fetch() {
//     let mut world = World::new();
//     world.register_component::<C>();
//     let query_1 = QueryState::<&A>::new(&mut world);
//     let query_2 = QueryState::<&B>::new(&mut world);
//     let _query: QueryState<&C> = query_1.join(&world, &query_2);
// }

// #[test]
// #[should_panic]
// fn cannot_join_wrong_filter() {
//     let mut world = World::new();
//     let query_1 = QueryState::<&A, Without<C>>::new(&mut world);
//     let query_2 = QueryState::<&B, Without<C>>::new(&mut world);
//     let _: QueryState<Entity, Changed<C>> = query_1.join_filtered(&world, &query_2);
// }

// #[test]
// #[should_panic]
// fn cannot_join_mutable_after_readonly() {
//     let mut world = World::new();
//     // Calling this method would mean we had aliasing queries.
//     fn bad(_: Query<(&mut A, &mut B)>, _: Query<&A>) {}
//     world
//         .run_system_once(|query_a: Query<&mut A>, mut query_b: Query<&mut B>| {
//             let mut readonly = query_a.as_readonly();
//             let mut lens: QueryLens<(&mut A, &mut B)> = readonly.join(&mut query_b);
//             bad(lens.query(), query_a.as_readonly());
//         })
//         .unwrap();
// }

// #[test]
// fn join_to_filtered_entity_mut() {
//     let mut world = World::new();
//     world.spawn((A(2), B(3)));

//     let query_1 = QueryState::<&mut A>::new(&mut world);
//     let query_2 = QueryState::<&mut B>::new(&mut world);
//     let mut new_query: QueryState<(Entity, FilteredEntityMut)> = query_1.join(&world, &query_2);

//     let (_entity, mut entity_mut) = new_query.single_mut(&mut world).unwrap();
//     assert!(entity_mut.get_mut::<A>().is_some());
//     assert!(entity_mut.get_mut::<B>().is_some());
// }

// #[test]
// fn query_respects_default_filters() {
//     let mut world = World::new();
//     world.spawn((A(0), B(0), D));
//     world.spawn((B(0), C(0), D));
//     world.spawn((C(0), D));

//     world.register_disabling_component::<C>();

//     // Without<C> only matches the first entity
//     let mut query = QueryState::<&D>::new(&mut world);
//     assert_eq!(1, query.iter(&world).count());

//     // With<C> matches the last two entities
//     let mut query = QueryState::<&D, With<C>>::new(&mut world);
//     assert_eq!(2, query.iter(&world).count());

//     // Has should bypass the filter entirely
//     let mut query = QueryState::<(&D, Has<C>)>::new(&mut world);
//     assert_eq!(3, query.iter(&world).count());

//     // Allow should bypass the filter entirely
//     let mut query = QueryState::<&D, Allow<C>>::new(&mut world);
//     assert_eq!(3, query.iter(&world).count());

//     // Other filters should still be respected
//     let mut query = QueryState::<(&D, Has<C>), Without<B>>::new(&mut world);
//     assert_eq!(1, query.iter(&world).count());
// }

// #[derive(Component)]
// struct Table;

// #[derive(Component)]
// #[component(storage = "SparseSet")]
// struct Sparse;

// #[derive(Component)]
// struct Dummy;

// #[test]
// fn query_default_filters_updates_is_dense() {
//     let mut world = World::new();
//     world.spawn((Dummy, Table, Sparse));
//     world.spawn((Dummy, Table));
//     world.spawn((Dummy, Sparse));

//     let mut query = QueryState::<&Dummy>::new(&mut world);
//     // There are no sparse components involved thus the query is dense
//     assert!(query.is_dense);
//     assert_eq!(3, query.query(&world).count());

//     world.register_disabling_component::<Sparse>();

//     let mut query = QueryState::<&Dummy>::new(&mut world);
//     // The query doesn't ask for sparse components, but the default filters adds
//     // a sparse component thus it is NOT dense
//     assert!(!query.is_dense);
//     assert_eq!(1, query.query(&world).count());

//     let mut df = DefaultQueryFilters::from_world(&mut world);
//     df.register_disabling_component(world.register_component::<Table>());
//     world.insert_resource(df);

//     let mut query = QueryState::<&Dummy>::new(&mut world);
//     // If the filter is instead a table components, the query can still be dense
//     assert!(query.is_dense);
//     assert_eq!(1, query.query(&world).count());

//     let mut query = QueryState::<&Sparse>::new(&mut world);
//     // But only if the original query was dense
//     assert!(!query.is_dense);
//     assert_eq!(1, query.query(&world).count());
// }
