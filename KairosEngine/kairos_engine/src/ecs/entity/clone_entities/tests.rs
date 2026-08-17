// #[cfg(feature = "bevy_reflect")]
// mod reflect {
//     use super::*;
//     use crate::reflect::{AppTypeRegistry, ReflectComponent, ReflectFromWorld};
//     use alloc::vec;
//     use bevy_reflect::{std_traits::ReflectDefault, FromType, Reflect, ReflectFromPtr};

//     #[test]
//     fn clone_entity_using_reflect() {
//         #[derive(Component, Reflect, Clone, PartialEq, Eq)]
//         #[reflect(Component)]
//         struct A {
//             field: usize,
//         }

//         let mut world = World::default();
//         world.init_resource::<AppTypeRegistry>();
//         let registry = world.get_resource::<AppTypeRegistry>().unwrap();
//         registry.write().register::<A>();

//         world.register_component::<A>();
//         let component = A { field: 5 };

//         let e = world.spawn(component.clone()).id();
//         let e_clone = world.spawn_empty().id();

//         EntityCloner::build_opt_out(&mut world)
//             .override_clone_behavior::<A>(ComponentCloneBehavior::reflect())
//             .clone_entity(e, e_clone);

//         assert!(world.get::<A>(e_clone).is_some_and(|c| *c == component));
//     }

//     #[test]
//     fn clone_entity_using_reflect_all_paths() {
//         #[derive(PartialEq, Eq, Default, Debug)]
//         struct NotClone;

//         // `reflect_clone`-based fast path
//         #[derive(Component, Reflect, PartialEq, Eq, Default, Debug)]
//         #[reflect(from_reflect = false)]
//         struct A {
//             field: usize,
//             field2: Vec<usize>,
//         }

//         // `ReflectDefault`-based fast path
//         #[derive(Component, Reflect, PartialEq, Eq, Default, Debug)]
//         #[reflect(Default)]
//         #[reflect(from_reflect = false)]
//         struct B {
//             field: usize,
//             field2: Vec<usize>,
//             #[reflect(ignore)]
//             ignored: NotClone,
//         }

//         // `ReflectFromReflect`-based fast path
//         #[derive(Component, Reflect, PartialEq, Eq, Default, Debug)]
//         struct C {
//             field: usize,
//             field2: Vec<usize>,
//             #[reflect(ignore)]
//             ignored: NotClone,
//         }

//         // `ReflectFromWorld`-based fast path
//         #[derive(Component, Reflect, PartialEq, Eq, Default, Debug)]
//         #[reflect(FromWorld)]
//         #[reflect(from_reflect = false)]
//         struct D {
//             field: usize,
//             field2: Vec<usize>,
//             #[reflect(ignore)]
//             ignored: NotClone,
//         }

//         let mut world = World::default();
//         world.init_resource::<AppTypeRegistry>();
//         let registry = world.get_resource::<AppTypeRegistry>().unwrap();
//         registry.write().register::<(A, B, C, D)>();

//         let a_id = world.register_component::<A>();
//         let b_id = world.register_component::<B>();
//         let c_id = world.register_component::<C>();
//         let d_id = world.register_component::<D>();
//         let component_a = A {
//             field: 5,
//             field2: vec![1, 2, 3, 4, 5],
//         };
//         let component_b = B {
//             field: 5,
//             field2: vec![1, 2, 3, 4, 5],
//             ignored: NotClone,
//         };
//         let component_c = C {
//             field: 6,
//             field2: vec![1, 2, 3, 4, 5],
//             ignored: NotClone,
//         };
//         let component_d = D {
//             field: 7,
//             field2: vec![1, 2, 3, 4, 5],
//             ignored: NotClone,
//         };

//         let e = world
//             .spawn((component_a, component_b, component_c, component_d))
//             .id();
//         let e_clone = world.spawn_empty().id();

//         EntityCloner::build_opt_out(&mut world)
//             .override_clone_behavior_with_id(a_id, ComponentCloneBehavior::reflect())
//             .override_clone_behavior_with_id(b_id, ComponentCloneBehavior::reflect())
//             .override_clone_behavior_with_id(c_id, ComponentCloneBehavior::reflect())
//             .override_clone_behavior_with_id(d_id, ComponentCloneBehavior::reflect())
//             .clone_entity(e, e_clone);

//         assert_eq!(world.get::<A>(e_clone), Some(world.get::<A>(e).unwrap()));
//         assert_eq!(world.get::<B>(e_clone), Some(world.get::<B>(e).unwrap()));
//         assert_eq!(world.get::<C>(e_clone), Some(world.get::<C>(e).unwrap()));
//         assert_eq!(world.get::<D>(e_clone), Some(world.get::<D>(e).unwrap()));
//     }

//     #[test]
//     fn read_source_component_reflect_should_return_none_on_invalid_reflect_from_ptr() {
//         #[derive(Component, Reflect)]
//         struct A;

//         #[derive(Component, Reflect)]
//         struct B;

