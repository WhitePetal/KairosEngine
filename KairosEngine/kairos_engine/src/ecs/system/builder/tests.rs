// use kairos_ecs_macros::{Component, Resource, SystemParam};

// use crate::ecs::{
//     change_detection::{Res, ResMut},
//     entity::Entities,
//     error::Result,
//     query::QueryBuilder,
//     system::{
//         DynParamBuilder, DynSystemParam, FilteredResourcesMutParamBuilder,
//         FilteredResourcesParamBuilder, Local, LocalBuilder, ParamBuilder, ParamSet,
//         ParamSetBuilder, Query, QueryParamBuilder, SystemParamBuilder,
//     },
//     world::{FilteredResources, FilteredResourcesMut, World},
// };

// #[derive(Component)]
// struct A;

// #[derive(Component)]
// struct B;

// #[derive(Component)]
// struct C;

// #[derive(Resource, Default, Reflect)]
// #[reflect(Resource)]
// struct R {
//     foo: usize,
// }

// fn local_system(local: Local<u64>) -> u64 {
//     *local
// }

// fn query_system(query: Query<()>) -> usize {
//     query.iter().count()
// }

// fn query_system_result(query: Query<()>) -> Result<usize> {
//     Ok(query.iter().count())
// }

// fn multi_param_system(a: Local<u64>, b: Local<u64>) -> u64 {
//     *a + *b + 1
// }

// #[test]
// fn local_builder() {
//     let mut world = World::new();

//     let system = (LocalBuilder(10),)
//         .build_state(&mut world)
//         .build_system(local_system);

//     let output = world.run_system_once(system).unwrap();
//     assert_eq!(output, 10);

//     let builder_system = (LocalBuilder(10),).build_system(local_system);

//     let output = world.run_system_once(builder_system).unwrap();
//     assert_eq!(output, 10);
// }

// #[test]
// fn query_builder() {
//     let mut world = World::new();

//     world.spawn(A);
//     world.spawn_empty();

//     let system = (QueryParamBuilder::new(|query| {
//         query.with::<A>();
//     }),)
//         .build_state(&mut world)
//         .build_system(query_system);

//     let output = world.run_system_once(system).unwrap();
//     assert_eq!(output, 1);

//     let builder_system = (QueryParamBuilder::new(|query| {
//         query.with::<A>();
//     }),)
//         .build_system(query_system);

//     let output = world.run_system_once(builder_system).unwrap();
//     assert_eq!(output, 1);
// }

// #[test]
// fn query_builder_system_result_fallible() {
//     let mut world = World::new();

//     world.spawn(A);
//     world.spawn_empty();

//     let system = (QueryParamBuilder::new(|query| {
//         query.with::<A>();
//     }),)
//         .build_state(&mut world)
//         .build_system(query_system_result);

//     // The type annotation here is necessary since the system
//     // could also return `Result<usize>`
//     let output: usize = world.run_system_once(system).unwrap();
//     assert_eq!(output, 1);

//     let builder_system = (QueryParamBuilder::new(|query| {
//         query.with::<A>();
//     }),)
//         .build_system(query_system_result);

//     // The type annotation here is necessary since the system
//     // could also return `Result<usize>`
//     let output: usize = world.run_system_once(builder_system).unwrap();
//     assert_eq!(output, 1);
// }

// #[test]
// fn query_builder_result_infallible() {
//     let mut world = World::new();

//     world.spawn(A);
//     world.spawn_empty();

//     let system = (QueryParamBuilder::new(|query| {
//         query.with::<A>();
//     }),)
//         .build_state(&mut world)
//         .build_system(query_system_result);

//     // The type annotation here is necessary since the system
//     // could also return `usize`
//     let output: Result<usize> = world.run_system_once(system).unwrap();
//     assert_eq!(output.unwrap(), 1);

//     let builder_system = (QueryParamBuilder::new(|query| {
//         query.with::<A>();
//     }),)
//         .build_system(query_system_result);

//     // The type annotation here is necessary since the system
//     // could also return `usize`
//     let output: Result<usize> = world.run_system_once(builder_system).unwrap();
//     assert_eq!(output.unwrap(), 1);
// }

// #[test]
// fn query_builder_state() {
//     let mut world = World::new();

//     world.spawn(A);
//     world.spawn_empty();

//     let state = QueryBuilder::new(&mut world).with::<A>().build();

//     let system = (state,).build_state(&mut world).build_system(query_system);

//     let output = world.run_system_once(system).unwrap();
//     assert_eq!(output, 1);

//     let state = QueryBuilder::new(&mut world).with::<A>().build();

//     let builder_system = (state,).build_system(query_system);

//     let output = world.run_system_once(builder_system).unwrap();
//     assert_eq!(output, 1);
// }

// #[test]
// fn multi_param_builder() {
//     let mut world = World::new();

//     world.spawn(A);
//     world.spawn_empty();

