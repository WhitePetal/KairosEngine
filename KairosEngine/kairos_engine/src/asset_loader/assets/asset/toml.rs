use std::{path::PathBuf};

use anyhow::Error;
use toml::Table;

use crate::asset_loader::assets::{DependencyLoadRequestEvent, asset::{self, AssetIndex, AssetLoader}};


#[derive(Debug)]
pub struct LoadedEvent {
    index: AssetIndex,
    table: Table
}
impl asset::LoadedEvent<Table> for LoadedEvent {
    fn get_index(&self) -> AssetIndex {
        self.index
    }

    fn get_asset(self) -> Table {
        self.table
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
        let toml = tokio::fs::read(path.clone()).await?;
        let table: Table = toml::from_slice(&toml)?;

        sender
            .send(LoadedEvent {
                index: asset_index,
                table,
            })
            .await?;
        Ok(())
    }
}
impl AssetLoader<LoadedEvent, Table> for Loader {
    fn load_asset(
        &self,
        path: std::path::PathBuf,
        asset_index: AssetIndex,
        loaded_sender: tokio::sync::mpsc::Sender<LoadedEvent>,
        // on_completed: Option<impl FnOnce(&mut A) -> () + Send + Sync + 'static>,
        denpendency_request_sender: tokio::sync::mpsc::Sender<crate::asset_loader::assets::DependencyLoadRequestEvent>,
    ) {
        tokio::spawn(Self::load(
            path,
            asset_index,
            loaded_sender,
            denpendency_request_sender,
        ));
    }
}