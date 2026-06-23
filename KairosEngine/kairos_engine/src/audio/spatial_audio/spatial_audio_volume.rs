use kira::{
    sound::static_sound::StaticSoundHandle,
};
use smallvec::SmallVec;

use crate::{
    asset_loader::assets::AudioAssetHandle, ecs::component::Component,
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
    pub audios: SmallVec<[AudioAssetHandle; 4]>,
    pub audio_handles: SmallVec<[SpatialSoundHandle; 4]>,
    pub auto_play: bool,
    pub state: SpatialAudioVolumeState,
}
impl Component for SpatialAudioVolumeComponent {}

impl SpatialAudioVolumeComponent {
    pub fn new(
        audios: SmallVec<[AudioAssetHandle; 4]>,
        auto_play: bool,
    ) -> Self {
        let mut audio_handles = SmallVec::new();
        for _ in 0..audios.len() {
            audio_handles.push(SpatialSoundHandle::None);
        }
        Self {
            audios,
            audio_handles,
            auto_play,
            state: SpatialAudioVolumeState::Created,
        }
    }
}
