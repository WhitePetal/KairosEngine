use std::{fmt::Debug, io::Cursor, path::PathBuf, sync::Arc};

use anyhow::Error;
use kira::sound::static_sound::StaticSoundData;
use tokio::sync::mpsc;

use crate::{
    asset_loader::{
        assets::{
            AssetHandle, DependencyLoadRequestEvent,
            asset::{self, AssetIndex, Assets, AssetsHandler, AssetsSystem},
        },
        consts,
    },
    audio::audio::{AudioAsset, SerializedAudioAsset},
};

mod pcm;
mod audio_ext;

pub use pcm::PcmAssetsSystem;
pub use audio_ext::AudioExtAssetsSystem;

pub type AudioAssetHandle = Arc<AssetHandle<AudioAssetsSystem>>;

pub struct LoadedEvent {
    index: AssetIndex,
    asset: AudioAsset,
}

impl Debug for LoadedEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedEvent").finish()
    }
}

impl asset::LoadedEvent<AudioAsset> for LoadedEvent {
    fn get_index(&self) -> AssetIndex {
        self.index
    }

    fn get_asset(self) -> AudioAsset {
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
        let toml_bytes = tokio::fs::read(&path).await?;
        let serialized_asset = tokio::task::spawn_blocking(move || {
            toml::from_slice::<SerializedAudioAsset>(&toml_bytes)
        })
        .await??;

        // Read the source audio file
        let source_path = &serialized_asset.source_path;
        let audio_bytes = tokio::fs::read(source_path).await?;
        let sound_data = tokio::task::spawn_blocking(move || {
            let cursor = Cursor::new(audio_bytes);
            let sound_data = StaticSoundData::from_cursor(cursor)?;
            Ok::<_, Error>(sound_data)
        })
        .await??;

        // Apply saved settings to the sound data
        let sound_data = serialized_asset
            .audio_asset_settings
            .apply_to_static_sound_data(sound_data);
        let asset = AudioAsset { sound_data };

        sender
            .send(LoadedEvent {
                index: asset_index,
                asset,
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

impl asset::AssetLoader<LoadedEvent, AudioAsset> for Loader {
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
pub struct AudioAssetsSystem {
    assets: Assets<Self>,
}
impl AudioAssetsSystem {
    pub fn new() -> Self {
        let loader = Loader {};
        let assets = Assets::<Self>::new(
            loader,
            consts::AUDIO_ASSETS_CAPACITY,
            consts::AUDIO_ASSETS_LOADED_CHANNEL_BUFFER_SIZE,
            consts::AUDIO_ASSETS_DROP_CHANNEL_BUFFER_SIZE,
        );
        Self { assets }
    }
}

impl AssetsHandler for AudioAssetsSystem {
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

impl Default for AudioAssetsSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetsSystem for AudioAssetsSystem {
    type AssetType = AudioAsset;

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
