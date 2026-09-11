// ============================================================
// PcmData — decoded interleaved PCM audio
// ============================================================

use std::{io::Cursor, path::Path, time::Duration};

use crate::asset::{
    Asset, AssetLoader, AssetWorldExt, LoadContext, Reader, VisitAssetDependencies,
};
use kairos_ecs::error::KairosError;
use kairos_ecs::world::World;
use kairos_tasks::ConditionalSendFuture;
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
            num_channels,
            num_samples,
            duration,
            samples,
        }
    }

    /// Load and decode an audio file into normalized f32 PCM (useful for testing).
    ///
    /// Supports MP3, WAV, FLAC, OGG (Vorbis) via symphonia.
    pub fn from_path(path: &Path) -> Result<Self, KairosError> {
        let extension = path.extension().and_then(|e| e.to_str()).map(str::to_owned);
        let bytes = std::fs::read(path).map_err(|error| {
            KairosError::error(format!("open audio file '{}': {error}", path.display()))
        })?;
        Self::from_bytes(bytes, extension.as_deref())
    }

    /// Decode encoded audio bytes into normalized f32 PCM.
    ///
    /// `extension` is a format hint for the probe; it may be omitted, in which
    /// case symphonia sniffs the container itself.
    pub fn from_bytes(bytes: Vec<u8>, extension: Option<&str>) -> Result<Self, KairosError> {
        let cursor = Cursor::new(bytes);
        let mss = MediaSourceStream::new(Box::new(cursor), Default::default());

        // Probe the format
        let mut hint = Hint::new();
        if let Some(ext) = extension {
            hint.with_extension(ext);
        }

        let probed = symphonia::default::get_probe()
            .format(
                &hint,
                mss,
                &FormatOptions::default(),
                &MetadataOptions::default(),
            )
            .map_err(|error| KairosError::error(format!("probe audio: {error}")))?;

        let mut format = probed.format;
        let track = format
            .default_track()
            .ok_or_else(|| KairosError::error("no default audio track found"))?;
        let track_id = track.id;

        let codec_params = track.codec_params.clone();
        let sample_rate = codec_params
            .sample_rate
            .ok_or_else(|| KairosError::error("unknown sample rate"))?;

        let mut decoder = symphonia::default::get_codecs()
            .make(&codec_params, &DecoderOptions::default())
            .map_err(|error| KairosError::error(format!("create decoder: {error}")))?;

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
                Err(e) => {
                    return Err(KairosError::error(format!("read audio packet: {e}")));
                }
            };

            if packet.track_id() != track_id {
                continue;
            }

            let decoded = decoder
                .decode(&packet)
                .map_err(|error| KairosError::error(format!("decode audio: {error}")))?;

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

impl Asset for PcmData {}
impl VisitAssetDependencies for PcmData {}

/// How many PCM slots [`Assets<PcmData>`](crate::asset::Assets)
/// preallocates. Carried over from the legacy stack's `PCM_ASSETS_CAPACITY`.
pub const PCM_ASSETS_CAPACITY: usize = 8;

/// Decodes an encoded audio file into [`PcmData`].
///
/// Resolved by extension (`.ogg`, `.wav`, `.mp3`, `.flac`) as well as by asset
/// type, so a source audio file can be loaded on its own.
#[derive(Debug)]
pub struct PcmDataLoader;

impl AssetLoader for PcmDataLoader {
    type Asset = PcmData;
    type Settings = ();
    type Error = KairosError;

    fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        load_context: &mut LoadContext,
    ) -> impl ConditionalSendFuture<Output = Result<PcmData, KairosError>> {
        async move {
            let extension = load_context
                .path()
                .path()
                .extension()
                .and_then(|extension| extension.to_str())
                .map(str::to_owned);
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;
            PcmData::from_bytes(bytes, extension.as_deref())
        }
    }

    fn extensions(&self) -> &[&str] {
        &["ogg", "wav", "mp3", "flac"]
    }
}

/// Registers the [`PcmData`] asset and its [`PcmDataLoader`] with the core.
///
/// Must run after [`crate::asset::install`], which creates the
/// `AssetServer` and the `AssetStages` this reads.
pub fn install(world: &mut World) {
    world.init_asset_with_capacity::<PcmData>(PCM_ASSETS_CAPACITY);
    world.register_asset_loader(PcmDataLoader);
}

/// A minimal 16-bit mono WAV carrying `samples` at `sample_rate`, used by the
/// audio loaders' tests to produce a decodable source without checking in a
/// binary fixture.
#[cfg(test)]
pub(crate) fn wav_bytes(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let data_len = samples.len() * 2;
    let file_len = 44 + data_len;
    let mut buf = Vec::with_capacity(file_len);
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&((file_len - 8) as u32).to_le_bytes());
    buf.extend_from_slice(b"WAVE");
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&(16u32).to_le_bytes());
    buf.extend_from_slice(&(1u16).to_le_bytes());
    buf.extend_from_slice(&(1u16).to_le_bytes());
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    buf.extend_from_slice(&(2u16).to_le_bytes());
    buf.extend_from_slice(&(16u16).to_le_bytes());
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&(data_len as u32).to_le_bytes());
    for &s in samples {
        buf.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    buf
}

#[cfg(test)]
mod test {
    use std::{thread, time::Duration};

    use crate::asset::{AssetServer, Assets, install};
    use kairos_ecs::schedule::ScheduleLabel;
    use kairos_ecs::world::World;

    use super::{PcmData, install as install_pcm, wav_bytes};

    /// The two ad-hoc stages the asset drivers are installed into.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    struct Tracking;

    impl ScheduleLabel for Tracking {
        fn dyn_clone(&self) -> Box<dyn ScheduleLabel> {
            Box::new(*self)
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    struct Events;

    impl ScheduleLabel for Events {
        fn dyn_clone(&self) -> Box<dyn ScheduleLabel> {
            Box::new(*self)
        }
    }

    /// A source `.wav` load through the core lands decoded PCM in
    /// `Assets<PcmData>`.
    #[test]
    fn pcm_loads_through_the_core() {
        let dir = tempfile::Builder::new()
            .tempdir_in(".")
            .expect("a temp dir in the cwd");
        let full_path = dir.path().join("Probe.wav");
        std::fs::write(&full_path, wav_bytes(&[0.0, 0.5, -0.5, 0.0], 44100))
            .expect("write the source audio");
        let cwd = std::env::current_dir().expect("the cwd");
        let rel_path = full_path
            .strip_prefix(&cwd)
            .expect("the temp dir is under the cwd")
            .to_path_buf();

        let mut world = World::new();
        install(&mut world, Tracking, Events);
        install_pcm(&mut world);

        let handle = world.resource::<AssetServer>().load::<PcmData>(rel_path);

        // The loader runs on the io task pool, so pump the tracking stage until
        // its result reaches the store.
        let mut loaded = None;
        for _ in 0..200 {
            world.run_schedule(Tracking);
            if let Some(pcm) = world.resource::<Assets<PcmData>>().get(handle.id()) {
                loaded = Some((pcm.sample_rate, pcm.num_channels, pcm.num_samples));
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }

        assert_eq!(loaded, Some((44100, 1, 4)));
    }
}
