// use crate::ecs::world::World;

// #[derive(Clone, Copy, Component)]
// struct ComponentA;

// #[test]
// fn spawn_batch_does_not_leak_entities() {
//     let mut world = World::new();
//     world.spawn_batch((0u32..50).filter(|&i| i & 1 > 0).map(|_| ComponentA));
//     let total_allocated = world.entity_allocator().inner.total_entity_indices();
//     world.entity_allocator_mut().inner.flush_freed();
//     world.entity_allocator_mut().alloc();
//     let reused = world.entity_allocator().inner.total_entity_indices() == total_allocated;
//     assert!(reused);
// }
