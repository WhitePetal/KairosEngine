use crate::asset::Handle;
use kira::sound::static_sound::StaticSoundHandle;

use crate::audio::audio::{AudioAsset, AudioState};
use kairos_ecs::component::Component;

#[derive(Component)]
pub struct BackgroundAudio {
    pub audio: Handle<AudioAsset>,
    pub handle: Option<StaticSoundHandle>,
    pub state: AudioState,
    pub auto_play: bool,
}

impl BackgroundAudio {
    pub fn new(audio: Handle<AudioAsset>, auto_play: bool) -> Self {
        Self {
            audio,
            handle: None,
            state: AudioState::Created,
            auto_play,
        }
    }
}
