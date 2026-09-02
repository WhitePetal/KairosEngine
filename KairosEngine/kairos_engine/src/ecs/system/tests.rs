// use std::{any::TypeId, rc::Rc};

// use kairos_ecs_macros::{Component, Resource};

// use crate::ecs::{
//     archetype::Archetypes,
//     bundle::Bundles,
//     change_detection::{NonSend, NonSendMut, Res, ResMut},
//     component::Components,
//     entity::{Entities, Entity},
//     error::Result,
//     lifecycle::{Add, RemovedComponents},
//     name::Name,
//     observer::On,
//     query::{
//         Added, AnyOf, Changed, NestedQuery, Or, QueryState, SpawnDetails, Spawned, With, Without,
//     },
//     system::{
//         Commands, ExclusiveMarker, In, InMut, IntoSystem, Local, ParamSet, Query, ScheduleSystem,
//         Single, StaticSystemParam, SystemState,
//     },
//     world::{DeferredWorld, EntityMut, EntityRef, FromWorld, World},
// };

// #[derive(Resource, PartialEq, Debug)]
// enum SystemRan {
//     Yes,
//     No,
// }

// #[derive(Component, Debug, Eq, PartialEq, Default)]
// struct A;
// #[derive(Component)]
// struct B;
// #[derive(Component)]
// struct C;
// #[derive(Component)]
// struct D;
// #[derive(Component)]
// struct E;
// #[derive(Component)]
// struct F;

// #[derive(Resource)]
// struct ResA;
// #[derive(Resource)]
// struct ResB;
// #[derive(Resource)]
// struct ResC;
// #[derive(Resource)]
// struct ResD;
// #[derive(Resource)]
// struct ResE;
// #[derive(Resource)]
// struct ResF;

// #[derive(Component, Debug)]
// struct W<T>(T);

// #[test]
// fn simple_system() {
//     fn sys(query: Query<&A>) {
//         for a in &query {
//             println!("{a:?}");
//         }
//     }

//     let mut system = IntoSystem::into_system(sys);
//     let mut world = World::new();
//     world.spawn(A);

//     system.initialize(&mut world);
//     system.run((), &mut world).unwrap();
// }

// fn run_system<Marker, S: IntoScheduleConfigs<ScheduleSystem, Marker>>(
//     world: &mut World,
//     system: S,
// ) {
//     let mut schedule = Schedule::default();
//     schedule.add_systems(system);
//     schedule.run(world);
// }

// #[test]
// fn get_many_is_ordered() {
//     use crate::ecs::resource::Resource;
//     const ENTITIES_COUNT: usize = 1000;

//     #[derive(Resource)]
//     struct EntitiesArray(Vec<Entity>);

//     fn query_system(
//         mut ran: ResMut<SystemRan>,
//         entities_array: Res<EntitiesArray>,
//         q: Query<&W<usize>>,
//     ) {
//         let entities_array: [Entity; ENTITIES_COUNT] = entities_array.0.clone().try_into().unwrap();

//         for (i, w) in (0..ENTITIES_COUNT).zip(q.get_many(entities_array).unwrap()) {
//             assert_eq!(i, w.0);
//         }

//         *ran = SystemRan::Yes;
//     }

//     fn query_system_mut(
//         mut ran: ResMut<SystemRan>,
//         entities_array: Res<EntitiesArray>,
//         mut q: Query<&mut W<usize>>,
//     ) {
//         let entities_array: [Entity; ENTITIES_COUNT] = entities_array.0.clone().try_into().unwrap();

//         for (i, w) in (0..ENTITIES_COUNT).zip(q.get_many_mut(entities_array).unwrap()) {
//             assert_eq!(i, w.0);
//         }

//         *ran = SystemRan::Yes;
//     }

//     let mut world = World::default();
//     world.insert_resource(SystemRan::No);
//     let entity_ids = (0..ENTITIES_COUNT)
//         .map(|i| world.spawn(W(i)).id())
//         .collect();
//     world.insert_resource(EntitiesArray(entity_ids));

//     run_system(&mut world, query_system);
//     assert_eq!(*world.resource::<SystemRan>(), SystemRan::Yes);

//     world.insert_resource(SystemRan::No);
//     run_system(&mut world, query_system_mut);
//     assert_eq!(*world.resource::<SystemRan>(), SystemRan::Yes);
// }

// #[test]
// fn or_param_set_system() {
//     // Regression test for issue #762
//     fn query_system(
//         mut ran: ResMut<SystemRan>,
//         mut set: ParamSet<(
//             Query<(), Or<(Changed<A>, Changed<B>)>>,
//             Query<(), Or<(Added<A>, Added<B>)>>,
//         )>,
//     ) {
//         let changed = set.p0().iter().count();
//         let added = set.p1().iter().count();

//         assert_eq!(changed, 1);
//         assert_eq!(added, 1);

//         *ran = SystemRan::Yes;
//     }

//     let mut world = World::default();
//     world.insert_resource(SystemRan::No);
//     world.spawn((A, B));

//     run_system(&mut world, query_system);

//     assert_eq!(*world.resource::<SystemRan>(), SystemRan::Yes);
// }

// #[test]
// fn changed_resource_system() {
//     use crate::ecs::resource::Resource;

//     #[derive(Resource)]
//     struct Flipper(bool);

//     #[derive(Resource)]
//     struct Added(usize);

//     #[derive(Resource)]
//     struct Changed(usize);

//     fn incr_e_on_flip(value: Res<Flipper>, mut changed: ResMut<Changed>, mut added: ResMut<Added>) {
//         if value.is_added() {
//             added.0 += 1;
//         }

//         if value.is_changed() {
//             changed.0 += 1;
//         }
//     }

//     let mut world = World::default();
//     world.insert_resource(Flipper(false));
//     world.insert_resource(Added(0));
//     world.insert_resource(Changed(0));

//     let mut schedule = Schedule::default();

//     schedule.add_systems((incr_e_on_flip, ApplyDeferred, World::clear_trackers).chain());

