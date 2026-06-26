use kira::sound::static_sound::StaticSoundHandle;

use crate::{
    asset_loader::assets::AudioAssetHandle, audio::audio::AudioState, ecs::component::Component,
};

pub struct BackgroundAudio {
    pub audio: AudioAssetHandle,
    pub handle: Option<StaticSoundHandle>,
    pub state: AudioState,
    pub auto_play: bool,
}
impl Component for BackgroundAudio {}

impl BackgroundAudio {
    pub fn new(audio: AudioAssetHandle, auto_play: bool) -> Self {
        Self {
            audio,
            handle: None,
            state: AudioState::Created,
            auto_play,
        }
    }
}
