// use crate::ecs::{
//     archetype::Archetype,
//     change_detection::Tick,
//     component::{Component, ComponentId, Components},
//     entity::Entity,
//     query::{
//         ArchetypeQueryData, FilteredAccess, IterQueryData, QueryData, ReadOnlyQueryData,
//         ReleaseStateQueryData, WorldQuery,
//     },
//     storage::{Table, TableRow},
//     system::Query,
//     world::{EntityRef, World, unsafe_world_cell::UnsafeWorldCell},
// };

// #[derive(Component)]
// pub struct A;

// #[derive(Component)]
// pub struct B;

// // Tests that each variant of struct can be used as a `WorldQuery`.
// #[test]
// fn world_query_struct_variants() {
//     #[derive(QueryData)]
//     pub struct NamedQuery {
//         id: Entity,
//         a: &'static A,
//     }

//     #[derive(QueryData)]
//     pub struct TupleQuery(&'static A, &'static B);

//     #[derive(QueryData)]
//     pub struct UnitQuery;

//     fn my_system(_: Query<(NamedQuery, TupleQuery, UnitQuery)>) {}

//     assert_is_system(my_system);
// }

// // Compile test for https://github.com/bevyengine/bevy/pull/8030.
// #[test]
// fn world_query_phantom_data() {
//     #[derive(QueryData)]
//     pub struct IgnoredQuery<Marker> {
//         id: Entity,
//         _marker: PhantomData<Marker>,
//     }

//     fn ignored_system(_: Query<IgnoredQuery<()>>) {}

//     assert_is_system(ignored_system);
// }

// #[test]
// fn derive_release_state() {
//     struct NonReleaseQueryData;

//     // SAFETY:
//     // `update_component_access` do nothing.
//     // This is sound because `fetch` does not access components.
//     unsafe impl WorldQuery for NonReleaseQueryData {
//         type Fetch<'w> = ();
//         type State = ();

//         fn shrink_fetch<'wlong: 'wshort, 'wshort>(_: Self::Fetch<'wlong>) -> Self::Fetch<'wshort> {}

//         unsafe fn init_fetch<'w, 's>(
//             _world: UnsafeWorldCell<'w>,
//             _state: &'s Self::State,
//             _last_run: Tick,
//             _this_run: Tick,
//         ) -> Self::Fetch<'w> {
//         }

//         const IS_DENSE: bool = true;

//         #[inline]
//         unsafe fn set_archetype<'w, 's>(
//             _fetch: &mut Self::Fetch<'w>,
//             _state: &'s Self::State,
//             _archetype: &'w Archetype,
//             _table: &Table,
//         ) {
//         }

//         #[inline]
//         unsafe fn set_table<'w, 's>(
//             _fetch: &mut Self::Fetch<'w>,
//             _state: &'s Self::State,
//             _table: &'w Table,
//         ) {
//         }

//         fn update_component_access(_state: &Self::State, _access: &mut FilteredAccess) {}

//         fn init_state(_world: &mut World) {}

//         fn get_state(_components: &Components) -> Option<()> {
//             Some(())
//         }

//         fn matches_component_set(
//             _state: &Self::State,
//             _set_contains_id: &impl Fn(ComponentId) -> bool,
//         ) -> bool {
//             true
//         }
//     }

//     // SAFETY: `Self` is the same as `Self::ReadOnly`
//     unsafe impl QueryData for NonReleaseQueryData {
//         type ReadOnly = Self;
//         const IS_READ_ONLY: bool = true;
//         const IS_ARCHETYPAL: bool = true;

//         type Item<'w, 's> = ();

//         fn shrink<'wlong: 'wshort, 'wshort, 's>(
//             _item: Self::Item<'wlong, 's>,
//         ) -> Self::Item<'wshort, 's> {
//         }

//         #[inline(always)]
//         unsafe fn fetch<'w, 's>(
//             _state: &'s Self::State,
//             _fetch: &mut Self::Fetch<'w>,
//             _entity: Entity,
//             _table_row: TableRow,
//         ) -> Option<Self::Item<'w, 's>> {
//             Some(())
//         }