//     schedule.run(&mut world);
//     assert_eq!(world.resource::<Added>().0, 1);
//     assert_eq!(world.resource::<Changed>().0, 1);

//     schedule.run(&mut world);
//     assert_eq!(world.resource::<Added>().0, 1);
//     assert_eq!(world.resource::<Changed>().0, 1);

//     world.resource_mut::<Flipper>().0 = true;
//     schedule.run(&mut world);
//     assert_eq!(world.resource::<Added>().0, 1);
//     assert_eq!(world.resource::<Changed>().0, 2);
// }

// #[test]
// #[should_panic = "error[B0001]"]
// fn option_has_no_filter_with() {
//     fn sys(_: Query<(Option<&A>, &mut B)>, _: Query<&mut B, Without<A>>) {}
//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// fn option_doesnt_remove_unrelated_filter_with() {
//     fn sys(_: Query<(Option<&A>, &mut B, &A)>, _: Query<&mut B, Without<A>>) {}
//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// fn any_of_working() {
//     fn sys(_: Query<AnyOf<(&mut A, &B)>>) {}
//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// fn any_of_with_and_without_common() {
//     fn sys(_: Query<(&mut D, &C, AnyOf<(&A, &B)>)>, _: Query<&mut D, Without<C>>) {}
//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// #[should_panic]
// fn any_of_with_mut_and_ref() {
//     fn sys(_: Query<AnyOf<(&mut A, &A)>>) {}
//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// #[should_panic]
// fn any_of_with_ref_and_mut() {
//     fn sys(_: Query<AnyOf<(&A, &mut A)>>) {}
//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// #[should_panic]
// fn any_of_with_mut_and_option() {
//     fn sys(_: Query<AnyOf<(&mut A, Option<&A>)>>) {}
//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// fn any_of_with_entity_and_mut() {
//     fn sys(_: Query<AnyOf<(Entity, &mut A)>>) {}
//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// fn any_of_with_empty_and_mut() {
//     fn sys(_: Query<AnyOf<((), &mut A)>>) {}
//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// #[should_panic = "error[B0001]"]
// fn any_of_has_no_filter_with() {
//     fn sys(_: Query<(AnyOf<(&A, ())>, &mut B)>, _: Query<&mut B, Without<A>>) {}
//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// #[should_panic]
// fn any_of_with_conflicting() {
//     fn sys(_: Query<AnyOf<(&mut A, &mut A)>>) {}
//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// fn any_of_has_filter_with_when_both_have_it() {
//     fn sys(_: Query<(AnyOf<(&A, &A)>, &mut B)>, _: Query<&mut B, Without<A>>) {}
//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// fn any_of_doesnt_remove_unrelated_filter_with() {
//     fn sys(_: Query<(AnyOf<(&A, ())>, &mut B, &A)>, _: Query<&mut B, Without<A>>) {}
//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// fn any_of_and_without() {
//     fn sys(_: Query<(AnyOf<(&A, &B)>, &mut C)>, _: Query<&mut C, (Without<A>, Without<B>)>) {}
//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// #[should_panic = "error[B0001]"]
// fn or_has_no_filter_with() {
//     fn sys(_: Query<&mut B, Or<(With<A>, With<B>)>>, _: Query<&mut B, Without<A>>) {}
//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// fn or_has_filter_with_when_both_have_it() {
//     fn sys(_: Query<&mut B, Or<(With<A>, With<A>)>>, _: Query<&mut B, Without<A>>) {}
//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// fn or_has_filter_with() {
//     fn sys(_: Query<&mut C, Or<(With<A>, With<B>)>>, _: Query<&mut C, (Without<A>, Without<B>)>) {}
//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// fn or_expanded_with_and_without_common() {
//     fn sys(_: Query<&mut D, (With<A>, Or<(With<B>, With<C>)>)>, _: Query<&mut D, Without<A>>) {}
//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// fn or_expanded_nested_with_and_without_common() {
//     fn sys(
//         _: Query<&mut E, (Or<((With<B>, With<C>), (With<C>, With<D>))>, With<A>)>,
//         _: Query<&mut E, (Without<B>, Without<D>)>,
//     ) {
//     }
//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// #[should_panic = "error[B0001]"]
// fn or_expanded_nested_with_and_disjoint_without() {
//     fn sys(
//         _: Query<&mut E, (Or<((With<B>, With<C>), (With<C>, With<D>))>, With<A>)>,
//         _: Query<&mut E, Without<D>>,
//     ) {
//     }
//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// #[should_panic = "error[B0001]"]
// fn or_expanded_nested_or_with_and_disjoint_without() {
//     fn sys(
//         _: Query<&mut D, Or<(Or<(With<A>, With<B>)>, Or<(With<A>, With<C>)>)>>,
//         _: Query<&mut D, Without<A>>,
//     ) {
//     }
//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// fn or_expanded_nested_with_and_common_nested_without() {
//     fn sys(
//         _: Query<&mut D, Or<((With<A>, With<B>), (With<B>, With<C>))>>,
//         _: Query<&mut D, Or<(Without<D>, Without<B>)>>,
//     ) {
//     }
//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// fn or_with_without_and_compatible_with_without() {
//     fn sys(_: Query<&mut C, Or<(With<A>, Without<B>)>>, _: Query<&mut C, (With<B>, Without<A>)>) {}
//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// #[should_panic = "error[B0001]"]
// fn with_and_disjoint_or_empty_without() {
//     fn sys(_: Query<&mut B, With<A>>, _: Query<&mut B, Or<((), Without<A>)>>) {}
//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// #[should_panic = "error[B0001]"]
// fn or_expanded_with_and_disjoint_nested_without() {
//     fn sys(
//         _: Query<&mut D, Or<(With<A>, With<B>)>>,
//         _: Query<&mut D, Or<(Without<A>, Without<B>)>>,
//     ) {
//     }
//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// #[should_panic = "error[B0001]"]
// fn or_expanded_nested_with_and_disjoint_nested_without() {
//     fn sys(
//         _: Query<&mut D, Or<((With<A>, With<B>), (With<B>, With<C>))>>,
//         _: Query<&mut D, Or<(Without<A>, Without<B>)>>,
//     ) {
//     }
//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// fn or_doesnt_remove_unrelated_filter_with() {
//     fn sys(_: Query<&mut B, (Or<(With<A>, With<B>)>, With<A>)>, _: Query<&mut B, Without<A>>) {}
//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// #[should_panic]
// fn conflicting_query_mut_system() {
//     fn sys(_q1: Query<&mut A>, _q2: Query<&mut A>) {}

