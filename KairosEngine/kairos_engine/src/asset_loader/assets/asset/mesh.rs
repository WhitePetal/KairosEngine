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
    graphics::mesh::{Mesh, SerializedMeshAsset},
};

#[derive(Debug)]
pub struct LoadedEvent {
    index: AssetIndex,
    asset: Mesh,
}
impl asset::LoadedEvent<Mesh> for LoadedEvent {
    #[inline(always)]
    fn get_index(&self) -> AssetIndex {
        self.index
    }

    #[inline(always)]
    fn get_asset(self) -> Mesh {
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
    async fn load_toml(path: &PathBuf) -> Result<SerializedMeshAsset, Error> {
        let toml = tokio::fs::read(path).await?;
        let serialized_mesh = toml::from_slice::<SerializedMeshAsset>(&toml)?;
        Ok(serialized_mesh)
    }
    async fn load_bin(path: &PathBuf) -> Result<Mesh, Error> {
        let bytes = tokio::fs::read(path.with_extension("mesh_bin")).await?;
        let mesh = rkyv::from_bytes::<Mesh, rkyv::rancor::Error>(&bytes)?;
        Ok(mesh)
    }
    async fn load(
        path: PathBuf,
        asset_index: AssetIndex,
        sender: mpsc::Sender<LoadedEvent>,
    ) -> Result<(), Error> {
        let (serialized_mesh, mesh) = tokio::join!(Self::load_toml(&path), Self::load_bin(&path),);
        let _serialized_mesh = serialized_mesh?;
        let mesh = mesh?;
        let mesh_asset = mesh;

        sender
            .send(LoadedEvent {
                index: asset_index,
                asset: mesh_asset,
            })
            .await?;
        Ok(())
    }
}
impl asset::AssetLoader<LoadedEvent, Mesh> for Loader {
    fn load_asset(
        &self,
        path: PathBuf,
        asset_index: AssetIndex,
        sender: mpsc::Sender<LoadedEvent>,
        // _on_completed: Option<impl FnOnce(&mut MeshAsset) -> () + Send + Sync + 'static>,
        _denpendency_request_sender: mpsc::Sender<DependencyLoadRequestEvent>,
    ) {
        tokio::spawn(Self::load(path, asset_index, sender));
    }
}

#[derive(Debug)]
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

impl Default for MeshAssetsSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetsSystem for MeshAssetsSystem {
    type AssetType = Mesh;

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
