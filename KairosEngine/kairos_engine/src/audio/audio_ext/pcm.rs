// ============================================================
// PcmData — decoded PCM audio
// ============================================================

use std::time::Duration;

/// Decoded PCM audio data, normalized to `[-1.0, 1.0]` f32.
/// Samples are interleaved when multi-channel.
#[derive(Debug, Clone, PartialEq)]
pub struct PcmData {
    /// Sample rate in Hz (e.g. 44100).
    pub sample_rate: u32,
    /// Total number of samples per channel.
    pub num_samples: usize,
    /// Total number of original channels.
    pub num_channels: usize,
    /// Duration of the audio.
    pub duration: Duration,
    /// Normalized f32 PCM samples, interleaved if multi-channel.
    pub samples: Vec<f32>,
}

impl PcmData {
    /// Get mono samples by averaging all channels into one.
    pub fn mono_samples(&self) -> Vec<f32> {
        if self.samples.is_empty() || self.num_channels == 0 {
            return Vec::new();
        }
        if self.num_channels == 1 {
            return self.samples.clone();
        }
        // Use chunks (not chunks_exact) to handle partial trailing frames
        self.samples
            .chunks(self.num_channels)
            .map(|frame| {
                let sum: f32 = frame.iter().sum();
                sum / frame.len() as f32
            })
            .collect()
    }
}
