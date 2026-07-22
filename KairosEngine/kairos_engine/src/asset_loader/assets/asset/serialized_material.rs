use std::path::PathBuf;

use anyhow::{Error, Ok};
use tokio::sync::mpsc::{self, Sender};

use crate::{
    asset_loader::{
        assets::{
            asset::{
                self, AssetIndex, Assets, AssetsHandler, AssetsSystem,
            },
        },
        consts,
    },
    graphics::material::SerializedMaterial,
};

// ============================================================
// LoadedEvent
// ============================================================

#[derive(Debug)]
pub struct LoadedEvent {
    index: AssetIndex,
    asset: SerializedMaterial,
}
impl asset::LoadedEvent<SerializedMaterial> for LoadedEvent {
    fn get_index(&self) -> AssetIndex {
        self.index
    }

    fn get_asset(self) -> SerializedMaterial {
        self.asset
    }
}

// ============================================================
// DropEvent
// ============================================================

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

// ============================================================
// Loader — pure TOML deserialization, no dependency loading
// ============================================================

#[derive(Debug)]
pub struct Loader {}
impl Loader {
    async fn load(
        path: PathBuf,
        asset_index: AssetIndex,
        sender: Sender<LoadedEvent>,
    ) -> Result<(), Error> {
        let toml = tokio::fs::read(path.clone()).await?;
        let serialized_material: SerializedMaterial = toml::from_slice(&toml)?;

        sender
            .send(LoadedEvent {
                index: asset_index,
                asset: serialized_material,
            })
            .await?;
        Ok(())
    }
}
impl asset::AssetLoader<LoadedEvent, SerializedMaterial> for Loader {
    fn load_asset(
        &self,
        path: std::path::PathBuf,
        asset_index: AssetIndex,
        sender: tokio::sync::mpsc::Sender<LoadedEvent>,
        _denpendency_request_sender: mpsc::Sender<crate::asset_loader::assets::DependencyLoadRequestEvent>,
    ) {
        tokio::spawn(Self::load(
            path,
            asset_index,
            sender,
        ));
    }
}

// ============================================================
// SerializedMaterialAssetsSystem
// ============================================================

#[derive(Debug)]
pub struct SerializedMaterialAssetsSystem {
    assets: Assets<Self>,
}
impl SerializedMaterialAssetsSystem {
    pub fn new() -> Self {
        let loader = Loader {};
        let assets = Assets::<Self>::new(
            loader,
            consts::SERIALIZED_MATERIAL_ASSETS_CAPACITY,
            consts::SERIALIZED_MATERIAL_ASSETS_LOADED_CHANNEL_BUFFER_SIZE,
            consts::SERIALIZED_MATERIAL_ASSETS_DROP_CHANNEL_BUFFER_SIZE,
        );
        Self { assets }
    }
}

impl AssetsHandler for SerializedMaterialAssetsSystem {
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

impl Default for SerializedMaterialAssetsSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetsSystem for SerializedMaterialAssetsSystem {
    type AssetType = SerializedMaterial;

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
