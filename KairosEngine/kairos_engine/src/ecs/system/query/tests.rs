use crate::ecs::{entity::Entity, query::QueryEntityError, world::World};

#[test]
fn get_many_uniqueness() {
    let mut world = World::new();

    let entities: Vec<Entity> = (0..10).map(|_| world.spawn_empty().id()).collect();

    let mut query_state = world.query::<Entity>();

    // It's best to test get_many_mut_inner directly, as it is shared
    // We don't care about aliased mutability for the read-only equivalent

    // SAFETY: Query does not access world data.
    assert!(
        query_state
            .query_mut(&mut world)
            .get_many_mut_inner::<10>(entities.clone().try_into().unwrap())
            .is_ok()
    );

    assert_eq!(
        query_state
            .query_mut(&mut world)
            .get_many_mut_inner([entities[0], entities[0]])
            .unwrap_err(),
        QueryEntityError::AliasedMutability(entities[0])
    );

    assert_eq!(
        query_state
            .query_mut(&mut world)
            .get_many_mut_inner([entities[0], entities[1], entities[0]])
            .unwrap_err(),
        QueryEntityError::AliasedMutability(entities[0])
    );

    assert_eq!(
        query_state
            .query_mut(&mut world)
            .get_many_mut_inner([entities[9], entities[9]])
            .unwrap_err(),
        QueryEntityError::AliasedMutability(entities[9])
    );
}
