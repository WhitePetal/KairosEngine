use std::fmt::Debug;

use kairos_asset::Assets;
use kira::{
    AudioManager, AudioManagerSettings, Capacities, DefaultBackend,
    backend::Backend,
    listener::ListenerId,
    sound::{PlaybackState, static_sound::StaticSoundData},
    track::MainTrackBuilder,
};

#[cfg(test)]
use kira::backend::mock::MockBackend;

use crate::{
    audio::{AudioAsset, AudioState},
    background::BackgroundAudio,
    spatial::{SpatialAudioConfig, SpatialAudioTracks},
};

use kairos_ecs::{
    resource::Resource,
    schedule::{IntoScheduleConfigs, ScheduleLabel, Schedules, SystemSet},
    world::World,
};

pub mod audio;
pub mod audio_ext;
pub mod background;
pub mod consts;
pub mod spatial;

mod driver;

#[cfg(test)]
mod test;

/// The backend is a type parameter purely so tests can run the per-frame driver
/// without an audio device; every call site uses the default (real) backend.
///
/// A [`Resource`], so the engine lives in the `World` and the per-frame driver
/// installed by [`install`] can reach it as a resource while also holding the
/// `&mut World` its entity queries need. The backend parameter is
/// deliberately left generic: the resource is keyed by the concrete type's
/// `TypeId`, and production only ever stores `AudioEngine<DefaultBackend>`.
#[derive(Resource)]
pub struct AudioEngine<B: Backend = DefaultBackend> {
    manager: AudioManager<B>,
    spatial_tracks: SpatialAudioTracks<B>,
}

impl<B: Backend> Debug for AudioEngine<B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioEngine").finish_non_exhaustive()
    }
}

impl AudioEngine<DefaultBackend> {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let manager = AudioManager::<DefaultBackend>::new(engine_manager_settings())?;
        Self::from_manager(manager)
    }
}

impl<B: Backend> AudioEngine<B> {
    fn from_manager(manager: AudioManager<B>) -> Result<Self, Box<dyn std::error::Error>> {
        let spatial_audio_config = SpatialAudioConfig::new(
            consts::MAX_SPATIAL_TRACK_COUNT,
            consts::MAX_SPATIAL_LISTENER_COUNT,
            consts::SPATIAL_AUDIO_CUT_OFF_DISTANCE_SQ,
            consts::SPATIAL_AUDIO_TRACK_LEAVING_DURATION,
        );
        let spatial_tracks = SpatialAudioTracks::<B>::new(spatial_audio_config)?;
        Ok(Self {
            manager,
            spatial_tracks,
        })
    }

    pub fn create_listener(&mut self) -> Option<ListenerId> {
        self.spatial_tracks.create_listener(&mut self.manager)
    }

    /// Play a static sound for preview purposes.
    /// Returns a handle that can be used to query state/position.
    pub fn play_sound(
        &mut self,
        sound_data: StaticSoundData,
    ) -> Result<kira::sound::static_sound::StaticSoundHandle, Box<dyn std::error::Error>> {
        let handle = self.manager.play(sound_data)?;
        Ok(handle)
    }

    /// Advances the audio engine by one frame: the spatial listener/volume state
    /// machine and the background-track state machine, both reading the
    /// [`Assets<AudioAsset>`] store from `world`.
    ///
    /// `AudioAsset` must be registered in `world` (the engine bootstrap does
    /// this); otherwise the driver has no store to resolve handles against.
    ///
    /// Crate-private: the driver (`driver::audio_update_system`) is the only
    /// caller, and exposing it would invite callers to drive the engine by hand
    /// and bypass the schedule.
    pub(crate) fn update(&mut self, world: &mut World, delta_time: f32) {
        // The audio store is a World resource, but the per-frame driver needs
        // `&mut World` for its entity queries at the same time, so the store is
        // scoped out for the duration of the update (`World::resource_scope`) and
        // passed down as the read side.
        world.resource_scope::<Assets<AudioAsset>, _>(|world, audios| {
            // spatial audio volumes system
            self.spatial_tracks
                .update(&*audios, &mut self.manager, world, delta_time);

            // update background
            //
            // The pre-fork code took the first `&mut BackgroundAudio` with
            // `query_mut::<&mut BackgroundAudio>().into_iter().next()`; the fork has no
            // `query_mut`, so this is the `world.query::<Q>()` + `iter_mut` form of the
            // same thing (deliberately `.next()`, *not* `single_mut`, which panics
            // unless the world holds exactly one background entity).
            let mut background_query = world.query::<&mut BackgroundAudio>();
            if let Some(mut background) = background_query.iter_mut(&mut *world).next() {
                match background.state {
                    AudioState::Created => {
                        if background.auto_play {
                            background.state = AudioState::WaitLoading;
                        }
                    }
                    AudioState::WaitLoading => {
                        // Polls every frame until the asset loader resolves the handle.
                        let audio = background.audio.id();
                        if let Some(audio) = audios.get(audio) {
                            background.handle = self.manager.play(audio.sound_data.clone()).ok();
                            background.state = AudioState::Playing;
                        }
                    }
                    AudioState::Playing => {
                        if let Some(handle) = &background.handle {
                            if handle.state() == PlaybackState::Stopped {
                                background.state = AudioState::Completed;
                            }
                        }
                    }
                    // Same placeholder as the pre-fork code: nothing ever moves a
                    // background audio into `Paused`, so this arm is unreachable today.
                    AudioState::Paused => todo!(),
                    AudioState::Completed => {
                        // now do nothing
                    }
                }
            }
        });
    }
}