//     let system = (LocalBuilder(0), ParamBuilder)
//         .build_state(&mut world)
//         .build_system(multi_param_system);

//     let output = world.run_system_once(system).unwrap();
//     assert_eq!(output, 1);

//     let builder_system = (LocalBuilder(0), ParamBuilder).build_system(multi_param_system);

//     let output = world.run_system_once(builder_system).unwrap();
//     assert_eq!(output, 1);
// }

// #[test]
// fn vec_builder() {
//     let mut world = World::new();

//     world.spawn((A, B, C));
//     world.spawn((A, B));
//     world.spawn((A, C));
//     world.spawn((A, C));
//     world.spawn_empty();

//     let system = (vec![
//         QueryParamBuilder::new_box(|builder| {
//             builder.with::<B>().without::<C>();
//         }),
//         QueryParamBuilder::new_box(|builder| {
//             builder.with::<C>().without::<B>();
//         }),
//     ],)
//         .build_state(&mut world)
//         .build_system(|params: Vec<Query<&mut A>>| {
//             let mut count: usize = 0;
//             params
//                 .into_iter()
//                 .for_each(|mut query| count += query.iter_mut().count());
//             count
//         });

//     // NOTE: this isn't compatible with `BuilderSystem`, because the system param builder isn't 'static

//     let output = world.run_system_once(system).unwrap();
//     assert_eq!(output, 3);
// }

// #[test]
// fn multi_param_builder_inference() {
//     let mut world = World::new();

//     world.spawn(A);
//     world.spawn_empty();

//     let system = (LocalBuilder(0u64), ParamBuilder::local::<u64>())
//         .build_state(&mut world)
//         .build_system(|a, b| *a + *b + 1);

//     // NOTE: this isn't compatible with `BuilderSystem`, because it uses parameter type inference

//     let output = world.run_system_once(system).unwrap();
//     assert_eq!(output, 1);
// }

// #[test]
// fn param_set_builder() {
//     let mut world = World::new();

//     world.spawn((A, B, C));
//     world.spawn((A, B));
//     world.spawn((A, C));
//     world.spawn((A, C));
//     world.spawn_empty();

//     let system = (ParamSetBuilder((
//         QueryParamBuilder::new(|builder| {
//             builder.with::<B>();
//         }),
//         QueryParamBuilder::new(|builder| {
//             builder.with::<C>();
//         }),
//     )),)
//         .build_state(&mut world)
//         .build_system(|mut params: ParamSet<(Query<&mut A>, Query<&mut A>)>| {
//             params.p0().iter().count() + params.p1().iter().count()
//         });

//     let output = world.run_system_once(system).unwrap();
//     assert_eq!(output, 5);

//     let builder_system = (ParamSetBuilder((
//         QueryParamBuilder::new(|builder| {
//             builder.with::<B>();
//         }),
//         QueryParamBuilder::new(|builder| {
//             builder.with::<C>();
//         }),
//     )),)
//         .build_system(|mut params: ParamSet<(Query<&mut A>, Query<&mut A>)>| {
//             params.p0().iter().count() + params.p1().iter().count()
//         });

//     let output = world.run_system_once(builder_system).unwrap();
//     assert_eq!(output, 5);
// }

// #[test]
// fn param_set_vec_builder() {
//     let mut world = World::new();

//     world.spawn((A, B, C));
//     world.spawn((A, B));
//     world.spawn((A, C));
//     world.spawn((A, C));
//     world.spawn_empty();

//     let system = (ParamSetBuilder(vec![
//         QueryParamBuilder::new_box(|builder| {
//             builder.with::<B>();
//         }),
//         QueryParamBuilder::new_box(|builder| {
//             builder.with::<C>();
//         }),
//     ]),)
//         .build_state(&mut world)
//         .build_system(|mut params: ParamSet<Vec<Query<&mut A>>>| {
//             let mut count = 0;
//             params.for_each(|mut query| count += query.iter_mut().count());
//             count
//         });

//     // NOTE: this isn't compatible with `BuilderSystem`, because the system param builder isn't 'static

//     let output = world.run_system_once(system).unwrap();
//     assert_eq!(output, 5);
// }

// #[test]
// fn dyn_builder() {
//     let mut world = World::new();

//     world.spawn(A);
//     world.spawn_empty();

//     let system = (
//         DynParamBuilder::new(LocalBuilder(3_usize)),
//         DynParamBuilder::new::<Query<()>>(QueryParamBuilder::new(|builder| {
//             builder.with::<A>();
//         })),
//         DynParamBuilder::new::<&Entities>(ParamBuilder),
//     )
//         .build_state(&mut world)
//         .build_system(
//             |mut p0: DynSystemParam, mut p1: DynSystemParam, mut p2: DynSystemParam| {
//                 let local = *p0.downcast_mut::<Local<usize>>().unwrap();
//                 let query_count = p1.downcast_mut::<Query<()>>().unwrap().iter().count();
//                 let _entities = p2.downcast_mut::<&Entities>().unwrap();
//                 assert!(p0.downcast_mut::<Query<()>>().is_none());
//                 local + query_count
//             },
//         );

