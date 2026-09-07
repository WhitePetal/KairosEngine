use std::{
    any::TypeId,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
};

use crate::{
    collections::{FixedHashMap, FixedHashSet},
    debug::{DebugName, MaybeLocation},
    ecs::{
        change_detection::{DetectChanges, DetectChangesMut, Mut, Res},
        component::{
            Component, ComponentCloneBehavior, ComponentDescriptor, ComponentInfo, StorageType,
        },
        entity::EntityHashSet,
        entity_disabling::{DefaultQueryFilters, Disabled},
        event::Event,
        observer::On,
        resource::Resource,
        world::{DeferredWorld, FromWorld, World, error::EntityMutableFetchError},
    },
    ptr::OwningPtr,
};

type ID = u8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DropLogItem {
    Create(ID),
    Drop(ID),
}

#[derive(Component)]
struct MayPanicInDrop {
    drop_log: Arc<Mutex<Vec<DropLogItem>>>,
    expected_panic_flag: Arc<AtomicBool>,
    should_panic: bool,
    id: u8,
}

impl MayPanicInDrop {
    fn new(
        drop_log: &Arc<Mutex<Vec<DropLogItem>>>,
        expected_panic_flag: &Arc<AtomicBool>,
        should_panic: bool,
        id: u8,
    ) -> Self {
        println!("creating component with id {id}");
        drop_log.lock().unwrap().push(DropLogItem::Create(id));

        Self {
            drop_log: Arc::clone(drop_log),
            expected_panic_flag: Arc::clone(expected_panic_flag),
            should_panic,
            id,
        }
    }
}

impl Drop for MayPanicInDrop {
    fn drop(&mut self) {
        println!("dropping component with id {}", self.id);

        {
            let mut drop_log = self.drop_log.lock().unwrap();
            drop_log.push(DropLogItem::Drop(self.id));
            // Don't keep the mutex while panicking, or we'll poison it.
            drop(drop_log);
        }

        if self.should_panic {
            self.expected_panic_flag.store(true, Ordering::SeqCst);
            panic!("testing what happens on panic inside drop");
        }
    }
}

struct DropTestHelper {
    drop_log: Arc<Mutex<Vec<DropLogItem>>>,
    /// Set to `true` right before we intentionally panic, so that if we get
    /// a panic, we know if it was intended or not.
    expected_panic_flag: Arc<AtomicBool>,
}