//         fn iter_access(_state: &Self::State) -> impl Iterator<Item = EcsAccessType<'_>> {
//             iter::empty()
//         }
//     }

//     // SAFETY: access is read only
//     unsafe impl ReadOnlyQueryData for NonReleaseQueryData {}

//     // SAFETY: access is read only
//     unsafe impl IterQueryData for NonReleaseQueryData {}

//     impl ArchetypeQueryData for NonReleaseQueryData {}

//     #[derive(QueryData)]
//     pub struct DerivedNonReleaseRead {
//         non_release: NonReleaseQueryData,
//         a: &'static A,
//     }

//     #[derive(QueryData)]
//     #[query_data(mutable)]
//     pub struct DerivedNonReleaseMutable {
//         non_release: NonReleaseQueryData,
//         a: &'static mut A,
//     }

//     #[derive(QueryData)]
//     pub struct DerivedReleaseRead {
//         a: &'static A,
//     }

//     #[derive(QueryData)]
//     #[query_data(mutable)]
//     pub struct DerivedReleaseMutable {
//         a: &'static mut A,
//     }

//     fn assert_is_release_state<Q: ReleaseStateQueryData>() {}

//     assert_is_release_state::<DerivedReleaseRead>();
//     assert_is_release_state::<DerivedReleaseMutable>();
// }

// // Ensures that each field of a `WorldQuery` struct's read-only variant
// // has the same visibility as its corresponding mutable field.
// #[test]
// fn read_only_field_visibility() {
//     mod private {
//         use super::*;

//         #[derive(QueryData)]
//         #[query_data(mutable)]
//         pub struct D {
//             pub a: &'static mut A,
//         }
//     }

//     let _ = private::DReadOnly { a: &A };

//     fn my_system(query: Query<private::D>) {
//         for q in &query {
//             let _ = &q.a;
//         }
//     }

//     assert_is_system(my_system);
// }

// // Ensures that metadata types generated by the WorldQuery macro
// // do not conflict with user-defined types.
// // Regression test for https://github.com/bevyengine/bevy/issues/8010.
// #[test]
// fn world_query_metadata_collision() {
//     // The metadata types generated would be named `ClientState` and `ClientFetch`,
//     // but they should rename themselves to avoid conflicts.
//     #[derive(QueryData)]
//     pub struct Client<S: ClientState> {
//         pub state: &'static S,
//         pub fetch: &'static ClientFetch,
//     }

//     pub trait ClientState: Component {}

//     #[derive(Component)]
//     pub struct ClientFetch;

//     #[derive(Component)]
//     pub struct C;

//     impl ClientState for C {}

//     fn client_system(_: Query<Client<C>>) {}

//     assert_is_system(client_system);
// }

// // Test that EntityRef::get_ref::<T>() returns a Ref<T> value with the correct
// // ticks when the EntityRef was retrieved from a Query.
// // See: https://github.com/bevyengine/bevy/issues/13735
// #[test]
// fn test_entity_ref_query_with_ticks() {
//     #[derive(Component)]
//     pub struct C;

//     fn system(query: Query<EntityRef>) {
//         for entity_ref in &query {
//             if let Some(c) = entity_ref.get_ref::<C>()
//                 && !c.is_added()
//             {
//                 panic!("Expected C to be added");
//             }
//         }
//     }

//     let mut world = World::new();
//     let mut schedule = Schedule::default();
//     schedule.add_systems(system);
//     world.spawn(C);

//     // reset the change ticks
//     world.clear_trackers();

//     // we want EntityRef to use the change ticks of the system
//     schedule.run(&mut world);
// }

// #[test]
// fn test_contiguous_query_data() {
//     #[derive(Component, PartialEq, Eq, Debug)]
//     pub struct C(i32);

//     #[derive(Component, PartialEq, Eq, Debug)]
//     pub struct D(bool);

//     let mut world = World::new();
//     world.spawn((C(0), D(true)));
//     world.spawn((C(1), D(false)));
//     world.spawn(C(2));

//     let mut query = world.query::<(&C, &D)>();
//     let mut iter = query.contiguous_iter(&world).unwrap();
//     let c = iter.next().unwrap();
//     assert_eq!(c.0, [C(0), C(1)].as_slice());
//     assert_eq!(c.1, [D(true), D(false)].as_slice());
//     assert!(iter.next().is_none());

