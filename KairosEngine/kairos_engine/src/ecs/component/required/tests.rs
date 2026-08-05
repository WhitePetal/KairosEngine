use crate::{ecs::{component::RequiredComponentsError, world::World}, ptr::OwningPtr};



// #[test]
// fn required_components() {
//     #[derive(Component)]
//     #[require(Y)]
//     struct X;

//     #[derive(Component)]
//     #[require(Z = new_z())]
//     struct Y {
//         value: String,
//     }

//     #[derive(Component)]
//     struct Z(u32);

//     impl Default for Y {
//         fn default() -> Self {
//             Self {
//                 value: "hello".to_string(),
//             }
//         }
//     }

//     fn new_z() -> Z {
//         Z(7)
//     }

//     let mut world = World::new();
//     let id = world.spawn(X).id();
//     assert_eq!(
//         "hello",
//         world.entity(id).get::<Y>().unwrap().value,
//         "Y should have the default value"
//     );
//     assert_eq!(
//         7,
//         world.entity(id).get::<Z>().unwrap().0,
//         "Z should have the value provided by the constructor defined in Y"
//     );

//     let id = world
//         .spawn((
//             X,
//             Y {
//                 value: "foo".to_string(),
//             },
//         ))
//         .id();
//     assert_eq!(
//         "foo",
//         world.entity(id).get::<Y>().unwrap().value,
//         "Y should have the manually provided value"
//     );
//     assert_eq!(
//         7,
//         world.entity(id).get::<Z>().unwrap().0,
//         "Z should have the value provided by the constructor defined in Y"
//     );

//     let id = world.spawn((X, Z(8))).id();
//     assert_eq!(
//         "hello",
//         world.entity(id).get::<Y>().unwrap().value,
//         "Y should have the default value"
//     );
//     assert_eq!(
//         8,
//         world.entity(id).get::<Z>().unwrap().0,
//         "Z should have the manually provided value"
//     );
// }

// #[test]
// fn generic_required_components() {
//     #[derive(Component)]
//     #[require(Y<usize>)]
//     struct X;

//     #[derive(Component, Default)]
//     struct Y<T> {
//         value: T,
//     }

//     let mut world = World::new();
//     let id = world.spawn(X).id();
//     assert_eq!(
//         0,
//         world.entity(id).get::<Y<usize>>().unwrap().value,
//         "Y should have the default value"
//     );
// }

// #[test]
// fn required_components_spawn_nonexistent_hooks() {
//     #[derive(Component)]
//     #[require(Y)]
//     struct X;

//     #[derive(Component, Default)]
//     struct Y;

//     #[derive(Resource)]
//     struct A(usize);

//     #[derive(Resource)]
//     struct I(usize);

//     let mut world = World::new();
//     world.insert_resource(A(0));
//     world.insert_resource(I(0));
//     world
//         .register_component_hooks::<Y>()
//         .on_add(|mut world, _| world.resource_mut::<A>().0 += 1)
//         .on_insert(|mut world, _| world.resource_mut::<I>().0 += 1);

//     // Spawn entity and ensure Y was added
//     assert!(world.spawn(X).contains::<Y>());

//     assert_eq!(world.resource::<A>().0, 1);
//     assert_eq!(world.resource::<I>().0, 1);
// }

// #[test]
// fn required_components_insert_existing_hooks() {
//     #[derive(Component)]
//     #[require(Y)]
//     struct X;

//     #[derive(Component, Default)]
//     struct Y;

//     #[derive(Resource)]
//     struct A(usize);

//     #[derive(Resource)]
//     struct I(usize);

//     let mut world = World::new();
//     world.insert_resource(A(0));
//     world.insert_resource(I(0));
//     world
//         .register_component_hooks::<Y>()
//         .on_add(|mut world, _| world.resource_mut::<A>().0 += 1)
//         .on_insert(|mut world, _| world.resource_mut::<I>().0 += 1);

//     // Spawn entity and ensure Y was added
//     assert!(world.spawn_empty().insert(X).contains::<Y>());

//     assert_eq!(world.resource::<A>().0, 1);
//     assert_eq!(world.resource::<I>().0, 1);
// }

// #[test]
// fn required_components_take_leaves_required() {
//     #[derive(Component)]
//     #[require(Y)]
//     struct X;

//     #[derive(Component, Default)]
//     struct Y;