//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// fn disjoint_query_mut_system() {
//     fn sys(_q1: Query<&mut A, With<B>>, _q2: Query<&mut A, Without<B>>) {}

//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// fn disjoint_query_mut_read_component_system() {
//     fn sys(_q1: Query<(&mut A, &B)>, _q2: Query<&mut A, Without<B>>) {}

//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// #[should_panic]
// fn conflicting_query_immut_system() {
//     fn sys(_q1: Query<&A>, _q2: Query<&mut A>) {}

//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// #[should_panic]
// fn changed_trackers_or_conflict() {
//     fn sys(_: Query<&mut A>, _: Query<(), Or<(Changed<A>,)>>) {}

//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// #[should_panic = "error[B0001]"]
// fn nested_query_conflicts_with_main_query() {
//     fn sys(_: Query<(&mut A, NestedQuery<&A>)>) {}

//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// #[should_panic = "error[B0001]"]
// fn nested_query_conflicts_with_earlier_query() {
//     fn sys(_: Query<&mut A>, _: Query<NestedQuery<&A>>) {}

//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// #[should_panic = "error[B0001]"]
// fn nested_query_conflicts_with_later_query() {
//     fn sys(_: Query<NestedQuery<&A>>, _: Query<&mut A>) {}

//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// fn query_set_system() {
//     fn sys(mut _set: ParamSet<(Query<&mut A>, Query<&A>)>) {}
//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// #[should_panic]
// fn conflicting_query_with_query_set_system() {
//     fn sys(_query: Query<&mut A>, _set: ParamSet<(Query<&mut A>, Query<&B>)>) {}

//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[test]
// #[should_panic]
// fn conflicting_query_sets_system() {
//     fn sys(_set_1: ParamSet<(Query<&mut A>,)>, _set_2: ParamSet<(Query<&mut A>, Query<&B>)>) {}

//     let mut world = World::default();
//     run_system(&mut world, sys);
// }

// #[derive(Default, Resource)]
// struct BufferRes {
//     _buffer: Vec<u8>,
// }

// fn test_for_conflicting_resources<Marker, S: IntoSystem<(), (), Marker>>(sys: S) {
//     let mut world = World::default();
//     world.insert_resource(BufferRes::default());
//     world.insert_resource(ResA);
//     world.insert_resource(ResB);
//     run_system(&mut world, sys);
// }

// #[test]
// #[should_panic]
// fn conflicting_system_resources() {
//     fn sys(_: ResMut<BufferRes>, _: Res<BufferRes>) {}
//     test_for_conflicting_resources(sys);
// }

// #[test]
// #[should_panic]
// fn conflicting_system_resources_reverse_order() {
//     fn sys(_: Res<BufferRes>, _: ResMut<BufferRes>) {}
//     test_for_conflicting_resources(sys);
// }

// #[test]
// #[should_panic]
// fn conflicting_system_resources_multiple_mutable() {
//     fn sys(_: ResMut<BufferRes>, _: ResMut<BufferRes>) {}
//     test_for_conflicting_resources(sys);
// }

// #[test]
// fn nonconflicting_system_resources() {
//     fn sys(_: Local<BufferRes>, _: ResMut<BufferRes>, _: Local<A>, _: ResMut<ResA>) {}
//     test_for_conflicting_resources(sys);
// }

// #[test]
// fn local_system() {
//     let mut world = World::default();
//     world.insert_resource(ProtoFoo { value: 1 });
//     world.insert_resource(SystemRan::No);

//     struct Foo {
//         value: u32,
//     }

//     #[derive(Resource)]
//     struct ProtoFoo {
//         value: u32,
//     }

//     impl FromWorld for Foo {
//         fn from_world(world: &mut World) -> Self {
//             Foo {
//                 value: world.resource::<ProtoFoo>().value + 1,
//             }
//         }
//     }

//     fn sys(local: Local<Foo>, mut system_ran: ResMut<SystemRan>) {
//         assert_eq!(local.value, 2);
//         *system_ran = SystemRan::Yes;
//     }

//     run_system(&mut world, sys);

//     // ensure the system actually ran
//     assert_eq!(*world.resource::<SystemRan>(), SystemRan::Yes);
// }

// #[test]
// #[expect(
//     dead_code,
//     reason = "The `NotSend1` and `NotSend2` structs is used to verify that a system will run, even if the system params include a non-Send resource. As such, the inner value doesn't matter."
// )]
// fn non_send_option_system() {
//     let mut world = World::default();

//     world.insert_resource(SystemRan::No);
//     // Two structs are used, one which is inserted and one which is not, to verify that wrapping
//     // non-Send resources in an `Option` will allow the system to run regardless of their
//     // existence.
//     struct NotSend1(Rc<i32>);
//     struct NotSend2(Rc<i32>);
//     world.insert_non_send(NotSend1(Rc::new(0)));

//     fn sys(
//         op: Option<NonSend<NotSend1>>,
//         mut _op2: Option<NonSendMut<NotSend2>>,
//         mut system_ran: ResMut<SystemRan>,
//     ) {
//         op.expect("NonSend should exist");
//         *system_ran = SystemRan::Yes;
//     }

//     run_system(&mut world, sys);
//     // ensure the system actually ran
//     assert_eq!(*world.resource::<SystemRan>(), SystemRan::Yes);
// }

