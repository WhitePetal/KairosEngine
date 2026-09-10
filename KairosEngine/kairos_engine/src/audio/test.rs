use std::{path::PathBuf, sync::Arc};

use kairos_ecs::{component::Component, entity::Entity, world::World};
use kira::{
    Frame,
    backend::mock::MockBackend,
    sound::{
        PlaybackState,
        static_sound::{StaticSoundData, StaticSoundSettings},
    },
};
use smallvec::SmallVec;

use crate::{
    asset_loader::assets::{AssetHandle, AssetsServer, AudioAssetHandle, AudioAssetsSystem},
    audio::{
        audio::{AudioAsset, AudioState},
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

use super::AudioEngine;

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
    let background_entity = world.spawn(BackgroundAudio::new(handle.clone(), true)).id();
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

// ---------------------------------------------------------------------------
// BackgroundAudio driver (`AudioEngine::update`)
// ---------------------------------------------------------------------------

/// `AudioEngine::update` must advance the background state machine every frame,
/// so one `update` per frame is enough to leave `Created` for `WaitLoading`.
#[test]
fn background_audio_leaves_created_on_the_first_update() {
    let mut harness = Harness::with_unloaded_asset(true);

    harness.update();

    assert_eq!(harness.state(), AudioState::WaitLoading);
}

/// With `auto_play` false the state machine stays put: `Created` only advances
/// when the flag tells it to.
#[test]
fn background_audio_with_auto_play_false_stays_created() {
    let mut harness = Harness::with_unloaded_asset(false);

    harness.update();
    harness.update();

    assert_eq!(harness.state(), AudioState::Created);
}

/// A world with no background entity must not panic — the query is simply empty.
#[test]
fn update_without_a_background_entity_is_a_no_op() {
    let mut engine = AudioEngine::new_mock();
    let mut assets_server = AssetsServer::new();
    let mut world = World::new();

    engine.update(&mut assets_server, &mut world, 0.0);
}

/// The full auto-play chain: `WaitLoading` polls until the asset resolves, then
/// starts playback and moves to `Playing`.
#[test]
fn background_audio_plays_once_its_asset_is_loaded() {
    let mut harness = Harness::with_loaded_asset(true);

    harness.update();
    assert_eq!(harness.state(), AudioState::WaitLoading);

    harness.update();

    assert_eq!(harness.state(), AudioState::Playing);
    let playing = harness
        .background()
        .handle
        .as_ref()
        .expect("a playback handle is stored");
    assert_eq!(playing.state(), PlaybackState::Playing);
}

/// `Playing` follows the kira handle: once the sound has run out the state
/// machine reports `Completed`.
#[test]
fn background_audio_completes_when_playback_stops() {
    let mut harness = Harness::with_loaded_asset(true);

    // Drive to `Playing`, then let the mock renderer consume the whole sound.
    harness.update();
    harness.update();
    assert_eq!(harness.state(), AudioState::Playing);

    harness.drain_renderer(2);
    harness.update();

    assert_eq!(harness.state(), AudioState::Completed);
}

/// One world, engine and asset server, with a single `BackgroundAudio` entity
/// spawned into it. Keeps the four driver tests down to their actual assertion.
struct Harness {
    engine: AudioEngine<MockBackend>,
    assets_server: AssetsServer,
    world: World,
    entity: Entity,
}

impl Harness {
    /// The handle never resolves, so the state machine parks in `WaitLoading`.
    fn with_unloaded_asset(auto_play: bool) -> Self {
        let handle = test_audio_asset_handle();
        Self::new(auto_play, AssetsServer::new(), handle)
    }

    /// The handle resolves to a loaded [`AudioAsset`].
    fn with_loaded_asset(auto_play: bool) -> Self {
        let (assets_server, handle) = assets_server_with_audio();
        Self::new(auto_play, assets_server, handle)
    }

    fn new(auto_play: bool, assets_server: AssetsServer, handle: AudioAssetHandle) -> Self {
        let mut world = World::new();
        let entity = world.spawn(BackgroundAudio::new(handle, auto_play)).id();
        Self {
            engine: AudioEngine::new_mock(),
            assets_server,
            world,
            entity,
        }
    }

    fn update(&mut self) {
        self.engine
            .update(&mut self.assets_server, &mut self.world, 0.0);
    }

    fn state(&self) -> AudioState {
        self.background().state
    }

    fn background(&self) -> &BackgroundAudio {
        self.world
            .entity(self.entity)
            .get::<BackgroundAudio>()
            .expect("background entity keeps its component")
    }

    /// Runs the sound through the mock renderer so kira publishes a new
    /// playback state. `StaticSoundHandle::state` only refreshes when the
    /// renderer's `on_start_processing` callback runs, so each `process` is
    /// paired with it.
    fn drain_renderer(&mut self, passes: usize) {
        for _ in 0..passes {
            self.engine.backend_mut().process();
            self.engine.backend_mut().on_start_processing();
        }
    }
}

/// An `AssetsServer` holding one already-loaded [`AudioAsset`], plus the handle
/// that resolves to it.
fn assets_server_with_audio() -> (AssetsServer, AudioAssetHandle) {
    let mut assets_server = AssetsServer::new();
    let handle = assets_server.insert::<AudioAssetsSystem>(
        AudioAsset {
            sound_data: test_sound_data(),
        },
        &PathBuf::from("test://background.audio"),
    );
    (assets_server, handle)
}

/// A very short sound so the mock renderer drains it within a few `process`
/// calls.
fn test_sound_data() -> StaticSoundData {
    StaticSoundData {
        sample_rate: 1,
        frames: (0..8).map(|_| Frame::ZERO).collect(),
        settings: StaticSoundSettings::default(),
        slice: None,
    }
}
