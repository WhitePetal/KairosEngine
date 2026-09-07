use kairos_ecs_macros::EntityEvent;

use crate::ecs::{entity::Entity, observer::{ObservedBy, Observer, On}, world::World};


// TODO!

#[test]
fn despawning_observer_removes_observed_by() {
    let mut world = World::new();

    #[derive(EntityEvent)]
    struct Hi {
        entity: Entity,
    }

    let target = world.spawn_empty().id();
    let observer = world
        .spawn(Observer::new(|_: On<Hi>| {}).with_entity(target))
        .id();

    assert_eq!(
        world.entity(target).get::<ObservedBy>().unwrap().get(),
        &[observer]
    );

    world.entity_mut(observer).despawn();

    assert!(!world.entity(target).contains::<ObservedBy>());
}

#[test]
fn despawning_observer_removes_observed_by_repeated() {
    let mut world = World::new();

    #[derive(EntityEvent)]
    struct Hi {
        entity: Entity,
    }

    let target = world.spawn_empty().id();
    let observer = world
        // Observe the same entity multiple times.
        .spawn(Observer::new(|_: On<Hi>| {}).with_entities([target, target]))
        .id();

    // Having an observer observe the same entity multiple times is likely not desired, but
    // preventing this is not worth it, so make sure it behaves correctly.
    assert_eq!(
        world.entity(target).get::<ObservedBy>().unwrap().get(),
        &[observer, observer]
    );

    world.entity_mut(observer).despawn();

    assert!(!world.entity(target).contains::<ObservedBy>());
}
