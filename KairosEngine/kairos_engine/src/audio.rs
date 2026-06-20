use std::fmt::Debug;

use kira::{AudioManager, AudioManagerSettings, DefaultBackend, sound::static_sound::StaticSoundData};



pub struct AudioEngine {
    manager: AudioManager
}

impl Debug for AudioEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioEngine").finish_non_exhaustive()
    }
}

impl AudioEngine {
    pub fn new() -> Option<Self> {
        let manager = AudioManager::<DefaultBackend>::new(AudioManagerSettings::default()).ok()?;
        Some(Self {
            manager
        })
    }

    pub fn play(&mut self) {
        let sound_data = StaticSoundData::from_file("res/audios/arp.ogg");
        if let Ok(sound_data) = sound_data {
            let _ = self.manager.play(sound_data);
        }
    }

    pub fn example1(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // let mut underwater_tweener = self.manager.add_modulator(builder)?;
        Ok(())
    }
}