//     let mut world = World::new();
//     let e = world.spawn(X).id();
//     let _ = world.entity_mut(e).take::<X>().unwrap();
//     assert!(world.entity_mut(e).contains::<Y>());
// }

// #[test]
// fn required_components_retain_keeps_required() {
//     #[derive(Component)]
//     #[require(Y)]
//     struct X;

//     #[derive(Component, Default)]
//     struct Y;

//     #[derive(Component, Default)]
//     struct Z;

//     let mut world = World::new();
//     let e = world.spawn((X, Z)).id();
//     world.entity_mut(e).retain::<X>();
//     assert!(world.entity_mut(e).contains::<X>());
//     assert!(world.entity_mut(e).contains::<Y>());
//     assert!(!world.entity_mut(e).contains::<Z>());
// }

// #[test]
// fn required_components_spawn_then_insert_no_overwrite() {
//     #[derive(Component)]
//     #[require(Y)]
//     struct X;

//     #[derive(Component, Default)]
//     struct Y(usize);

//     let mut world = World::new();
//     let id = world.spawn((X, Y(10))).id();
//     world.entity_mut(id).insert(X);

//     assert_eq!(
//         10,
//         world.entity(id).get::<Y>().unwrap().0,
//         "Y should still have the manually provided value"
//     );
// }

// #[test]
// fn dynamic_required_components() {
//     #[derive(Component)]
//     #[require(Y)]
//     struct X;

//     #[derive(Component, Default)]
//     struct Y;

//     let mut world = World::new();
//     let x_id = world.register_component::<X>();

//     let mut e = world.spawn_empty();

//     // SAFETY: x_id is a valid component id
//     OwningPtr::make(X, |ptr| unsafe {
//         e.insert_by_id(x_id, ptr);
//     });

//     assert!(e.contains::<Y>());
// }

// #[test]
// fn remove_component_and_its_runtime_required_components() {
//     #[derive(Component)]
//     struct X;

//     #[derive(Component, Default)]
//     struct Y;

//     #[derive(Component, Default)]
//     struct Z;

//     #[derive(Component)]
//     struct V;

//     let mut world = World::new();
//     world.register_required_components::<X, Y>();
//     world.register_required_components::<Y, Z>();

//     let e = world.spawn((X, V)).id();
//     assert!(world.entity(e).contains::<X>());
//     assert!(world.entity(e).contains::<Y>());
//     assert!(world.entity(e).contains::<Z>());
//     assert!(world.entity(e).contains::<V>());

//     //check that `remove` works as expected
//     world.entity_mut(e).remove::<X>();
//     assert!(!world.entity(e).contains::<X>());
//     assert!(world.entity(e).contains::<Y>());
//     assert!(world.entity(e).contains::<Z>());
//     assert!(world.entity(e).contains::<V>());

//     world.entity_mut(e).insert(X);
//     assert!(world.entity(e).contains::<X>());
//     assert!(world.entity(e).contains::<Y>());
//     assert!(world.entity(e).contains::<Z>());
//     assert!(world.entity(e).contains::<V>());

//     //remove `X` again and ensure that `Y` and `Z` was removed too
//     world.entity_mut(e).remove_with_requires::<X>();
//     assert!(!world.entity(e).contains::<X>());
//     assert!(!world.entity(e).contains::<Y>());
//     assert!(!world.entity(e).contains::<Z>());
//     assert!(world.entity(e).contains::<V>());
// }

// #[test]
// fn remove_component_and_its_required_components() {
//     #[derive(Component)]
//     #[require(Y)]
//     struct X;

//     #[derive(Component, Default)]
//     #[require(Z)]
//     struct Y;

//     #[derive(Component, Default)]
//     struct Z;

//     #[derive(Component)]
//     struct V;

//     let mut world = World::new();

//     let e = world.spawn((X, V)).id();
//     assert!(world.entity(e).contains::<X>());
//     assert!(world.entity(e).contains::<Y>());
//     assert!(world.entity(e).contains::<Z>());
//     assert!(world.entity(e).contains::<V>());

//     //check that `remove` works as expected
//     world.entity_mut(e).remove::<X>();
//     assert!(!world.entity(e).contains::<X>());
//     assert!(world.entity(e).contains::<Y>());
//     assert!(world.entity(e).contains::<Z>());
//     assert!(world.entity(e).contains::<V>());

//     world.entity_mut(e).insert(X);
//     assert!(world.entity(e).contains::<X>());
//     assert!(world.entity(e).contains::<Y>());
//     assert!(world.entity(e).contains::<Z>());
//     assert!(world.entity(e).contains::<V>());

