//! P1 wiring test: the next-generation asset core mounted on the engine's own
//! `PreUpdate`/`PostUpdate` stages (#206).
//!
//! It drives the real [`build_world`] bootstrap `Engine::new` uses — only the
//! audio device is skipped — then registers one stand-in asset type by hand, on
//! top of the types (`Font`) that bootstrap now registers itself (#207).

use super::{PostUpdate, PreUpdate};
use crate::graphics::{material::Material, material::SerializedMaterial, shader::ShaderAsset, texture::Texture};
use crate::kairos_editor::editor_assets::{Text, Toml};
use crate::kairos_editor::syntax::SyntaxHighlightSettings;
use crate::kairos_ui::font::Font;
use kairos_asset::next::{
    Asset, AssetEventSystems, AssetServer, AssetStages, AssetTrackingSystems, AssetWorldExt,
    Assets, VisitAssetDependencies,
};
use kairos_ecs::schedule::{ScheduleLabel, Schedules, SystemSet};

use crate::kairos_editor::build_world;

/// A loadless asset type, registered only to give the drivers a concrete type
/// to mount for.
struct TestAsset;

impl Asset for TestAsset {}
impl VisitAssetDependencies for TestAsset {}

/// The asset core records the engine's own stages, lands its resources in the
/// World, and mounts both driver `SystemSet`s into `PreUpdate`/`PostUpdate`.
#[test]
fn asset_core_mounts_on_the_engine_stages() {
    // The exact World `Engine::new` builds, minus the audio device.
    let mut world = build_world();
    world.init_asset::<TestAsset>();

    // The caller-supplied stages are the engine's real ones, not ad-hoc labels.
    let stages = world.resource::<AssetStages>();
    assert_eq!(stages.tracking, PreUpdate.intern());
    assert_eq!(stages.event, PostUpdate.intern());

    // The server and the per-type store are World resources.
    assert!(
        world.get_resource::<AssetServer>().is_some(),
        "AssetServer must be a World resource"
    );
    assert!(
        world.get_resource::<Assets<TestAsset>>().is_some(),
        "init_asset must register the store for the type"
    );

    // Run both stages once so their schedules build (`systems_in_set` requires
    // an initialized schedule), then check the two sets actually landed there.
    world.run_schedule(PreUpdate);
    world.run_schedule(PostUpdate);

    let schedules = world.resource::<Schedules>();
    let tracking = schedules
        .get(PreUpdate)
        .expect("PreUpdate schedule must exist after install")
        .graph()
        .systems_in_set(AssetTrackingSystems.intern())
        .expect("AssetTrackingSystems must be a set in PreUpdate");
    assert!(
        !tracking.is_empty(),
        "PreUpdate must host the asset tracking drivers"
    );

    let events = schedules
        .get(PostUpdate)
        .expect("PostUpdate schedule must exist after install")
        .graph()
        .systems_in_set(AssetEventSystems.intern())
        .expect("AssetEventSystems must be a set in PostUpdate");
    assert!(
        !events.is_empty(),
        "PostUpdate must host the asset event drivers"
    );
}

/// The first migrated asset type, `Font`, registers its store during the engine
/// bootstrap — P2's per-asset `init_asset` calls (#207).
#[test]
fn build_world_registers_the_font_asset() {
    let world = build_world();
    assert!(
        world.get_resource::<Assets<Font>>().is_some(),
        "build_world must register the Font store"
    );
}

/// The S2 leaf types — `Text`, `Toml`, and the `SyntaxHighlightSettings` their
/// editors consume — register their stores during the engine bootstrap too.
#[test]
fn build_world_registers_the_s2_leaf_assets() {
    let world = build_world();
    assert!(
        world.get_resource::<Assets<Text>>().is_some(),
        "build_world must register the Text store"
    );
    assert!(
        world.get_resource::<Assets<Toml>>().is_some(),
        "build_world must register the Toml store"
    );
    assert!(
        world
            .get_resource::<Assets<SyntaxHighlightSettings>>()
            .is_some(),
        "build_world must register the SyntaxHighlightSettings store"
    );
}

/// The S3 graphics cluster — `ShaderAsset`, `Texture`, `Material`, and the
/// editable `SerializedMaterial` — registers its stores during the engine
/// bootstrap too.
#[test]
fn build_world_registers_the_s3_graphics_assets() {
    let world = build_world();
    assert!(
        world.get_resource::<Assets<ShaderAsset>>().is_some(),
        "build_world must register the ShaderAsset store"
    );
    assert!(
        world.get_resource::<Assets<Texture>>().is_some(),
        "build_world must register the Texture store"
    );
    assert!(
        world.get_resource::<Assets<Material>>().is_some(),
        "build_world must register the Material store"
    );
    assert!(
        world.get_resource::<Assets<SerializedMaterial>>().is_some(),
        "build_world must register the SerializedMaterial store"
    );
}
