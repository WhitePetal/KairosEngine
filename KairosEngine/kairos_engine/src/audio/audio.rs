use kira::sound::static_sound::StaticSoundData;

#[derive(Debug, Clone)]
pub struct AudioAsset {
    pub sound_data: StaticSoundData,
}

impl AudioAsset {
    pub fn new(sound_data: StaticSoundData) -> Self {
        Self { sound_data }
    }
}
