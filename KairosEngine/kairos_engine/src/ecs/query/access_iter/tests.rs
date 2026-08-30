use crate::ecs::{
    query::{
        WorldQuery,
        access_iter::{has_conflicts_large, has_conflicts_small},
        has_conflicts,
    },
    world::{EntityMut, EntityMutExcept, EntityRef, EntityRefExcept, World},
};

// #[derive(Component)]
// struct C1;

// #[derive(Component)]
// struct C2;

// fn setup_world() -> World {
//     let world = World::new();
//     let mut world = world;
//     world.register_component::<C1>();
//     world.register_component::<C2>();
//     world
// }

// #[test]
// fn simple_compatible() {
//     let world = setup_world();
//     let c = world.components();

//     // Compatible
//     let state = <&mut C1 as WorldQuery>::get_state(c).unwrap();
//     assert!(has_conflicts_small::<&mut C1>(&state).is_ok());
//     assert!(has_conflicts_large::<&mut C1>(&state).is_ok());
//     assert!(has_conflicts::<&mut C1>(c).is_ok());

//     let state = <&C1 as WorldQuery>::get_state(c).unwrap();
//     assert!(has_conflicts_small::<&C1>(&state).is_ok());
//     assert!(has_conflicts_large::<&C1>(&state).is_ok());
//     assert!(has_conflicts::<&C1>(c).is_ok());

//     let state = <(&C1, &C1) as WorldQuery>::get_state(c).unwrap();
//     assert!(has_conflicts_small::<(&C1, &C1)>(&state).is_ok());
//     assert!(has_conflicts_large::<(&C1, &C1)>(&state).is_ok());
//     assert!(has_conflicts::<(&C1, &C1)>(c).is_ok());
// }

// #[test]
// #[should_panic(expected = "conflicts")]
// fn conflict_component_read_conflicts_write() {
//     let world = setup_world();
//     let c = world.components();
//     let state = <(&C1, &mut C1) as WorldQuery>::get_state(c).unwrap();
//     assert!(has_conflicts_small::<(&C1, &mut C1)>(&state).is_err());
//     assert!(has_conflicts_large::<(&C1, &mut C1)>(&state).is_err());
//     let _ = has_conflicts::<(&C1, &mut C1)>(c);
// }

// #[test]
// #[should_panic(expected = "conflicts")]
// fn conflict_component_write_conflicts_read() {
//     let world = setup_world();
//     let c = world.components();
//     let state = <(&mut C1, &C1) as WorldQuery>::get_state(c).unwrap();
//     assert!(has_conflicts_small::<(&mut C1, &C1)>(&state).is_err());
//     assert!(has_conflicts_large::<(&mut C1, &C1)>(&state).is_err());
//     let _ = has_conflicts::<(&mut C1, &C1)>(c);
// }

// #[test]
// #[should_panic(expected = "conflicts")]
// fn conflict_component_write_conflicts_write() {
//     let world = setup_world();
//     let c = world.components();
//     let state = <(&mut C1, &mut C1) as WorldQuery>::get_state(c).unwrap();
//     assert!(has_conflicts_small::<(&mut C1, &mut C1)>(&state).is_err());
//     assert!(has_conflicts_large::<(&mut C1, &mut C1)>(&state).is_err());
//     let _ = has_conflicts::<(&mut C1, &mut C1)>(c);
// }

// #[test]
// fn entity_ref_compatible() {
//     let world = setup_world();
//     let c = world.components();

//     // Compatible
//     let state = <(EntityRef, &C1) as WorldQuery>::get_state(c).unwrap();
//     assert!(has_conflicts_small::<(EntityRef, &C1)>(&state).is_ok());
//     assert!(has_conflicts_large::<(EntityRef, &C1)>(&state).is_ok());
//     assert!(has_conflicts::<(EntityRef, &C1)>(c).is_ok());

//     let state = <(&C1, EntityRef) as WorldQuery>::get_state(c).unwrap();
//     assert!(has_conflicts_small::<(&C1, EntityRef)>(&state).is_ok());
//     assert!(has_conflicts_large::<(&C1, EntityRef)>(&state).is_ok());
//     assert!(has_conflicts::<(&C1, EntityRef)>(c).is_ok());

