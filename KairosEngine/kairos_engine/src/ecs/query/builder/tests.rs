// use crate::ecs::{entity::Entity, query::{QueryBuilder, With}, world::{EntityMut, EntityMutExcept, EntityRefExcept, FilteredEntityMut, FilteredEntityRef, World}};

// #[derive(Component, PartialEq, Debug)]
// struct A(usize);

// #[derive(Component, PartialEq, Debug)]
// struct B(usize);

// #[derive(Component, PartialEq, Debug)]
// struct C(usize);

// #[derive(Component)]
// struct D;

// #[test]
// fn builder_with_without_static() {
//     let mut world = World::new();
//     let entity_a = world.spawn((A(0), B(0))).id();
//     let entity_b = world.spawn((A(0), C(0))).id();

//     let mut query_a = QueryBuilder::<Entity>::new(&mut world)
//         .with::<A>()
//         .without::<C>()
//         .build();
//     assert_eq!(entity_a, query_a.single(&world).unwrap());

//     let mut query_b = QueryBuilder::<Entity>::new(&mut world)
//         .with::<A>()
//         .without::<B>()
//         .build();
//     assert_eq!(entity_b, query_b.single(&world).unwrap());
// }

// #[test]
// fn builder_with_without_dynamic() {
//     let mut world = World::new();
//     let entity_a = world.spawn((A(0), B(0))).id();
//     let entity_b = world.spawn((A(0), C(0))).id();
//     let component_id_a = world.register_component::<A>();
//     let component_id_b = world.register_component::<B>();
//     let component_id_c = world.register_component::<C>();

//     let mut query_a = QueryBuilder::<Entity>::new(&mut world)
//         .with_id(component_id_a)
//         .without_id(component_id_c)
//         .build();
//     assert_eq!(entity_a, query_a.single(&world).unwrap());

//     let mut query_b = QueryBuilder::<Entity>::new(&mut world)
//         .with_id(component_id_a)
//         .without_id(component_id_b)
//         .build();
//     assert_eq!(entity_b, query_b.single(&world).unwrap());
// }

// #[test]
// fn builder_or() {
//     let mut world = World::new();
//     world.spawn((A(0), B(0), D));
//     world.spawn((B(0), D));
//     world.spawn((C(0), D));

//     let mut query_a = QueryBuilder::<&D>::new(&mut world)
//         .or(|builder| {
//             builder.with::<A>();
//             builder.with::<B>();
//         })
//         .build();
//     assert_eq!(2, query_a.iter(&world).count());

//     let mut query_b = QueryBuilder::<&D>::new(&mut world)
//         .or(|builder| {
//             builder.with::<A>();
//             builder.without::<B>();
//         })
//         .build();
//     dbg!(&query_b.component_access);
//     assert_eq!(2, query_b.iter(&world).count());

//     let mut query_c = QueryBuilder::<&D>::new(&mut world)
//         .or(|builder| {
//             builder.with::<A>();
//             builder.with::<B>();
//             builder.with::<C>();
//         })
//         .build();
//     assert_eq!(3, query_c.iter(&world).count());
// }

// #[test]
// fn builder_transmute() {
//     let mut world = World::new();
//     world.spawn(A(0));
//     world.spawn((A(1), B(0)));
//     let mut query = QueryBuilder::<()>::new(&mut world)
//         .with::<B>()
//         .transmute::<&A>()
//         .build();

//     query.iter(&world).for_each(|a| assert_eq!(a.0, 1));
// }

// #[test]
// fn builder_static_components() {
//     let mut world = World::new();
//     let entity = world.spawn((A(0), B(1))).id();

//     let mut query = QueryBuilder::<FilteredEntityRef>::new(&mut world)
//         .data::<&A>()
//         .data::<&B>()
//         .build();

//     let entity_ref = query.single(&world).unwrap();

//     assert_eq!(entity, entity_ref.id());

//     let a = entity_ref.get::<A>().unwrap();
//     let b = entity_ref.get::<B>().unwrap();

//     assert_eq!(0, a.0);
//     assert_eq!(1, b.0);
// }

// #[test]
// fn builder_dynamic_components() {
//     let mut world = World::new();
//     let entity = world.spawn((A(0), B(1))).id();
//     let component_id_a = world.register_component::<A>();
//     let component_id_b = world.register_component::<B>();

//     let mut query = QueryBuilder::<FilteredEntityRef>::new(&mut world)
//         .ref_id(component_id_a)
//         .ref_id(component_id_b)
//         .build();

//     let entity_ref = query.single(&world).unwrap();

//     assert_eq!(entity, entity_ref.id());

//     let a = entity_ref.get_by_id(component_id_a).unwrap();
//     let b = entity_ref.get_by_id(component_id_b).unwrap();

//     // SAFETY: We set these pointers to point to these components
//     unsafe {
//         assert_eq!(0, a.deref::<A>().0);
//         assert_eq!(1, b.deref::<B>().0);
//     }
// }

// #[test]
// fn builder_provide_access() {
//     let mut world = World::new();
//     world.spawn((A(0), B(1), D));

//     let mut query =
//         QueryBuilder::<(Entity, FilteredEntityRef, FilteredEntityMut), With<D>>::new(
//             &mut world,
//         )
//         .data::<&mut A>()
//         .data::<&B>()
//         .build();

