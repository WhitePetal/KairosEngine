use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Weak},
};

use anyhow::Error;
use tokio::sync::mpsc::{Receiver, Sender};

use crate::{asset_loader::{consts}, graphics::texture::TextureAsset};

#[derive(Clone, Copy)]
pub struct AssetIndex {
    index: usize,
    version: u32,
}
impl AssetIndex {
    pub fn new(index: usize) -> Self {
        Self { index, version: 0 }
    }
}
impl From<RecyledAssetIndex> for AssetIndex {
    #[inline(always)]
    fn from(value: RecyledAssetIndex) -> Self {
        Self {
            index: value.0.index,
            version: value.0.version,
        }
    }
}

pub struct RecyledAssetIndex(AssetIndex);
impl From<AssetIndex> for RecyledAssetIndex {
    #[inline(always)]
    fn from(value: AssetIndex) -> Self {
        let mut index = value;
        index.version = index.version + 1;
        Self(index)
    }
}

pub struct TextureHandle {
    index: AssetIndex,
    drop_sender: Sender<DropEvent>,
}
impl TextureHandle {
    pub fn new(index: AssetIndex, sender: Sender<DropEvent>) -> Self {
        Self {
            index,
            drop_sender: sender,
        }
    }
}
impl Drop for TextureHandle {
    fn drop(&mut self) {
        let index = self.index;
        let _ = self.drop_sender.send(DropEvent { index });
    }
}

pub struct LoadedEvent {
    index: AssetIndex,
    asset: TextureAsset,
}

pub struct DropEvent {
    index: AssetIndex,
}

pub struct AssetInfo {
    weak: Weak<TextureHandle>,
    relive_drops: usize,
    path: String,
}
impl AssetInfo {
    pub fn new(weak: Weak<TextureHandle>, path: String) -> Self {
        Self {
            weak,
            relive_drops: 0,
            path,
        }
    }
}

pub enum Entry<T> {
    None,
    Loading { version: u32 },
    Some { value: T, version: u32 },
}

struct CountHeader(usize);
impl CountHeader {
    fn next(&mut self) -> AssetIndex {
        let index = AssetIndex { index: self.0, version: 0 };
        self.0 = self.0 + 1;
        index
    }
}

pub struct TextureAssets {
    storages: Vec<Entry<TextureAsset>>,
    infos: Vec<AssetInfo>,
    recyled_indexs: Vec<RecyledAssetIndex>,
    path_to_index: HashMap<String, AssetIndex>,
    asset_loaded_sender: Sender<LoadedEvent>,
    asset_loaded_recever: Receiver<LoadedEvent>,
    asset_drop_sender: Sender<DropEvent>,
    asset_drop_recever: Receiver<DropEvent>,
    head: CountHeader,
}

impl TextureAssets {
    pub fn new(capacity: usize) -> Self {
        let (asset_loaded_sender, asset_loaded_recever) = tokio::sync::mpsc::channel(consts::TEXTURE_ASSETS_LOADED_CHANNEL_BUFFER_SIZE);
        let (asset_drop_sender, asset_drop_recever) = tokio::sync::mpsc::channel(consts::TEXTURE_ASSETS_DROP_CHANNEL_BUFFER_SIZE);
        Self {
            storages: Vec::with_capacity(capacity),
            infos: Vec::with_capacity(capacity),
            recyled_indexs: Vec::with_capacity(capacity),
            path_to_index: HashMap::with_capacity(capacity),
            asset_loaded_sender,
            asset_loaded_recever,
            asset_drop_sender,
            asset_drop_recever,
            head: CountHeader(0),
        }
    }