//     //remove `X` again and ensure that `Y` and `Z` was removed too
//     world.entity_mut(e).remove_with_requires::<X>();
//     assert!(!world.entity(e).contains::<X>());
//     assert!(!world.entity(e).contains::<Y>());
//     assert!(!world.entity(e).contains::<Z>());
//     assert!(world.entity(e).contains::<V>());
// }

// #[test]
// fn remove_bundle_and_his_required_components() {
//     #[derive(Component, Default)]
//     #[require(Y)]
//     struct X;

//     #[derive(Component, Default)]
//     struct Y;

//     #[derive(Component, Default)]
//     #[require(W)]
//     struct Z;

//     #[derive(Component, Default)]
//     struct W;

//     #[derive(Component)]
//     struct V;

//     #[derive(Bundle, Default)]
//     struct TestBundle {
//         x: X,
//         z: Z,
//     }

//     let mut world = World::new();
//     let e = world.spawn((TestBundle::default(), V)).id();

//     assert!(world.entity(e).contains::<X>());
//     assert!(world.entity(e).contains::<Y>());
//     assert!(world.entity(e).contains::<Z>());
//     assert!(world.entity(e).contains::<W>());
//     assert!(world.entity(e).contains::<V>());

//     world.entity_mut(e).remove_with_requires::<TestBundle>();
//     assert!(!world.entity(e).contains::<X>());
//     assert!(!world.entity(e).contains::<Y>());
//     assert!(!world.entity(e).contains::<Z>());
//     assert!(!world.entity(e).contains::<W>());
//     assert!(world.entity(e).contains::<V>());
// }

// #[test]
// fn runtime_required_components() {
//     // Same as `required_components` test but with runtime registration

//     #[derive(Component)]
//     struct X;

//     #[derive(Component)]
//     struct Y {
//         value: String,
//     }

//     #[derive(Component)]
//     struct Z(u32);

//     impl Default for Y {
//         fn default() -> Self {
//             Self {
//                 value: "hello".to_string(),
//             }
//         }
//     }

//     let mut world = World::new();

//     world.register_required_components::<X, Y>();
//     world.register_required_components_with::<Y, Z>(|| Z(7));

//     let id = world.spawn(X).id();

//     assert_eq!(
//         "hello",
//         world.entity(id).get::<Y>().unwrap().value,
//         "Y should have the default value"
//     );
//     assert_eq!(
//         7,
//         world.entity(id).get::<Z>().unwrap().0,
//         "Z should have the value provided by the constructor defined in Y"
//     );

//     let id = world
//         .spawn((
//             X,
//             Y {
//                 value: "foo".to_string(),
//             },
//         ))
//         .id();
//     assert_eq!(
//         "foo",
//         world.entity(id).get::<Y>().unwrap().value,
//         "Y should have the manually provided value"
//     );
//     assert_eq!(
//         7,
//         world.entity(id).get::<Z>().unwrap().0,
//         "Z should have the value provided by the constructor defined in Y"
//     );

//     let id = world.spawn((X, Z(8))).id();
//     assert_eq!(
//         "hello",
//         world.entity(id).get::<Y>().unwrap().value,
//         "Y should have the default value"
//     );
//     assert_eq!(
//         8,
//         world.entity(id).get::<Z>().unwrap().0,
//         "Z should have the manually provided value"
//     );
// }

// #[test]
// fn runtime_required_components_override_1() {
//     #[derive(Component)]
//     struct X;

//     #[derive(Component, Default)]
//     struct Y;

//     #[derive(Component)]
//     struct Z(u32);

//     let mut world = World::new();

//     // - X requires Y with default constructor
//     // - Y requires Z with custom constructor
//     // - X requires Z with custom constructor (more specific than X -> Y -> Z)
//     world.register_required_components::<X, Y>();
//     world.register_required_components_with::<Y, Z>(|| Z(5));
//     world.register_required_components_with::<X, Z>(|| Z(7));

//     let id = world.spawn(X).id();

//     assert_eq!(
//         7,
//         world.entity(id).get::<Z>().unwrap().0,
//         "Z should have the value provided by the constructor defined in X"
//     );
// }

// #[test]
// fn runtime_required_components_override_2() {
//     // Same as `runtime_required_components_override_1` test but with different registration order

//     #[derive(Component)]
//     struct X;

//     #[derive(Component, Default)]
//     struct Y;

