use std::path::PathBuf;

use anyhow::{Error, Ok};
use tokio::sync::mpsc::{self};

use crate::{
    asset_loader::{
        assets::{DependencyLoadRequestEvent, asset::{self, AssetIndex, Assets, AssetsHandler, AssetsSystem}},
        consts,
    },
    graphics::mesh::MeshAsset,
};

pub struct LoadedEvent {
    index: AssetIndex,
    asset: MeshAsset,
}
impl asset::LoadedEvent<MeshAsset> for LoadedEvent {
    #[inline(always)]
    fn new(index: AssetIndex, asset: MeshAsset) -> Self {
        Self { index, asset }
    }

    #[inline(always)]
    fn get_index(&self) -> AssetIndex {
        self.index
    }

    #[inline(always)]
    fn get_asset(self) -> MeshAsset {
        self.asset
    }
}

pub struct DropEvent {
    index: AssetIndex,
}
impl asset::DropEvent for DropEvent {
    #[inline(always)]
    fn new(index: AssetIndex) -> Self {
        Self { index }
    }

    #[inline(always)]
    fn get_index(&self) -> AssetIndex {
        self.index
    }
}

pub struct Loader {}
impl Loader {
    async fn load(
        path: PathBuf,
        asset_index: AssetIndex,
        sender: mpsc::Sender<LoadedEvent>,
    ) -> Result<(), Error> {
        let toml = tokio::fs::read(path).await?;
        let mesh_asset = toml::from_slice(&toml)?;
        sender
            .send(LoadedEvent {
                index: asset_index,
                asset: mesh_asset,
            })
            .await?;
        Ok(())
    }
}
impl asset::AssetLoader<LoadedEvent> for Loader {
    fn load_asset(&self, path: PathBuf, asset_index: AssetIndex, sender: mpsc::Sender<LoadedEvent>, denpendency_request_sender: mpsc::Sender<DependencyLoadRequestEvent>) {
        tokio::spawn(Self::load(path, asset_index, sender));
    }
}

pub struct MeshAssetsSystem {
    assets: Assets<Self>,
}
impl MeshAssetsSystem {
    pub fn new() -> Self {
        let loader = Loader {};
        let assets = Assets::<Self>::new(
            loader,
            consts::MESH_ASSETS_CAPACITY,
            consts::MESH_ASSETS_LOADED_CHANNEL_BUFFER_SIZE,
            consts::MESH_ASSETS_DROP_CHANNEL_BUFFER_SIZE,
        );
        Self { assets }
    }
}

impl AssetsHandler for MeshAssetsSystem {
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

impl AssetsSystem for MeshAssetsSystem {
    type AssetType = MeshAsset;

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
