use kairos_ecs_macros::{FromTemplate, Resource};

use crate::error::Result;
use crate::world::World;

#[test]
fn option_template() {
    #[derive(FromTemplate)]
    struct Handle(String);

    #[derive(FromTemplate)]
    struct Foo {
        #[template(built_in)]
        handle: Option<Handle>,
    }

    let mut world = World::new();
    let foo_template = FooTemplate {
        handle: Some(HandleTemplate("handle_path".to_string())).into(),
    };
    let foo = world.spawn_empty().build_template(&foo_template).unwrap();
    assert_eq!(foo.handle.unwrap().0, "handle_path".to_string());
}

/// A resource a template build can read, mutate, and locate.
#[derive(Resource)]
struct Counter(u32);

#[test]
fn template_context_reaches_resources() {
    let mut world = World::new();
    world.insert_resource(Counter(1));
    let entity = world.spawn_empty().id();

    let resource_entity = world.entity_mut(entity).resource_entity::<Counter>();
    assert!(
        resource_entity.is_some(),
        "an inserted resource must have an associated entity"
    );

    world
        .entity_mut(entity)
        .template_context(|context| {
            assert_eq!(context.resource::<Counter>().0, 1);
            assert_eq!(
                context.resource_entity::<Counter>(),
                resource_entity,
                "the context must resolve the same resource entity as the world"
            );
            context.resource_mut::<Counter>().0 = 2;
            Result::Ok(())
        })
        .unwrap();

    assert_eq!(world.resource::<Counter>().0, 2);
}