// #[test]
// #[expect(
//     dead_code,
//     reason = "The `NotSend1` and `NotSend2` structs are used to verify that a system will run, even if the system params include a non-Send resource. As such, the inner value doesn't matter."
// )]
// fn non_send_system() {
//     let mut world = World::default();

//     world.insert_resource(SystemRan::No);
//     struct NotSend1(Rc<i32>);
//     struct NotSend2(Rc<i32>);

//     world.insert_non_send(NotSend1(Rc::new(1)));
//     world.insert_non_send(NotSend2(Rc::new(2)));

//     fn sys(
//         _op: NonSend<NotSend1>,
//         mut _op2: NonSendMut<NotSend2>,
//         mut system_ran: ResMut<SystemRan>,
//     ) {
//         *system_ran = SystemRan::Yes;
//     }

//     run_system(&mut world, sys);
//     assert_eq!(*world.resource::<SystemRan>(), SystemRan::Yes);
// }

// #[test]
// fn function_system_as_exclusive() {
//     let mut world = World::default();

//     world.insert_resource(SystemRan::No);

//     fn sys(_marker: ExclusiveMarker, mut system_ran: ResMut<SystemRan>) {
//         *system_ran = SystemRan::Yes;
//     }

//     let mut sys = IntoSystem::into_system(sys);
//     sys.initialize(&mut world);
//     assert!(sys.is_exclusive());

//     run_system(&mut world, sys);
//     assert_eq!(*world.resource::<SystemRan>(), SystemRan::Yes);
// }

// #[test]
// fn removal_tracking() {
//     let mut world = World::new();

//     let entity_to_despawn = world.spawn(W(1)).id();
//     let entity_to_remove_w_from = world.spawn(W(2)).id();
//     let spurious_entity = world.spawn_empty().id();

//     // Track which entities we want to operate on
//     #[derive(Resource)]
//     struct Despawned(Entity);
//     world.insert_resource(Despawned(entity_to_despawn));

//     #[derive(Resource)]
//     struct Removed(Entity);
//     world.insert_resource(Removed(entity_to_remove_w_from));

//     // Verify that all the systems actually ran
//     #[derive(Default, Resource)]
//     struct NSystems(usize);
//     world.insert_resource(NSystems::default());

//     // First, check that removal detection is triggered if and only if we despawn an entity with the correct component
//     world.entity_mut(entity_to_despawn).despawn();
//     world.entity_mut(spurious_entity).despawn();

//     fn validate_despawn(
//         mut removed_i32: RemovedComponents<W<i32>>,
//         despawned: Res<Despawned>,
//         mut n_systems: ResMut<NSystems>,
//     ) {
//         assert_eq!(
//             removed_i32.read().collect::<Vec<_>>(),
//             &[despawned.0],
//             "despawning causes the correct entity to show up in the 'RemovedComponent' system parameter."
//         );

//         n_systems.0 += 1;
//     }

//     run_system(&mut world, validate_despawn);

//     // Reset the trackers to clear the buffer of removed components
//     // Ordinarily, this is done in a system added by MinimalPlugins
//     world.clear_trackers();

//     // Then, try removing a component
//     world.spawn(W(3));
//     world.spawn(W(4));
//     world.entity_mut(entity_to_remove_w_from).remove::<W<i32>>();

//     fn validate_remove(
//         mut removed_i32: RemovedComponents<W<i32>>,
//         despawned: Res<Despawned>,
//         removed: Res<Removed>,
//         mut n_systems: ResMut<NSystems>,
//     ) {
//         // The despawned entity from the previous frame was
//         // double buffered so we now have it in this system as well.
//         assert_eq!(
//             removed_i32.read().collect::<Vec<_>>(),
//             &[despawned.0, removed.0],
//             "removing a component causes the correct entity to show up in the 'RemovedComponent' system parameter."
//         );

//         n_systems.0 += 1;
//     }

//     run_system(&mut world, validate_remove);

//     // Verify that both systems actually ran
//     assert_eq!(world.resource::<NSystems>().0, 2);
// }

// #[test]
// fn world_collections_system() {
//     let mut world = World::default();
//     world.insert_resource(SystemRan::No);
//     world.spawn((W(42), W(true)));
//     fn sys(
//         archetypes: &Archetypes,
//         components: &Components,
//         entities: &Entities,
//         bundles: &Bundles,
//         query: Query<Entity, With<W<i32>>>,
//         mut system_ran: ResMut<SystemRan>,
//     ) {
//         assert_eq!(query.iter().count(), 1, "entity exists");
//         for entity in &query {
//             let location = entities.get_spawned(entity).unwrap();
//             let archetype = archetypes.get(location.archetype_id).unwrap();
//             let archetype_components = archetype.components();
//             let bundle_id = bundles
//                 .get_id(TypeId::of::<(W<i32>, W<bool>)>())
//                 .expect("Bundle used to spawn entity should exist");
//             let bundle_info = bundles.get(bundle_id).unwrap();
//             let mut bundle_components = bundle_info.contributed_components().to_vec();
//             bundle_components.sort();
//             for component_id in &bundle_components {
//                 assert!(
//                     components.get_info(*component_id).is_some(),
//                     "every bundle component exists in Components"
//                 );
//             }
//             assert_eq!(
//                 bundle_components, archetype_components,
//                 "entity's bundle components exactly match entity's archetype components"
//             );
//         }
//         *system_ran = SystemRan::Yes;
//     }

//     run_system(&mut world, sys);

//     // ensure the system actually ran
//     assert_eq!(*world.resource::<SystemRan>(), SystemRan::Yes);
// }

// #[test]
// fn get_system_conflicts() {
//     fn sys_x(_: Res<ResA>, _: Res<ResB>, _: Query<(&C, &D)>) {}

//     fn sys_y(_: Res<ResA>, _: ResMut<ResB>, _: Query<(&C, &mut D)>) {}

//     let mut world = World::default();
//     let mut x = IntoSystem::into_system(sys_x);
//     let mut y = IntoSystem::into_system(sys_y);
//     let x_access = x.initialize(&mut world);
//     let y_access = y.initialize(&mut world);