//     // NOTE: this isn't compatible with `BuilderSystem`, because the system param builder isn't 'static

//     let output = world.run_system_once(system).unwrap();
//     assert_eq!(output, 4);
// }

// #[derive(SystemParam)]
// #[system_param(builder)]
// struct CustomParam<'w, 's> {
//     query: Query<'w, 's, ()>,
//     local: Local<'s, usize>,
// }

// #[test]
// fn custom_param_builder() {
//     let mut world = World::new();

//     world.spawn(A);
//     world.spawn_empty();

//     let system = (CustomParamBuilder {
//         local: LocalBuilder(100),
//         query: QueryParamBuilder::new(|builder| {
//             builder.with::<A>();
//         }),
//     },)
//         .build_state(&mut world)
//         .build_system(|param: CustomParam| *param.local + param.query.iter().count());

//     let output = world.run_system_once(system).unwrap();
//     assert_eq!(output, 101);

//     let builder_system = (CustomParamBuilder {
//         local: LocalBuilder(100),
//         query: QueryParamBuilder::new(|builder| {
//             builder.with::<A>();
//         }),
//     },)
//         .build_system(|param: CustomParam| *param.local + param.query.iter().count());

//     let output = world.run_system_once(builder_system).unwrap();
//     assert_eq!(output, 101);
// }

// #[test]
// fn filtered_resource_conflicts_read_with_res() {
//     let mut world = World::new();
//     (
//         ParamBuilder::resource(),
//         FilteredResourcesParamBuilder::new(|builder| {
//             builder.add_read::<R>();
//         }),
//     )
//         .build_state(&mut world)
//         .build_system(|_r: Res<R>, _fr: FilteredResources| {});
// }

// #[test]
// #[should_panic]
// fn filtered_resource_conflicts_read_with_resmut() {
//     let mut world = World::new();
//     (
//         ParamBuilder::resource_mut(),
//         FilteredResourcesParamBuilder::new(|builder| {
//             builder.add_read::<R>();
//         }),
//     )
//         .build_state(&mut world)
//         .build_system(|_r: ResMut<R>, _fr: FilteredResources| {});
// }

// #[test]
// #[should_panic]
// fn filtered_resource_conflicts_read_all_with_resmut() {
//     let mut world = World::new();
//     (
//         ParamBuilder::resource_mut(),
//         FilteredResourcesParamBuilder::new(|builder| {
//             builder.add_read_all();
//         }),
//     )
//         .build_state(&mut world)
//         .build_system(|_r: ResMut<R>, _fr: FilteredResources| {});
// }

// #[test]
// fn filtered_resource_mut_conflicts_read_with_res() {
//     let mut world = World::new();
//     (
//         ParamBuilder::resource(),
//         FilteredResourcesMutParamBuilder::new(|builder| {
//             builder.add_read::<R>();
//         }),
//     )
//         .build_state(&mut world)
//         .build_system(|_r: Res<R>, _fr: FilteredResourcesMut| {});
// }

// #[test]
// #[should_panic]
// fn filtered_resource_mut_conflicts_read_with_resmut() {
//     let mut world = World::new();
//     (
//         ParamBuilder::resource_mut(),
//         FilteredResourcesMutParamBuilder::new(|builder| {
//             builder.add_read::<R>();
//         }),
//     )
//         .build_state(&mut world)
//         .build_system(|_r: ResMut<R>, _fr: FilteredResourcesMut| {});
// }

// #[test]
// #[should_panic]
// fn filtered_resource_mut_conflicts_write_with_res() {
//     let mut world = World::new();
//     (
//         ParamBuilder::resource(),
//         FilteredResourcesMutParamBuilder::new(|builder| {
//             builder.add_write::<R>();
//         }),
//     )
//         .build_state(&mut world)
//         .build_system(|_r: Res<R>, _fr: FilteredResourcesMut| {});
// }

// #[test]
// #[should_panic]
// fn filtered_resource_mut_conflicts_write_all_with_res() {
//     let mut world = World::new();
//     (
//         ParamBuilder::resource(),
//         FilteredResourcesMutParamBuilder::new(|builder| {
//             builder.add_write_all();
//         }),
//     )
//         .build_state(&mut world)
//         .build_system(|_r: Res<R>, _fr: FilteredResourcesMut| {});
// }

// #[test]
// #[should_panic]
// fn filtered_resource_mut_conflicts_write_with_resmut() {
//     let mut world = World::new();
//     (
//         ParamBuilder::resource_mut(),
//         FilteredResourcesMutParamBuilder::new(|builder| {
//             builder.add_write::<R>();
//         }),
//     )
//         .build_state(&mut world)
//         .build_system(|_r: ResMut<R>, _fr: FilteredResourcesMut| {});
// }
