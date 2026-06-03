use std::path::PathBuf;

use anyhow::Error;
use tokio::sync::mpsc::Sender;

use crate::{
    asset_loader::{
        assets::asset::{self, AssetIndex, Assets, AssetsHandler, AssetsSystem},
        consts,
    },
    graphics::texture::TextureAsset,
};

#[derive(Debug)]
pub struct LoadedEvent {
    index: AssetIndex,
    asset: TextureAsset,
}
impl asset::LoadedEvent<TextureAsset> for LoadedEvent {
    #[inline(always)]
    fn new(index: asset::AssetIndex, asset: TextureAsset) -> Self {
        Self { index, asset }
    }

    #[inline(always)]
    fn get_index(&self) -> asset::AssetIndex {
        self.index
    }

    #[inline(always)]
    fn get_asset(self) -> TextureAsset {
        self.asset
    }
}

#[derive(Debug)]
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

#[derive(Debug)]
pub struct Loader {}
impl Loader {
    async fn load_toml(path: &PathBuf) -> Result<TextureAsset, Error> {
        println!("bef read toml");
        let toml = tokio::fs::read(path).await?;
        println!("aft read toml");
        let texture = toml::from_slice::<TextureAsset>(&toml)?;
        Ok(texture)
    }
    async fn load_bin(path: &PathBuf) -> Result<Vec<u8>, Error> {
        println!("bef read bin");
        let bytes = tokio::fs::read(path.with_extension("texture_bin")).await?;
        println!("aft read bin");
        let data = rkyv::from_bytes::<Vec<u8>, rkyv::rancor::Error>(&bytes)?;
        Ok(data)
    }
    async fn load(
        path: PathBuf,
        asset_index: AssetIndex,
        sender: tokio::sync::mpsc::Sender<LoadedEvent>,
    ) -> Result<(), Error> {
        let (texture, data) = tokio::join!(Self::load_toml(&path), Self::load_bin(&path),);
        let mut texture = texture?;
        let data = data?;

        texture.texture.data = data;

        sender
            .send(LoadedEvent {
                index: asset_index,
                asset: texture,
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
        sender: Sender<LoadedEvent>,
    ) {
        tokio::spawn(Self::load(path, asset_index, sender));
    }
}

#[derive(Debug)]
pub struct TextureAssetsSystem {
    assets: Assets<Self>,
}

impl TextureAssetsSystem {
    pub fn new() -> Self {
        let loader = Loader {};
        let assets = Assets::<Self>::new(
            loader,
            consts::TEXTURE_ASSETS_CAPACITY,
            consts::TEXTURE_ASSETS_LOADED_CHANNEL_BUFFER_SIZE,
            consts::TEXTURE_ASSETS_DROP_CHANNEL_BUFFER_SIZE,
        );
        Self { assets }
    }
}

impl AssetsHandler for TextureAssetsSystem {
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

impl AssetsSystem for TextureAssetsSystem {
    type AssetType = TextureAsset;

    type LoadedEvent = LoadedEvent;

    type DropEvent = DropEvent;

    type Loader = Loader;

    #[inline(always)]
    fn get_assets(&self) -> &super::Assets<Self> {
        &self.assets
    }

    #[inline(always)]
    fn get_assets_mut(&mut self) -> &mut super::Assets<Self> {
        &mut self.assets
    }
}