//     #[derive(Component)]
//     struct Z(u32);

//     let mut world = World::new();

//     // - X requires Y with default constructor
//     // - X requires Z with custom constructor (more specific than X -> Y -> Z)
//     // - Y requires Z with custom constructor
//     world.register_required_components::<X, Y>();
//     world.register_required_components_with::<X, Z>(|| Z(7));
//     world.register_required_components_with::<Y, Z>(|| Z(5));

//     let id = world.spawn(X).id();

//     assert_eq!(
//         7,
//         world.entity(id).get::<Z>().unwrap().0,
//         "Z should have the value provided by the constructor defined in X"
//     );
// }

// #[test]
// fn runtime_required_components_propagate_up() {
//     // `A` requires `B` directly.
//     #[derive(Component)]
//     #[require(B)]
//     struct A;

//     #[derive(Component, Default)]
//     struct B;

//     #[derive(Component, Default)]
//     struct C;

//     let mut world = World::new();

//     // `B` requires `C` with a runtime registration.
//     // `A` should also require `C` because it requires `B`.
//     world.register_required_components::<B, C>();

//     let id = world.spawn(A).id();

//     assert!(world.entity(id).get::<C>().is_some());
// }

// #[test]
// fn runtime_required_components_propagate_up_even_more() {
//     #[derive(Component)]
//     struct A;

//     #[derive(Component, Default)]
//     struct B;

//     #[derive(Component, Default)]
//     struct C;

//     #[derive(Component, Default)]
//     struct D;

//     let mut world = World::new();

//     world.register_required_components::<A, B>();
//     world.register_required_components::<B, C>();
//     world.register_required_components::<C, D>();

//     let id = world.spawn(A).id();

//     assert!(world.entity(id).get::<D>().is_some());
// }

// #[test]
// fn runtime_required_components_deep_require_does_not_override_shallow_require() {
//     #[derive(Component)]
//     struct A;
//     #[derive(Component, Default)]
//     struct B;
//     #[derive(Component, Default)]
//     struct C;
//     #[derive(Component)]
//     struct Counter(i32);
//     #[derive(Component, Default)]
//     struct D;

//     let mut world = World::new();

//     world.register_required_components::<A, B>();
//     world.register_required_components::<B, C>();
//     world.register_required_components::<C, D>();
//     world.register_required_components_with::<D, Counter>(|| Counter(2));
//     // This should replace the require constructor in A since it is
//     // shallower.
//     world.register_required_components_with::<C, Counter>(|| Counter(1));

//     let id = world.spawn(A).id();

//     // The "shallower" of the two components is used.
//     assert_eq!(world.entity(id).get::<Counter>().unwrap().0, 1);
// }

// #[test]
// fn runtime_required_components_deep_require_does_not_override_shallow_require_deep_subtree_after_shallow(
// ) {
//     #[derive(Component)]
//     struct A;
//     #[derive(Component, Default)]
//     struct B;
//     #[derive(Component, Default)]
//     struct C;
//     #[derive(Component, Default)]
//     struct D;
//     #[derive(Component, Default)]
//     struct E;
//     #[derive(Component)]
//     struct Counter(i32);
//     #[derive(Component, Default)]
//     struct F;

//     let mut world = World::new();

//     world.register_required_components::<A, B>();
//     world.register_required_components::<B, C>();
//     world.register_required_components::<C, D>();
//     world.register_required_components::<D, E>();
//     world.register_required_components_with::<E, Counter>(|| Counter(1));
//     world.register_required_components_with::<F, Counter>(|| Counter(2));
//     world.register_required_components::<E, F>();

//     let id = world.spawn(A).id();

//     // The "shallower" of the two components is used.
//     assert_eq!(world.entity(id).get::<Counter>().unwrap().0, 1);
// }

// #[test]
// fn runtime_required_components_existing_archetype() {
//     #[derive(Component)]
//     struct X;

//     #[derive(Component, Default)]
//     struct Y;

//     let mut world = World::new();

//     // Registering required components after the archetype has already been created should panic.
//     // This may change in the future.
//     world.spawn(X);
//     assert!(matches!(
//         world.try_register_required_components::<X, Y>(),
//         Err(RequiredComponentsError::ArchetypeExists(_))
//     ));
// }

// #[test]
// fn runtime_required_components_fail_with_duplicate() {
//     #[derive(Component)]
//     #[require(Y)]
//     struct X;

