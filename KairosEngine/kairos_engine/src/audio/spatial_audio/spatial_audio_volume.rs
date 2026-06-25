use kira::{listener::ListenerId, sound::static_sound::StaticSoundHandle};
use smallvec::SmallVec;

use crate::{asset_loader::assets::AudioAssetHandle, ecs::component::Component};

pub const SMALL_VEC_AUDIO_COUNT: usize = 4;

#[derive(Debug, Clone, Copy)]
pub enum SpatialAudioVolumeState {
    Created,
    WaitLoading,
    Playing,
    Paused,
    Completed,
}

pub enum SpatialSoundHandle {
    Some(StaticSoundHandle),
    Err,
}

#[derive(Debug, Clone, Copy)]
pub struct SpatialAudioVolumeTrackKey {
    pub listener_id: ListenerId,
    pub track_index: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct SpatialAudioVolumeTrackLeaving {
    pub track_key: SpatialAudioVolumeTrackKey,
    pub timer: f32,
}

#[derive(Debug, Clone, Copy)]
pub enum SpatialAudioVolumeTrackState {
    Playing(SpatialAudioVolumeTrackKey),
    Leaving(SpatialAudioVolumeTrackLeaving),
    Leaved(SpatialAudioVolumeTrackKey),
}

pub struct SpatialAudioVolume {
    pub audios: SmallVec<[AudioAssetHandle; SMALL_VEC_AUDIO_COUNT]>,
    pub audio_handles: SmallVec<[SpatialSoundHandle; SMALL_VEC_AUDIO_COUNT]>,
    pub auto_play: bool,
    pub state: SpatialAudioVolumeState,
    pub track_states: Vec<SpatialAudioVolumeTrackState>,
    pub playing_time: f32,
}
impl Component for SpatialAudioVolume {}

impl SpatialAudioVolume {
    pub fn new(audios: SmallVec<[AudioAssetHandle; 4]>, auto_play: bool, start_time: f32) -> Self {
        Self {
            audios,
            audio_handles: SmallVec::new(),
            auto_play,
            state: SpatialAudioVolumeState::Created,
            track_states: Vec::new(),
            playing_time: start_time,
        }
    }
}
