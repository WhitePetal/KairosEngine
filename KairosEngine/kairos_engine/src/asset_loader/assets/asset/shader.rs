use std::path::PathBuf;

use anyhow::{Error, Ok};
use tokio::sync::mpsc::{self};

use crate::{
    asset_loader::{
        assets::{
            DependencyLoadRequestEvent,
            asset::{self, AssetIndex, Assets, AssetsHandler, AssetsSystem},
        },
        consts,
    },
    graphics::shader::{Meta, ShaderAsset},
};

#[derive(Debug)]
pub struct LoadedEvent {
    index: AssetIndex,
    asset: ShaderAsset,
}
impl asset::LoadedEvent<ShaderAsset> for LoadedEvent {
    fn get_index(&self) -> AssetIndex {
        self.index
    }

    fn get_asset(self) -> ShaderAsset {
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
        sender: mpsc::Sender<LoadedEvent>,
    ) -> Result<(), Error> {
        let file = tokio::fs::read(&path).await?;
        let shader_string = String::from_utf8(file)?;
        let shader_asset = ShaderAsset {
            meta: Meta { source_path: path },
            shader_string,
        };
        sender
            .send(LoadedEvent {
                index: asset_index,
                asset: shader_asset,
            })
            .await?;
        Ok(())
    }
}
impl asset::AssetLoader<LoadedEvent, ShaderAsset> for Loader {
    fn load_asset(
        &self,
        path: PathBuf,
        asset_index: AssetIndex,
        sender: mpsc::Sender<LoadedEvent>,
        // _on_completed: Option<impl FnOnce(&mut ShaderAsset) -> () + Send + Sync + 'static>,
        _denpendency_request_sender: mpsc::Sender<DependencyLoadRequestEvent>,
    ) {
        tokio::spawn(Self::load(path, asset_index, sender));
    }
}

#[derive(Debug)]
pub struct ShaderAssetsSystem {
    assets: Assets<Self>,
}
impl ShaderAssetsSystem {
    pub fn new() -> Self {
        let loader = Loader {};
        let assets = Assets::<Self>::new(
            loader,
            consts::SHADER_ASSETS_CAPACITY,
            consts::SHADER_ASSETS_LOADED_CHANNEL_BUFFER_SIZE,
            consts::SHADER_ASSETS_DROP_CHANNEL_BUFFER_SIZE,
        );
        Self { assets }
    }
}
impl AssetsHandler for ShaderAssetsSystem {
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
impl AssetsSystem for ShaderAssetsSystem {
    type AssetType = ShaderAsset;

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