//     let mut query = world.query::<&C>();
//     let mut iter = query.contiguous_iter(&world).unwrap();
//     let mut present = [false; 3];
//     let mut len = 0;
//     for _ in 0..2 {
//         let c = iter.next().unwrap();
//         for c in c {
//             present[c.0 as usize] = true;
//             len += 1;
//         }
//     }
//     assert!(iter.next().is_none());
//     assert_eq!(len, 3);
//     assert_eq!(present, [true; 3]);

//     let mut query = world.query::<&mut C>();
//     let mut iter = query.contiguous_iter_mut(&mut world).unwrap();
//     for _ in 0..2 {
//         let c = iter.next().unwrap();
//         for c in c {
//             c.0 *= 2;
//         }
//     }
//     assert!(iter.next().is_none());
//     let mut iter = query.contiguous_iter(&world).unwrap();
//     let mut present = [false; 6];
//     let mut len = 0;
//     for _ in 0..2 {
//         let c = iter.next().unwrap();
//         for c in c {
//             present[c.0 as usize] = true;
//             len += 1;
//         }
//     }
//     assert_eq!(present, [true, false, true, false, true, false]);
//     assert_eq!(len, 3);

//     let mut query = world.query_filtered::<&C, Without<D>>();
//     let mut iter = query.contiguous_iter(&world).unwrap();
//     assert_eq!(iter.next().unwrap(), &[C(4)]);
//     assert!(iter.next().is_none());
// }

// #[test]
// fn sparse_set_contiguous_query() {
//     #[derive(Component, Debug, PartialEq, Eq)]
//     #[component(storage = "SparseSet")]
//     pub struct S(i32);

//     let mut world = World::new();
//     world.spawn(S(0));

//     let mut query = world.query::<&mut S>();
//     let iter = query.contiguous_iter_mut(&mut world);
//     assert!(iter.is_err());
// }

// #[test]
// fn any_of_contiguous_test() {
//     #[derive(Component, Debug, Clone, Copy)]
//     pub struct C(i32);

//     #[derive(Component, Debug, Clone, Copy)]
//     pub struct D(i32);

//     let mut world = World::new();
//     world.spawn((C(0), D(1)));
//     world.spawn(C(2));
//     world.spawn(D(3));
//     world.spawn(());

//     let mut query = world.query::<AnyOf<(&C, &D)>>();
//     let iter = query.contiguous_iter(&world).unwrap();
//     let mut present = [false; 4];

//     for (c, d) in iter {
//         assert!(c.is_some() || d.is_some());
//         let c = c.unwrap_or_default();
//         let d = d.unwrap_or_default();
//         for i in 0..c.len().max(d.len()) {
//             let c = c.get(i).cloned();
//             let d = d.get(i).cloned();
//             if let Some(C(c)) = c {
//                 assert!(!present[c as usize]);
//                 present[c as usize] = true;
//             }
//             if let Some(D(d)) = d {
//                 assert!(!present[d as usize]);
//                 present[d as usize] = true;
//             }
//         }
//     }

//     assert_eq!(present, [true; 4]);
// }

// #[test]
// fn option_contiguous_test() {
//     #[derive(Component, Clone, Copy)]
//     struct C(i32);

//     #[derive(Component, Clone, Copy)]
//     struct D(i32);

//     let mut world = World::new();
//     world.spawn((C(0), D(1)));
//     world.spawn(D(2));
//     world.spawn(C(3));

//     let mut query = world.query::<(Option<&C>, &D)>();
//     let iter = query.contiguous_iter(&world).unwrap();
//     let mut present = [false; 3];

//     for (c, d) in iter {
//         let c = c.unwrap_or_default();
//         for i in 0..d.len() {
//             let c = c.get(i).cloned();
//             let D(d) = d[i];
//             if let Some(C(c)) = c {
//                 assert!(!present[c as usize]);
//                 present[c as usize] = true;
//             }
//             assert!(!present[d as usize]);
//             present[d as usize] = true;
//         }
//     }

//     assert_eq!(present, [true; 3]);
// }
