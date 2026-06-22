use std::{path::PathBuf, sync::Arc};

use anyhow::{Error, Ok};
use tokio::sync::{
    mpsc::{self, Sender},
    oneshot,
};

use crate::{
    asset_loader::{
        assets::{
            AssetHandle, DependencyLoadRequest, DependencyLoadRequestEvent, TextureAssetsSystem,
            asset::{
                self, AssetIndex, Assets, AssetsHandler, AssetsSystem, shader::ShaderAssetsSystem,
            },
        },
        consts,
    },
    graphics::material::MaterialAsset,
};

#[derive(Debug)]
pub struct LoadedEvent {
    index: AssetIndex,
    asset: MaterialAsset,
}
impl asset::LoadedEvent<MaterialAsset> for LoadedEvent {
    fn get_index(&self) -> AssetIndex {
        self.index
    }

    fn get_asset(self) -> MaterialAsset {
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
        let toml = tokio::fs::read(path).await?;
        let mut material_asset: MaterialAsset = toml::from_slice(&toml)?;
        // load shader and texture
        let (shader_setback_sender, shader_setback_recever) =
            oneshot::channel::<Arc<AssetHandle<ShaderAssetsSystem>>>();
        let (texture_setback_sender, texture_setback_recever) =
            oneshot::channel::<Arc<AssetHandle<TextureAssetsSystem>>>();

        let _ = tokio::join!(
            denpendency_request_sender.send(Box::new(
                DependencyLoadRequest::<ShaderAssetsSystem> {
                    dependency_path: material_asset.meta.shader_path.clone(),
                    setback_sender: shader_setback_sender
                }
            )),
            denpendency_request_sender.send(Box::new(
                DependencyLoadRequest::<TextureAssetsSystem> {
                    dependency_path: material_asset.meta.texture_path.clone(),
                    setback_sender: texture_setback_sender
                }
            ))
        );

        material_asset.material.shader = Some(shader_setback_recever.await?);
        material_asset.material.texture = Some(texture_setback_recever.await?);

        sender
            .send(LoadedEvent {
                index: asset_index,
                asset: material_asset,
            })
            .await?;
        Ok(())
    }
}
impl asset::AssetLoader<LoadedEvent, MaterialAsset> for Loader {
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
        self.assets.handle_receves();
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Default for MaterialAssetsSystem {
    fn default() -> Self {
        Self::new()
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