//     let conflicts = x_access.get_conflicts(&y_access);
//     let b_id = world.components().get_id(TypeId::of::<ResB>()).unwrap();
//     let d_id = world.components().get_id(TypeId::of::<D>()).unwrap();
//     assert_eq!(conflicts, vec![b_id, d_id].into());
// }

// #[test]
// fn query_is_empty() {
//     fn without_filter(not_empty: Query<&A>, empty: Query<&B>) {
//         assert!(!not_empty.is_empty());
//         assert!(empty.is_empty());
//     }

//     fn with_filter(not_empty: Query<&A, With<C>>, empty: Query<&A, With<D>>) {
//         assert!(!not_empty.is_empty());
//         assert!(empty.is_empty());
//     }

//     let mut world = World::default();
//     world.spawn(A).insert(C);

//     let mut without_filter = IntoSystem::into_system(without_filter);
//     without_filter.initialize(&mut world);
//     without_filter.run((), &mut world).unwrap();

//     let mut with_filter = IntoSystem::into_system(with_filter);
//     with_filter.initialize(&mut world);
//     with_filter.run((), &mut world).unwrap();
// }

// #[test]
// fn can_have_16_parameters() {
//     fn sys_x(
//         _: Res<ResA>,
//         _: Res<ResB>,
//         _: Res<ResC>,
//         _: Res<ResD>,
//         _: Res<ResE>,
//         _: Res<ResF>,
//         _: Query<&A>,
//         _: Query<&B>,
//         _: Query<&C>,
//         _: Query<&D>,
//         _: Query<&E>,
//         _: Query<&F>,
//         _: Query<(&A, &B)>,
//         _: Query<(&C, &D)>,
//         _: Query<(&E, &F)>,
//     ) {
//     }
//     fn sys_y(
//         _: (
//             Res<ResA>,
//             Res<ResB>,
//             Res<ResC>,
//             Res<ResD>,
//             Res<ResE>,
//             Res<ResF>,
//             Query<&A>,
//             Query<&B>,
//             Query<&C>,
//             Query<&D>,
//             Query<&E>,
//             Query<&F>,
//             Query<(&A, &B)>,
//             Query<(&C, &D)>,
//             Query<(&E, &F)>,
//         ),
//     ) {
//     }
//     let mut world = World::default();
//     let mut x = IntoSystem::into_system(sys_x);
//     let mut y = IntoSystem::into_system(sys_y);
//     x.initialize(&mut world);
//     y.initialize(&mut world);
// }

// #[test]
// fn read_system_state() {
//     #[derive(Eq, PartialEq, Debug, Resource)]
//     struct A(usize);

//     #[derive(Component, Eq, PartialEq, Debug)]
//     struct B(usize);

//     let mut world = World::default();
//     world.insert_resource(A(42));
//     world.spawn(B(7));

//     let mut system_state: SystemState<(
//         Res<A>,
//         Option<Single<&B>>,
//         ParamSet<(Query<&C>, Query<&D>)>,
//     )> = SystemState::new(&mut world);
//     let (a, query, _) = system_state.get(&world).unwrap();
//     assert_eq!(*a, A(42), "returned resource matches initial value");
//     assert_eq!(
//         **query.unwrap(),
//         B(7),
//         "returned component matches initial value"
//     );
// }

// #[test]
// fn write_system_state() {
//     #[derive(Resource, Eq, PartialEq, Debug)]
//     struct A(usize);

//     #[derive(Component, Eq, PartialEq, Debug)]
//     struct B(usize);

//     let mut world = World::default();
//     world.insert_resource(A(42));
//     world.spawn(B(7));

//     let mut system_state: SystemState<(ResMut<A>, Option<Single<&mut B>>)> =
//         SystemState::new(&mut world);

//     // The following line shouldn't compile because the parameters used are not ReadOnlySystemParam
//     // let (a, query) = system_state.get(&world);

//     let (a, query) = system_state.get_mut(&mut world).unwrap();
//     assert_eq!(*a, A(42), "returned resource matches initial value");
//     assert_eq!(
//         **query.unwrap(),
//         B(7),
//         "returned component matches initial value"
//     );
// }

// #[test]
// fn system_state_change_detection() {
//     #[derive(Component, Eq, PartialEq, Debug)]
//     struct A(usize);

//     let mut world = World::default();
//     let entity = world.spawn(A(1)).id();

//     let mut system_state: SystemState<Option<Single<&A, Changed<A>>>> =
//         SystemState::new(&mut world);
//     {
//         let query = system_state.get(&world).unwrap();
//         assert_eq!(**query.unwrap(), A(1));
//     }

//     {
//         let query = system_state.get(&world).unwrap();
//         assert!(query.is_none());
//     }

//     world.entity_mut(entity).get_mut::<A>().unwrap().0 = 2;
//     {
//         let query = system_state.get(&world).unwrap();
//         assert_eq!(**query.unwrap(), A(2));
//     }
// }

// #[test]
// fn system_state_spawned() {
//     let mut world = World::default();
//     world.spawn(A);
//     let spawn_tick = world.change_tick();

//     let mut system_state: SystemState<Option<Single<(&A, SpawnDetails), Spawned>>> =
//         SystemState::new(&mut world);
//     {
//         let query = system_state.get(&world).unwrap();
//         assert_eq!(query.unwrap().1.spawn_tick(), spawn_tick);
//     }

//     {
//         let query = system_state.get(&world).unwrap();
//         assert!(query.is_none());
//     }
// }

// #[test]
// #[should_panic]
// fn system_state_invalid_world() {
//     let mut world = World::default();
//     let mut system_state = SystemState::<Query<&A>>::new(&mut world);
//     let mismatched_world = World::default();
//     system_state.get(&mismatched_world).unwrap();
// }

// #[test]
// fn system_state_archetype_update() {
//     #[derive(Component, Eq, PartialEq, Debug)]
//     struct A(usize);