//         fn test_handler(source: &SourceComponent, ctx: &mut ComponentCloneCtx) {
//             let registry = ctx.type_registry().unwrap();
//             assert!(source.read_reflect(&registry.read()).is_none());
//         }

//         let mut world = World::default();
//         world.init_resource::<AppTypeRegistry>();
//         let registry = world.get_resource::<AppTypeRegistry>().unwrap();
//         {
//             let mut registry = registry.write();
//             registry.register::<A>();
//             registry
//                 .get_mut(TypeId::of::<A>())
//                 .unwrap()
//                 .insert(<ReflectFromPtr as FromType<B>>::from_type());
//         }

//         let e = world.spawn(A).id();
//         let e_clone = world.spawn_empty().id();

//         EntityCloner::build_opt_out(&mut world)
//             .override_clone_behavior::<A>(ComponentCloneBehavior::Custom(test_handler))
//             .clone_entity(e, e_clone);
//     }

//     #[test]
//     fn clone_entity_specialization() {
//         #[derive(Component, Reflect, PartialEq, Eq)]
//         #[reflect(Component)]
//         struct A {
//             field: usize,
//         }

//         impl Clone for A {
//             fn clone(&self) -> Self {
//                 Self { field: 10 }
//             }
//         }

//         let mut world = World::default();
//         world.init_resource::<AppTypeRegistry>();
//         let registry = world.get_resource::<AppTypeRegistry>().unwrap();
//         registry.write().register::<A>();

//         let component = A { field: 5 };

//         let e = world.spawn(component.clone()).id();
//         let e_clone = world.spawn_empty().id();

//         EntityCloner::build_opt_out(&mut world).clone_entity(e, e_clone);

//         assert!(world
//             .get::<A>(e_clone)
//             .is_some_and(|comp| *comp == A { field: 10 }));
//     }

//     #[test]
//     fn clone_entity_using_reflect_should_skip_without_panic() {
//         // Not reflected
//         #[derive(Component, PartialEq, Eq, Default, Debug)]
//         struct A;

//         // No valid type data and not `reflect_clone`-able
//         #[derive(Component, Reflect, PartialEq, Eq, Default, Debug)]
//         #[reflect(Component)]
//         #[reflect(from_reflect = false)]
//         struct B(#[reflect(ignore)] PhantomData<()>);

//         let mut world = World::default();

//         // No AppTypeRegistry
//         let e = world.spawn((A, B(Default::default()))).id();
//         let e_clone = world.spawn_empty().id();
//         EntityCloner::build_opt_out(&mut world)
//             .override_clone_behavior::<A>(ComponentCloneBehavior::reflect())
//             .override_clone_behavior::<B>(ComponentCloneBehavior::reflect())
//             .clone_entity(e, e_clone);
//         assert_eq!(world.get::<A>(e_clone), None);
//         assert_eq!(world.get::<B>(e_clone), None);

//         // With AppTypeRegistry
//         world.init_resource::<AppTypeRegistry>();
//         let registry = world.get_resource::<AppTypeRegistry>().unwrap();
//         registry.write().register::<B>();

//         let e = world.spawn((A, B(Default::default()))).id();
//         let e_clone = world.spawn_empty().id();
//         EntityCloner::build_opt_out(&mut world).clone_entity(e, e_clone);
//         assert_eq!(world.get::<A>(e_clone), None);
//         assert_eq!(world.get::<B>(e_clone), None);
//     }

//     #[test]
//     fn clone_with_reflect_from_world() {
//         #[derive(Component, Reflect, PartialEq, Eq, Debug)]
//         #[reflect(Component, FromWorld, from_reflect = false)]
//         struct SomeRef(
//             #[entities] Entity,
//             // We add an ignored field here to ensure `reflect_clone` fails and `FromWorld` is used
//             #[reflect(ignore)] PhantomData<()>,
//         );

//         #[derive(Resource)]
//         struct FromWorldCalled(bool);

//         impl FromWorld for SomeRef {
//             fn from_world(world: &mut World) -> Self {
//                 world.insert_resource(FromWorldCalled(true));
//                 SomeRef(Entity::PLACEHOLDER, Default::default())
//             }
//         }
//         let mut world = World::new();
//         let registry = AppTypeRegistry::default();
//         registry.write().register::<SomeRef>();
//         world.insert_resource(registry);

//         let a = world.spawn_empty().id();
//         let b = world.spawn_empty().id();
//         let c = world.spawn(SomeRef(a, Default::default())).id();
//         let d = world.spawn_empty().id();
//         let mut map = EntityHashMap::<Entity>::new();
//         map.insert(a, b);
//         map.insert(c, d);

//         let cloned = EntityCloner::default().clone_entity_mapped(&mut world, c, &mut map);
//         assert_eq!(
//             *world.entity(cloned).get::<SomeRef>().unwrap(),
//             SomeRef(b, Default::default())
//         );
//         assert!(world.resource::<FromWorldCalled>().0);
//     }
// }

