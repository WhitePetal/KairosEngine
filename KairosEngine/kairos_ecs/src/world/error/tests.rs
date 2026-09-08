use crate::{
    entity::Entity,
    event::EntityEvent,
    observer::On,
    system::{Commands, Query, RunSystemOnce, command::trigger},
    world::World,
};

// Inspired by https://github.com/bevyengine/bevy/issues/19623
#[test]
fn fixing_panicking_entity_commands() {
    #[derive(EntityEvent)]
    struct Kill(Entity);

    #[derive(EntityEvent)]
    struct FollowupEvent(Entity);

    fn despawn(kill: On<Kill>, mut commands: Commands) {
        commands.entity(kill.event_target()).despawn();
    }

    fn followup(kill: On<Kill>, mut commands: Commands) {
        // When using a simple .trigger() here, this panics because the entity has already been despawned.
        // Instead, we need to use `.queue_handled` or `.queue_silenced` to avoid the panic.
        commands.queue_silenced(trigger(FollowupEvent(kill.event_target())));
    }

    let mut world = World::new();
    // This test would pass if the order of these statements were swapped,
    // even with panicking entity commands
    world.add_observer(followup);
    world.add_observer(despawn);

    // Create an entity to test these observers with
    world.spawn_empty();

    // Trigger a kill event on the entity
    fn kill_everything(mut commands: Commands, query: Query<Entity>) {
        for id in query.iter() {
            commands.trigger(Kill(id));
        }
    }
    world.run_system_once(kill_everything).unwrap();
}
