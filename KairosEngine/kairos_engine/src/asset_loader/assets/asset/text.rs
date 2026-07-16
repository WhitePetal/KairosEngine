use std::path::PathBuf;

use anyhow::Error;

use crate::asset_loader::{
    assets::{
        DependencyLoadRequestEvent,
        asset::{self, AssetIndex, AssetLoader, Assets, AssetsHandler, AssetsSystem},
    },
    consts,
};

#[derive(Debug)]
pub struct LoadedEvent {
    index: AssetIndex,
    content: String,
}
impl asset::LoadedEvent<String> for LoadedEvent {
    fn get_index(&self) -> AssetIndex {
        self.index
    }

    fn get_asset(self) -> String {
        self.content
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
        sender: tokio::sync::mpsc::Sender<LoadedEvent>,
        _denpendency_request_sender: tokio::sync::mpsc::Sender<DependencyLoadRequestEvent>,
    ) -> Result<(), Error> {
        println!("load text");
        let content = tokio::fs::read_to_string(path.clone()).await?;
        println!("load text success");

        sender
            .send(LoadedEvent {
                index: asset_index,
                content,
            })
            .await?;
        Ok(())
    }
}
impl AssetLoader<LoadedEvent, String> for Loader {
    fn load_asset(
        &self,
        path: std::path::PathBuf,
        asset_index: AssetIndex,
        loaded_sender: tokio::sync::mpsc::Sender<LoadedEvent>,
        // on_completed: Option<impl FnOnce(&mut A) -> () + Send + Sync + 'static>,
        denpendency_request_sender: tokio::sync::mpsc::Sender<
            crate::asset_loader::assets::DependencyLoadRequestEvent,
        >,
    ) {
        tokio::spawn(Self::load(
            path,
            asset_index,
            loaded_sender,
            denpendency_request_sender,
        ));
    }
}

#[derive(Debug)]
pub struct TextAssetsSystem {
    assets: Assets<Self>,
}
impl TextAssetsSystem {
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

impl AssetsHandler for TextAssetsSystem {
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

impl Default for TextAssetsSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetsSystem for TextAssetsSystem {
    type AssetType = String;

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