// #[test]
// fn clone_entity_using_clone() {
//     #[derive(Component, Clone, PartialEq, Eq)]
//     struct A {
//         field: usize,
//     }

//     let mut world = World::default();

//     let component = A { field: 5 };

//     let e = world.spawn(component.clone()).id();
//     let e_clone = world.spawn_empty().id();

//     EntityCloner::build_opt_out(&mut world).clone_entity(e, e_clone);

//     assert!(world.get::<A>(e_clone).is_some_and(|c| *c == component));
// }

// #[test]
// fn clone_entity_with_allow_filter() {
//     #[derive(Component, Clone, PartialEq, Eq)]
//     struct A {
//         field: usize,
//     }

//     #[derive(Component, Clone)]
//     struct B;

//     let mut world = World::default();

//     let component = A { field: 5 };

//     let e = world.spawn((component.clone(), B)).id();
//     let e_clone = world.spawn_empty().id();

//     EntityCloner::build_opt_in(&mut world)
//         .allow::<A>()
//         .clone_entity(e, e_clone);

//     assert!(world.get::<A>(e_clone).is_some_and(|c| *c == component));
//     assert!(world.get::<B>(e_clone).is_none());
// }

// #[test]
// fn clone_entity_with_deny_filter() {
//     #[derive(Component, Clone, PartialEq, Eq)]
//     struct A {
//         field: usize,
//     }

//     #[derive(Component, Clone)]
//     #[require(C)]
//     struct B;

//     #[derive(Component, Clone, Default)]
//     struct C;

//     let mut world = World::default();

//     let component = A { field: 5 };

//     let e = world.spawn((component.clone(), B, C)).id();
//     let e_clone = world.spawn_empty().id();

//     EntityCloner::build_opt_out(&mut world)
//         .deny::<C>()
//         .clone_entity(e, e_clone);

//     assert!(world.get::<A>(e_clone).is_some_and(|c| *c == component));
//     assert!(world.get::<B>(e_clone).is_none());
//     assert!(world.get::<C>(e_clone).is_none());
// }

// #[test]
// fn clone_entity_with_deny_filter_without_required_by() {
//     #[derive(Component, Clone)]
//     #[require(B { field: 5 })]
//     struct A;

//     #[derive(Component, Clone, PartialEq, Eq)]
//     struct B {
//         field: usize,
//     }

//     let mut world = World::default();

//     let e = world.spawn((A, B { field: 10 })).id();
//     let e_clone = world.spawn_empty().id();

//     EntityCloner::build_opt_out(&mut world)
//         .without_required_by_components(|builder| {
//             builder.deny::<B>();
//         })
//         .clone_entity(e, e_clone);

//     assert!(world.get::<A>(e_clone).is_some());
//     assert!(world
//         .get::<B>(e_clone)
//         .is_some_and(|c| *c == B { field: 5 }));
// }

// #[test]
// fn clone_entity_with_deny_filter_if_new() {
//     #[derive(Component, Clone, PartialEq, Eq)]
//     struct A {
//         field: usize,
//     }

//     #[derive(Component, Clone)]
//     struct B;

//     #[derive(Component, Clone)]
//     struct C;

//     let mut world = World::default();

//     let e = world.spawn((A { field: 5 }, B, C)).id();
//     let e_clone = world.spawn(A { field: 8 }).id();

//     EntityCloner::build_opt_out(&mut world)
//         .deny::<B>()
//         .insert_mode(InsertMode::Keep)
//         .clone_entity(e, e_clone);

//     assert!(world
//         .get::<A>(e_clone)
//         .is_some_and(|c| *c == A { field: 8 }));
//     assert!(world.get::<B>(e_clone).is_none());
//     assert!(world.get::<C>(e_clone).is_some());
// }

// #[test]
// fn allow_and_allow_if_new_always_allows() {
//     #[derive(Component, Clone, PartialEq, Debug)]
//     struct A(u8);

//     let mut world = World::default();
//     let e = world.spawn(A(1)).id();
//     let e_clone1 = world.spawn(A(2)).id();

//     EntityCloner::build_opt_in(&mut world)
//         .allow_if_new::<A>()
//         .allow::<A>()
//         .clone_entity(e, e_clone1);

//     assert_eq!(world.get::<A>(e_clone1), Some(&A(1)));

//     let e_clone2 = world.spawn(A(2)).id();

//     EntityCloner::build_opt_in(&mut world)
//         .allow::<A>()
//         .allow_if_new::<A>()
//         .clone_entity(e, e_clone2);

//     assert_eq!(world.get::<A>(e_clone2), Some(&A(1)));
// }

// #[test]
// fn with_and_without_required_components_include_required() {
//     #[derive(Component, Clone, PartialEq, Debug)]
//     #[require(B(5))]
//     struct A;

//     #[derive(Component, Clone, PartialEq, Debug)]
//     struct B(u8);

