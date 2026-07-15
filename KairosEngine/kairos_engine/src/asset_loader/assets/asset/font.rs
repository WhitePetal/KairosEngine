use std::path::PathBuf;

use anyhow::Error;
use tokio::sync::mpsc;

use crate::{
    asset_loader::{
        assets::{
            DependencyLoadRequestEvent,
            asset::{self, AssetIndex, Assets, AssetsHandler, AssetsSystem},
        },
        consts,
    },
    kairos_ui::font::Font,
};

#[derive(Debug)]
pub struct LoadedEvent {
    index: AssetIndex,
    asset: Font,
}
impl asset::LoadedEvent<Font> for LoadedEvent {
    fn get_index(&self) -> AssetIndex {
        self.index
    }

    fn get_asset(self) -> Font {
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
        _denpendency_request_sender: mpsc::Sender<DependencyLoadRequestEvent>,
    ) -> Result<(), Error> {
        let bytes = tokio::fs::read(path.clone()).await?;
        let font = Font { bytes };

        sender
            .send(LoadedEvent {
                index: asset_index,
                asset: font,
            })
            .await?;
        Ok(())
    }
}
impl asset::AssetLoader<LoadedEvent, Font> for Loader {
    fn load_asset(
        &self,
        path: std::path::PathBuf,
        asset_index: AssetIndex,
        sender: mpsc::Sender<LoadedEvent>,
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
pub struct FontAssetsSystem {
    assets: Assets<Self>,
}
impl FontAssetsSystem {
    pub fn new() -> Self {
        let loader = Loader {};
        let assets = Assets::<Self>::new(
            loader,
            consts::FONT_ASSETS_CAPACITY,
            consts::FONT_ASSETS_LOADED_CHANNEL_BUFFER_SIZE,
            consts::FONT_ASSETS_DROP_CHANNEL_BUFFER_SIZE,
        );
        Self { assets }
    }
}

impl AssetsHandler for FontAssetsSystem {
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

impl Default for FontAssetsSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetsSystem for FontAssetsSystem {
    type AssetType = Font;

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
