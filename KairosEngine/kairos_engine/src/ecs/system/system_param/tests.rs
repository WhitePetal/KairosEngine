// TODO!
// #[test]
// #[should_panic]
// fn non_send_alias() {
//     #[derive(Resource)]
//     struct A(usize);
//     fn my_system(mut res0: NonSendMut<A>, mut res1: NonSendMut<A>) {
//         res0.0 += 1;
//         res1.0 += 1;
//     }
//     let mut world = World::new();
//     world.insert_non_send(A(42));
//     let mut schedule = crate::schedule::Schedule::default();
//     schedule.add_systems(my_system);
//     schedule.run(&mut world);
// }

// #[test]
// #[should_panic]
// fn non_send_and_entities() {
//     #[derive(Resource)]
//     struct A(usize);
//     fn my_system(mut ns: NonSendMut<A>, _: Query<EntityMut>) {
//         ns.0 += 1;
//     }
//     assert_is_system(my_system);
// }

// #[test]
// #[should_panic]
// fn res_and_entities() {
//     #[derive(Resource)]
//     struct A(usize);
//     fn my_system(mut res: ResMut<A>, _: Query<EntityMut>) {
//         res.0 += 1;
//     }
//     assert_is_system(my_system);
// }

// #[test]
// fn res_and_entities_filtered() {
//     #[derive(Resource)]
//     struct A(usize);
//     fn res_system(mut res: ResMut<A>, _: Query<EntityMut, Without<IsResource>>) {
//         res.0 += 1;
//     }
//     assert_is_system(res_system);

//     fn non_send_system(mut ns: NonSendMut<A>, _: Query<EntityMut, Without<A>>) {
//         ns.0 += 1;
//     }

//     assert_is_system(non_send_system);
// }

// // Compile test for https://github.com/bevyengine/bevy/pull/2838.
// #[test]
// fn system_param_generic_bounds() {
//     #[derive(SystemParam)]
//     pub struct SpecialQuery<
//         'w,
//         's,
//         D: QueryData + Send + Sync + 'static,
//         F: QueryFilter + Send + Sync + 'static = (),
//     > {
//         _query: Query<'w, 's, D, F>,
//     }

//     fn my_system(_: SpecialQuery<(), ()>) {}
//     assert_is_system(my_system);
// }

// // Compile tests for https://github.com/bevyengine/bevy/pull/6694.
// #[test]
// fn system_param_flexibility() {
//     #[derive(SystemParam)]
//     pub struct SpecialRes<'w, T: Resource> {
//         _res: Res<'w, T>,
//     }

//     #[derive(SystemParam)]
//     pub struct SpecialLocal<'s, T: FromWorld + Send + 'static> {
//         _local: Local<'s, T>,
//     }

//     #[derive(Resource)]
//     struct R;

//     fn my_system(_: SpecialRes<R>, _: SpecialLocal<u32>) {}
//     assert_is_system(my_system);
// }

// #[derive(Resource)]
// pub struct R<const I: usize>;

// // Compile test for https://github.com/bevyengine/bevy/pull/7001.
// #[test]
// fn system_param_const_generics() {
//     #[expect(
//         dead_code,
//         reason = "This struct is used to ensure that const generics are supported as a SystemParam; thus, the inner value never needs to be read."
//     )]
//     #[derive(SystemParam)]
//     pub struct ConstGenericParam<'w, const I: usize>(Res<'w, R<I>>);

//     fn my_system(_: ConstGenericParam<0>, _: ConstGenericParam<1000>) {}
//     assert_is_system(my_system);
// }

// // Compile test for https://github.com/bevyengine/bevy/pull/6867.
// #[test]
// fn system_param_field_limit() {
//     #[derive(SystemParam)]
//     pub struct LongParam<'w> {
//         // Each field should be a distinct type so there will
//         // be an error if the derive messes up the field order.
//         _r0: Res<'w, R<0>>,
//         _r1: Res<'w, R<1>>,
//         _r2: Res<'w, R<2>>,
//         _r3: Res<'w, R<3>>,
//         _r4: Res<'w, R<4>>,
//         _r5: Res<'w, R<5>>,
//         _r6: Res<'w, R<6>>,
//         _r7: Res<'w, R<7>>,
//         _r8: Res<'w, R<8>>,
//         _r9: Res<'w, R<9>>,
//         _r10: Res<'w, R<10>>,
//         _r11: Res<'w, R<11>>,
//         _r12: Res<'w, R<12>>,
//         _r13: Res<'w, R<13>>,
//         _r14: Res<'w, R<14>>,
//         _r15: Res<'w, R<15>>,
//         _r16: Res<'w, R<16>>,
//     }

//     fn long_system(_: LongParam) {}
//     assert_is_system(long_system);
// }

// // Compile test for https://github.com/bevyengine/bevy/pull/6919.
// // Regression test for https://github.com/bevyengine/bevy/issues/7447.
// #[test]
// fn system_param_phantom_data() {
//     #[derive(SystemParam)]
//     struct PhantomParam<'w, T: Resource, Marker: 'static> {
//         _foo: Res<'w, T>,
//         marker: PhantomData<&'w Marker>,
//     }

//     fn my_system(_: PhantomParam<R<0>, ()>) {}
//     assert_is_system(my_system);
// }