//     let state = <(EntityRef, EntityRef) as WorldQuery>::get_state(c).unwrap();
//     assert!(has_conflicts_small::<(EntityRef, EntityRef)>(&state).is_ok());
//     assert!(has_conflicts_large::<(EntityRef, EntityRef)>(&state).is_ok());
//     assert!(has_conflicts::<(EntityRef, EntityRef)>(c).is_ok());
// }

// #[test]
// #[should_panic(expected = "conflicts")]
// fn entity_ref_conflicts_component_write() {
//     let world = setup_world();
//     let c = world.components();
//     let state = <(EntityRef, &mut C1) as WorldQuery>::get_state(c).unwrap();
//     assert!(has_conflicts_small::<(EntityRef, &mut C1)>(&state).is_err());
//     assert!(has_conflicts_large::<(EntityRef, &mut C1)>(&state).is_err());
//     let _ = has_conflicts::<(EntityRef, &mut C1)>(c);
// }

// #[test]
// #[should_panic(expected = "conflicts")]
// fn component_write_conflicts_entity_ref() {
//     let world = setup_world();
//     let c = world.components();
//     let state = <(&mut C1, EntityRef) as WorldQuery>::get_state(c).unwrap();
//     assert!(has_conflicts_small::<(&mut C1, EntityRef)>(&state).is_err());
//     assert!(has_conflicts_large::<(&mut C1, EntityRef)>(&state).is_err());
//     let _ = has_conflicts::<(&mut C1, EntityRef)>(c);
// }

// #[test]
// #[should_panic(expected = "conflicts")]
// fn entity_mut_conflicts_component_read() {
//     let world = setup_world();
//     let c = world.components();
//     let state = <(EntityMut, &C1) as WorldQuery>::get_state(c).unwrap();
//     assert!(has_conflicts_small::<(EntityMut, &C1)>(&state).is_err());
//     assert!(has_conflicts_large::<(EntityMut, &C1)>(&state).is_err());
//     let _ = has_conflicts::<(EntityMut, &C1)>(c);
// }

// #[test]
// #[should_panic(expected = "conflicts")]
// fn component_read_conflicts_entity_mut() {
//     let world = setup_world();
//     let c = world.components();
//     let state = <(&C1, EntityMut) as WorldQuery>::get_state(c).unwrap();
//     assert!(has_conflicts_small::<(&C1, EntityMut)>(&state).is_err());
//     assert!(has_conflicts_large::<(&C1, EntityMut)>(&state).is_err());
//     let _ = has_conflicts::<(&C1, EntityMut)>(c);
// }

// #[test]
// #[should_panic(expected = "conflicts")]
// fn entity_mut_conflicts_component_write() {
//     let world = setup_world();
//     let c = world.components();
//     let state = <(EntityMut, &mut C1) as WorldQuery>::get_state(c).unwrap();
//     assert!(has_conflicts_small::<(EntityMut, &mut C1)>(&state).is_err());
//     assert!(has_conflicts_large::<(EntityMut, &mut C1)>(&state).is_err());
//     let _ = has_conflicts::<(EntityMut, &mut C1)>(c);
// }

// #[test]
// #[should_panic(expected = "conflicts")]
// fn component_write_conflicts_entity_mut() {
//     let world = setup_world();
//     let c = world.components();
//     let state = <(&mut C1, EntityMut) as WorldQuery>::get_state(c).unwrap();
//     assert!(has_conflicts_small::<(&mut C1, EntityMut)>(&state).is_err());
//     assert!(has_conflicts_large::<(&mut C1, EntityMut)>(&state).is_err());
//     let _ = has_conflicts::<(&mut C1, EntityMut)>(c);
// }

// #[test]
// #[should_panic(expected = "conflicts")]
// fn entity_mut_conflicts_entity_ref() {
//     let world = setup_world();
//     let c = world.components();
//     let state = <(EntityMut, EntityRef) as WorldQuery>::get_state(c).unwrap();
//     assert!(has_conflicts_small::<(EntityMut, EntityRef)>(&state).is_err());
//     assert!(has_conflicts_large::<(EntityMut, EntityRef)>(&state).is_err());
//     let _ = has_conflicts::<(EntityMut, EntityRef)>(c);
// }