    pub fn handle_recves(&mut self) {
        while let Ok(event) = self.asset_drop_recever.try_recv() {
            let index = event.index;
            let pos = index.index as usize;
            let info = &mut self.infos[pos];
            if info.relive_drops > 0 {
                info.relive_drops = info.relive_drops - 1;
                continue;
            }
            self.storages[pos] = Entry::None;
            self.recyled_indexs.push(index.into());
            self.path_to_index.remove(&info.path);
        }

        while let Ok(event) = self.asset_loaded_recever.try_recv() {
            self.storages[event.index.index as usize] = Entry::Some {
                value: event.asset,
                version: event.index.version,
            };
        }
    }

    pub fn load(&mut self, path: String) -> Arc<TextureHandle> {
        let asset_index = self.load_asset_index(&path);

        let asset_handle = self.load_asset_handle(&path, asset_index);

        asset_handle
    }

    fn load_asset_index(&mut self, path: &str) -> AssetIndex {
        if let Some(index) = self.path_to_index.get(path) {
            *index
        } else {
            let index = {
                if let Some(index) = self.recyled_indexs.pop() {
                    let index: AssetIndex = index.into();
                    self.storages[index.index as usize] = Entry::None;
                    index
                } else {
                    self.storages.push(Entry::None);
                    let index = self.head.next();
                    index
                }
            };

            self.path_to_index.insert(path.to_string(), index);
            index
        }
    }

    fn load_asset_handle(&mut self, path: &str, asset_index: AssetIndex) -> Arc<TextureHandle> {
        match &self.storages[asset_index.index as usize] {
            Entry::None => {
                let loaded_sender = self.asset_loaded_sender.clone();
                tokio::spawn(Self::load_asset(PathBuf::from(path), asset_index, loaded_sender));
                let (handle, info) = self.create_asset_handle(path, asset_index);
                self.infos.push(info);
                handle
            },
            Entry::Loading { version } | Entry::Some { version , ..} => {
                if *version < asset_index.version {
                    let loaded_sender = self.asset_loaded_sender.clone();
                     tokio::spawn(Self::load_asset(PathBuf::from(path), asset_index, loaded_sender));
                    let (handle, info) = self.create_asset_handle(path, asset_index);
                    self.infos[asset_index.index] = info;
                    handle
                } else {
                    self.get_asset_handle(asset_index)
                }
            },
        }
    }

    #[inline(always)]
    fn create_asset_handle(&mut self, path: &str, asset_index: AssetIndex) -> (Arc<TextureHandle>, AssetInfo) {
        let sender = self.asset_drop_sender.clone();
        let handle = TextureHandle::new(asset_index, sender);
        let handle = Arc::new(handle);
        let info = AssetInfo::new(Arc::downgrade(&handle), path.to_string());
        (handle, info)
    }

    #[inline(always)]
    fn get_asset_handle(&mut self, asset_index: AssetIndex) -> Arc<TextureHandle> {
        let info = &mut self.infos[asset_index.index];
        if let Some(handle) = info.weak.upgrade() {
            handle
        } else {
            info.relive_drops = info.relive_drops + 1;
            let sender = self.asset_drop_sender.clone();
            let handle = TextureHandle::new(asset_index, sender);
            let handle = Arc::new(handle);
            info.weak = Arc::downgrade(&handle);
            handle
        }
    }

    pub fn get(&self, handle: &TextureHandle) -> Option<&TextureAsset> {
        let entry = &self.storages[handle.index.index as usize];
        match entry {
            Entry::None | Entry::Loading { .. } => None,
            Entry::Some { value, version } => {
                if handle.index.version == *version {
                    Some(&value)
                } else {
                    None
                }
            }
        }
    }

    async fn load_asset(path: PathBuf, asset_index: AssetIndex, sender: Sender<LoadedEvent>) -> Result<(), Error> {
        let (texture, data) = tokio::join!(
            Self::load_toml(&path),
            Self::load_bin(&path),
        );
        let mut texture = texture?;
        let data = data?;

        texture.texture.data = data;

        sender.send(LoadedEvent { index: asset_index, asset: texture }).await?;
        Ok(())
    }

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
}