impl DropTestHelper {
    pub fn new() -> Self {
        Self {
            drop_log: Arc::new(Mutex::new(Vec::<DropLogItem>::new())),
            expected_panic_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn make_component(&self, should_panic: bool, id: ID) -> MayPanicInDrop {
        MayPanicInDrop::new(&self.drop_log, &self.expected_panic_flag, should_panic, id)
    }

    pub fn finish(self, panic_res: std::thread::Result<()>) -> Vec<DropLogItem> {
        let drop_log = self.drop_log.lock().unwrap();
        let expected_panic_flag = self.expected_panic_flag.load(Ordering::SeqCst);

        if !expected_panic_flag {
            match panic_res {
                Ok(()) => panic!("Expected a panic but it didn't happen"),
                Err(e) => std::panic::resume_unwind(e),
            }
        }

        drop_log.to_owned()
    }
}

#[test]
fn panic_while_overwriting_component() {
    let helper = DropTestHelper::new();

    let res = std::panic::catch_unwind(|| {
        let mut world = World::new();
        world
            .spawn_empty()
            .insert(helper.make_component(true, 0))
            .insert(helper.make_component(false, 1));

        println!("Done inserting! Dropping world...");
    });

    let drop_log = helper.finish(res);

    assert_eq!(
        &*drop_log,
        [
            DropLogItem::Create(0),
            DropLogItem::Create(1),
            DropLogItem::Drop(0),
            DropLogItem::Drop(1),
        ]
    );
}

#[derive(Resource)]
struct TestResource(u32);

#[derive(Resource)]
struct TestResource2(String);

#[derive(Resource)]
struct TestResource3;

#[test]
fn get_resource_by_id() {
    let mut world = World::new();
    world.insert_resource(TestResource(42));
    let component_id = world
        .components()
        .get_valid_id(TypeId::of::<TestResource>())
        .unwrap();

    let resource = world.get_resource_by_id(component_id).unwrap();
    // SAFETY: `TestResource` is the correct resource type
    let resource = unsafe { resource.deref::<TestResource>() };

    assert_eq!(resource.0, 42);
}

#[test]
fn get_resource_mut_by_id() {
    let mut world = World::new();
    world.insert_resource(TestResource(42));
    let component_id = world
        .components()
        .get_valid_id(TypeId::of::<TestResource>())
        .unwrap();

    {
        let mut resource = world.get_resource_mut_by_id(component_id).unwrap();
        resource.set_changed();
        // SAFETY: `TestResource` is the correct resource type
        let resource = unsafe { resource.into_inner().deref_mut::<TestResource>() };
        resource.0 = 43;
    }

    let resource = world.get_resource_by_id(component_id).unwrap();
    // SAFETY: `TestResource` is the correct resource type
    let resource = unsafe { resource.deref::<TestResource>() };

    assert_eq!(resource.0, 43);
}

#[test]
fn iter_resources() {
    let mut world = World::new();
    // Remove DefaultQueryFilters so it doesn't show up in the iterator
    world.remove_resource::<DefaultQueryFilters>();
    world.insert_resource(TestResource(42));
    world.insert_resource(TestResource2("Hello, world!".to_string()));
    world.insert_resource(TestResource3);
    world.remove_resource::<TestResource3>();

    let mut iter = world.iter_resources();

    let (info, ptr) = iter.next().unwrap();
    assert_eq!(info.name(), DebugName::type_name::<TestResource>());
    // SAFETY: We know that the resource is of type `TestResource`
    assert_eq!(unsafe { ptr.deref::<TestResource>().0 }, 42);

    let (info, ptr) = iter.next().unwrap();
    assert_eq!(info.name(), DebugName::type_name::<TestResource2>());
    assert_eq!(
        // SAFETY: We know that the resource is of type `TestResource2`
        unsafe { &ptr.deref::<TestResource2>().0 },
        &"Hello, world!".to_string()
    );

    assert!(iter.next().is_none());
}

#[test]
fn iter_resources_mut() {
    let mut world = World::new();
    // Remove DefaultQueryFilters so it doesn't show up in the iterator
    world.remove_resource::<DefaultQueryFilters>();
    world.insert_resource(TestResource(42));
    world.insert_resource(TestResource2("Hello, world!".to_string()));
    world.insert_resource(TestResource3);
    world.remove_resource::<TestResource3>();

    let mut iter = world.iter_resources_mut();

    let (info, mut mut_untyped) = iter.next().unwrap();
    assert_eq!(info.name(), DebugName::type_name::<TestResource>());
    // SAFETY: We know that the resource is of type `TestResource`
    unsafe {
        mut_untyped.as_mut().deref_mut::<TestResource>().0 = 43;
    };

    let (info, mut mut_untyped) = iter.next().unwrap();
    assert_eq!(info.name(), DebugName::type_name::<TestResource2>());
    // SAFETY: We know that the resource is of type `TestResource2`
    unsafe {
        mut_untyped.as_mut().deref_mut::<TestResource2>().0 = "Hello, world?".to_string();
    };

    assert!(iter.next().is_none());
    drop(iter);

    assert_eq!(world.resource::<TestResource>().0, 43);
    assert_eq!(
        world.resource::<TestResource2>().0,
        "Hello, world?".to_string()
    );
}

#[test]
fn custom_non_send_with_layout() {
    static DROP_COUNT: AtomicU32 = AtomicU32::new(0);

    let mut world = World::new();

    // SAFETY: the drop function is valid for the layout and the data will be safe to access from any thread
    let descriptor = unsafe {
        ComponentDescriptor::new_with_layout(
            "Custom Test Component".to_string(),
            StorageType::Table,
            core::alloc::Layout::new::<[u8; 8]>(),
            Some(|ptr| {
                let data = ptr.read::<[u8; 8]>();
                assert_eq!(data, [0, 1, 2, 3, 4, 5, 6, 7]);
                DROP_COUNT.fetch_add(1, Ordering::SeqCst);
            }),
            true,
            ComponentCloneBehavior::Default,
            None,
        )
    };

    let component_id = world.register_component_with_descriptor(descriptor);

    let value: [u8; 8] = [0, 1, 2, 3, 4, 5, 6, 7];
    OwningPtr::make(value, |ptr| {
        // SAFETY: value is valid for the component layout
        unsafe {
            world.insert_non_send_by_id(component_id, ptr, MaybeLocation::caller());
        }
    });

    // SAFETY: [u8; 8] is the correct type for the resource
    let data = unsafe {
        world
            .get_non_send_by_id(component_id)
            .unwrap()
            .deref::<[u8; 8]>()
    };
    assert_eq!(*data, [0, 1, 2, 3, 4, 5, 6, 7]);

    assert!(world.remove_non_send_by_id(component_id).is_some());

    assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 1);
}

#[derive(Resource)]
struct TestFromWorld(u32);
impl FromWorld for TestFromWorld {
    fn from_world(world: &mut World) -> Self {
        let b = world.resource::<TestResource>();
        Self(b.0)
    }
}

#[test]
fn init_resource_does_not_overwrite() {
    let mut world = World::new();
    world.insert_resource(TestResource(0));
    world.init_resource::<TestFromWorld>();
    world.insert_resource(TestResource(1));
    world.init_resource::<TestFromWorld>();

    let resource = world.resource::<TestFromWorld>();

    assert_eq!(resource.0, 0);
}

#[test]
fn init_non_send_does_not_overwrite() {
    let mut world = World::new();
    world.insert_resource(TestResource(0));
    world.init_non_send::<TestFromWorld>();
    world.insert_resource(TestResource(1));
    world.init_non_send::<TestFromWorld>();

    let resource = world.non_send::<TestFromWorld>();

    assert_eq!(resource.0, 0);
}

#[derive(Component)]
struct Foo;

#[derive(Component)]
struct Bar;

#[derive(Component)]
struct Baz;

#[test]
fn inspect_entity_components() {
    let mut world = World::new();
    let ent0 = world.spawn((Foo, Bar, Baz)).id();
    let ent1 = world.spawn((Foo, Bar)).id();
    let ent2 = world.spawn((Bar, Baz)).id();
    let ent3 = world.spawn((Foo, Baz)).id();
    let ent4 = world.spawn(Foo).id();
    let ent5 = world.spawn(Bar).id();
    let ent6 = world.spawn(Baz).id();

    fn to_type_ids(component_infos: Vec<&ComponentInfo>) -> FixedHashSet<Option<TypeId>> {
        component_infos
            .into_iter()
            .map(ComponentInfo::type_id)
            .collect()
    }

    let foo_id = TypeId::of::<Foo>();
    let bar_id = TypeId::of::<Bar>();
    let baz_id = TypeId::of::<Baz>();
    assert_eq!(
        to_type_ids(world.inspect_entity(ent0).unwrap().collect()),
        [Some(foo_id), Some(bar_id), Some(baz_id)]
            .into_iter()
            .collect::<FixedHashSet<_>>()
    );
    assert_eq!(
        to_type_ids(world.inspect_entity(ent1).unwrap().collect()),
        [Some(foo_id), Some(bar_id)]
            .into_iter()
            .collect::<FixedHashSet<_>>()
    );
    assert_eq!(
        to_type_ids(world.inspect_entity(ent2).unwrap().collect()),
        [Some(bar_id), Some(baz_id)]
            .into_iter()
            .collect::<FixedHashSet<_>>()
    );
    assert_eq!(
        to_type_ids(world.inspect_entity(ent3).unwrap().collect()),
        [Some(foo_id), Some(baz_id)]
            .into_iter()
            .collect::<FixedHashSet<_>>()
    );
    assert_eq!(
        to_type_ids(world.inspect_entity(ent4).unwrap().collect()),
        [Some(foo_id)].into_iter().collect::<FixedHashSet<_>>()
    );
    assert_eq!(
        to_type_ids(world.inspect_entity(ent5).unwrap().collect()),
        [Some(bar_id)].into_iter().collect::<FixedHashSet<_>>()
    );
    assert_eq!(
        to_type_ids(world.inspect_entity(ent6).unwrap().collect()),
        [Some(baz_id)].into_iter().collect::<FixedHashSet<_>>()
    );
}

#[test]
fn iterate_entities() {
    let mut world = World::new();
    let mut entity_counters = <FixedHashMap<_, _>>::default();

    let iterate_and_count_entities = |world: &World, entity_counters: &mut FixedHashMap<_, _>| {
        entity_counters.clear();
        for entity in world.iter_entities() {
            let counter = entity_counters.entry(entity.id()).or_insert(0);
            *counter += 1;
        }
    };

    // Adding one entity and validating iteration
    let ent0 = world.spawn((Foo, Bar, Baz)).id();

    iterate_and_count_entities(&world, &mut entity_counters);
    assert_eq!(entity_counters[&ent0], 1);
    assert_eq!(entity_counters.len(), 2);

    // Spawning three more entities and then validating iteration
    let ent1 = world.spawn((Foo, Bar)).id();
    let ent2 = world.spawn((Bar, Baz)).id();
    let ent3 = world.spawn((Foo, Baz)).id();

    iterate_and_count_entities(&world, &mut entity_counters);

    assert_eq!(entity_counters[&ent0], 1);
    assert_eq!(entity_counters[&ent1], 1);
    assert_eq!(entity_counters[&ent2], 1);
    assert_eq!(entity_counters[&ent3], 1);
    assert_eq!(entity_counters.len(), 5);

    // Despawning first entity and then validating the iteration
    assert!(world.despawn(ent0));

    iterate_and_count_entities(&world, &mut entity_counters);

    assert_eq!(entity_counters[&ent1], 1);
    assert_eq!(entity_counters[&ent2], 1);
    assert_eq!(entity_counters[&ent3], 1);
    assert_eq!(entity_counters.len(), 4);

    // Spawning three more entities, despawning three and then validating the iteration
    let ent4 = world.spawn(Foo).id();
    let ent5 = world.spawn(Bar).id();
    let ent6 = world.spawn(Baz).id();

    assert!(world.despawn(ent2));
    assert!(world.despawn(ent3));
    assert!(world.despawn(ent4));

    iterate_and_count_entities(&world, &mut entity_counters);

    assert_eq!(entity_counters[&ent1], 1);
    assert_eq!(entity_counters[&ent5], 1);
    assert_eq!(entity_counters[&ent6], 1);
    assert_eq!(entity_counters.len(), 4);

    // Despawning remaining entities and then validating the iteration
    assert!(world.despawn(ent1));
    assert!(world.despawn(ent5));
    assert!(world.despawn(ent6));

    iterate_and_count_entities(&world, &mut entity_counters);

    assert_eq!(entity_counters.len(), 1);
}

#[test]
fn spawn_empty_bundle() {
    let mut world = World::new();
    world.spawn(());
}

#[test]
fn get_entity() {
    let mut world = World::new();

    let e1 = world.spawn_empty().id();
    let e2 = world.spawn_empty().id();

    assert!(world.get_entity(e1).is_ok());
    assert!(world.get_entity([e1, e2]).is_ok());
    assert!(
        world
            .get_entity(&[e1, e2] /* this is an array not a slice */)
            .is_ok()
    );
    assert!(world.get_entity(&vec![e1, e2][..]).is_ok());
    assert!(
        world
            .get_entity(&EntityHashSet::from_iter([e1, e2]))
            .is_ok()
    );

    world.entity_mut(e1).despawn();

    assert_eq!(
        Err(e1),
        world.get_entity(e1).map(|_| {}).map_err(|e| e.entity())
    );
    assert_eq!(
        Err(e1),
        world
            .get_entity([e1, e2])
            .map(|_| {})
            .map_err(|e| e.entity())
    );
    assert_eq!(
        Err(e1),
        world
            .get_entity(&[e1, e2] /* this is an array not a slice */)
            .map(|_| {})
            .map_err(|e| e.entity())
    );
    assert_eq!(
        Err(e1),
        world
            .get_entity(&vec![e1, e2][..])
            .map(|_| {})
            .map_err(|e| e.entity())
    );
    assert_eq!(
        Err(e1),
        world
            .get_entity(&EntityHashSet::from_iter([e1, e2]))
            .map(|_| {})
            .map_err(|e| e.entity())
    );
}

#[test]
fn get_entity_mut() {
    let mut world = World::new();

    let e1 = world.spawn_empty().id();
    let e2 = world.spawn_empty().id();

    assert!(world.get_entity_mut(e1).is_ok());
    assert!(world.get_entity_mut([e1, e2]).is_ok());
    assert!(
        world
            .get_entity_mut(&[e1, e2] /* this is an array not a slice */)
            .is_ok()
    );
    assert!(world.get_entity_mut(&vec![e1, e2][..]).is_ok());
    assert!(
        world
            .get_entity_mut(&EntityHashSet::from_iter([e1, e2]))
            .is_ok()
    );

    assert_eq!(
        Err(EntityMutableFetchError::AliasedMutability(e1)),
        world.get_entity_mut([e1, e2, e1]).map(|_| {})
    );
    assert_eq!(
        Err(EntityMutableFetchError::AliasedMutability(e1)),
        world
            .get_entity_mut(&[e1, e2, e1] /* this is an array not a slice */)
            .map(|_| {})
    );
    assert_eq!(
        Err(EntityMutableFetchError::AliasedMutability(e1)),
        world.get_entity_mut(&vec![e1, e2, e1][..]).map(|_| {})
    );
    // Aliased mutability isn't allowed by HashSets
    assert!(
        world
            .get_entity_mut(&EntityHashSet::from_iter([e1, e2, e1]))
            .is_ok()
    );

    world.entity_mut(e1).despawn();
    assert!(world.get_entity_mut(e2).is_ok());

    assert!(matches!(
        world.get_entity_mut(e1).map(|_| {}),
        Err(EntityMutableFetchError::NotSpawned(e)) if e.entity() == e1
    ));
    assert!(matches!(
        world.get_entity_mut([e1, e2]).map(|_| {}),
        Err(EntityMutableFetchError::NotSpawned(e)) if e.entity() == e1));
    assert!(matches!(
        world
            .get_entity_mut(&[e1, e2] /* this is an array not a slice */)
            .map(|_| {}),
        Err(EntityMutableFetchError::NotSpawned(e)) if e.entity() == e1));
    assert!(matches!(
        world.get_entity_mut(&vec![e1, e2][..]).map(|_| {}),
        Err(EntityMutableFetchError::NotSpawned(e)) if e.entity() == e1,
    ));
    assert!(matches!(
        world
            .get_entity_mut(&EntityHashSet::from_iter([e1, e2]))
            .map(|_| {}),
        Err(EntityMutableFetchError::NotSpawned(e)) if e.entity() == e1));
}

#[test]
#[track_caller]
fn entity_spawn_despawn_tracking() {
    use core::panic::Location;

    let mut world = World::new();
    let entity = world.spawn_empty().id();
    assert_eq!(
        world.entities.entity_get_spawned_or_despawned_by(entity),
        MaybeLocation::new(Some(Location::caller()))
    );
    assert_eq!(
        world.entities.entity_get_spawn_or_despawn_tick(entity),
        Some(world.change_tick())
    );
    let new = world.despawn_no_free(entity).unwrap();
    assert_eq!(
        world.entities.entity_get_spawned_or_despawned_by(entity),
        MaybeLocation::new(Some(Location::caller()))
    );
    assert_eq!(
        world.entities.entity_get_spawn_or_despawn_tick(entity),
        Some(world.change_tick())
    );

    world.spawn_empty_at(new).unwrap();
    assert_eq!(entity.index(), new.index());
    assert_eq!(
        world.entities.entity_get_spawned_or_despawned_by(entity),
        MaybeLocation::new(None)
    );
    assert_eq!(
        world.entities.entity_get_spawn_or_despawn_tick(entity),
        None
    );
    world.despawn(new);
    assert_eq!(
        world.entities.entity_get_spawned_or_despawned_by(entity),
        MaybeLocation::new(None)
    );
    assert_eq!(
        world.entities.entity_get_spawn_or_despawn_tick(entity),
        None
    );
}

#[test]
fn new_world_has_disabling() {
    let mut world = World::new();
    world.spawn(Foo);
    world.spawn((Foo, Disabled));
    assert_eq!(1, world.query::<&Foo>().iter(&world).count());

    // If we explicitly remove the resource, no entities should be filtered anymore
    world.remove_resource::<DefaultQueryFilters>();
    assert_eq!(2, world.query::<&Foo>().iter(&world).count());
}

#[test]
fn entities_and_commands() {
    #[derive(Component, PartialEq, Debug)]
    struct Foo(u32);

    let mut world = World::new();

    let eid = world.spawn(Foo(35)).id();

    let (mut fetcher, mut commands) = world.entities_and_commands();
    let emut = fetcher.get_mut(eid).unwrap();
    commands.entity(eid).despawn();
    assert_eq!(emut.get::<Foo>().unwrap(), &Foo(35));

    world.flush();

    assert!(world.get_entity(eid).is_err());
}

#[test]
fn resource_query_after_resource_scope() {
    #[derive(Event)]
    struct EventA;

    #[derive(Resource)]
    struct ResourceA;

    let mut world = World::default();

    world.insert_resource(ResourceA);
    world.add_observer(move |_event: On<EventA>, _res: Res<ResourceA>| {});
    world.resource_scope(|world, _res: Mut<ResourceA>| {
        // since we use commands, this should trigger outside of the resource_scope, so the observer should work.
        world.commands().trigger(EventA);
    });
}

#[test]
fn entities_and_commands_deferred() {
    #[derive(Component, PartialEq, Debug)]
    struct Foo(u32);

    let mut world = World::new();

    let eid = world.spawn(Foo(1)).id();

    let mut dworld = DeferredWorld::from(&mut world);

    let (mut fetcher, mut commands) = dworld.entities_and_commands();
    let emut = fetcher.get_mut(eid).unwrap();
    commands.entity(eid).despawn();
    assert_eq!(emut.get::<Foo>().unwrap(), &Foo(1));

    world.flush();

    assert!(world.get_entity(eid).is_err());
}

#[test]
fn resource_scope_ticks() {
    #[derive(Resource)]
    struct R;

    let mut world = World::new();
    world.insert_resource(R);
    world.resource_scope(|world, r: Mut<R>| {
        assert_eq!(world.change_tick(), r.added());
        assert_eq!(world.change_tick(), r.last_changed());
        world.increment_change_tick();
    });
    assert_eq!(world.change_tick(), world.resource_ref::<R>().added());
    assert_eq!(
        world.change_tick(),
        world.resource_ref::<R>().last_changed()
    );
}
