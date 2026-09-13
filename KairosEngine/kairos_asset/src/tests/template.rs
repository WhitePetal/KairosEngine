//! Tests for the template integration: `HandleTemplate<A>` and
//! `impl FromTemplate for Handle<A>`.

use kairos_ecs::world::World;
use kairos_ecs_macros::FromTemplate;

use crate::{Asset, Assets, Handle, HandleTemplate, VisitAssetDependencies, asset_value};

/// An asset whose value is a number, so a template build can be observed.
#[derive(Debug, PartialEq, Eq)]
struct TestAsset(u32);

impl Asset for TestAsset {}
impl VisitAssetDependencies for TestAsset {}

/// A component with a `Handle<A>` field. The derive must resolve that field
/// through [`FromTemplate`], picking `HandleTemplate<A>` as its template type.
#[derive(FromTemplate)]
struct Holder {
    handle: Handle<TestAsset>,
}

#[test]
fn derive_resolves_handle_fields_through_handle_template() {
    let template = HolderTemplate {
        handle: HandleTemplate::value(TestAsset(7)),
    };

    let mut world = World::new();
    world.insert_resource(Assets::<TestAsset>::default());

    let holder = world.spawn_empty().build_template(&template).unwrap();

    assert!(holder.handle.is_strong());
    assert_eq!(
        world
            .resource::<Assets<TestAsset>>()
            .get(&holder.handle)
            .map(|asset| asset.0),
        Some(7)
    );
}

#[test]
fn handle_template_value_adds_once_and_caches_the_handle() {
    let template = HandleTemplate::<TestAsset>::value(TestAsset(1));

    let mut world = World::new();
    world.insert_resource(Assets::<TestAsset>::default());

    let first = world.spawn_empty().build_template(&template).unwrap();
    let second = world.spawn_empty().build_template(&template).unwrap();

    assert_eq!(first, second, "the cached handle must be reused");
    assert_eq!(world.resource::<Assets<TestAsset>>().len(), 1);
}

#[test]
fn asset_value_is_the_value_constructor() {
    let template = asset_value(TestAsset(3));

    let mut world = World::new();
    world.insert_resource(Assets::<TestAsset>::default());

    let handle = world.spawn_empty().build_template(&template).unwrap();
    assert_eq!(
        world
            .resource::<Assets<TestAsset>>()
            .get(&handle)
            .map(|asset| asset.0),
        Some(3)
    );
}

#[test]
fn handle_template_from_handle_clones_it() {
    let handle = Handle::<TestAsset>::default();
    let template = HandleTemplate::from(handle.clone());

    let mut world = World::new();
    let built = world.spawn_empty().build_template(&template).unwrap();

    assert_eq!(built, handle);
    assert!(built.is_uuid());
}

#[test]
fn handle_template_default_is_the_default_handle() {
    let template = HandleTemplate::<TestAsset>::default();

    let mut world = World::new();
    let built = world.spawn_empty().build_template(&template).unwrap();

    assert_eq!(built, Handle::<TestAsset>::default());
}

#[test]
fn handle_template_from_str_is_the_path_variant() {
    let template: HandleTemplate<TestAsset> = "some/path.asset".into();
    assert!(matches!(template, HandleTemplate::Path(_)));
}
