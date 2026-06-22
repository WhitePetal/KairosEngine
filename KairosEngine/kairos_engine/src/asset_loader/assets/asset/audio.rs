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
    audio::audio::AudioAsset,
};

pub type AudioAssetHandle = Arc<AssetHandle<AudioAssetsSystem>>;

pub struct LoadedEvent {
    index: AssetIndex,
    asset: AudioAsset,
    // on_completed: Option<Box<dyn FnOnce(&mut AudioAsset) -> () + Send + Sync>>,
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
        // if let Some(on_completed) = self.on_completed.take() {
        //     println!("audio asset on_completed: {:?}", self.index);
        //     on_completed(&mut self.asset);
        // }
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
    async fn load(
        path: PathBuf,
        asset_index: AssetIndex,
        // on_completed: Option<impl FnOnce(&mut AudioAsset) -> () + Send + Sync + 'static>,
        sender: mpsc::Sender<LoadedEvent>,
    ) -> Result<(), Error> {
        let bytes = tokio::fs::read(&path).await?;
        let sound_data = tokio::task::spawn_blocking(move || {
            let cursor = Cursor::new(bytes);
            let sound_data = StaticSoundData::from_cursor(cursor)?;
            Ok::<_, Error>(sound_data)
        })
        .await??;
        // f(&mut sound_data);
        let asset = AudioAsset::new(sound_data);
        // let on_completed: Option<Box<dyn for<'a> FnOnce(&'a mut AudioAsset) -> () + Send + Sync>> =
        //     match on_completed {
        //         Some(x) => Some(Box::new(x)),
        //         None => None,
        // };
        sender
            .send(LoadedEvent {
                index: asset_index,
                // on_completed,
                asset,
            })
            .await?;
        Ok(())
    }
}
impl asset::AssetLoader<LoadedEvent, AudioAsset> for Loader {
    fn load_asset(
        &self,
        path: PathBuf,
        asset_index: AssetIndex,
        sender: mpsc::Sender<LoadedEvent>,
        // on_completed: Option<impl FnOnce(&mut AudioAsset) -> () + Send + Sync + 'static>,
        _denpendency_request_sender: mpsc::Sender<DependencyLoadRequestEvent>,
    ) {
        tokio::spawn(Self::load(
            path,
            asset_index,
            // on_completed,
            sender,
        ));
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
