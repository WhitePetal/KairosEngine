// ============================================================
// PcmData — decoded PCM audio
// ============================================================

use std::{fs::File, path::Path, time::Duration};

use symphonia::core::{
    audio::SampleBuffer, codecs::DecoderOptions, formats::FormatOptions, io::MediaSourceStream,
    meta::MetadataOptions, probe::Hint,
};

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

    /// Create PcmData from raw samples (useful for testing).
    pub fn from_raw(sample_rate: u32, num_channels: usize, samples: Vec<f32>) -> Self {
        let num_samples = if num_channels == 0 {
            0
        } else {
            samples.len() / num_channels
        };
        let duration = Duration::from_secs_f64(num_samples as f64 / sample_rate as f64);
        Self {
            sample_rate,
            num_samples,
            num_channels,
            duration,
            samples,
        }
    }

    /// Load and decode an audio file into normalized f32 PCM (useful for testing).
    ///
    /// Supports MP3, WAV, FLAC, OGG (Vorbis) via symphonia.
    pub fn from_path(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let file =
            File::open(path).map_err(|e| format!("open audio file '{}': {e}", path.display()))?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        // Probe the format
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        let probed = symphonia::default::get_probe().format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )?;

        let mut format = probed.format;
        let track = format
            .default_track()
            .ok_or("no default audio track found")?;
        let track_id = track.id;

        let codec_params = track.codec_params.clone();
        let sample_rate = codec_params.sample_rate.ok_or("unknown sample rate")?;

        let mut decoder =
            symphonia::default::get_codecs().make(&codec_params, &DecoderOptions::default())?;

        // Decode all packets
        let mut all_samples: Vec<f32> = Vec::new();
        let mut num_channels: usize = 0;
        let max_frames = codec_params.n_frames.unwrap_or(u64::MAX).min(50_000_000); // cap at ~20min @ 44100 to avoid OOM

        let mut frame_count = 0u64;
        loop {
            if frame_count >= max_frames {
                break;
            }

            let packet = match format.next_packet() {
                Ok(p) => p,
                Err(symphonia::core::errors::Error::IoError(ref e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    break;
                }
                Err(e) => return Err(Box::new(e)),
            };

            if packet.track_id() != track_id {
                continue;
            }

            let decoded = decoder.decode(&packet)?;

            // ---- CRITICAL: skip zero-frame packets ----
            // symphonia-core 0.5.5 has a bug where copy_interleaved_ref
            // panics on zero-frame AudioBufferRef with "range start index
            // N out of range for slice". This happens because the internal
            // loop iterates channels (`self.buf[ch..]`) even when the buffer
            // is empty.
            let num_frames = decoded.frames();
            if num_frames == 0 {
                continue;
            }

            let spec = *decoded.spec();
            if num_channels == 0 {
                num_channels = spec.channels.count();
            }

            frame_count += num_frames as u64;

            let mut sample_buf = SampleBuffer::<f32>::new(num_frames as u64, spec);
            sample_buf.copy_interleaved_ref(decoded);
            all_samples.extend_from_slice(sample_buf.samples());
        }

        if num_channels == 0 {
            num_channels = 1; // fallback: mono
        }

        let num_samples = all_samples.len() / num_channels;
        let duration = Duration::from_secs_f64(num_samples as f64 / sample_rate as f64);

        Ok(Self {
            sample_rate,
            num_samples,
            num_channels,
            duration,
            samples: all_samples,
        })
    }
}
