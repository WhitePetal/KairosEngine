use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Weak},
};

use anyhow::Error;
use crossbeam_channel::{Receiver, Sender};
use tokio::{fs::File, io::AsyncReadExt};

use crate::graphics::texture::TextureAsset;

#[derive(Clone, Copy)]
pub struct AssetIndex {
    index: u32,
    version: u32,
}
impl AssetIndex {
    pub fn new(index: u32) -> Self {
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
    Some { value: T, version: u32 },
}

pub struct TextureAssets {
    storages: Vec<Entry<TextureAsset>>,
    infos: Vec<Option<AssetInfo>>,
    recyled_indexs: Vec<RecyledAssetIndex>,
    path_to_index: HashMap<String, AssetIndex>,
    asset_drop_sender: Sender<DropEvent>,
    asset_drop_recver: Receiver<DropEvent>,
    head: u32,
}

impl TextureAssets {
    pub fn new(capacity: usize) -> Self {
        let (asset_drop_sender, asset_drop_recver) = crossbeam_channel::unbounded();
        Self {
            storages: Vec::with_capacity(capacity),
            infos: Vec::with_capacity(capacity),
            recyled_indexs: Vec::with_capacity(capacity),
            path_to_index: HashMap::with_capacity(capacity),
            asset_drop_sender,
            asset_drop_recver,
            head: 0,
        }
    }

    pub fn handle_recves(&mut self) {
        while let Ok(event) = self.asset_drop_recver.try_recv() {
            let index = event.index;
            let pos = index.index as usize;
            let info = self.infos[pos].as_mut().unwrap();
            if info.relive_drops > 0 {
                info.relive_drops = info.relive_drops - 1;
                continue;
            }
            self.storages[pos] = Entry::None;
            self.recyled_indexs.push(index.into());
            self.path_to_index.remove(&info.path);
            self.infos[pos] = None;
        }
    }

    pub async fn load(&mut self, path: String) -> Result<Arc<TextureHandle>, Error> {
        // let path: String = load_path.to_str().unwrap().into();
        if let Some(index) = self.path_to_index.get(&path) {
            let entry = &self.storages[index.index as usize];
            match entry {
                Entry::None => {}
                Entry::Some { version, .. } => {
                    if index.version == *version {
                        if let Some(info) = self.infos[index.index as usize].as_mut() {
                            if let Some(arc) = info.weak.upgrade() {
                                return Ok(arc);
                            } else {
                                info.relive_drops = info.relive_drops + 1;
                                let sender = self.asset_drop_sender.clone();
                                let handle = TextureHandle::new(*index, sender);
                                let handle = Arc::new(handle);
                                info.weak = Arc::downgrade(&handle);
                                return Ok(handle);
                            }
                        }
                    }
                }
            }
        }

        let texture = Self::load_asset(Path::new(&path)).await?;
        let sender = self.asset_drop_sender.clone();
        if let Some(index) = self.recyled_indexs.pop() {
            let index = index.into();
            let handle = TextureHandle::new(index, sender);
            let handle = Arc::new(handle);
            let info = Some(AssetInfo::new(Arc::downgrade(&handle), path.clone()));
            self.storages[index.index as usize] = Entry::Some {
                value: texture,
                version: index.version,
            };
            self.infos[index.index as usize] = info;
            self.path_to_index.insert(path, index);
            Ok(handle)
        } else {
            let index = AssetIndex::new(self.head);
            let handle = TextureHandle::new(index, sender);
            let handle = Arc::new(handle);
            let info = Some(AssetInfo::new(Arc::downgrade(&handle), path.clone()));
            self.storages.push(Entry::Some {
                value: texture,
                version: index.version,
            });
            self.infos.push(info);
            self.path_to_index.insert(path, index);
            self.head = self.head + 1;
            Ok(handle)
        }
    }

    pub fn get(&self, handle: &TextureHandle) -> Option<&TextureAsset> {
        let entry = &self.storages[handle.index.index as usize];
        match entry {
            Entry::None => None,
            Entry::Some { value, version } => {
                if handle.index.version == *version {
                    Some(&value)
                } else {
                    None
                }
            }
        }
    }

    async fn load_asset(path: &Path) -> Result<TextureAsset, Error> {
        let toml_bytes = {
            let mut toml_f = File::open(path).await?;
            let mut toml_bytes = Vec::<u8>::new();
            toml_f.read_to_end(&mut toml_bytes).await?;
            toml_bytes
        };
        let mut texture = toml::from_slice::<TextureAsset>(&toml_bytes)?;

        let bin_bytes = {
            let bin_path = path.with_extension("texture_bin");
            let mut bin_f = File::open(bin_path).await?;
            let mut bin_bytes = Vec::<u8>::new();
            bin_f.read_to_end(&mut bin_bytes).await?;
            bin_bytes
        };
        let data = rkyv::from_bytes::<Vec<u8>, rkyv::rancor::Error>(&bin_bytes)?;
        texture.texture.data = data;
        Ok(texture)
    }
}
