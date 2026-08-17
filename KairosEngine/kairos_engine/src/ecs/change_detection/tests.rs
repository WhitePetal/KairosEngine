// #[derive(Component, PartialEq)]
// struct C;

// #[derive(Resource)]
// struct R;

// #[derive(Resource, PartialEq)]
// struct R2(u8);

// impl Deref for R2 {
//     type Target = u8;
//     fn deref(&self) -> &u8 {
//         &self.0
//     }
// }

// impl DerefMut for R2 {
//     fn deref_mut(&mut self) -> &mut u8 {
//         &mut self.0
//     }
// }

// #[test]
// fn change_expiration() {
//     fn change_detected(query: Option<Single<Ref<C>>>) -> bool {
//         query.unwrap().is_changed()
//     }

//     fn change_expired(query: Option<Single<Ref<C>>>) -> bool {
//         query.unwrap().is_changed()
//     }

//     let mut world = World::new();

//     // component added: 1, changed: 1
//     world.spawn(C);

//     let mut change_detected_system = IntoSystem::into_system(change_detected);
//     let mut change_expired_system = IntoSystem::into_system(change_expired);
//     change_detected_system.initialize(&mut world);
//     change_expired_system.initialize(&mut world);

//     // world: 1, system last ran: 0, component changed: 1
//     // The spawn will be detected since it happened after the system "last ran".
//     assert!(change_detected_system.run((), &mut world).unwrap());

//     // world: 1 + MAX_CHANGE_AGE
//     let change_tick = world.change_tick.get_mut();
//     *change_tick = change_tick.wrapping_add(MAX_CHANGE_AGE);

//     // Both the system and component appeared `MAX_CHANGE_AGE` ticks ago.
//     // Since we clamp things to `MAX_CHANGE_AGE` for determinism,
//     // `ComponentTicks::is_changed` will now see `MAX_CHANGE_AGE > MAX_CHANGE_AGE`
//     // and return `false`.
//     assert!(!change_expired_system.run((), &mut world).unwrap());
// }

// #[test]
// fn change_tick_wraparound() {
//     let mut world = World::new();
//     world.last_change_tick = Tick::new(u32::MAX);
//     *world.change_tick.get_mut() = 0;

//     // component added: 0, changed: 0
//     world.spawn(C);

//     world.increment_change_tick();

//     // Since the world is always ahead, as long as changes can't get older than `u32::MAX` (which we ensure),
//     // the wrapping difference will always be positive, so wraparound doesn't matter.
//     let mut query = world.query::<Ref<C>>();
//     assert!(query.single(&world).unwrap().is_changed());
// }

// #[test]
// fn change_tick_scan() {
//     let mut world = World::new();

//     // component added: 1, changed: 1
//     world.spawn(C);

//     // a bunch of stuff happens, the component is now older than `MAX_CHANGE_AGE`
//     *world.change_tick.get_mut() += MAX_CHANGE_AGE + CHECK_TICK_THRESHOLD;
//     let change_tick = world.change_tick();

//     let mut query = world.query::<Ref<C>>();
//     for tracker in query.iter(&world) {
//         let ticks_since_insert = change_tick.relative_to(*tracker.ticks.added).get();
//         let ticks_since_change = change_tick.relative_to(*tracker.ticks.changed).get();
//         assert!(ticks_since_insert > MAX_CHANGE_AGE);
//         assert!(ticks_since_change > MAX_CHANGE_AGE);
//     }

//     // scan change ticks and clamp those at risk of overflow
//     world.check_change_ticks();

//     for tracker in query.iter(&world) {
//         let ticks_since_insert = change_tick.relative_to(*tracker.ticks.added).get();
//         let ticks_since_change = change_tick.relative_to(*tracker.ticks.changed).get();
//         assert_eq!(ticks_since_insert, MAX_CHANGE_AGE);
//         assert_eq!(ticks_since_change, MAX_CHANGE_AGE);
//     }
// }

// #[test]
// fn mut_from_res_mut() {
//     let mut component_ticks = ComponentTicks {
//         added: Tick::new(1),
//         changed: Tick::new(2),
//     };
//     let mut caller = MaybeLocation::caller();
//     let ticks = ComponentTicksMut {
//         added: &mut component_ticks.added,
//         changed: &mut component_ticks.changed,
//         changed_by: caller.as_mut(),
//         last_run: Tick::new(3),
//         this_run: Tick::new(4),
//     };
//     let mut res = R {};

//     let res_mut = ResMut {
//         value: &mut res,
//         ticks,
//     };

//     let into_mut: Mut<R> = res_mut.into();
//     assert_eq!(1, into_mut.ticks.added.get());
//     assert_eq!(2, into_mut.ticks.changed.get());
//     assert_eq!(3, into_mut.ticks.last_run.get());
//     assert_eq!(4, into_mut.ticks.this_run.get());
// }

// #[test]
// fn mut_new() {
//     let mut component_ticks = ComponentTicks {
//         added: Tick::new(1),
//         changed: Tick::new(3),
//     };
//     let mut res = R {};
//     let mut caller = MaybeLocation::caller();

//     let val = Mut::new(
//         &mut res,
//         &mut component_ticks.added,
//         &mut component_ticks.changed,
//         Tick::new(2), // last_run
//         Tick::new(4), // this_run
//         caller.as_mut(),
//     );

//     assert!(!val.is_added());
//     assert!(val.is_changed());
// }