//     let mut world = World::default();
//     let e = world.spawn((A, B(10))).id();
//     let e_clone1 = world.spawn_empty().id();
//     EntityCloner::build_opt_in(&mut world)
//         .without_required_components(|builder| {
//             builder.allow::<A>();
//         })
//         .allow::<A>()
//         .clone_entity(e, e_clone1);

//     assert_eq!(world.get::<B>(e_clone1), Some(&B(10)));

//     let e_clone2 = world.spawn_empty().id();

//     EntityCloner::build_opt_in(&mut world)
//         .allow::<A>()
//         .without_required_components(|builder| {
//             builder.allow::<A>();
//         })
//         .clone_entity(e, e_clone2);

//     assert_eq!(world.get::<B>(e_clone2), Some(&B(10)));
// }

// #[test]
// fn clone_required_becoming_explicit() {
//     #[derive(Component, Clone, PartialEq, Debug)]
//     #[require(B(5))]
//     struct A;

//     #[derive(Component, Clone, PartialEq, Debug)]
//     struct B(u8);

//     let mut world = World::default();
//     let e = world.spawn((A, B(10))).id();
//     let e_clone1 = world.spawn(B(20)).id();
//     EntityCloner::build_opt_in(&mut world)
//         .allow::<A>()
//         .allow::<B>()
//         .clone_entity(e, e_clone1);

//     assert_eq!(world.get::<B>(e_clone1), Some(&B(10)));

//     let e_clone2 = world.spawn(B(20)).id();
//     EntityCloner::build_opt_in(&mut world)
//         .allow::<A>()
//         .allow::<B>()
//         .clone_entity(e, e_clone2);

//     assert_eq!(world.get::<B>(e_clone2), Some(&B(10)));
// }

// #[test]
// fn required_not_cloned_because_requiring_missing() {
//     #[derive(Component, Clone)]
//     #[require(B)]
//     struct A;

//     #[derive(Component, Clone, Default)]
//     struct B;

//     let mut world = World::default();
//     let e = world.spawn(B).id();
//     let e_clone1 = world.spawn_empty().id();

//     EntityCloner::build_opt_in(&mut world)
//         .allow::<A>()
//         .clone_entity(e, e_clone1);

//     assert!(world.get::<B>(e_clone1).is_none());
// }

// #[test]
// fn clone_entity_with_required_components() {
//     #[derive(Component, Clone, PartialEq, Debug)]
//     #[require(B)]
//     struct A;

//     #[derive(Component, Clone, PartialEq, Debug, Default)]
//     #[require(C(5))]
//     struct B;

//     #[derive(Component, Clone, PartialEq, Debug)]
//     struct C(u32);

//     let mut world = World::default();

//     let e = world.spawn(A).id();
//     let e_clone = world.spawn_empty().id();

//     EntityCloner::build_opt_in(&mut world)
//         .allow::<B>()
//         .clone_entity(e, e_clone);

//     assert_eq!(world.entity(e_clone).get::<A>(), None);
//     assert_eq!(world.entity(e_clone).get::<B>(), Some(&B));
//     assert_eq!(world.entity(e_clone).get::<C>(), Some(&C(5)));
// }

// #[test]
// fn clone_entity_with_default_required_components() {
//     #[derive(Component, Clone, PartialEq, Debug)]
//     #[require(B)]
//     struct A;

//     #[derive(Component, Clone, PartialEq, Debug, Default)]
//     #[require(C(5))]
//     struct B;

//     #[derive(Component, Clone, PartialEq, Debug)]
//     struct C(u32);

//     let mut world = World::default();

//     let e = world.spawn((A, C(0))).id();
//     let e_clone = world.spawn_empty().id();

//     EntityCloner::build_opt_in(&mut world)
//         .without_required_components(|builder| {
//             builder.allow::<A>();
//         })
//         .clone_entity(e, e_clone);

//     assert_eq!(world.entity(e_clone).get::<A>(), Some(&A));
//     assert_eq!(world.entity(e_clone).get::<B>(), Some(&B));
//     assert_eq!(world.entity(e_clone).get::<C>(), Some(&C(5)));
// }

// #[test]
// fn clone_entity_with_missing_required_components() {
//     #[derive(Component, Clone, PartialEq, Debug)]
//     #[require(B)]
//     struct A;

//     #[derive(Component, Clone, PartialEq, Debug, Default)]
//     #[require(C(5))]
//     struct B;

//     #[derive(Component, Clone, PartialEq, Debug)]
//     struct C(u32);

//     let mut world = World::default();

//     let e = world.spawn(A).remove::<C>().id();
//     let e_clone = world.spawn_empty().id();

//     EntityCloner::build_opt_in(&mut world)
//         .allow::<A>()
//         .clone_entity(e, e_clone);

//     assert_eq!(world.entity(e_clone).get::<A>(), Some(&A));
//     assert_eq!(world.entity(e_clone).get::<B>(), Some(&B));
//     assert_eq!(world.entity(e_clone).get::<C>(), Some(&C(5)));
// }