// #[test]
// #[should_panic(expected = "conflicts")]
// fn entity_ref_conflicts_entity_mut() {
//     let world = setup_world();
//     let c = world.components();
//     let state = <(EntityRef, EntityMut) as WorldQuery>::get_state(c).unwrap();
//     assert!(has_conflicts_small::<(EntityRef, EntityMut)>(&state).is_err());
//     assert!(has_conflicts_large::<(EntityRef, EntityMut)>(&state).is_err());
//     let _ = has_conflicts::<(EntityRef, EntityMut)>(c);
// }

// #[test]
// fn entity_ref_except_compatible() {
//     let world = setup_world();
//     let c = world.components();

//     // Compatible
//     let state = <(EntityRefExcept<C1>, &mut C1) as WorldQuery>::get_state(c).unwrap();
//     assert!(has_conflicts_small::<(EntityRefExcept<C1>, &mut C1)>(&state).is_ok());
//     assert!(has_conflicts_large::<(EntityRefExcept<C1>, &mut C1)>(&state).is_ok());
//     assert!(has_conflicts::<(EntityRefExcept<C1>, &mut C1)>(c).is_ok());

//     let state = <(&mut C1, EntityRefExcept<C1>) as WorldQuery>::get_state(c).unwrap();
//     assert!(has_conflicts_small::<(&mut C1, EntityRefExcept<C1>)>(&state).is_ok());
//     assert!(has_conflicts_large::<(&mut C1, EntityRefExcept<C1>)>(&state).is_ok());
//     assert!(has_conflicts::<(&mut C1, EntityRefExcept<C1>)>(c).is_ok());

//     let state = <(&C2, EntityRefExcept<C1>) as WorldQuery>::get_state(c).unwrap();
//     assert!(has_conflicts_small::<(&C2, EntityRefExcept<C1>)>(&state).is_ok());
//     assert!(has_conflicts_large::<(&C2, EntityRefExcept<C1>)>(&state).is_ok());
//     assert!(has_conflicts::<(&C2, EntityRefExcept<C1>)>(c).is_ok());

//     let state = <(&mut C1, EntityRefExcept<(C1, C2)>) as WorldQuery>::get_state(c).unwrap();
//     assert!(has_conflicts_small::<(&mut C1, EntityRefExcept<(C1, C2)>)>(&state).is_ok());
//     assert!(has_conflicts_large::<(&mut C1, EntityRefExcept<(C1, C2)>)>(&state).is_ok());
//     assert!(has_conflicts::<(&mut C1, EntityRefExcept<(C1, C2)>)>(c).is_ok());

//     let state = <(EntityRefExcept<(C1, C2)>, &mut C1) as WorldQuery>::get_state(c).unwrap();
//     assert!(has_conflicts_small::<(EntityRefExcept<(C1, C2)>, &mut C1)>(&state).is_ok());
//     assert!(has_conflicts_large::<(EntityRefExcept<(C1, C2)>, &mut C1)>(&state).is_ok());
//     assert!(has_conflicts::<(EntityRefExcept<(C1, C2)>, &mut C1)>(c).is_ok());

//     let state =
//         <(&mut C1, &mut C2, EntityRefExcept<(C1, C2)>) as WorldQuery>::get_state(c).unwrap();
//     assert!(
//         has_conflicts_small::<(&mut C1, &mut C2, EntityRefExcept<(C1, C2)>)>(&state).is_ok()
//     );
//     assert!(
//         has_conflicts_large::<(&mut C1, &mut C2, EntityRefExcept<(C1, C2)>)>(&state).is_ok()
//     );
//     assert!(has_conflicts::<(&mut C1, &mut C2, EntityRefExcept<(C1, C2)>)>(c).is_ok());

//     let state =
//         <(&mut C1, EntityRefExcept<(C1, C2)>, &mut C2) as WorldQuery>::get_state(c).unwrap();
//     assert!(
//         has_conflicts_small::<(&mut C1, EntityRefExcept<(C1, C2)>, &mut C2)>(&state).is_ok()
//     );
//     assert!(
//         has_conflicts_large::<(&mut C1, EntityRefExcept<(C1, C2)>, &mut C2)>(&state).is_ok()
//     );
//     assert!(has_conflicts::<(&mut C1, EntityRefExcept<(C1, C2)>, &mut C2)>(c).is_ok());

