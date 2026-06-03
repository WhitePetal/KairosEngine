use std::path::PathBuf;

use anyhow::{Error, Ok};
use tokio::sync::mpsc::Sender;

use crate::{
    asset_loader::{
        assets::asset::{self, AssetIndex, Assets, AssetsHandler, AssetsSystem},
        consts,
    },
    graphics::material::MaterialAsset,
};

pub struct LoadedEvent {
    index: AssetIndex,
    asset: MaterialAsset,
}
impl asset::LoadedEvent<MaterialAsset> for LoadedEvent {
    fn new(index: AssetIndex, asset: MaterialAsset) -> Self {
        Self { index, asset }
    }

    fn get_index(&self) -> AssetIndex {
        self.index
    }

    fn get_asset(self) -> MaterialAsset {
        self.asset
    }
}

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

pub struct Loader {}
impl Loader {
    async fn load(
        path: PathBuf,
        asset_index: AssetIndex,
        sender: Sender<LoadedEvent>,
    ) -> Result<(), Error> {
        let toml = tokio::fs::read(path).await?;
        let material_asset = toml::from_slice(&toml)?;
        // load shader and texture
        todo!();
        sender
            .send(LoadedEvent {
                index: asset_index,
                asset: material_asset,
            })
            .await?;
        Ok(())
    }
}
impl asset::AssetLoader<LoadedEvent> for Loader {
    fn load_asset(
        &self,
        path: std::path::PathBuf,
        asset_index: AssetIndex,
        sender: tokio::sync::mpsc::Sender<LoadedEvent>,
    ) {
        tokio::spawn(Self::load(path, asset_index, sender));
    }
}

pub struct MaterialAssetsSystem {
    assets: Assets<Self>,
}
impl MaterialAssetsSystem {
    pub fn new() -> Self {
        let loader = Loader {};
        let assets = Assets::<Self>::new(
            loader,
            consts::MATERIAL_ASSETS_CAPACITY,
            consts::MATERIAL_ASSETS_LOADED_CHANNEL_BUFFER_SIZE,
            consts::MATERIAL_ASSETS_DROP_CHANNEL_BUFFER_SIZE,
        );
        Self { assets }
    }
}

impl AssetsHandler for MaterialAssetsSystem {
    fn handle_receves(&mut self) {
        self.handle_receves();
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl AssetsSystem for MaterialAssetsSystem {
    type AssetType = MaterialAsset;

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