//     // The `FilteredEntityRef` only has read access, so the `FilteredEntityMut` can have read access without conflicts
//     let (_entity, entity_ref_1, mut entity_ref_2) = query.single_mut(&mut world).unwrap();
//     assert!(entity_ref_1.get::<A>().is_some());
//     assert!(entity_ref_1.get::<B>().is_some());
//     assert!(entity_ref_2.get::<A>().is_some());
//     assert!(entity_ref_2.get_mut::<A>().is_none());
//     assert!(entity_ref_2.get::<B>().is_some());
//     assert!(entity_ref_2.get_mut::<B>().is_none());

//     let mut query =
//         QueryBuilder::<(Entity, FilteredEntityMut, FilteredEntityMut), With<D>>::new(
//             &mut world,
//         )
//         .data::<&mut A>()
//         .data::<&B>()
//         .build();

//     // The first `FilteredEntityMut` has write access to A, so the second one cannot have write access
//     let (_entity, mut entity_ref_1, mut entity_ref_2) = query.single_mut(&mut world).unwrap();
//     assert!(entity_ref_1.get::<A>().is_some());
//     assert!(entity_ref_1.get_mut::<A>().is_some());
//     assert!(entity_ref_1.get::<B>().is_some());
//     assert!(entity_ref_1.get_mut::<B>().is_none());
//     assert!(entity_ref_2.get::<A>().is_none());
//     assert!(entity_ref_2.get_mut::<A>().is_none());
//     assert!(entity_ref_2.get::<B>().is_some());
//     assert!(entity_ref_2.get_mut::<B>().is_none());

//     let mut query = QueryBuilder::<(FilteredEntityMut, &mut A, &B), With<D>>::new(&mut world)
//         .data::<&mut A>()
//         .data::<&mut B>()
//         .build();

//     // Any `A` access would conflict with `&mut A`, and write access to `B` would conflict with `&B`.
//     let (mut entity_ref, _a, _b) = query.single_mut(&mut world).unwrap();
//     assert!(entity_ref.get::<A>().is_none());
//     assert!(entity_ref.get_mut::<A>().is_none());
//     assert!(entity_ref.get::<B>().is_some());
//     assert!(entity_ref.get_mut::<B>().is_none());

//     let mut query = QueryBuilder::<(FilteredEntityMut, &mut A, &B), With<D>>::new(&mut world)
//         .data::<EntityMut>()
//         .build();

//     // Same as above, but starting from "all" access
//     let (mut entity_ref, _a, _b) = query.single_mut(&mut world).unwrap();
//     assert!(entity_ref.get::<A>().is_none());
//     assert!(entity_ref.get_mut::<A>().is_none());
//     assert!(entity_ref.get::<B>().is_some());
//     assert!(entity_ref.get_mut::<B>().is_none());

//     let mut query =
//         QueryBuilder::<(FilteredEntityMut, EntityMutExcept<A>), With<D>>::new(&mut world)
//             .data::<EntityMut>()
//             .build();

//     // Removing `EntityMutExcept<A>` just leaves A
//     let (mut entity_ref_1, _entity_ref_2) = query.single_mut(&mut world).unwrap();
//     assert!(entity_ref_1.get::<A>().is_some());
//     assert!(entity_ref_1.get_mut::<A>().is_some());
//     assert!(entity_ref_1.get::<B>().is_none());
//     assert!(entity_ref_1.get_mut::<B>().is_none());

//     let mut query =
//         QueryBuilder::<(FilteredEntityMut, EntityRefExcept<A>), With<D>>::new(&mut world)
//             .data::<EntityMut>()
//             .build();

//     // Removing `EntityRefExcept<A>` just leaves A, plus read access
//     let (mut entity_ref_1, _entity_ref_2) = query.single_mut(&mut world).unwrap();
//     assert!(entity_ref_1.get::<A>().is_some());
//     assert!(entity_ref_1.get_mut::<A>().is_some());
//     assert!(entity_ref_1.get::<B>().is_some());
//     assert!(entity_ref_1.get_mut::<B>().is_none());
// }

// /// Regression test for issue #14348
// #[test]
// fn builder_static_dense_dynamic_sparse() {
//     #[derive(Component)]
//     struct Dense;

//     #[derive(Component)]
//     #[component(storage = "SparseSet")]
//     struct Sparse;

//     let mut world = World::new();

//     world.spawn(Dense);
//     world.spawn((Dense, Sparse));

//     let mut query = QueryBuilder::<&Dense>::new(&mut world)
//         .with::<Sparse>()
//         .build();

//     let matched = query.iter(&world).count();
//     assert_eq!(matched, 1);
// }

// #[test]
// fn builder_dynamic_can_be_dense() {
//     #[derive(Component)]
//     #[component(storage = "SparseSet")]
//     struct Sparse;

//     let mut world = World::new();

//     // FilteredEntityRef and FilteredEntityMut are dense by default
//     let query = QueryBuilder::<FilteredEntityRef>::new(&mut world).build();
//     assert!(query.is_dense);

//     let query = QueryBuilder::<FilteredEntityMut>::new(&mut world).build();
//     assert!(query.is_dense);

//     // Adding a required sparse term makes the query sparse
//     let query = QueryBuilder::<FilteredEntityRef>::new(&mut world)
//         .data::<&Sparse>()
//         .build();
//     assert!(!query.is_dense);

//     // Adding an optional sparse term lets it remain dense
//     let query = QueryBuilder::<FilteredEntityRef>::new(&mut world)
//         .data::<Option<&Sparse>>()
//         .build();
//     assert!(query.is_dense);
// }