//     #[derive(Component, Eq, PartialEq, Debug)]
//     struct B(usize);

//     let mut world = World::default();
//     world.spawn(A(1));

//     let mut system_state = SystemState::<Query<&A>>::new(&mut world);
//     {
//         let query = system_state.get(&world).unwrap();
//         assert_eq!(
//             query.iter().collect::<Vec<_>>(),
//             vec![&A(1)],
//             "exactly one component returned"
//         );
//     }

//     world.spawn((A(2), B(2)));
//     {
//         let query = system_state.get(&world).unwrap();
//         assert_eq!(
//             query.iter().collect::<Vec<_>>(),
//             vec![&A(1), &A(2)],
//             "components from both archetypes returned"
//         );
//     }
// }

// #[test]
// #[expect(
//     dead_code,
//     reason = "This test exists to show that read-only world-only queries can return data that lives as long as `'world`."
// )]
// fn long_life_test() {
//     struct ResourceHolder<'w> {
//         value: &'w ResA,
//     }

//     struct Holder<'w> {
//         value: &'w A,
//     }

//     struct State {
//         state: SystemState<Res<'static, ResA>>,
//         state_q: SystemState<Query<'static, 'static, &'static A>>,
//     }

//     impl State {
//         fn hold_res<'w>(&mut self, world: &'w World) -> ResourceHolder<'w> {
//             let a = self.state.get(world).unwrap();
//             ResourceHolder {
//                 value: a.into_inner(),
//             }
//         }
//         fn hold_component<'w>(&mut self, world: &'w World, entity: Entity) -> Holder<'w> {
//             let q = self.state_q.get(world).unwrap();
//             let a = q.get_inner(entity).unwrap();
//             Holder { value: a }
//         }
//         fn hold_components<'w>(&mut self, world: &'w World) -> Vec<Holder<'w>> {
//             let mut components = Vec::new();
//             let q = self.state_q.get(world).unwrap();
//             for a in q.iter_inner() {
//                 components.push(Holder { value: a });
//             }
//             components
//         }
//     }
// }

// #[test]
// fn immutable_mut_test() {
//     #[derive(Component, Eq, PartialEq, Debug, Clone, Copy)]
//     struct A(usize);

//     let mut world = World::default();
//     world.spawn(A(1));
//     world.spawn(A(2));

//     let mut system_state = SystemState::<Query<&mut A>>::new(&mut world);
//     {
//         let mut query = system_state.get_mut(&mut world).unwrap();
//         assert_eq!(
//             query.iter_mut().map(|m| *m).collect::<Vec<A>>(),
//             vec![A(1), A(2)],
//             "both components returned by iter_mut of &mut"
//         );
//         assert_eq!(
//             query.iter().collect::<Vec<&A>>(),
//             vec![&A(1), &A(2)],
//             "both components returned by iter of &mut"
//         );
//     }
// }

// #[test]
// fn convert_mut_to_immut() {
//     {
//         let mut world = World::new();

//         fn mutable_query(mut query: Query<&mut A>) {
//             for _ in &mut query {}

//             immutable_query(query.as_readonly());
//         }

//         fn immutable_query(_: Query<&A>) {}

//         let mut sys = IntoSystem::into_system(mutable_query);
//         sys.initialize(&mut world);
//     }

//     {
//         let mut world = World::new();

//         fn mutable_query(mut query: Query<Option<&mut A>>) {
//             for _ in &mut query {}

//             immutable_query(query.as_readonly());
//         }

//         fn immutable_query(_: Query<Option<&A>>) {}

//         let mut sys = IntoSystem::into_system(mutable_query);
//         sys.initialize(&mut world);
//     }

//     {
//         let mut world = World::new();

//         fn mutable_query(mut query: Query<(&mut A, &B)>) {
//             for _ in &mut query {}

//             immutable_query(query.as_readonly());
//         }

//         fn immutable_query(_: Query<(&A, &B)>) {}

//         let mut sys = IntoSystem::into_system(mutable_query);
//         sys.initialize(&mut world);
//     }

//     {
//         let mut world = World::new();

//         fn mutable_query(mut query: Query<(&mut A, &mut B)>) {
//             for _ in &mut query {}

//             immutable_query(query.as_readonly());
//         }

//         fn immutable_query(_: Query<(&A, &B)>) {}

//         let mut sys = IntoSystem::into_system(mutable_query);
//         sys.initialize(&mut world);
//     }

//     {
//         let mut world = World::new();

//         fn mutable_query(mut query: Query<(&mut A, &mut B), With<C>>) {
//             for _ in &mut query {}

//             immutable_query(query.as_readonly());
//         }

//         fn immutable_query(_: Query<(&A, &B), With<C>>) {}

//         let mut sys = IntoSystem::into_system(mutable_query);
//         sys.initialize(&mut world);
//     }

//     {
//         let mut world = World::new();

//         fn mutable_query(mut query: Query<(&mut A, &mut B), Without<C>>) {
//             for _ in &mut query {}

//             immutable_query(query.as_readonly());
//         }

//         fn immutable_query(_: Query<(&A, &B), Without<C>>) {}

//         let mut sys = IntoSystem::into_system(mutable_query);
//         sys.initialize(&mut world);
//     }

//     {
//         let mut world = World::new();

//         fn mutable_query(mut query: Query<(&mut A, &mut B), Added<C>>) {
//             for _ in &mut query {}

//             immutable_query(query.as_readonly());
//         }

//         fn immutable_query(_: Query<(&A, &B), Added<C>>) {}

//         let mut sys = IntoSystem::into_system(mutable_query);
//         sys.initialize(&mut world);
//     }

//     {
//         let mut world = World::new();

//         fn mutable_query(mut query: Query<(&mut A, &mut B), Changed<C>>) {
//             for _ in &mut query {}

//             immutable_query(query.as_readonly());
//         }

//         fn immutable_query(_: Query<(&A, &B), Changed<C>>) {}

