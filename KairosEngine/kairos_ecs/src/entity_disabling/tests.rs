use kairos_ecs_macros::Component;

use crate::{
    component::ComponentId,
    entity_disabling::{DefaultQueryFilters, Disabled},
    query::{FilteredAccess, Has, With},
    world::{EntityMut, EntityRef, World},
};

#[test]
fn filters_modify_access() {
    let mut filters = DefaultQueryFilters::empty();
    filters.register_disabling_component(ComponentId::new(1));

    // A component access with an unrelated component
    let mut component_access = FilteredAccess::default();
    component_access.access_mut().add_read(ComponentId::new(2));

    let mut applied_access = component_access.clone();
    filters.modify_access(&mut applied_access);
    assert_eq!(0, applied_access.with_filters().count());
    assert_eq!(
        vec![ComponentId::new(1)],
        applied_access.without_filters().collect::<Vec<_>>()
    );

    // We add a with filter, now we expect to see both filters
    component_access.and_with(ComponentId::new(4));

    let mut applied_access = component_access.clone();
    filters.modify_access(&mut applied_access);
    assert_eq!(
        vec![ComponentId::new(4)],
        applied_access.with_filters().collect::<Vec<_>>()
    );
    assert_eq!(
        vec![ComponentId::new(1)],
        applied_access.without_filters().collect::<Vec<_>>()
    );

    let copy = component_access.clone();
    // We add a rule targeting a default component, that filter should no longer be added
    component_access.and_with(ComponentId::new(1));

    let mut applied_access = component_access.clone();
    filters.modify_access(&mut applied_access);
    assert_eq!(
        vec![ComponentId::new(1), ComponentId::new(4)],
        applied_access.with_filters().collect::<Vec<_>>()
    );
    assert_eq!(0, applied_access.without_filters().count());

    // Archetypal access should also filter rules
    component_access = copy.clone();
    component_access
        .access_mut()
        .add_archetypal(ComponentId::new(1));

    let mut applied_access = component_access.clone();
    filters.modify_access(&mut applied_access);
    assert_eq!(
        vec![ComponentId::new(4)],
        applied_access.with_filters().collect::<Vec<_>>()
    );
    assert_eq!(0, applied_access.without_filters().count());
}

#[derive(Component)]
struct CustomDisabled;

#[derive(Component)]
struct Dummy;

#[test]
fn multiple_disabling_components() {
    let mut world = World::new();
    world.register_disabling_component::<CustomDisabled>();

    // Use powers of two so we can uniquely identify the set of matching archetypes from the count.
    world.spawn(Dummy);
    world.spawn_batch((0..2).map(|_| (Dummy, Disabled)));
    world.spawn_batch((0..4).map(|_| (Dummy, CustomDisabled)));
    world.spawn_batch((0..8).map(|_| (Dummy, Disabled, CustomDisabled)));

    let mut query = world.query::<&Dummy>();
    assert_eq!(1, query.iter(&world).count());

    let mut query = world.query_filtered::<EntityRef, With<Dummy>>();
    assert_eq!(1, query.iter(&world).count());

    let mut query = world.query_filtered::<EntityMut, With<Dummy>>();
    assert_eq!(1, query.iter(&world).count());

    let mut query = world.query_filtered::<&Dummy, With<Disabled>>();
    assert_eq!(2, query.iter(&world).count());

    let mut query = world.query_filtered::<Has<Disabled>, With<Dummy>>();
    assert_eq!(3, query.iter(&world).count());

    let mut query = world.query_filtered::<&Dummy, With<CustomDisabled>>();
    assert_eq!(4, query.iter(&world).count());

    let mut query = world.query_filtered::<Has<CustomDisabled>, With<Dummy>>();
    assert_eq!(5, query.iter(&world).count());

    let mut query = world.query_filtered::<&Dummy, (With<Disabled>, With<CustomDisabled>)>();
    assert_eq!(8, query.iter(&world).count());

    let mut query = world.query_filtered::<(Has<Disabled>, Has<CustomDisabled>), With<Dummy>>();
    assert_eq!(15, query.iter(&world).count());

    // This seems like it ought to count as a mention of `Disabled`, but it does not.
    // We don't consider read access, since that would count `EntityRef` as a mention of *all* components.
    let mut query = world.query_filtered::<Option<&Disabled>, With<Dummy>>();
    assert_eq!(1, query.iter(&world).count());
}
