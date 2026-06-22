use kira::{
    listener::ListenerHandle,
    sound::static_sound::StaticSoundHandle,
    track::{SpatialTrackBuilder, SpatialTrackHandle},
};
use smallvec::SmallVec;

use crate::{
    asset_loader::assets::AudioAssetHandle, audio::AudioEngine, ecs::component::Component,
    spatial::TransformComponent,
};

#[derive(Debug, Clone, Copy)]
pub enum SpatialAudioVolumeState {
    Created,
    WaitLoading,
    Playing,
    Paused,
    Completed,
}

pub enum SpatialSoundHandle {
    None,
    Some(StaticSoundHandle),
    Err,
}

pub struct SpatialAudioVolumeComponent {
    pub track: Option<SpatialTrackHandle>,
    pub audios: SmallVec<[AudioAssetHandle; 4]>,
    pub audio_handles: SmallVec<[SpatialSoundHandle; 4]>,
    pub auto_play: bool,
    pub state: SpatialAudioVolumeState,
}
impl Component for SpatialAudioVolumeComponent {}

impl SpatialAudioVolumeComponent {
    pub fn new(
        audio_engine: &mut AudioEngine,
        listener: &ListenerHandle,
        transform: TransformComponent,
        audios: SmallVec<[AudioAssetHandle; 4]>,
        auto_play: bool,
    ) -> Self {
        let track = match audio_engine.manager.add_spatial_sub_track(
            listener,
            transform.position,
            SpatialTrackBuilder::new(),
        ) {
            Ok(track) => Some(track),
            Err(err) => {
                println!("Create Spatial Track Handle Error: {:?}", err);
                None
            }
        };
        let mut audio_handles = SmallVec::new();
        for _ in 0..audios.len() {
            audio_handles.push(SpatialSoundHandle::None);
        }
        Self {
            track,
            audios,
            audio_handles,
            auto_play,
            state: SpatialAudioVolumeState::Created,
        }
    }
}
