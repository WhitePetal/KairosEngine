use kairos_asset::{Assets, Handle};
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

/// An unloaded [`Handle<AudioAsset>`] for structural tests: the default weak
/// handle names nothing, so it can be moved into a component but never resolves
/// an asset.
fn test_audio_asset_handle() -> Handle<AudioAsset> {
    Handle::default()
}

/// A world carrying just the audio asset store the per-frame driver reads. The
/// store is all `AudioEngine::update` needs beyond the entities.
fn audio_world() -> World {
    let mut world = World::new();
    world.insert_resource(Assets::<AudioAsset>::with_capacity(0));
    world
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
    let mut world = audio_world();

    engine.update(&mut world, 0.0);
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

/// One world, engine and asset store, with a single `BackgroundAudio` entity
/// spawned into it. Keeps the four driver tests down to their actual assertion.
struct Harness {
    engine: AudioEngine<MockBackend>,
    world: World,
    entity: Entity,
}

impl Harness {
    /// The handle never resolves, so the state machine parks in `WaitLoading`.
    fn with_unloaded_asset(auto_play: bool) -> Self {
        Self::spawn(auto_play, audio_world(), test_audio_asset_handle())
    }

    /// The handle resolves to a loaded [`AudioAsset`] in the world's store.
    fn with_loaded_asset(auto_play: bool) -> Self {
        let mut world = audio_world();
        let handle = world
            .resource_mut::<Assets<AudioAsset>>()
            .add(AudioAsset {
                sound_data: test_sound_data(),
            });
        Self::spawn(auto_play, world, handle)
    }

    fn spawn(auto_play: bool, mut world: World, handle: Handle<AudioAsset>) -> Self {
        let entity = world.spawn(BackgroundAudio::new(handle, auto_play)).id();
        Self {
            engine: AudioEngine::new_mock(),
            world,
            entity,
        }
    }

    fn update(&mut self) {
        self.engine.update(&mut self.world, 0.0);
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