// #[test]
// fn skipped_required_components_counter_is_reset_on_early_return() {
//     #[derive(Component, Clone, PartialEq, Debug, Default)]
//     #[require(B(5))]
//     struct A;

//     #[derive(Component, Clone, PartialEq, Debug)]
//     struct B(u32);

//     #[derive(Component, Clone, PartialEq, Debug, Default)]
//     struct C;

//     let mut world = World::default();

//     let e1 = world.spawn(C).id();
//     let e2 = world.spawn((A, B(0))).id();
//     let e_clone = world.spawn_empty().id();

//     let mut builder = EntityCloner::build_opt_in(&mut world);
//     builder.allow::<(A, C)>();
//     let mut cloner = builder.finish();
//     cloner.clone_entity(&mut world, e1, e_clone);
//     cloner.clone_entity(&mut world, e2, e_clone);

//     assert_eq!(world.entity(e_clone).get::<B>(), Some(&B(0)));
// }

// #[test]
// fn clone_entity_with_dynamic_components() {
//     const COMPONENT_SIZE: usize = 10;
//     fn test_handler(source: &SourceComponent, ctx: &mut ComponentCloneCtx) {
//         // SAFETY: the passed in ptr corresponds to copy-able data that matches the type of the source / target component
//         unsafe {
//             ctx.write_target_component_ptr(source.ptr());
//         }
//     }

//     let mut world = World::default();

//     let layout = Layout::array::<u8>(COMPONENT_SIZE).unwrap();
//     // SAFETY:
//     // - No drop command is required
//     // - The component will store [u8; COMPONENT_SIZE], which is Send + Sync
//     let descriptor = unsafe {
//         ComponentDescriptor::new_with_layout(
//             "DynamicComp",
//             StorageType::Table,
//             layout,
//             None,
//             true,
//             ComponentCloneBehavior::Custom(test_handler),
//             None,
//         )
//     };
//     let component_id = world.register_component_with_descriptor(descriptor);

//     let mut entity = world.spawn_empty();
//     let data = [5u8; COMPONENT_SIZE];

//     // SAFETY:
//     // - ptr points to data represented by component_id ([u8; COMPONENT_SIZE])
//     // - component_id is from the same world as entity
//     OwningPtr::make(data, |ptr| unsafe {
//         entity.insert_by_id(component_id, ptr);
//     });
//     let entity = entity.id();

//     let entity_clone = world.spawn_empty().id();
//     EntityCloner::build_opt_out(&mut world).clone_entity(entity, entity_clone);

//     let ptr = world.get_by_id(entity, component_id).unwrap();
//     let clone_ptr = world.get_by_id(entity_clone, component_id).unwrap();
//     // SAFETY: ptr and clone_ptr store component represented by [u8; COMPONENT_SIZE]
//     unsafe {
//         assert_eq!(
//             core::slice::from_raw_parts(ptr.as_ptr(), COMPONENT_SIZE),
//             core::slice::from_raw_parts(clone_ptr.as_ptr(), COMPONENT_SIZE),
//         );
//     }
// }

// #[test]
// fn recursive_clone() {
//     let mut world = World::new();
//     let root = world.spawn_empty().id();
//     let child1 = world.spawn(ChildOf(root)).id();
//     let grandchild = world.spawn(ChildOf(child1)).id();
//     let child2 = world.spawn(ChildOf(root)).id();

//     let clone_root = world.spawn_empty().id();
//     EntityCloner::build_opt_out(&mut world)
//         .linked_cloning(true)
//         .clone_entity(root, clone_root);

//     let root_children = world
//         .entity(clone_root)
//         .get::<Children>()
//         .unwrap()
//         .iter()
//         .cloned()
//         .collect::<Vec<_>>();

//     assert!(root_children.iter().all(|e| *e != child1 && *e != child2));
//     assert_eq!(root_children.len(), 2);
//     assert_eq!(
//         (
//             world.get::<ChildOf>(root_children[0]),
//             world.get::<ChildOf>(root_children[1])
//         ),
//         (Some(&ChildOf(clone_root)), Some(&ChildOf(clone_root)))
//     );
//     let child1_children = world.entity(root_children[0]).get::<Children>().unwrap();
//     assert_eq!(child1_children.len(), 1);
//     assert_ne!(child1_children[0], grandchild);
//     assert!(world.entity(root_children[1]).get::<Children>().is_none());
//     assert_eq!(
//         world.get::<ChildOf>(child1_children[0]),
//         Some(&ChildOf(root_children[0]))
//     );

//     assert_eq!(
//         world.entity(root).get::<Children>().unwrap().deref(),
//         &[child1, child2]
//     );
// }

// #[test]
// fn cloning_with_required_components_preserves_existing() {
//     #[derive(Component, Clone, PartialEq, Debug, Default)]
//     #[require(B(5))]
//     struct A;

//     #[derive(Component, Clone, PartialEq, Debug)]
//     struct B(u32);

//     let mut world = World::default();

