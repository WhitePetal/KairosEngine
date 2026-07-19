use std::{path::PathBuf, sync::Arc};

use anyhow::{Error, Ok};
use tokio::sync::{
    mpsc::{self, Sender},
    oneshot,
};

use crate::{
    asset_loader::{
        assets::{
            AssetHandle, AudioAssetsSystem, DependencyLoadRequest, DependencyLoadRequestEvent,
            asset::{self, AssetIndex, Assets, AssetsHandler, AssetsSystem, PcmAssetsSystem},
        },
        consts,
    },
    audio::{audio::SerializedAudioAsset, audio_ext::AudioExt},
};

#[derive(Debug)]
pub struct LoadedEvent {
    index: AssetIndex,
    asset: AudioExt,
}
impl asset::LoadedEvent<AudioExt> for LoadedEvent {
    fn get_index(&self) -> AssetIndex {
        self.index
    }

    fn get_asset(self) -> AudioExt {
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
        sender: Sender<LoadedEvent>,
        denpendency_request_sender: mpsc::Sender<DependencyLoadRequestEvent>,
    ) -> Result<(), Error> {
        let toml_bytes = tokio::fs::read(&path).await?;
        let serialized_asset = tokio::task::spawn_blocking(move || {
            toml::from_slice::<SerializedAudioAsset>(&toml_bytes)
        })
        .await??;

        // load auido and pcm and ..
        let (audio_setback_sender, audio_setback_recever) =
            oneshot::channel::<Arc<AssetHandle<AudioAssetsSystem>>>();
        let (pcm_setback_sender, pcm_setback_recever) =
            oneshot::channel::<Arc<AssetHandle<PcmAssetsSystem>>>();

        let mut audio_ext = AudioExt::default();

        let _ = tokio::join!(
            denpendency_request_sender.send(Box::new(DependencyLoadRequest::<AudioAssetsSystem> {
                dependency_path: path.clone(),
                setback_sender: audio_setback_sender
            })),
            denpendency_request_sender.send(Box::new(DependencyLoadRequest::<PcmAssetsSystem> {
                dependency_path: serialized_asset.source_path.clone(),
                setback_sender: pcm_setback_sender
            }))
        );

        audio_ext.audio = Some(audio_setback_recever.await?);
        audio_ext.pcm = Some(pcm_setback_recever.await?);

        sender
            .send(LoadedEvent {
                index: asset_index,
                asset: audio_ext,
            })
            .await?;
        Ok(())
    }
}
impl asset::AssetLoader<LoadedEvent, AudioExt> for Loader {
    fn load_asset(
        &self,
        path: std::path::PathBuf,
        asset_index: AssetIndex,
        sender: tokio::sync::mpsc::Sender<LoadedEvent>,
        // _on_completed: Option<impl FnOnce(&mut MaterialAsset) -> () + Send + Sync + 'static>,
        denpendency_request_sender: mpsc::Sender<DependencyLoadRequestEvent>,
    ) {
        tokio::spawn(Self::load(
            path,
            asset_index,
            sender,
            denpendency_request_sender,
        ));
    }
}

#[derive(Debug)]
pub struct AudioExtAssetsSystem {
    assets: Assets<Self>,
}
impl AudioExtAssetsSystem {
    pub fn new() -> Self {
        let loader = Loader {};
        let assets = Assets::<Self>::new(
            loader,
            consts::AUDIO_EXT_ASSETS_CAPACITY,
            consts::AUDIO_EXT_ASSETS_LOADED_CHANNEL_BUFFER_SIZE,
            consts::AUDIO_EXT_ASSETS_DROP_CHANNEL_BUFFER_SIZE,
        );
        Self { assets }
    }
}

impl AssetsHandler for AudioExtAssetsSystem {
    fn handle_receves(&mut self) {
        self.assets.handle_receves();
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Default for AudioExtAssetsSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetsSystem for AudioExtAssetsSystem {
    type AssetType = AudioExt;

    type LoadedEvent = LoadedEvent;

    type DropEvent = DropEvent;

    type Loader = Loader;

    fn get_assets(&self) -> &Assets<Self>
    where
        Self: Sized,
    {
        &self.assets
    }

    fn get_assets_mut(&mut self) -> &mut Assets<Self>
    where
        Self: Sized,
    {
        &mut self.assets
    }
}