//     #[derive(Component, Default)]
//     struct Y;

//     let mut world = World::new();

//     // This should fail: Tried to register Y as a requirement for X, but the requirement already exists.
//     assert!(matches!(
//         world.try_register_required_components::<X, Y>(),
//         Err(RequiredComponentsError::DuplicateRegistration(_, _))
//     ));
// }

// #[test]
// fn required_components_bundle_priority() {
//     #[derive(Component, PartialEq, Eq, Clone, Copy, Debug)]
//     struct MyRequired(bool);

//     #[derive(Component, Default)]
//     #[require(MyRequired(false))]
//     struct MiddleMan;

//     #[derive(Component, Default)]
//     #[require(MiddleMan)]
//     struct ConflictingRequire;

//     #[derive(Component, Default)]
//     #[require(MyRequired(true))]
//     struct MyComponent;

//     let mut world = World::new();
//     let order_a = world
//         .spawn((ConflictingRequire, MyComponent))
//         .get::<MyRequired>()
//         .cloned();
//     let order_b = world
//         .spawn((MyComponent, ConflictingRequire))
//         .get::<MyRequired>()
//         .cloned();

//     assert_eq!(order_a, Some(MyRequired(false)));
//     assert_eq!(order_b, Some(MyRequired(true)));
// }

// #[test]
// #[should_panic]
// fn required_components_recursion_errors() {
//     #[derive(Component, Default)]
//     #[require(B)]
//     struct A;

//     #[derive(Component, Default)]
//     #[require(C)]
//     struct B;

//     #[derive(Component, Default)]
//     #[require(B)]
//     struct C;

//     World::new().register_component::<A>();
// }

// #[test]
// #[should_panic]
// fn required_components_self_errors() {
//     #[derive(Component, Default)]
//     #[require(A)]
//     struct A;

//     World::new().register_component::<A>();
// }

// #[test]
// fn regression_19333() {
//     #[derive(Component)]
//     struct X(usize);

//     #[derive(Default, Component)]
//     #[require(X(0))]
//     struct Base;

//     #[derive(Default, Component)]
//     #[require(X(1), Base)]
//     struct A;

//     #[derive(Default, Component)]
//     #[require(A, Base)]
//     struct B;

//     #[derive(Default, Component)]
//     #[require(B, Base)]
//     struct C;

//     let mut w = World::new();

//     assert_eq!(w.spawn(B).get::<X>().unwrap().0, 1);
//     assert_eq!(w.spawn(C).get::<X>().unwrap().0, 1);
// }

// #[test]
// fn required_components_depth_first_2v1() {
//     #[derive(Component)]
//     struct X(usize);

//     #[derive(Component)]
//     #[require(Left, Right)]
//     struct Root;

//     #[derive(Component, Default)]
//     #[require(LeftLeft)]
//     struct Left;

//     #[derive(Component, Default)]
//     #[require(X(0))] // This is at depth 2 but is more on the left of the tree
//     struct LeftLeft;

//     #[derive(Component, Default)]
//     #[require(X(1))] //. This is at depth 1 but is more on the right of the tree
//     struct Right;

//     let mut world = World::new();

//     // LeftLeft should have priority over Right
//     assert_eq!(world.spawn(Root).get::<X>().unwrap().0, 0);
// }

// #[test]
// fn required_components_depth_first_3v1() {
//     #[derive(Component)]
//     struct X(usize);

//     #[derive(Component)]
//     #[require(Left, Right)]
//     struct Root;

//     #[derive(Component, Default)]
//     #[require(LeftLeft)]
//     struct Left;

//     #[derive(Component, Default)]
//     #[require(LeftLeftLeft)]
//     struct LeftLeft;

//     #[derive(Component, Default)]
//     #[require(X(0))] // This is at depth 3 but is more on the left of the tree
//     struct LeftLeftLeft;

//     #[derive(Component, Default)]
//     #[require(X(1))] //. This is at depth 1 but is more on the right of the tree
//     struct Right;

//     let mut world = World::new();

//     // LeftLeftLeft should have priority over Right
//     assert_eq!(world.spawn(Root).get::<X>().unwrap().0, 0);
// }

// #[test]
// fn runtime_required_components_depth_first_2v1() {
//     #[derive(Component)]
//     struct X(usize);

//     #[derive(Component)]
//     struct Root;

//     #[derive(Component, Default)]
//     struct Left;

//     #[derive(Component, Default)]
//     struct LeftLeft;