//     let e = world.spawn((A, B(0))).id();
//     let e_clone = world.spawn(B(1)).id();

//     EntityCloner::build_opt_in(&mut world)
//         .allow::<A>()
//         .clone_entity(e, e_clone);

//     assert_eq!(world.entity(e_clone).get::<A>(), Some(&A));
//     assert_eq!(world.entity(e_clone).get::<B>(), Some(&B(1)));
// }

// #[test]
// fn move_without_clone() {
//     #[derive(Component, PartialEq, Debug)]
//     #[component(storage = "SparseSet")]
//     struct A;

//     #[derive(Component, PartialEq, Debug)]
//     struct B(Vec<u8>);

//     let mut world = World::default();
//     let e = world.spawn((A, B(alloc::vec![1, 2, 3]))).id();
//     let e_clone = world.spawn_empty().id();
//     let mut builder = EntityCloner::build_opt_out(&mut world);
//     builder.move_components(true);
//     let mut cloner = builder.finish();

//     cloner.clone_entity(&mut world, e, e_clone);

//     assert_eq!(world.get::<A>(e), None);
//     assert_eq!(world.get::<B>(e), None);

//     assert_eq!(world.get::<A>(e_clone), Some(&A));
//     assert_eq!(world.get::<B>(e_clone), Some(&B(alloc::vec![1, 2, 3])));
// }

// #[test]
// fn move_with_remove_hook() {
//     #[derive(Component, PartialEq, Debug)]
//     #[component(on_remove=remove_hook)]
//     struct B(Option<Vec<u8>>);

//     fn remove_hook(mut world: DeferredWorld, ctx: HookContext) {
//         world.get_mut::<B>(ctx.entity).unwrap().0.take();
//     }

//     let mut world = World::default();
//     let e = world.spawn(B(Some(alloc::vec![1, 2, 3]))).id();
//     let e_clone = world.spawn_empty().id();
//     let mut builder = EntityCloner::build_opt_out(&mut world);
//     builder.move_components(true);
//     let mut cloner = builder.finish();

//     cloner.clone_entity(&mut world, e, e_clone);

//     assert_eq!(world.get::<B>(e), None);
//     assert_eq!(world.get::<B>(e_clone), Some(&B(None)));
// }

// #[test]
// fn move_with_deferred() {
//     #[derive(Component, PartialEq, Debug)]
//     #[component(clone_behavior=Custom(custom))]
//     struct A(u32);

//     #[derive(Component, PartialEq, Debug)]
//     struct B(u32);

//     fn custom(_src: &SourceComponent, ctx: &mut ComponentCloneCtx) {
//         // Clone using deferred
//         let source = ctx.source();
//         ctx.queue_deferred(move |world, mapper| {
//             let target = mapper.get_mapped(source);
//             world.entity_mut(target).insert(A(10));
//         });
//     }

//     let mut world = World::default();
//     let e = world.spawn((A(0), B(1))).id();
//     let e_clone = world.spawn_empty().id();
//     let mut builder = EntityCloner::build_opt_out(&mut world);
//     builder.move_components(true);
//     let mut cloner = builder.finish();

//     cloner.clone_entity(&mut world, e, e_clone);

//     assert_eq!(world.get::<A>(e), None);
//     assert_eq!(world.get::<A>(e_clone), Some(&A(10)));
//     assert_eq!(world.get::<B>(e), None);
//     assert_eq!(world.get::<B>(e_clone), Some(&B(1)));
// }

// #[test]
// fn move_relationship() {
//     #[derive(Component, Clone, PartialEq, Eq, Debug)]
//     #[relationship(relationship_target=Target)]
//     struct Source(Entity);

//     #[derive(Component, Clone, PartialEq, Eq, Debug)]
//     #[relationship_target(relationship=Source)]
//     struct Target(Vec<Entity>);

//     #[derive(Component, PartialEq, Debug)]
//     struct A(u32);

//     let mut world = World::default();
//     let e_target = world.spawn(A(1)).id();
//     let e_source = world.spawn((A(2), Source(e_target))).id();

//     let mut builder = EntityCloner::build_opt_out(&mut world);
//     builder.move_components(true);
//     let mut cloner = builder.finish();

//     let e_source_moved = world.spawn_empty().id();

//     cloner.clone_entity(&mut world, e_source, e_source_moved);

//     assert_eq!(world.get::<A>(e_source), None);
//     assert_eq!(world.get::<A>(e_source_moved), Some(&A(2)));
//     assert_eq!(world.get::<Source>(e_source), None);
//     assert_eq!(world.get::<Source>(e_source_moved), Some(&Source(e_target)));
//     assert_eq!(
//         world.get::<Target>(e_target),
//         Some(&Target(alloc::vec![e_source_moved]))
//     );

//     let e_target_moved = world.spawn_empty().id();

//     cloner.clone_entity(&mut world, e_target, e_target_moved);