// #[test]
// fn mut_from_non_send_mut() {
//     let mut component_ticks = ComponentTicks {
//         added: Tick::new(1),
//         changed: Tick::new(2),
//     };
//     let mut caller = MaybeLocation::caller();
//     let ticks = ComponentTicksMut {
//         added: &mut component_ticks.added,
//         changed: &mut component_ticks.changed,
//         changed_by: caller.as_mut(),
//         last_run: Tick::new(3),
//         this_run: Tick::new(4),
//     };
//     let mut res = R {};

//     let non_send_mut = NonSendMut {
//         value: &mut res,
//         ticks,
//     };

//     let into_mut: Mut<R> = non_send_mut.into();
//     assert_eq!(1, into_mut.ticks.added.get());
//     assert_eq!(2, into_mut.ticks.changed.get());
//     assert_eq!(3, into_mut.ticks.last_run.get());
//     assert_eq!(4, into_mut.ticks.this_run.get());
// }

// #[test]
// fn map_mut() {
//     use super::*;
//     struct Outer(i64);

//     let last_run = Tick::new(2);
//     let this_run = Tick::new(3);
//     let mut component_ticks = ComponentTicks {
//         added: Tick::new(1),
//         changed: Tick::new(2),
//     };
//     let mut caller = MaybeLocation::caller();
//     let ticks = ComponentTicksMut {
//         added: &mut component_ticks.added,
//         changed: &mut component_ticks.changed,
//         changed_by: caller.as_mut(),
//         last_run,
//         this_run,
//     };

//     let mut outer = Outer(0);

//     let ptr = Mut {
//         value: &mut outer,
//         ticks,
//     };
//     assert!(!ptr.is_changed());

//     // Perform a mapping operation.
//     let mut inner = ptr.map_unchanged(|x| &mut x.0);
//     assert!(!inner.is_changed());

//     // Mutate the inner value.
//     *inner = 64;
//     assert!(inner.is_changed());
//     // Modifying one field of a component should flag a change for the entire component.
//     assert!(component_ticks.is_changed(last_run, this_run));
// }

// #[test]
// fn set_if_neq() {
//     let mut world = World::new();

//     world.insert_resource(R2(0));
//     // Resources are Changed when first added
//     world.increment_change_tick();
//     // This is required to update world::last_change_tick
//     world.clear_trackers();

//     let mut r = world.resource_mut::<R2>();
//     assert!(!r.is_changed(), "Resource must begin unchanged.");

//     r.set_if_neq(R2(0));
//     assert!(
//         !r.is_changed(),
//         "Resource must not be changed after setting to the same value."
//     );

//     r.set_if_neq(R2(3));
//     assert!(
//         r.is_changed(),
//         "Resource must be changed after setting to a different value."
//     );
// }

// #[test]
// fn as_deref_mut() {
//     let mut world = World::new();

//     world.insert_resource(R2(0));
//     // Resources are Changed when first added
//     world.increment_change_tick();
//     // This is required to update world::last_change_tick
//     world.clear_trackers();

//     let mut r = world.resource_mut::<R2>();
//     assert!(!r.is_changed(), "Resource must begin unchanged.");

//     let mut r = r.as_deref_mut();
//     assert!(
//         !r.is_changed(),
//         "Dereferencing should not mark the item as changed yet"
//     );

//     r.set_if_neq(3);
//     assert!(
//         r.is_changed(),
//         "Resource must be changed after setting to a different value."
//     );
// }

// #[test]
// fn mut_untyped_to_reflect() {
//     let last_run = Tick::new(2);
//     let this_run = Tick::new(3);
//     let mut component_ticks = ComponentTicks {
//         added: Tick::new(1),
//         changed: Tick::new(2),
//     };
//     let mut caller = MaybeLocation::caller();
//     let ticks = ComponentTicksMut {
//         added: &mut component_ticks.added,
//         changed: &mut component_ticks.changed,
//         changed_by: caller.as_mut(),
//         last_run,
//         this_run,
//     };

//     let mut value: i32 = 5;

//     let value = MutUntyped {
//         value: PtrMut::from(&mut value),
//         ticks,
//     };

//     let reflect_from_ptr = <ReflectFromPtr as FromType<i32>>::from_type();

//     let mut new = value.map_unchanged(|ptr| {
//         // SAFETY: The underlying type of `ptr` matches `reflect_from_ptr`.
//         unsafe { reflect_from_ptr.as_reflect_mut(ptr) }
//     });

//     assert!(!new.is_changed());

//     new.reflect_mut();

//     assert!(new.is_changed());
// }

// #[test]
// fn mut_untyped_from_mut() {
//     let mut component_ticks = ComponentTicks {
//         added: Tick::new(1),
//         changed: Tick::new(2),
//     };
//     let mut caller = MaybeLocation::caller();
//     let ticks = ComponentTicksMut {
//         added: &mut component_ticks.added,
//         changed: &mut component_ticks.changed,
//         changed_by: caller.as_mut(),
//         last_run: Tick::new(3),
//         this_run: Tick::new(4),
//     };
//     let mut c = C {};

//     let mut_typed = Mut {
//         value: &mut c,
//         ticks,
//     };

//     let into_mut: MutUntyped = mut_typed.into();
//     assert_eq!(1, into_mut.ticks.added.get());
//     assert_eq!(2, into_mut.ticks.changed.get());
//     assert_eq!(3, into_mut.ticks.last_run.get());
//     assert_eq!(4, into_mut.ticks.this_run.get());
// }