//     let state =
//         <(EntityRefExcept<(C1, C2)>, &mut C1, &mut C2) as WorldQuery>::get_state(c).unwrap();
//     assert!(
//         has_conflicts_small::<(EntityRefExcept<(C1, C2)>, &mut C1, &mut C2)>(&state).is_ok()
//     );
//     assert!(
//         has_conflicts_large::<(EntityRefExcept<(C1, C2)>, &mut C1, &mut C2)>(&state).is_ok()
//     );
//     assert!(has_conflicts::<(EntityRefExcept<(C1, C2)>, &mut C1, &mut C2)>(c).is_ok());
// }

// #[test]
// #[should_panic(expected = "conflicts")]
// fn entity_ref_except_conflicts_component_write() {
//     let world = setup_world();
//     let c = world.components();
//     let state = <(EntityRefExcept<C1>, &mut C2) as WorldQuery>::get_state(c).unwrap();
//     assert!(has_conflicts_small::<(EntityRefExcept<C1>, &mut C2)>(&state).is_err());
//     assert!(has_conflicts_large::<(EntityRefExcept<C1>, &mut C2)>(&state).is_err());
//     let _ = has_conflicts::<(EntityRefExcept<C1>, &mut C2)>(c);
// }

// #[test]
// #[should_panic(expected = "conflicts")]
// fn component_write_conflicts_entity_ref_except() {
//     let world = setup_world();
//     let c = world.components();
//     let state = <(&mut C2, EntityRefExcept<C1>) as WorldQuery>::get_state(c).unwrap();
//     assert!(has_conflicts_small::<(&mut C2, EntityRefExcept<C1>)>(&state).is_err());
//     assert!(has_conflicts_large::<(&mut C2, EntityRefExcept<C1>)>(&state).is_err());
//     let _ = has_conflicts::<(&mut C2, EntityRefExcept<C1>)>(c);
// }

// #[test]
// fn entity_mut_except_compatible() {
//     let world = setup_world();
//     let c = world.components();

//     // Compatible
//     let state = <(EntityMutExcept<C1>, &mut C1) as WorldQuery>::get_state(c).unwrap();
//     assert!(has_conflicts_small::<(EntityMutExcept<C1>, &mut C1)>(&state).is_ok());
//     assert!(has_conflicts_large::<(EntityMutExcept<C1>, &mut C1)>(&state).is_ok());
//     assert!(has_conflicts::<(EntityMutExcept<C1>, &mut C1)>(c).is_ok());

//     let state = <(&mut C1, EntityMutExcept<C1>) as WorldQuery>::get_state(c).unwrap();
//     assert!(has_conflicts_small::<(&mut C1, EntityMutExcept<C1>)>(&state).is_ok());
//     assert!(has_conflicts_large::<(&mut C1, EntityMutExcept<C1>)>(&state).is_ok());
//     assert!(has_conflicts::<(&mut C1, EntityMutExcept<C1>)>(c).is_ok());

//     let state = <(&mut C1, EntityMutExcept<(C1, C2)>) as WorldQuery>::get_state(c).unwrap();
//     assert!(has_conflicts_small::<(&mut C1, EntityMutExcept<(C1, C2)>)>(&state).is_ok());
//     assert!(has_conflicts_large::<(&mut C1, EntityMutExcept<(C1, C2)>)>(&state).is_ok());
//     assert!(has_conflicts::<(&mut C1, EntityMutExcept<(C1, C2)>)>(c).is_ok());

//     let state = <(EntityMutExcept<(C1, C2)>, &mut C1) as WorldQuery>::get_state(c).unwrap();
//     assert!(has_conflicts_small::<(EntityMutExcept<(C1, C2)>, &mut C1)>(&state).is_ok());
//     assert!(has_conflicts_large::<(EntityMutExcept<(C1, C2)>, &mut C1)>(&state).is_ok());
//     assert!(has_conflicts::<(EntityMutExcept<(C1, C2)>, &mut C1)>(c).is_ok());

