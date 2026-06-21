use std::fmt::Debug;

use kira::{
    AudioManager, AudioManagerSettings, Decibels, DefaultBackend, Easing, Mapping, Mix, Value,
    effect::{filter::FilterBuilder, reverb::ReverbBuilder},
    modulator::tweener::{TweenerBuilder, TweenerHandle},
    sound::static_sound::{StaticSoundData, StaticSoundSettings},
    track::{TrackBuilder, TrackHandle},
};

pub mod audio;

pub struct AudioEngine {
    manager: AudioManager,
    lead_track: Option<TrackHandle>,
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
            manager,
            lead_track: None,
        })
    }

    pub fn play(&mut self) {
        let sound_data = StaticSoundData::from_file("res/audios/arp.ogg");
        if let Ok(sound_data) = sound_data {
            let _ = self.manager.play(sound_data);
        }
    }

    pub fn example1(&mut self) -> Result<TweenerHandle, Box<dyn std::error::Error>> {
        let underwater_tweener = self
            .manager
            .add_modulator(TweenerBuilder { initial_value: 0.0 })?;
        self.lead_track = Some(
            self.manager.add_sub_track(
                TrackBuilder::new()
                    .with_effect(FilterBuilder::new().cutoff(Value::from_modulator(
                        &underwater_tweener,
                        Mapping {
                            input_range: (0.0, 1.0),
                            output_range: (20_000.0, 2000.0),
                            easing: Easing::Linear,
                        },
                    )))
                    .with_effect(ReverbBuilder::new().mix(Value::from_modulator(
                        &underwater_tweener,
                        Mapping {
                            input_range: (0.0, 1.0),
                            output_range: (Mix::DRY, Mix(1.0 / 3.0)),
                            easing: Easing::Linear,
                        },
                    ))),
            )?,
        );

        let music_duration = 21.0 + 1.0 / 3.0;
        let common_sound_settings =
            StaticSoundSettings::new().loop_region(music_duration / 2.0..music_duration);
        let arp =
            StaticSoundData::from_file("res/audios/arp.ogg")?.with_settings(common_sound_settings);
        let bass = StaticSoundData::from_file("res/audios/bass.ogg")?
            .with_settings(common_sound_settings)
            .volume(Value::from_modulator(
                &underwater_tweener,
                Mapping {
                    input_range: (0.0, 1.0),
                    output_range: (Decibels::IDENTITY, Decibels::SILENCE),
                    easing: Easing::Linear,
                },
            ));
        let drums = StaticSoundData::from_file("res/audios/drums.ogg")?
            .with_settings(common_sound_settings)
            .volume(Value::from_modulator(
                &underwater_tweener,
                Mapping {
                    input_range: (0.0, 1.0),
                    output_range: (Decibels::IDENTITY, Decibels::SILENCE),
                    easing: Easing::Linear,
                },
            ));

        let lead =
            StaticSoundData::from_file("res/audios/lead.ogg")?.with_settings(common_sound_settings);
        let pad = StaticSoundData::from_file("res/audios/pad.ogg")?
            .with_settings(common_sound_settings)
            .volume(Value::from_modulator(
                &underwater_tweener,
                Mapping {
                    input_range: (0.0, 1.0),
                    output_range: (Decibels::IDENTITY, Decibels::SILENCE),
                    easing: Easing::Linear,
                },
            ));

        self.manager.play(arp)?;
        self.manager.play(bass)?;
        self.manager.play(drums)?;
        if let Some(lead_track) = self.lead_track.as_mut() {
            lead_track.play(lead)?;
        }
        self.manager.play(pad)?;

        Ok(underwater_tweener)
    }
}
