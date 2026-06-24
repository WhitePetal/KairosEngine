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
    None,
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

pub enum SpatialAudioVolumeTrackState {
    Playing(SpatialAudioVolumeTrackKey),
    Leaving(SpatialAudioVolumeTrackLeaving),
    Leaved(SpatialAudioVolumeTrackKey),
}

pub struct SpatialAudioVolumeComponent {
    pub audios: SmallVec<[AudioAssetHandle; SMALL_VEC_AUDIO_COUNT]>,
    pub audio_handles: SmallVec<[SpatialSoundHandle; SMALL_VEC_AUDIO_COUNT]>,
    pub auto_play: bool,
    pub state: SpatialAudioVolumeState,
    pub track_states: Vec<SpatialAudioVolumeTrackState>,
    pub playimg_time: f32,
}
impl Component for SpatialAudioVolumeComponent {}

impl SpatialAudioVolumeComponent {
    pub fn new(audios: SmallVec<[AudioAssetHandle; 4]>, auto_play: bool) -> Self {
        let mut audio_handles = SmallVec::new();
        for _ in 0..audios.len() {
            audio_handles.push(SpatialSoundHandle::None);
        }
        Self {
            audios,
            audio_handles,
            auto_play,
            state: SpatialAudioVolumeState::Created,
            track_states: Vec::new(),
            playimg_time: 0.0,
        }
    }
}
