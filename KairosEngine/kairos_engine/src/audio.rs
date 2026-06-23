use std::fmt::Debug;

use kira::{
    AudioManager, AudioManagerSettings, Capacities, DefaultBackend, listener::ListenerHandle, track::MainTrackBuilder,
};

use crate::{
    asset_loader::assets::AssetsServer, audio::spatial_audio::{SpatialAudioConfig, SpatialAudioTracks}, ecs::world::World, math::{float3, quaternion},
};

pub mod consts;
pub mod audio;
pub mod spatial_audio;

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
        let spatial_audio_config = SpatialAudioConfig::new(consts::MAX_SPATIAL_TRACK_COUNT, consts::MAX_SPATIAL_LISTENER_COUNT, consts::SPATIAL_AUDIO_CUT_OFF_DISTANCE);
        let spatial_tracks = SpatialAudioTracks::new(spatial_audio_config);
        Ok(Self {
            manager,
            spatial_tracks,
        })
    }

    pub fn create_listener(&mut self) -> Option<ListenerHandle> {
        self.manager.add_listener(float3::ZERO, quaternion::IDENTITY).ok()
    }

    pub fn update(&mut self, assets_server: &mut AssetsServer, world: &mut World) {
        // spatial audio volumes system
        self.spatial_tracks.update(assets_server, &mut self.manager,  world);
    }
}
