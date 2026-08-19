use crate::ecs::world::World;

// #[test]
// #[should_panic = "is forbidden"]
// fn as_unsafe_world_cell_readonly_world_mut_forbidden() {
//     let world = World::new();
//     let world_cell = world.as_unsafe_world_cell_readonly();
//     // SAFETY: this invalid usage will be caught by a runtime panic.
//     let _ = unsafe { world_cell.world_mut() };
// }

// #[derive(Resource)]
// struct R;

// #[test]
// #[should_panic = "is forbidden"]
// fn as_unsafe_world_cell_readonly_resource_mut_forbidden() {
//     let mut world = World::new();
//     world.insert_resource(R);
//     let world_cell = world.as_unsafe_world_cell_readonly();
//     // SAFETY: this invalid usage will be caught by a runtime panic.
//     let _ = unsafe { world_cell.get_resource_mut::<R>() };
// }

// #[derive(Component)]
// struct C;

// #[test]
// #[should_panic = "is forbidden"]
// fn as_unsafe_world_cell_readonly_component_mut_forbidden() {
//     let mut world = World::new();
//     let entity = world.spawn(C).id();
//     let world_cell = world.as_unsafe_world_cell_readonly();
//     let entity_cell = world_cell.get_entity(entity).unwrap();
//     // SAFETY: this invalid usage will be caught by a runtime panic.
//     let _ = unsafe { entity_cell.get_mut::<C>() };
// }