//         let mut sys = IntoSystem::into_system(mutable_query);
//         sys.initialize(&mut world);
//     }

//     {
//         let mut world = World::new();

//         fn mutable_query(mut query: Query<(&mut A, &mut B, SpawnDetails), Spawned>) {
//             for _ in &mut query {}

//             immutable_query(query.as_readonly());
//         }

//         fn immutable_query(_: Query<(&A, &B, SpawnDetails), Spawned>) {}

//         let mut sys = IntoSystem::into_system(mutable_query);
//         sys.initialize(&mut world);
//     }
// }

// #[test]
// fn commands_param_set() {
//     // Regression test for #4676
//     let mut world = World::new();
//     let entity = world.spawn_empty().id();

//     run_system(
//         &mut world,
//         move |mut commands_set: ParamSet<(Commands, Commands)>| {
//             commands_set.p0().entity(entity).insert(A);
//             commands_set.p1().entity(entity).insert(B);
//         },
//     );

//     let entity = world.entity(entity);
//     assert!(entity.contains::<A>());
//     assert!(entity.contains::<B>());
// }

// #[test]
// fn into_iter_impl() {
//     let mut world = World::new();
//     world.spawn(W(42u32));
//     run_system(&mut world, |mut q: Query<&mut W<u32>>| {
//         for mut a in &mut q {
//             assert_eq!(a.0, 42);
//             a.0 = 0;
//         }
//         for a in &q {
//             assert_eq!(a.0, 0);
//         }
//     });
// }

// #[test]
// #[should_panic]
// fn assert_system_does_not_conflict() {
//     fn system(_query: Query<(&mut W<u32>, &mut W<u32>)>) {}
//     super::assert_system_does_not_conflict(system);
// }

// #[test]
// #[should_panic]
// fn assert_world_and_entity_mut_system_does_conflict_first() {
//     fn system(_query: &World, _q2: Query<EntityMut>) {}
//     super::assert_system_does_not_conflict(system);
// }

// #[test]
// #[should_panic]
// fn assert_world_and_entity_mut_system_does_conflict_second() {
//     fn system(_: Query<EntityMut>, _: &World) {}
//     super::assert_system_does_not_conflict(system);
// }

// #[test]
// #[should_panic]
// fn assert_entity_ref_and_entity_mut_system_does_conflict() {
//     fn system(_query: Query<EntityRef>, _q2: Query<EntityMut>) {}
//     super::assert_system_does_not_conflict(system);
// }

// #[test]
// #[should_panic]
// fn assert_entity_mut_system_does_conflict() {
//     fn system(_query: Query<EntityMut>, _q2: Query<EntityMut>) {}
//     super::assert_system_does_not_conflict(system);
// }

// #[test]
// #[should_panic]
// fn assert_deferred_world_and_entity_ref_system_does_conflict_first() {
//     fn system(_world: DeferredWorld, _query: Query<EntityRef>) {}
//     super::assert_system_does_not_conflict(system);
// }

// #[test]
// #[should_panic]
// fn assert_deferred_world_and_entity_ref_system_does_conflict_second() {
//     fn system(_query: Query<EntityRef>, _world: DeferredWorld) {}
//     super::assert_system_does_not_conflict(system);
// }

// #[test]
// fn assert_deferred_world_and_empty_query_does_not_conflict_first() {
//     fn system(_world: DeferredWorld, _query: Query<Entity>) {}
//     super::assert_system_does_not_conflict(system);
// }

// #[test]
// fn assert_deferred_world_and_empty_query_does_not_conflict_second() {
//     fn system(_query: Query<Entity>, _world: DeferredWorld) {}
//     super::assert_system_does_not_conflict(system);
// }

// #[test]
// #[should_panic]
// fn panic_inside_system() {
//     let mut world = World::new();
//     let system: fn() = || {
//         panic!("this system panics");
//     };
//     run_system(&mut world, system);
// }

// #[test]
// fn assert_systems() {
//     use core::str::FromStr;

//     use crate::ecs::system::assert_is_system;

//     /// Mocks a system that returns a value of type `T`.
//     fn returning<T>() -> T {
//         unimplemented!()
//     }

//     /// Mocks an exclusive system that takes an input and returns an output.
//     fn exclusive_in_out<A, B>(_: In<A>, _: &mut World) -> B {
//         unimplemented!()
//     }

//     fn static_system_param(_: StaticSystemParam<Query<'static, 'static, &W<u32>>>) {
//         unimplemented!()
//     }

//     fn exclusive_with_state(
//         _: &mut World,
//         _: Local<bool>,
//         _: (&mut QueryState<&W<i32>>, &mut SystemState<Query<&W<u32>>>),
//         _: (),
//     ) {
//         unimplemented!()
//     }

//     fn not(In(val): In<bool>) -> bool {
//         !val
//     }

//     assert_is_system(returning::<Result<u32, std::io::Error>>.map(Result::unwrap));
//     assert_is_system(returning::<Option<()>>.map(drop));
//     assert_is_system(returning::<&str>.map(u64::from_str).map(Result::unwrap));
//     assert_is_system(static_system_param);
//     assert_is_system(
//         exclusive_in_out::<(), Result<(), std::io::Error>>.map(|_out| {
//             #[cfg(feature = "trace")]
//             if let Err(error) = _out {
//                 tracing::error!("{}", error);
//             }
//         }),
//     );
//     assert_is_system(exclusive_with_state);
//     assert_is_system(returning::<bool>.pipe(exclusive_in_out::<bool, ()>));

//     returning::<()>.run_if(returning::<bool>.pipe(not));
// }

// #[test]
// fn pipe_change_detection() {
//     #[derive(Resource, Default)]
//     struct Flag;

//     #[derive(Default)]
//     struct Info {
//         // If true, the respective system will mutate `Flag`.
//         do_first: bool,
//         do_second: bool,

//         // Will be set to true if the respective system saw that `Flag` changed.
//         first_flag: bool,
//         second_flag: bool,
//     }