//     assert_eq!(world.get::<A>(e_target), None);
//     assert_eq!(world.get::<A>(e_target_moved), Some(&A(1)));
//     assert_eq!(world.get::<Target>(e_target), None);
//     assert_eq!(
//         world.get::<Target>(e_target_moved),
//         Some(&Target(alloc::vec![e_source_moved]))
//     );
//     assert_eq!(
//         world.get::<Source>(e_source_moved),
//         Some(&Source(e_target_moved))
//     );
// }

// #[test]
// fn move_hierarchy() {
//     #[derive(Component, PartialEq, Debug)]
//     struct A(u32);

//     let mut world = World::default();
//     let e_parent = world.spawn(A(1)).id();
//     let e_child1 = world.spawn((A(2), ChildOf(e_parent))).id();
//     let e_child2 = world.spawn((A(3), ChildOf(e_parent))).id();
//     let e_child1_1 = world.spawn((A(4), ChildOf(e_child1))).id();

//     let e_parent_clone = world.spawn_empty().id();

//     let mut builder = EntityCloner::build_opt_out(&mut world);
//     builder.move_components(true).linked_cloning(true);
//     let mut cloner = builder.finish();

//     cloner.clone_entity(&mut world, e_parent, e_parent_clone);

//     assert_eq!(world.get::<A>(e_parent), None);
//     assert_eq!(world.get::<A>(e_child1), None);
//     assert_eq!(world.get::<A>(e_child2), None);
//     assert_eq!(world.get::<A>(e_child1_1), None);

//     let mut children = world.get::<Children>(e_parent_clone).unwrap().iter();
//     let e_child1_clone = *children.next().unwrap();
//     let e_child2_clone = *children.next().unwrap();
//     let mut children = world.get::<Children>(e_child1_clone).unwrap().iter();
//     let e_child1_1_clone = *children.next().unwrap();

//     assert_eq!(world.get::<A>(e_parent_clone), Some(&A(1)));
//     assert_eq!(world.get::<A>(e_child1_clone), Some(&A(2)));
//     assert_eq!(
//         world.get::<ChildOf>(e_child1_clone),
//         Some(&ChildOf(e_parent_clone))
//     );
//     assert_eq!(world.get::<A>(e_child2_clone), Some(&A(3)));
//     assert_eq!(
//         world.get::<ChildOf>(e_child2_clone),
//         Some(&ChildOf(e_parent_clone))
//     );
//     assert_eq!(world.get::<A>(e_child1_1_clone), Some(&A(4)));
//     assert_eq!(
//         world.get::<ChildOf>(e_child1_1_clone),
//         Some(&ChildOf(e_child1_clone))
//     );
// }

// // Original: E1 Target{target: [E2], data: [4,5,6]}
// //            | E2 Source{target: E1, data: [1,2,3]}
// //
// // Cloned:   E3 Target{target: [], data: [4,5,6]}
// #[test]
// fn clone_relationship_with_data() {
//     #[derive(Component, Clone)]
//     #[relationship(relationship_target=Target)]
//     struct Source {
//         #[relationship]
//         target: Entity,
//         data: Vec<u8>,
//     }

//     #[derive(Component, Clone)]
//     #[relationship_target(relationship=Source)]
//     struct Target {
//         #[relationship]
//         target: Vec<Entity>,
//         data: Vec<u8>,
//     }

//     let mut world = World::default();
//     let e_target = world.spawn_empty().id();
//     let e_source = world
//         .spawn(Source {
//             target: e_target,
//             data: alloc::vec![1, 2, 3],
//         })
//         .id();
//     world.get_mut::<Target>(e_target).unwrap().data = alloc::vec![4, 5, 6];

//     let builder = EntityCloner::build_opt_out(&mut world);
//     let mut cloner = builder.finish();

//     let e_target_clone = world.spawn_empty().id();
//     cloner.clone_entity(&mut world, e_target, e_target_clone);

//     let target = world.get::<Target>(e_target).unwrap();
//     let cloned_target = world.get::<Target>(e_target_clone).unwrap();

//     assert_eq!(cloned_target.data, target.data);
//     assert_eq!(target.target, alloc::vec![e_source]);
//     assert_eq!(cloned_target.target.len(), 0);

//     let source = world.get::<Source>(e_source).unwrap();

//     assert_eq!(source.data, alloc::vec![1, 2, 3]);
// }

// // Original: E1 Target{target: [E2], data: [4,5,6]}
// //            | E2 Source{target: E1, data: [1,2,3]}
// //
// // Cloned:   E3 Target{target: [E4], data: [4,5,6]}
// //            | E4 Source{target: E3, data: [1,2,3]}
// #[test]
// fn clone_linked_relationship_with_data() {
//     #[derive(Component, Clone)]
//     #[relationship(relationship_target=Target)]
//     struct Source {
//         #[relationship]
//         target: Entity,
//         data: Vec<u8>,
//     }

//     #[derive(Component, Clone)]
//     #[relationship_target(relationship=Source, linked_spawn)]
//     struct Target {
//         #[relationship]
//         target: Vec<Entity>,
//         data: Vec<u8>,
//     }

