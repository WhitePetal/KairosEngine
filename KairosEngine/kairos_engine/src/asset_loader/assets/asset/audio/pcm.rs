use std::{fmt::Debug, io::Cursor, path::PathBuf, time::Duration};

use anyhow::Error;
use symphonia::core::{
    audio::SampleBuffer, codecs::DecoderOptions, formats::FormatOptions, io::MediaSourceStream,
    meta::MetadataOptions, probe::Hint,
};
use tokio::sync::mpsc;

use crate::{
    asset_loader::{
        assets::{
            DependencyLoadRequestEvent,
            asset::{self, AssetIndex, Assets, AssetsHandler, AssetsSystem},
        },
        consts,
    },
    audio::audio_ext::pcm::PcmData,
};

pub struct LoadedEvent {
    index: AssetIndex,
    asset: PcmData,
}

impl Debug for LoadedEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedEvent").finish()
    }
}

impl asset::LoadedEvent<PcmData> for LoadedEvent {
    fn get_index(&self) -> AssetIndex {
        self.index
    }

    fn get_asset(self) -> PcmData {
        self.asset
    }
}

#[derive(Debug)]
pub struct DropEvent {
    index: AssetIndex,
}

impl asset::DropEvent for DropEvent {
    fn new(index: AssetIndex) -> Self {
        Self { index }
    }

    fn get_index(&self) -> AssetIndex {
        self.index
    }
}

#[derive(Debug)]
pub struct Loader {}

impl Loader {
    /// Load an audio asset from a `.audio` TOML file.
    /// The TOML contains `meta.source_path` pointing to the original audio file
    /// and `settings` that are applied to the decoded sound data.
    async fn load_asset(
        path: PathBuf,
        asset_index: AssetIndex,
        sender: mpsc::Sender<LoadedEvent>,
    ) -> Result<(), Error> {
        // Extract extension before moving path into the blocking closure
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_owned());

        // Read the entire audio file asynchronously
        let audio_bytes = tokio::fs::read(&path).await?;

        // Decode in a blocking task — symphonia is CPU-bound, not async
        let pcm_data = tokio::task::spawn_blocking(move || {
            let cursor = Cursor::new(audio_bytes);
            let mss = MediaSourceStream::new(Box::new(cursor), Default::default());

            // Probe the format
            let mut hint = Hint::new();
            if let Some(ref ext) = ext {
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
                .ok_or_else(|| anyhow::anyhow!("no default audio track found"))?;
            let track_id = track.id;

            let codec_params = track.codec_params.clone();
            let sample_rate = codec_params
                .sample_rate
                .ok_or_else(|| anyhow::anyhow!("unknown sample rate"))?;

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
                    Err(e) => return Err(anyhow::Error::new(e)),
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

            Ok::<PcmData, Error>(PcmData {
                sample_rate,
                num_samples,
                num_channels,
                duration,
                samples: all_samples,
            })
        })
        .await??;

        sender
            .send(LoadedEvent {
                index: asset_index,
                asset: pcm_data,
            })
            .await?;

        Ok(())
    }

    async fn load(
        path: PathBuf,
        asset_index: AssetIndex,
        sender: mpsc::Sender<LoadedEvent>,
    ) -> Result<(), Error> {
        Self::load_asset(path, asset_index, sender).await
    }
}

impl asset::AssetLoader<LoadedEvent, PcmData> for Loader {
    fn load_asset(
        &self,
        path: PathBuf,
        asset_index: AssetIndex,
        sender: mpsc::Sender<LoadedEvent>,
        _denpendency_request_sender: mpsc::Sender<DependencyLoadRequestEvent>,
    ) {
        tokio::spawn(Self::load(path, asset_index, sender));
    }
}

#[derive(Debug)]
pub struct PcmAssetsSystem {
    assets: Assets<Self>,
}
impl PcmAssetsSystem {
    pub fn new() -> Self {
        let loader = Loader {};
        let assets = Assets::<Self>::new(
            loader,
            consts::PCM_ASSETS_CAPACITY,
            consts::PCM_ASSETS_LOADED_CHANNEL_BUFFER_SIZE,
            consts::PCM_ASSETS_DROP_CHANNEL_BUFFER_SIZE,
        );
        Self { assets }
    }
}

impl AssetsHandler for PcmAssetsSystem {
    #[inline(always)]
    fn handle_receves(&mut self) {
        self.assets.handle_receves();
    }

    #[inline(always)]
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    #[inline(always)]
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Default for PcmAssetsSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetsSystem for PcmAssetsSystem {
    type AssetType = PcmData;

    type LoadedEvent = LoadedEvent;

    type DropEvent = DropEvent;

    type Loader = Loader;

    #[inline(always)]
    fn get_assets(&self) -> &Assets<Self> {
        &self.assets
    }

    #[inline(always)]
    fn get_assets_mut(&mut self) -> &mut Assets<Self> {
        &mut self.assets
    }
}
