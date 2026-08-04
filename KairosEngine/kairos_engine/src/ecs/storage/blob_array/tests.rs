use crate::ecs::world::World;

#[derive(Component)]
struct PanicOnDrop;

impl Drop for PanicOnDrop {
    fn drop(&mut self) {
        panic!("PanicOnDrop is being Dropped");
    }
}

#[test]
#[should_panic(expected = "PanicOnDrop is being Dropped")]
fn make_sure_zst_components_get_dropped() {
    let mut world = World::new();

    world.spawn(PanicOnDrop);
}
