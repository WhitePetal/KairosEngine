// use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

// use kairos_ecs_macros::{Component, Resource};

// use crate::{
//     debug::MaybeLocation,
//     ecs::{
//         entity::Entity,
//         lifecycle::HookContext,
//         resource::IsResource,
//         world::{DeferredWorld, World},
//     },
//     ptr::OwningPtr,
// };

// #[test]
// fn unique_resource_entities() {
//     #[derive(Default, Resource)]
//     struct TestResource1;

//     #[derive(Resource)]
//     #[expect(dead_code, reason = "field needed for testing")]
//     struct TestResource2(String);

//     #[derive(Resource)]
//     #[expect(dead_code, reason = "field needed for testing")]
//     struct TestResource3(u8);

//     let mut world = World::new();
//     let start = world.entities().count_spawned();
//     let id1 = world.init_resource::<TestResource1>();
//     assert_eq!(world.entities().count_spawned(), start + 1);
//     world.insert_resource(TestResource2(String::from("Foo")));
//     assert_eq!(world.entities().count_spawned(), start + 2);
//     // like component registration, which just makes it known to the world that a component exists,
//     // registering a resource should not spawn an entity.
//     let id3 = world.register_component::<TestResource3>();
//     assert_eq!(world.entities().count_spawned(), start + 2);
//     OwningPtr::make(20_u8, |ptr| {
//         // SAFETY: id was just initialized and corresponds to a resource.
//         unsafe {
//             world.insert_resource_by_id(id3, ptr, MaybeLocation::caller());
//         }
//     });
//     assert_eq!(world.entities().count_spawned(), start + 3);
//     let e3 = world.resource_entities().get(id3).unwrap();
//     assert!(world.remove_resource_by_id(id3));
//     // the entity is stable: removing the resource should only remove the component from the entity, not despawn the entity
//     assert_eq!(world.entities().count_spawned(), start + 3);
//     OwningPtr::make(20_u8, |ptr| {
//         // SAFETY: id was just initialized and corresponds to a resource.
//         unsafe {
//             world.insert_resource_by_id(id3, ptr, MaybeLocation::caller());
//         }
//     });
//     assert_eq!(e3, world.resource_entities().get(id3).unwrap());
//     // again, the entity is stable: see previous explanation
//     let e1 = world.resource_entities().get(id1).unwrap();
//     world.remove_resource::<TestResource1>();
//     assert_eq!(world.entities().count_spawned(), start + 3);
//     world.init_resource::<TestResource1>();
//     assert_eq!(e1, world.resource_entities().get(id1).unwrap());
//     // make sure that trying to add a resource twice results, doesn't change the entity count
//     world.insert_resource(TestResource2(String::from("Bar")));
//     assert_eq!(world.entities().count_spawned(), start + 3);
// }

// #[test]
// fn is_resource_presence() {
//     #[derive(Default, Resource)]
//     struct TestResource;

//     let mut world = World::new();
//     let id = world.init_resource::<TestResource>();

//     assert!(world.get_resource::<TestResource>().is_some());

//     let mut query = world.query::<(Entity, &TestResource, &IsResource)>();
//     let first_entity = {
//         let resources = query.iter(&world).collect::<Vec<_>>();
//         assert_eq!(resources.len(), 1);
//         let (entity, _test_resource, is_resource) = resources[0];
//         assert_eq!(is_resource.resource_component_id(), id);
//         entity
//     };

//     // Removing IsResource should invalidate the current TestResource entity
//     // This uses commands because IsResource's despawn-on-removal invalidates the EntityWorldMut and panics
//     world.entity_mut(first_entity).remove::<IsResource>();
//     assert!(world.get_resource::<TestResource>().is_none());

//     assert!(
//         !world.entity(first_entity).contains::<TestResource>(),
//         "Removing IsResource should also remove the Resource component it corresponds to"
//     );

//     world.init_resource::<TestResource>();
//     let second_entity = {
//         let resources = query.iter(&world).collect::<Vec<_>>();
//         assert_eq!(resources.len(), 1);
//         let (entity, _test_resource, is_resource) = resources[0];
//         assert_eq!(is_resource.resource_component_id(), id);
//         entity
//     };

//     assert_ne!(
//         first_entity, second_entity,
//         "The first resource entity was invalidated, so the second initialization should be new"
//     );

//     let id = world.spawn(TestResource).id();
//     // This spawned resource conflicts with the canonical resource, so it was cleaned up.
//     assert!(world.entity(id).get::<TestResource>().is_none());
//     assert!(world.entity(id).get::<IsResource>().is_none());
//     assert!(world.entity(second_entity).get::<TestResource>().is_some());
//     assert!(world.entity(second_entity).get::<IsResource>().is_some());
// }

// #[test]
// fn derive_resource_component_features() {
//     static ON_ADD_CALLED: AtomicBool = AtomicBool::new(false);

//     #[derive(Resource)]
//     #[component(immutable, on_add)]
//     struct TestResource;
//     impl TestResource {
//         fn on_add(_: DeferredWorld, _: HookContext) {
//             ON_ADD_CALLED.store(true, Relaxed);
//         }
//     }

//     let mut world = World::new();
//     world.insert_resource(TestResource);

//     assert!(ON_ADD_CALLED.load(Relaxed));
//     assert!(world.get_resource::<TestResource>().is_some());
// }

// #[test]
// fn derive_resource_require_features() {
//     #[derive(Component, Default)]
//     struct RequiredComponent;

//     #[derive(Resource)]
//     #[require(RequiredComponent)]
//     struct TestResource;

//     let mut world = World::new();
//     world.insert_resource(TestResource);

//     assert_eq!(
//         world
//             .query::<(&TestResource, &RequiredComponent)>()
//             .iter(&world)
//             .count(),
//         1
//     );
// }
