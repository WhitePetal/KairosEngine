use crate::ecs::{
    entity::{Entity, EntityHashMap, EntityMapper, SceneEntityMapper},
    world::World,
};

#[test]
fn entity_mapper() {
    let mut map = EntityHashMap::default();
    let mut world = World::new();
    let mut mapper = SceneEntityMapper::new(&mut map, &world);

    let mapped_ent = Entity::from_raw_u32(1).unwrap();
    let dead_ref = mapper.get_mapped(mapped_ent);

    assert_eq!(
        dead_ref,
        mapper.get_mapped(mapped_ent),
        "should persist the allocated mapping from the previous line"
    );
    assert_eq!(
        mapper.get_mapped(Entity::from_raw_u32(2).unwrap()).index(),
        dead_ref.index(),
        "should re-use the same index for further dead refs"
    );

    mapper.finish(&mut world);
    let freed_dead_ref = world.entities().resolve_from_index(dead_ref.index());
    assert!(
        freed_dead_ref
            .generation()
            .cmp_approx(&dead_ref.generation())
            .is_gt()
    );
    assert!(world.entities().check_can_spawn_at(freed_dead_ref).is_ok());
}

#[test]
fn world_scope_reserves_generations() {
    let mut map = EntityHashMap::default();
    let mut world = World::new();

    let dead_ref = SceneEntityMapper::world_scope(&mut map, &mut world, |_, mapper| {
        mapper.get_mapped(Entity::from_raw_u32(0).unwrap())
    });

    let freed_dead_ref = world.entities().resolve_from_index(dead_ref.index());
    assert!(
        freed_dead_ref
            .generation()
            .cmp_approx(&dead_ref.generation())
            .is_gt()
    );
    assert!(world.entities().check_can_spawn_at(freed_dead_ref).is_ok());
}
