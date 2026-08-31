// use crate::ecs::{query::QueryEntityError, world::World};

// #[test]
// fn query_does_not_match() {
//     let mut world = World::new();

//     #[derive(Component)]
//     struct Present1;
//     #[derive(Component)]
//     struct Present2;
//     #[derive(Component, Debug, PartialEq)]
//     struct NotPresent;

//     let entity = world.spawn((Present1, Present2));

//     let (entity, archetype_id) = (entity.id(), entity.archetype().id());

//     let result = world.query::<&NotPresent>().get(&world, entity);

//     assert_eq!(
//         result,
//         Err(QueryEntityError::QueryDoesNotMatch(entity, archetype_id))
//     );
// }
