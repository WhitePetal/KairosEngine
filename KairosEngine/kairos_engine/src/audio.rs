use std::fmt::Debug;

use kira::{
    AudioManager, AudioManagerSettings, Decibels, DefaultBackend, Easing, Mapping, Mix, Semitones, Value, clock::{ClockHandle, ClockSpeed, ClockTime}, effect::{filter::FilterBuilder, reverb::ReverbBuilder}, modulator::{lfo::{LfoBuilder, LfoHandle}, tweener::{TweenerBuilder, TweenerHandle}}, sound::static_sound::{StaticSoundData, StaticSoundHandle, StaticSoundSettings}, track::{TrackBuilder, TrackHandle}
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

    pub fn dynamic_music(&mut self) -> Result<TweenerHandle, Box<dyn std::error::Error>> {
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

    pub fn ghost_noise(&mut self) -> Result<LfoHandle, Box<dyn std::error::Error>> {
        let amplitude_lfo = self.manager.add_modulator(LfoBuilder::new().frequency(0.093))?;
        let frequency_lfo = self.manager.add_modulator(LfoBuilder::new().frequency(0.038))?;
        let playback_rate_lfo = self.manager.add_modulator(
            LfoBuilder::new()
                .amplitude(Value::from_modulator(
                    &amplitude_lfo, 
                    Mapping { 
                        input_range: (-1.0, 1.0), 
                        output_range: (0.5, 1.5), 
                        easing: Easing::Linear 
                    }
                ))
                .frequency(Value::from_modulator(
                    &frequency_lfo, 
                    Mapping { 
                        input_range: (-1.0, 1.0), 
                        output_range: (1.0, 4.0), 
                        easing: Easing::Linear
                    }
                ))
        )?;

        let sound = StaticSoundData::from_file("res/audios/sine.wav")?
            .volume(1.0 / 3.0)
            .loop_region(..)
            .playback_rate(Value::from_modulator(
                &playback_rate_lfo, 
                Mapping { 
                    input_range: (-1.0, 1.0), 
                    output_range: (Semitones(56.0).into(), Semitones(64.0).into()), 
                    easing: Easing::Linear
                }
            ));
        self.manager.play(sound)?;
        Ok(playback_rate_lfo)
    }

    pub fn metronome(&mut self) -> Result<ClockHandle, Box<dyn std::error::Error>> {
        let sound_data = StaticSoundData::from_file("res/audios/blip.ogg")?;
        let mut clock = self.manager.add_clock(ClockSpeed::TicksPerMinute(120.0))?;
        self.manager.play(sound_data.playback_rate(2.0).start_time(ClockTime {
            clock: clock.id(),
            ticks: 0,
            fraction: 0.0
        }))?;
        self.manager.play(sound_data.playback_rate(1.0).start_time(ClockTime {
            clock: clock.id(),
            ticks: 1,
            fraction: 0.0
        }))?;

        clock.start();

        Ok(clock)
    }
    pub fn metronome_update(&mut self, pre_clock_time: &mut ClockTime, clock: &ClockHandle) {
        let sound_data = StaticSoundData::from_file("res/audios/blip.ogg").unwrap();
        let cur_audio_clock_time = clock.time();
        if cur_audio_clock_time.ticks > pre_clock_time.ticks {
            let playback_rate = {
                if (cur_audio_clock_time.ticks + 1) % 4 == 0 {
                    2.0
                } else {
                    1.0
                }
            };
            let _ = self.manager.play(sound_data.playback_rate(playback_rate).start_time(cur_audio_clock_time + 1));
            *pre_clock_time = clock.time();
        }
    }

    pub fn score_counter(&mut self) -> Result<StaticSoundHandle, Box<dyn std::error::Error>> {
        let sound_data = StaticSoundData::from_file("res/audios/score.ogg")?
            .playback_rate(1.5)
            .loop_region(..0.06);
        
        let handle = self.manager.play(sound_data)?;
        Ok(handle)
    }
    pub fn score_counter_update(&mut self, sound_handle: &mut StaticSoundHandle, time: f32) {
        if time > 5.0 {
            sound_handle.set_loop_region(None);
        }
    }

    pub fn seamless_loop_with_intro(&mut self) {
        if let Ok(sound_data) = StaticSoundData::from_file("res/audios/drums_intro.ogg") {
            let sound_data = sound_data.loop_region(3.6..6.0);
            let _ = self.manager.play(sound_data);
        }
    }
}