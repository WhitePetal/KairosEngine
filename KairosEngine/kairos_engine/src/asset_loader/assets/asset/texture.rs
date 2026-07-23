use std::path::PathBuf;

use anyhow::Error;
use tokio::sync::mpsc::{self};

use crate::{
    asset_loader::{
        assets::{
            DependencyLoadRequestEvent,
            asset::{self, AssetIndex, Assets, AssetsHandler, AssetsSystem},
        },
        consts,
    },
    graphics::texture::{PixelDatas, SerializedTexture, Texture},
    graphics::texture::format::TextureFormat,
};

#[derive(Debug)]
pub struct LoadedEvent {
    index: AssetIndex,
    asset: Texture,
}
impl asset::LoadedEvent<Texture> for LoadedEvent {
    #[inline(always)]
    fn get_index(&self) -> asset::AssetIndex {
        self.index
    }

    #[inline(always)]
    fn get_asset(self) -> Texture {
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
    async fn load_toml(path: &PathBuf) -> Result<SerializedTexture, Error> {
        let toml = tokio::fs::read(path).await?;
        let texture = toml::from_slice::<SerializedTexture>(&toml)?;
        Ok(texture)
    }
    async fn load_bin(path: &PathBuf, format: TextureFormat) -> Result<Vec<PixelDatas>, Error> {
        let bytes = tokio::fs::read(path.with_extension("texture_bin")).await?;
        // Existing files are rkyv — try that first.
        if let Ok(data) = rkyv::from_bytes::<Vec<u8>, rkyv::rancor::Error>(&bytes) {
            return Ok(vec![PixelDatas::U8(data)]);
        }
        if let Ok(data) = rkyv::from_bytes::<Vec<Vec<u8>>, rkyv::rancor::Error>(&bytes) {
            return Ok(data.into_iter().map(PixelDatas::U8).collect());
        }
        // New custom binary format.
        crate::kairos_editor::serialize_asset::texture::deserialize_pixel_datas(&bytes, format)
    }
    async fn load(
        path: PathBuf,
        asset_index: AssetIndex,
        sender: tokio::sync::mpsc::Sender<LoadedEvent>,
    ) -> Result<(), Error> {
        let serialized = Self::load_toml(&path).await?;
        let data = Self::load_bin(&path, serialized.format).await?;

        let texture = Texture {
            width: serialized.width,
            height: serialized.height,
            format: serialized.format,
            data,
            sampler: serialized.sampler.clone(),
        };

        sender
            .send(LoadedEvent {
                index: asset_index,
                asset: texture,
            })
            .await?;
        Ok(())
    }
}
impl asset::AssetLoader<LoadedEvent, Texture> for Loader {
    fn load_asset(
        &self,
        path: std::path::PathBuf,
        asset_index: AssetIndex,
        sender: mpsc::Sender<LoadedEvent>,
        _denpendency_request_sender: mpsc::Sender<DependencyLoadRequestEvent>,
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

impl Default for TextureAssetsSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetsSystem for TextureAssetsSystem {
    type AssetType = Texture;

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