//     let state =
//         <(&mut C1, &mut C2, EntityMutExcept<(C1, C2)>) as WorldQuery>::get_state(c).unwrap();
//     assert!(
//         has_conflicts_small::<(&mut C1, &mut C2, EntityMutExcept<(C1, C2)>)>(&state).is_ok()
//     );
//     assert!(
//         has_conflicts_large::<(&mut C1, &mut C2, EntityMutExcept<(C1, C2)>)>(&state).is_ok()
//     );
//     assert!(has_conflicts::<(&mut C1, &mut C2, EntityMutExcept<(C1, C2)>)>(c).is_ok());

//     let state =
//         <(&mut C1, EntityMutExcept<(C1, C2)>, &mut C2) as WorldQuery>::get_state(c).unwrap();
//     assert!(
//         has_conflicts_small::<(&mut C1, EntityMutExcept<(C1, C2)>, &mut C2)>(&state).is_ok()
//     );
//     assert!(
//         has_conflicts_large::<(&mut C1, EntityMutExcept<(C1, C2)>, &mut C2)>(&state).is_ok()
//     );
//     assert!(has_conflicts::<(&mut C1, EntityMutExcept<(C1, C2)>, &mut C2)>(c).is_ok());

//     let state =
//         <(EntityMutExcept<(C1, C2)>, &mut C1, &mut C2) as WorldQuery>::get_state(c).unwrap();
//     assert!(
//         has_conflicts_small::<(EntityMutExcept<(C1, C2)>, &mut C1, &mut C2)>(&state).is_ok()
//     );
//     assert!(
//         has_conflicts_large::<(EntityMutExcept<(C1, C2)>, &mut C1, &mut C2)>(&state).is_ok()
//     );
//     assert!(has_conflicts::<(EntityMutExcept<(C1, C2)>, &mut C1, &mut C2)>(c).is_ok());
// }

// #[test]
// #[should_panic(expected = "conflicts")]
// fn entity_mut_except_conflicts_component_read() {
//     let world = setup_world();
//     let c = world.components();
//     let state = <(EntityMutExcept<C1>, &C2) as WorldQuery>::get_state(c).unwrap();
//     assert!(has_conflicts_small::<(EntityMutExcept<C1>, &C2)>(&state).is_err());
//     assert!(has_conflicts_large::<(EntityMutExcept<C1>, &C2)>(&state).is_err());
//     let _ = has_conflicts::<(EntityMutExcept<C1>, &C2)>(c);
// }

// #[test]
// #[should_panic(expected = "conflicts")]
// fn component_read_conflicts_entity_mut_except() {
//     let world = setup_world();
//     let c = world.components();
//     let state = <(&C2, EntityMutExcept<C1>) as WorldQuery>::get_state(c).unwrap();
//     assert!(has_conflicts_small::<(&C2, EntityMutExcept<C1>)>(&state).is_err());
//     assert!(has_conflicts_large::<(&C2, EntityMutExcept<C1>)>(&state).is_err());
//     let _ = has_conflicts::<(&C2, EntityMutExcept<C1>)>(c);
// }

// #[test]
// #[should_panic(expected = "conflicts")]
// fn entity_mut_except_conflicts_component_write() {
//     let world = setup_world();
//     let c = world.components();
//     let state = <(EntityMutExcept<C1>, &mut C2) as WorldQuery>::get_state(c).unwrap();
//     assert!(has_conflicts_small::<(EntityMutExcept<C1>, &mut C2)>(&state).is_err());
//     assert!(has_conflicts_large::<(EntityMutExcept<C1>, &mut C2)>(&state).is_err());
//     let _ = has_conflicts::<(EntityMutExcept<C1>, &mut C2)>(c);
// }

// #[test]
// #[should_panic(expected = "conflicts")]
// fn component_write_conflicts_entity_mut_except() {
//     let world = setup_world();
//     let c = world.components();
//     let state = <(&mut C2, EntityMutExcept<C1>) as WorldQuery>::get_state(c).unwrap();
//     assert!(has_conflicts_small::<(&mut C2, EntityMutExcept<C1>)>(&state).is_err());
//     assert!(has_conflicts_large::<(&mut C2, EntityMutExcept<C1>)>(&state).is_err());
//     let _ = has_conflicts::<(&mut C2, EntityMutExcept<C1>)>(c);
// }
