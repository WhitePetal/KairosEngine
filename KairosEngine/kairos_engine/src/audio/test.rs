use std::sync::Arc;

use kairos_ecs::{component::Component, entity::Entity, world::World};
use smallvec::SmallVec;

use crate::{
    asset_loader::assets::{AssetHandle, AudioAssetHandle, AudioAssetsSystem},
    audio::{
        background::BackgroundAudio,
        spatial::{
            spatial_audio_listener::SpatialAudioListenerComponent,
            spatial_audio_reverb::{SpatialAudioReverb, SpatialAudioReverbBound},
            spatial_audio_volume::SpatialAudioVolume,
        },
    },
    math::float3,
    spatial::AABB,
};

/// Compile-time proof that every audio entity type is a `Component` (and thus
/// `Send + Sync + 'static`). This stops compiling if any `#[derive(Component)]`
/// on an audio type is dropped.
#[test]
fn audio_types_implement_component() {
    fn assert_component<T: Component>() {}

    assert_component::<SpatialAudioListenerComponent>();
    assert_component::<SpatialAudioReverb>();
    assert_component::<SpatialAudioReverbBound>();
    assert_component::<SpatialAudioVolume>();
    assert_component::<BackgroundAudio>();
}

#[test]
fn audio_components_can_be_spawned_into_a_world() {
    let mut world = World::new();

    let (reverb, bound) = test_reverb();
    let reverb_entity = world.spawn((reverb, bound)).id();
    assert_has::<SpatialAudioReverb>(&world, reverb_entity);
    assert_has::<SpatialAudioReverbBound>(&world, reverb_entity);

    let handle = test_audio_asset_handle();
    let background_entity = world
        .spawn(BackgroundAudio::new(handle.clone(), true))
        .id();
    assert_has::<BackgroundAudio>(&world, background_entity);

    let volume = SpatialAudioVolume::new(SmallVec::from_vec(vec![handle]), true, 0.0);
    let volume_entity = world.spawn(volume).id();
    assert_has::<SpatialAudioVolume>(&world, volume_entity);
}

/// The spatial audio system queries the reverb and its bound together, so both
/// must be visible to the same ECS query.
#[test]
fn reverb_and_bound_are_queryable_together() {
    let mut world = World::new();

    let (reverb, bound) = test_reverb();
    world.spawn((reverb, bound));

    let mut query = world.query::<(&SpatialAudioReverbBound, &SpatialAudioReverb)>();
    assert_eq!(query.iter(&world).count(), 1);
}

fn assert_has<T: Component>(world: &World, entity: Entity) {
    assert!(world.entity(entity).get::<T>().is_some());
}

fn test_reverb() -> (SpatialAudioReverb, SpatialAudioReverbBound) {
    SpatialAudioReverb::new(
        10.0,
        0.0,
        1.0,
        0.5,
        0.5,
        0.3,
        AABB {
            min: float3::new(-1.0, -1.0, -1.0),
            max: float3::new(1.0, 1.0, 1.0),
        },
    )
}

/// Builds an unloaded [`AudioAssetHandle`] for structural tests: the handle is
/// just an index plus a drop channel, so a standalone channel makes it
/// constructible and droppable without a real `AssetsServer`. It can be moved
/// into a component, but never resolves an asset.
fn test_audio_asset_handle() -> AudioAssetHandle {
    use crate::asset_loader::assets::asset::{AssetIndex, AssetsSystem};

    let (drop_sender, _drop_receiver) =
        tokio::sync::mpsc::channel::<<AudioAssetsSystem as AssetsSystem>::DropEvent>(1);
    Arc::new(AssetHandle::new(AssetIndex::new(0), drop_sender))
}