//     fn first(In(mut info): In<Info>, mut flag: ResMut<Flag>) -> Info {
//         if flag.is_changed() {
//             info.first_flag = true;
//         }
//         if info.do_first {
//             *flag = Flag;
//         }

//         info
//     }

//     fn second(In(mut info): In<Info>, mut flag: ResMut<Flag>) -> Info {
//         if flag.is_changed() {
//             info.second_flag = true;
//         }
//         if info.do_second {
//             *flag = Flag;
//         }

//         info
//     }

//     let mut world = World::new();
//     world.init_resource::<Flag>();
//     let mut sys = IntoSystem::into_system(first.pipe(second));
//     sys.initialize(&mut world);

//     sys.run(default(), &mut world).unwrap();

//     // The second system should observe a change made in the first system.
//     let info = sys
//         .run(
//             Info {
//                 do_first: true,
//                 ..default()
//             },
//             &mut world,
//         )
//         .unwrap();
//     assert!(!info.first_flag);
//     assert!(info.second_flag);

//     // When a change is made in the second system, the first system
//     // should observe it the next time they are run.
//     let info1 = sys
//         .run(
//             Info {
//                 do_second: true,
//                 ..default()
//             },
//             &mut world,
//         )
//         .unwrap();
//     let info2 = sys.run(default(), &mut world).unwrap();
//     assert!(!info1.first_flag);
//     assert!(!info1.second_flag);
//     assert!(info2.first_flag);
//     assert!(!info2.second_flag);
// }

// #[test]
// fn test_combinator_clone() {
//     let mut world = World::new();
//     #[derive(Resource)]
//     struct A;
//     #[derive(Resource)]
//     struct B;
//     #[derive(Resource, PartialEq, Eq, Debug)]
//     struct C(i32);

//     world.insert_resource(A);
//     world.insert_resource(C(0));
//     let mut sched = Schedule::default();
//     sched.add_systems(
//         (
//             |mut res: ResMut<C>| {
//                 res.0 += 1;
//             },
//             |mut res: ResMut<C>| {
//                 res.0 += 2;
//             },
//         )
//             .distributive_run_if(resource_exists::<A>.or_eager(resource_exists::<B>)),
//     );
//     sched.initialize(&mut world).unwrap();
//     sched.run(&mut world);
//     assert_eq!(world.get_resource(), Some(&C(3)));
// }

// #[test]
// #[cfg_attr(not(feature = "debug"), ignore)]
// #[should_panic(
//     expected = "Encountered an error in system `bevy_ecs::system::tests::simple_fallible_system::sys`: error"
// )]
// fn simple_fallible_system() {
//     fn sys() -> Result {
//         Err("error")?;
//         Ok(())
//     }

//     let mut world = World::new();
//     run_system(&mut world, sys);
// }

// #[test]
// #[cfg_attr(not(feature = "debug"), ignore)]
// #[should_panic(
//     expected = "Encountered an error in system `bevy_ecs::system::tests::simple_fallible_exclusive_system::sys`: error"
// )]
// fn simple_fallible_exclusive_system() {
//     fn sys(_world: &mut World) -> Result {
//         Err("error")?;
//         Ok(())
//     }

//     let mut world = World::new();
//     run_system(&mut world, sys);
// }

// // Regression test for
// // https://github.com/bevyengine/bevy/issues/18778
// //
// // Dear rustc team, please reach out if you encounter this
// // in a crater run and we can work something out!
// //
// // These todo! macro calls should never be removed;
// // they're intended to demonstrate real-world usage
// // in a way that's clearer than simply calling `panic!`
// //
// // Because type inference behaves differently for functions and closures,
// // we need to test both, in addition to explicitly annotating the return type
// // to ensure that there are no upstream regressions there.
// #[test]
// fn nondiverging_never_trait_impls() {
//     // This test is a compilation test:
//     // no meaningful logic is ever actually evaluated.
//     // It is simply intended to check that the correct traits are implemented
//     // when todo! or similar nondiverging panics are used.
//     let mut world = World::new();
//     let mut schedule = Schedule::default();

//     fn sys(_query: Query<&Name>) {
//         todo!()
//     }

//     schedule.add_systems(sys);
//     schedule.add_systems(|_query: Query<&Name>| {});
//     schedule.add_systems(|_query: Query<&Name>| todo!());
//     schedule.add_systems(|_query: Query<&Name>| -> () { todo!() });

//     fn obs(_event: On<Add, Name>) {
//         todo!()
//     }

//     world.add_observer(obs);
//     world.add_observer(|_event: On<Add, Name>| {});
//     world.add_observer(|_event: On<Add, Name>| todo!());
//     world.add_observer(|_event: On<Add, Name>| -> () { todo!() });

//     fn my_command(_world: &mut World) {
//         todo!()
//     }

//     world.commands().queue(my_command);
//     world.commands().queue(|_world: &mut World| {});
//     world.commands().queue(|_world: &mut World| todo!());
//     world
//         .commands()
//         .queue(|_world: &mut World| -> () { todo!() });
// }

// #[test]
// fn with_input() {
//     fn sys(InMut(v): InMut<usize>) {
//         *v += 1;
//     }

//     let mut world = World::new();
//     let mut system = IntoSystem::into_system(sys.with_input(42));
//     system.initialize(&mut world);
//     system.run((), &mut world).unwrap();
//     assert_eq!(*system.value(), 43);
// }

// #[test]
// fn with_input_from() {
//     struct TestData(usize);

//     impl FromWorld for TestData {
//         fn from_world(_world: &mut World) -> Self {
//             Self(5)
//         }
//     }

//     fn sys(InMut(v): InMut<TestData>) {
//         v.0 += 1;
//     }

//     let mut world = World::new();
//     let mut system = IntoSystem::into_system(sys.with_input_from::<TestData>());
//     assert!(system.value().is_none());
//     system.initialize(&mut world);
//     assert!(system.value().is_some());
//     system.run((), &mut world).unwrap();
//     assert_eq!(system.value().unwrap().0, 6);
// }
