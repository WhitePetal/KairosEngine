use std::fmt::Debug;

use kira::{
    AudioManager, AudioManagerSettings, Capacities, DefaultBackend,
    listener::ListenerId,
    sound::{PlaybackState, static_sound::StaticSoundData},
    track::MainTrackBuilder,
};

use crate::{
    asset_loader::assets::AssetsServer,
    audio::{
        audio::AudioState,
        background::BackgroundAudio,
        spatial::{SpatialAudioConfig, SpatialAudioTracks},
    },
    ecs::world::World,
};

pub mod audio;
pub mod audio_ext;
pub mod background;
pub mod consts;
pub mod spatial;

pub struct AudioEngine {
    manager: AudioManager,
    spatial_tracks: SpatialAudioTracks,
}

impl Debug for AudioEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioEngine").finish_non_exhaustive()
    }
}

impl AudioEngine {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let manager = AudioManager::<DefaultBackend>::new(AudioManagerSettings {
            capacities: Capacities {
                sub_track_capacity: 1024,
                ..Default::default()
            },
            internal_buffer_size: 2048,
            main_track_builder: MainTrackBuilder::new().sound_capacity(2048),
            ..Default::default()
        })?;
        let spatial_audio_config = SpatialAudioConfig::new(
            consts::MAX_SPATIAL_TRACK_COUNT,
            consts::MAX_SPATIAL_LISTENER_COUNT,
            consts::SPATIAL_AUDIO_CUT_OFF_DISTANCE_SQ,
            consts::SPATIAL_AUDIO_TRACK_LEAVING_DURATION,
        );
        let spatial_tracks = SpatialAudioTracks::new(spatial_audio_config)?;
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

    pub fn update(&mut self, assets_server: &mut AssetsServer, world: &mut World, delta_time: f32) {
        // spatial audio volumes system
        self.spatial_tracks
            .update(assets_server, &mut self.manager, world, delta_time);

        // update backgroun
        let background = world.query_mut::<&mut BackgroundAudio>().into_iter().next();
        if let Some(mut background) = background {
            match background.state {
                AudioState::Created => {
                    if background.auto_play {
                        background.state = AudioState::WaitLoading;
                    }
                }
                AudioState::WaitLoading => {
                    let audio = assets_server.get(&background.audio);
                    if let Some(audio) = audio {
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
                AudioState::Paused => todo!(),
                AudioState::Completed => {
                    // now do nothing
                }
            }
        }
    }
}