/// Capacities [`AudioEngine`] runs with, shared by the real and mock backends so
/// tests exercise the same resource budget as production.
fn engine_manager_settings<B>() -> AudioManagerSettings<B>
where
    B: Backend,
    B::Settings: Default,
{
    AudioManagerSettings {
        capacities: Capacities {
            sub_track_capacity: 1024,
            ..Default::default()
        },
        internal_buffer_size: 2048,
        main_track_builder: MainTrackBuilder::new().sound_capacity(2048),
        ..Default::default()
    }
}

#[cfg(test)]
impl AudioEngine<MockBackend> {
    /// An [`AudioEngine`] on kira's device-free mock backend. The caller must
    /// drive [`MockBackend::process`] via [`Self::backend_mut`] for playback
    /// state to advance.
    pub(crate) fn new_mock() -> Self {
        let manager = AudioManager::<MockBackend>::new(engine_manager_settings())
            .expect("the mock backend cannot fail to initialise");
        Self::from_manager(manager).expect("spatial track config is valid")
    }

    pub(crate) fn backend_mut(&mut self) -> &mut MockBackend {
        self.manager.backend_mut()
    }
}

/// The system set that groups the per-frame audio driver, so callers can order
/// their own systems relative to it.
///
/// The set exists so that "audio must run after the systems that write the
/// listener's position" is an explicit scheduling interface rather than a
/// convention of the stage the driver happens to be mounted in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AudioUpdateSystems;

// Implemented by hand rather than derived: the `SystemSet` derive lives in
// `kairos_ecs_macros`, which this crate does not depend on. `kairos_asset`'s
// `AssetTrackingSystems` folds its repeated impls the same way.
impl SystemSet for AudioUpdateSystems {
    fn dyn_clone(&self) -> Box<dyn SystemSet> {
        Box::new(*self)
    }
}

/// Installs audio into `world`: the [`AudioEngine`] resource and its per-frame
/// audio driver, mounted into `update_stage` under [`AudioUpdateSystems`].
///
/// `update_stage` is the per-frame schedule the driver registers into — the
/// stage that runs strictly after the `First`-stage clock advance and the
/// `PreUpdate` asset tracking, so a frame's `delta_time` and freshly resolved
/// [`AudioAsset`] handles are both in hand. The schedule
/// skeleton (and its label) belongs to the application, not to this crate, so
/// the caller passes it in: `kairos_engine` supplies its `Update` stage.
///
/// # Panics
///
/// If the schedule named by `update_stage` does not exist yet: registering the
/// driver into a stage that is about to be replaced would silently drop it, so a
/// missing stage at install time is a bootstrap order bug (mirroring
/// `kairos_physics::install` and `kairos_graphics::install`).
pub fn install(world: &mut World, engine: AudioEngine, update_stage: impl ScheduleLabel) {
    world.insert_resource(engine);

    let mut schedules = world.get_resource_or_init::<Schedules>();
    let update = schedules
        .get_mut(update_stage)
        .expect("the `Update` schedule must exist: install the schedule rails first");
    update.configure_sets(AudioUpdateSystems);
    update.add_systems(driver::audio_update_system.in_set(AudioUpdateSystems));
}