// // Compile tests for https://github.com/bevyengine/bevy/pull/6957.
// #[test]
// fn system_param_struct_variants() {
//     #[derive(SystemParam)]
//     pub struct UnitParam;

//     #[expect(
//         dead_code,
//         reason = "This struct is used to ensure that tuple structs are supported as a SystemParam; thus, the inner values never need to be read."
//     )]
//     #[derive(SystemParam)]
//     pub struct TupleParam<'w, 's, R: Resource, L: FromWorld + Send + 'static>(
//         Res<'w, R>,
//         Local<'s, L>,
//     );

//     fn my_system(_: UnitParam, _: TupleParam<R<0>, u32>) {}
//     assert_is_system(my_system);
// }

// // Regression test for https://github.com/bevyengine/bevy/issues/4200.
// #[test]
// fn system_param_private_fields() {
//     #[derive(Resource)]
//     struct PrivateResource;

//     #[expect(
//         dead_code,
//         reason = "This struct is used to ensure that SystemParam's derive can't leak private fields; thus, the inner values never need to be read."
//     )]
//     #[derive(SystemParam)]
//     pub struct EncapsulatedParam<'w>(Res<'w, PrivateResource>);

//     fn my_system(_: EncapsulatedParam) {}
//     assert_is_system(my_system);
// }

// // Regression test for https://github.com/bevyengine/bevy/issues/7103.
// #[test]
// fn system_param_where_clause() {
//     #[derive(SystemParam)]
//     pub struct WhereParam<'w, 's, D>
//     where
//         D: 'static + QueryData,
//     {
//         _q: Query<'w, 's, D, ()>,
//     }

//     fn my_system(_: WhereParam<()>) {}
//     assert_is_system(my_system);
// }

// // Regression test for https://github.com/bevyengine/bevy/issues/1727.
// #[test]
// fn system_param_name_collision() {
//     #[derive(Resource)]
//     pub struct FetchState;

//     #[derive(SystemParam)]
//     pub struct Collide<'w> {
//         _x: Res<'w, FetchState>,
//     }

//     fn my_system(_: Collide) {}
//     assert_is_system(my_system);
// }

// // Regression test for https://github.com/bevyengine/bevy/issues/8192.
// #[test]
// fn system_param_invariant_lifetime() {
//     #[derive(SystemParam)]
//     pub struct InvariantParam<'w, 's> {
//         _set: ParamSet<'w, 's, (Query<'w, 's, ()>,)>,
//     }

//     fn my_system(_: InvariantParam) {}
//     assert_is_system(my_system);
// }

// // Compile test for https://github.com/bevyengine/bevy/pull/9589.
// #[test]
// fn non_sync_local() {
//     fn non_sync_system(cell: Local<RefCell<u8>>) {
//         assert_eq!(*cell.borrow(), 0);
//     }

//     let mut world = World::new();
//     let mut schedule = crate::schedule::Schedule::default();
//     schedule.add_systems(non_sync_system);
//     schedule.run(&mut world);
// }

// // Regression test for https://github.com/bevyengine/bevy/issues/10207.
// #[test]
// fn param_set_non_send_first() {
//     fn non_send_param_set(mut p: ParamSet<(NonSend<*mut u8>, ())>) {
//         let _ = p.p0();
//         p.p1();
//     }

//     let mut world = World::new();
//     world.insert_non_send(core::ptr::null_mut::<u8>());
//     let mut schedule = crate::schedule::Schedule::default();
//     schedule.add_systems((non_send_param_set, non_send_param_set, non_send_param_set));
//     schedule.run(&mut world);
// }

// // Regression test for https://github.com/bevyengine/bevy/issues/10207.
// #[test]
// fn param_set_non_send_second() {
//     fn non_send_param_set(mut p: ParamSet<((), NonSendMut<*mut u8>)>) {
//         p.p0();
//         let _ = p.p1();
//     }

//     let mut world = World::new();
//     world.insert_non_send(core::ptr::null_mut::<u8>());
//     let mut schedule = crate::schedule::Schedule::default();
//     schedule.add_systems((non_send_param_set, non_send_param_set, non_send_param_set));
//     schedule.run(&mut world);
// }

// fn _dyn_system_param_type_inference(mut p: DynSystemParam) {
//     // Make sure the downcast() methods are able to infer their type parameters from the use of the return type.
//     // This is just a compilation test, so there is nothing to run.
//     let _query: Query<()> = p.downcast_mut().unwrap();
//     let _query: Query<()> = p.downcast_mut_inner().unwrap();
//     let _query: Query<()> = p.downcast().unwrap();
// }

// #[test]
// #[should_panic]
// fn missing_resource_error() {
//     #[derive(Resource)]
//     pub struct MissingResource;

//     let mut schedule = crate::schedule::Schedule::default();
//     schedule.add_systems(res_system);
//     let mut world = World::new();
//     schedule.run(&mut world);

//     fn res_system(_: Res<MissingResource>) {}
// }

// #[test]
// #[should_panic]
// fn missing_message_error() {
//     use crate::prelude::{Message, MessageReader};

//     #[derive(Message)]
//     pub struct MissingEvent;

//     let mut schedule = crate::schedule::Schedule::default();
//     schedule.add_systems(message_system);
//     let mut world = World::new();
//     schedule.run(&mut world);

//     fn message_system(_: MessageReader<MissingEvent>) {}
// }