//     let mut world = World::default();
//     let e_target = world.spawn_empty().id();
//     let e_source = world
//         .spawn(Source {
//             target: e_target,
//             data: alloc::vec![1, 2, 3],
//         })
//         .id();
//     world.get_mut::<Target>(e_target).unwrap().data = alloc::vec![4, 5, 6];

//     let mut builder = EntityCloner::build_opt_out(&mut world);
//     builder.linked_cloning(true);
//     let mut cloner = builder.finish();

//     let e_target_clone = world.spawn_empty().id();
//     cloner.clone_entity(&mut world, e_target, e_target_clone);

//     let target = world.get::<Target>(e_target).unwrap();
//     let cloned_target = world.get::<Target>(e_target_clone).unwrap();

//     assert_eq!(cloned_target.data, target.data);
//     assert_eq!(target.target, alloc::vec![e_source]);
//     assert_eq!(cloned_target.target.len(), 1);

//     let source = world.get::<Source>(e_source).unwrap();
//     let cloned_source = world.get::<Source>(cloned_target.target[0]).unwrap();

//     assert_eq!(cloned_source.data, source.data);
//     assert_eq!(source.target, e_target);
//     assert_eq!(cloned_source.target, e_target_clone);
// }

// // Original: E1
// //           E2
// //
// // Moved:    E3 Target{target: [], data: [4,5,6]}
// #[test]
// fn move_relationship_with_data() {
//     #[derive(Component, Clone, PartialEq, Eq, Debug)]
//     #[relationship(relationship_target=Target)]
//     struct Source {
//         #[relationship]
//         target: Entity,
//         data: Vec<u8>,
//     }

//     #[derive(Component, Clone, PartialEq, Eq, Debug)]
//     #[relationship_target(relationship=Source)]
//     struct Target {
//         #[relationship]
//         target: Vec<Entity>,
//         data: Vec<u8>,
//     }

//     let source_data = alloc::vec![1, 2, 3];
//     let target_data = alloc::vec![4, 5, 6];

//     let mut world = World::default();
//     let e_target = world.spawn_empty().id();
//     let e_source = world
//         .spawn(Source {
//             target: e_target,
//             data: source_data.clone(),
//         })
//         .id();
//     world.get_mut::<Target>(e_target).unwrap().data = target_data.clone();

//     let mut builder = EntityCloner::build_opt_out(&mut world);
//     builder.move_components(true);
//     let mut cloner = builder.finish();

//     let e_target_moved = world.spawn_empty().id();
//     cloner.clone_entity(&mut world, e_target, e_target_moved);

//     assert_eq!(world.get::<Target>(e_target), None);
//     assert_eq!(
//         world.get::<Source>(e_source),
//         Some(&Source {
//             data: source_data,
//             target: e_target_moved,
//         })
//     );
//     assert_eq!(
//         world.get::<Target>(e_target_moved),
//         Some(&Target {
//             target: alloc::vec![e_source],
//             data: target_data
//         })
//     );
// }

// // Original: E1
// //           E2
// //
// // Moved:    E3 Target{target: [E4], data: [4,5,6]}
// //            | E4 Source{target: E3, data: [1,2,3]}
// #[test]
// fn move_linked_relationship_with_data() {
//     #[derive(Component, Clone, PartialEq, Eq, Debug)]
//     #[relationship(relationship_target=Target)]
//     struct Source {
//         #[relationship]
//         target: Entity,
//         data: Vec<u8>,
//     }

//     #[derive(Component, Clone, PartialEq, Eq, Debug)]
//     #[relationship_target(relationship=Source, linked_spawn)]
//     struct Target {
//         #[relationship]
//         target: Vec<Entity>,
//         data: Vec<u8>,
//     }

//     let source_data = alloc::vec![1, 2, 3];
//     let target_data = alloc::vec![4, 5, 6];

//     let mut world = World::default();
//     let e_target = world.spawn_empty().id();
//     let e_source = world
//         .spawn(Source {
//             target: e_target,
//             data: source_data.clone(),
//         })
//         .id();
//     world.get_mut::<Target>(e_target).unwrap().data = target_data.clone();

//     let mut builder = EntityCloner::build_opt_out(&mut world);
//     builder.move_components(true).linked_cloning(true);
//     let mut cloner = builder.finish();

//     let e_target_moved = world.spawn_empty().id();
//     cloner.clone_entity(&mut world, e_target, e_target_moved);

//     assert_eq!(world.get::<Target>(e_target), None);
//     assert_eq!(world.get::<Source>(e_source), None);

//     let moved_target = world.get::<Target>(e_target_moved).unwrap();
//     assert_eq!(moved_target.data, target_data);
//     assert_eq!(moved_target.target.len(), 1);

//     let moved_source = world.get::<Source>(moved_target.target[0]).unwrap();
//     assert_eq!(moved_source.data, source_data);
//     assert_eq!(moved_source.target, e_target_moved);
// }
