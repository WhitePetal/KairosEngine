use std::{path::PathBuf, time::Duration};

use kira::{
    Decibels, Panning, PlaybackRate, StartTime, Tween, Value,
    sound::{
        EndPosition, PlaybackPosition, Region,
        static_sound::{StaticSoundData, StaticSoundSettings},
    },
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct SerializedAudioAsset {
    pub source_path: PathBuf,
    pub audio_asset_settings: SerializedAudioAssetSettings,
}

/// Serializable settings for an AudioAsset.
/// These mirror the runtime [`StaticSoundSettings`] plus sample rate and slice info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedAudioAssetSettings {
    pub sample_rate: u32,
    pub reverse: bool,
    /// Volume in decibels. 0.0 = identity (no change).
    pub volume: f32,
    /// Playback rate factor. 1.0 = normal speed.
    pub playback_rate: f64,
    /// Panning value. 0.0 = center, -1.0 = hard left, 1.0 = hard right.
    pub panning: f32,
    /// Loop region start in seconds, if any.
    pub loop_start_seconds: Option<f64>,
    /// Loop region end in seconds, if any.
    pub loop_end_seconds: Option<f64>,
    /// Start position in seconds. 0.0 = beginning.
    pub start_position_seconds: f64,
    /// Fade-in duration in seconds, if any.
    pub fade_in_seconds: Option<f64>,
    /// Slice start frame index, if the audio is sliced.
    pub slice_start: Option<usize>,
    /// Slice end frame index, if the audio is sliced.
    pub slice_end: Option<usize>,
}

impl Default for SerializedAudioAssetSettings {
    fn default() -> Self {
        Self {
            sample_rate: 44100,
            reverse: false,
            volume: 0.0,
            playback_rate: 1.0,
            panning: 0.0,
            loop_start_seconds: None,
            loop_end_seconds: None,
            start_position_seconds: 0.0,
            fade_in_seconds: None,
            slice_start: None,
            slice_end: None,
        }
    }
}

impl SerializedAudioAssetSettings {
    /// Extract settings from a [`StaticSoundData`].
    pub fn from_static_sound_data(data: &StaticSoundData) -> Self {
        let settings = &data.settings;
        let volume = match &settings.volume {
            Value::Fixed(db) => db.0,
            _ => 0.0, // Non-fixed values default to identity for serialization
        };
        let playback_rate = match &settings.playback_rate {
            Value::Fixed(rate) => rate.0,
            _ => 1.0,
        };
        let panning = match &settings.panning {
            Value::Fixed(pan) => pan.0,
            _ => 0.0,
        };
        let loop_region = settings.loop_region.map(|region| {
            let start = match region.start {
                PlaybackPosition::Seconds(s) => s,
                PlaybackPosition::Samples(n) => n as f64 / data.sample_rate as f64,
            };
            let end = match region.end {
                EndPosition::EndOfAudio => data.duration().as_secs_f64(),
                EndPosition::Custom(pos) => match pos {
                    PlaybackPosition::Seconds(s) => s,
                    PlaybackPosition::Samples(n) => n as f64 / data.sample_rate as f64,
                },
            };
            (start, end)
        });
        let start_position_seconds = match settings.start_position {
            PlaybackPosition::Seconds(s) => s,
            PlaybackPosition::Samples(n) => n as f64 / data.sample_rate as f64,
        };
        let fade_in_seconds = settings
            .fade_in_tween
            .map(|tween| tween.duration.as_secs_f64());

        Self {
            sample_rate: data.sample_rate,
            reverse: settings.reverse,
            volume,
            playback_rate,
            panning,
            loop_start_seconds: loop_region.map(|(s, _)| s),
            loop_end_seconds: loop_region.map(|(_, e)| e),
            start_position_seconds,
            fade_in_seconds,
            slice_start: data.slice.map(|(s, _)| s),
            slice_end: data.slice.map(|(_, e)| e),
        }
    }

    /// Apply these settings to a [`StaticSoundData`], returning a new one.
    pub fn apply_to_static_sound_data(&self, mut data: StaticSoundData) -> StaticSoundData {
        data.sample_rate = self.sample_rate;
        data.settings = StaticSoundSettings {
            start_time: StartTime::Immediate,
            start_position: PlaybackPosition::Seconds(self.start_position_seconds),
            loop_region: {
                if let (Some(start), Some(end)) = (self.loop_start_seconds, self.loop_end_seconds) {
                    Some(Region {
                        start: PlaybackPosition::Seconds(start),
                        end: EndPosition::Custom(PlaybackPosition::Seconds(end)),
                    })
                } else if let Some(start) = self.loop_start_seconds {
                    Some(Region {
                        start: PlaybackPosition::Seconds(start),
                        end: EndPosition::EndOfAudio,
                    })
                } else {
                    None
                }
            },
            reverse: self.reverse,
            volume: Value::Fixed(Decibels(self.volume)),
            playback_rate: Value::Fixed(PlaybackRate(self.playback_rate)),
            panning: Value::Fixed(Panning(self.panning)),
            fade_in_tween: self.fade_in_seconds.map(|secs| Tween {
                duration: Duration::from_secs_f64(secs),
                ..Default::default()
            }),
        };
        data.slice = match (self.slice_start, self.slice_end) {
            (Some(start), Some(end)) => Some((start, end)),
            _ => None,
        };
        data
    }
}

/// An audio asset containing both runtime sound data and serializable metadata.
#[derive(Debug, Clone)]
pub struct AudioAsset {
    pub sound_data: StaticSoundData,
}
