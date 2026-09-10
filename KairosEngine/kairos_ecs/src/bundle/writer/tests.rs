use crate::{bundle::BundleScratch, component::Component, name::Name, world::World};

#[test]
fn write_component() {
    #[derive(Component)]
    struct X;

    let mut world = World::new();
    let mut bundle_scratch = BundleScratch::default();
    let mut bundle_writer = bundle_scratch.writer();
    // SAFETY: the same world is used for every bundle_writer operation
    unsafe {
        let mut components = world.components_registrator();
        bundle_writer.push_component(&mut components, X);
        bundle_writer.push_component(&mut components, Name::new("Hi"));
        let mut entity = world.spawn_empty();
        bundle_writer.write(&mut entity);

        assert_eq!(entity.get::<Name>().unwrap().as_str(), "Hi");
        assert!(entity.contains::<X>());
    }
}
