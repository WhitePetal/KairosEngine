use crate::ecs::{entity::Entity, world::World};

use super::UniqueEntityIter;

#[test]
fn todo() {
    todo!()
}

// #[derive(Component, Clone)]
// pub struct Thing;

// #[expect(
//     clippy::iter_skip_zero,
//     reason = "The `skip(0)` is used to ensure that the `Skip` iterator implements `EntitySet`, which is needed to pass the iterator as the `entities` parameter."
// )]
// #[test]
// fn preserving_uniqueness() {
//     let mut world = World::new();

//     let mut query = QueryState::<&mut Thing>::new(&mut world);

//     let spawn_batch: Vec<Entity> = world.spawn_batch(vec![Thing; 1000]).collect();

//     // SAFETY: SpawnBatchIter is `EntitySetIterator`,
//     let mut unique_entity_iter =
//         unsafe { UniqueEntityIter::from_iter_unchecked(spawn_batch.iter()) };

//     let entity_set = unique_entity_iter
//         .by_ref()
//         .filter(|_| true)
//         .fuse()
//         .inspect(|_| ())
//         .rev()
//         .skip(0)
//         .skip_while(|_| false)
//         .take(1000)
//         .take_while(|_| true)
//         .step_by(2)
//         .cloned();

//     // With `iter_many_mut` collecting is not possible, because you need to drop each `Mut`/`&mut` before the next is retrieved.
//     let _results: Vec<Mut<Thing>> = query.iter_many_unique_mut(&mut world, entity_set).collect();
// }

// #[test]
// fn nesting_queries() {
//     let mut world = World::new();

//     world.spawn_batch(vec![Thing; 1000]);

//     pub fn system(mut thing_entities: Query<Entity, With<Thing>>, mut things: Query<&mut Thing>) {
//         things.iter_many_unique(thing_entities.iter());
//         things.iter_many_unique_mut(thing_entities.iter_mut());
//     }

//     let mut schedule = Schedule::default();
//     schedule.add_systems(system);
//     schedule.run(&mut world);
// }