//     #[derive(Component, Default)]
//     struct Right;

//     // Register bottom up: registering higher level components should pick up lower level ones.
//     let mut world = World::new();
//     world.register_required_components_with::<LeftLeft, X>(|| X(0));
//     world.register_required_components_with::<Right, X>(|| X(1));
//     world.register_required_components::<Left, LeftLeft>();
//     world.register_required_components::<Root, Left>();
//     world.register_required_components::<Root, Right>();
//     assert_eq!(world.spawn(Root).get::<X>().unwrap().0, 0);

//     // Register top down: registering lower components should propagate to higher ones
//     let mut world = World::new();
//     world.register_required_components::<Root, Left>(); // Note: still register Left before Right
//     world.register_required_components::<Root, Right>();
//     world.register_required_components::<Left, LeftLeft>();
//     world.register_required_components_with::<Right, X>(|| X(1));
//     world.register_required_components_with::<LeftLeft, X>(|| X(0));
//     assert_eq!(world.spawn(Root).get::<X>().unwrap().0, 0);

//     // Register top down again, but this time LeftLeft before Right
//     let mut world = World::new();
//     world.register_required_components::<Root, Left>();
//     world.register_required_components::<Root, Right>();
//     world.register_required_components::<Left, LeftLeft>();
//     world.register_required_components_with::<LeftLeft, X>(|| X(0));
//     world.register_required_components_with::<Right, X>(|| X(1));
//     assert_eq!(world.spawn(Root).get::<X>().unwrap().0, 0);
// }

// #[test]
// fn runtime_required_components_propagate_metadata_alternate() {
//     #[derive(Component, Default)]
//     #[require(L1)]
//     struct L0;

//     #[derive(Component, Default)]
//     struct L1;

//     #[derive(Component, Default)]
//     #[require(L3)]
//     struct L2;

//     #[derive(Component, Default)]
//     struct L3;

//     #[derive(Component, Default)]
//     #[require(L5)]
//     struct L4;

//     #[derive(Component, Default)]
//     struct L5;

//     // Try to piece the 3 requirements together
//     let mut world = World::new();
//     world.register_required_components::<L1, L2>();
//     world.register_required_components::<L3, L4>();
//     let e = world.spawn(L0).id();
//     assert!(world
//         .query::<(&L0, &L1, &L2, &L3, &L4, &L5)>()
//         .get(&world, e)
//         .is_ok());

//     // Repeat but in the opposite order
//     let mut world = World::new();
//     world.register_required_components::<L3, L4>();
//     world.register_required_components::<L1, L2>();
//     let e = world.spawn(L0).id();
//     assert!(world
//         .query::<(&L0, &L1, &L2, &L3, &L4, &L5)>()
//         .get(&world, e)
//         .is_ok());
// }

// #[test]
// fn runtime_required_components_propagate_metadata_chain() {
//     #[derive(Component, Default)]
//     #[require(L1)]
//     struct L0;

//     #[derive(Component, Default)]
//     struct L1;

//     #[derive(Component, Default)]
//     struct L2;

//     #[derive(Component, Default)]
//     #[require(L4)]
//     struct L3;

//     #[derive(Component, Default)]
//     struct L4;

//     // Try to piece the 3 requirements together
//     let mut world = World::new();
//     world.register_required_components::<L1, L2>();
//     world.register_required_components::<L2, L3>();
//     let e = world.spawn(L0).id();
//     assert!(world
//         .query::<(&L0, &L1, &L2, &L3, &L4)>()
//         .get(&world, e)
//         .is_ok());

//     // Repeat but in the opposite order
//     let mut world = World::new();
//     world.register_required_components::<L2, L3>();
//     world.register_required_components::<L1, L2>();
//     let e = world.spawn(L0).id();
//     assert!(world
//         .query::<(&L0, &L1, &L2, &L3, &L4)>()
//         .get(&world, e)
//         .is_ok());
// }

// #[test]
// fn runtime_required_components_cyclic() {
//     #[derive(Component, Default)]
//     #[require(B)]
//     struct A;

//     #[derive(Component, Default)]
//     struct B;

//     #[derive(Component, Default)]
//     struct C;

//     let mut world = World::new();

//     assert!(world.try_register_required_components::<B, C>().is_ok());
//     assert!(matches!(
//         world.try_register_required_components::<C, A>(),
//         Err(RequiredComponentsError::CyclicRequirement(_, _))
//     ));
// }
