use std::{any::Any, collections::HashMap, path::PathBuf, pin::Pin, sync::{Arc, Weak}};

use anyhow::Error;
use tokio::sync::mpsc::{Sender, Receiver};



#[derive(Clone, Copy)]
pub struct AssetIndex {
    index: usize,
    version: u32,
}
pub struct RecyledAssetIndex(AssetIndex);
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
impl From<AssetIndex> for RecyledAssetIndex {
    #[inline(always)]
    fn from(value: AssetIndex) -> Self {
        let mut index = value;
        index.version = index.version + 1;
        Self(index)
    }
}


pub trait DropEvent {
    fn new(index: AssetIndex) -> Self;

    fn get_index(&self) -> AssetIndex;
}
pub trait LoadedEvent<T> {
    fn new(index: AssetIndex, asset: T) -> Self;

    fn get_index(&self) -> AssetIndex;

    fn get_asset(self) -> T;
}

pub struct AssetHandle<T> where T: DropEvent {
    index: AssetIndex,
    drop_sender: Sender<T>,
}
impl<T> AssetHandle<T> where T: DropEvent {
    pub fn new(index: AssetIndex, sender: Sender<T>) -> Self {
        Self { index, drop_sender: sender }
    }
}
impl<T> Drop for AssetHandle<T> where T: DropEvent {
    fn drop(&mut self) {
        let index = self.index;
        let _ = self.drop_sender.send(T::new(index));
    }
}

struct AssetInfo<T> where T: DropEvent {
    weak: Weak<AssetHandle<T>>,
    relive_drops: usize,
    path: PathBuf,
}
impl<T> AssetInfo<T> where T: DropEvent {
    pub fn new(weak: Weak<AssetHandle<T>>, path: PathBuf) -> Self {
        Self { weak, relive_drops: 0, path }
    }
}

pub enum Entry<T> {
    None,
    Loading { version: u32 },
    Some { value: T, version: u32 },
}

struct CounterHeader(usize);
impl CounterHeader {
    fn next(&mut self) -> AssetIndex {
        let index = AssetIndex { index: self.0, version: 0 };
        self.0 = self.0 + 1;
        index
    }
}

pub trait AssetsHandler: Any {
    fn handle_receves(&mut self);

    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub trait AssetsSystem: AssetsHandler {
    type AssetType;
    type LoadedEvent: LoadedEvent<Self::AssetType>;
    type DropEvent: DropEvent;
    type Loader: AssetLoader<Self::LoadedEvent>;

    fn get_assets(&self) -> &Assets<Self::AssetType, Self::LoadedEvent, Self::DropEvent, Self::Loader>;
    fn get_assets_mut(&mut self) -> &mut Assets<Self::AssetType, Self::LoadedEvent, Self::DropEvent, Self::Loader>;
}

pub trait AssetLoader<T> {
    fn load_asset(&self, path: PathBuf, asset_index: AssetIndex, sender: Sender<T>) 
        -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'static>>;
}

pub struct Assets<V, L, D, Loader> where L: LoadedEvent<V>, D: DropEvent, Loader: AssetLoader<L> {
    storages: Vec<Entry<V>>,
    infos: Vec<AssetInfo<D>>,
    recyled_indexs: Vec<RecyledAssetIndex>,
    path_to_index: HashMap<PathBuf, AssetIndex>,
    asset_loaded_sender: Sender<L>,
    asset_loaded_recever: Receiver<L>,
    asset_drop_sender: Sender<D>,
    asset_drop_recever: Receiver<D>,
    head: CounterHeader,
    loader: Loader,
}

impl<V, L, D, Loader> Assets<V, L, D, Loader> where L: LoadedEvent<V>, D: DropEvent, Loader: AssetLoader<L> {
    pub fn new(loader: Loader, capacity: usize, loaded_channel_buffer_size: usize, drop_channel_buffer_size: usize) -> Self {
        let (asset_loaded_sender, asset_loaded_recever) = tokio::sync::mpsc::channel(loaded_channel_buffer_size);
        let (asset_drop_sender, asset_drop_recever) = tokio::sync::mpsc::channel(drop_channel_buffer_size);
        Self {
            storages: Vec::with_capacity(capacity),
            infos: Vec::with_capacity(capacity),
            recyled_indexs: Vec::with_capacity(capacity),
            path_to_index: HashMap::with_capacity(capacity),
            asset_loaded_sender,
            asset_loaded_recever,
            asset_drop_sender,
            asset_drop_recever,
            head: CounterHeader(0),
            loader
        }
    }

    pub fn handle_receves(&mut self) {
        while let Ok(event) = self.asset_drop_recever.try_recv() {
            let index = event.get_index();
            let pos = index.index;
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
            let index = event.get_index();
            self.storages[index.index] = Entry::Some {
                value: event.get_asset(),
                version: index.version,
            };
        }
    }

    pub fn load(&mut self, path: PathBuf) -> Arc<AssetHandle<D>> {
        let asset_index = self.load_asset_index(&path);

        let asset_handle = self.load_asset_handle(&path, asset_index);

        asset_handle
    }

    fn load_asset_index(&mut self, path: &PathBuf) -> AssetIndex {
        if let Some(index) = self.path_to_index.get(path) {
            *index
        } else {
            let index = {
                if let Some(index) = self.recyled_indexs.pop() {
                    let index: AssetIndex = index.into();
                    self.storages[index.index] = Entry::None;
                    index
                } else {
                    self.storages.push(Entry::None);
                    let index = self.head.next();
                    index
                }
            };

            self.path_to_index.insert(path.clone(), index);
            index
        }
    }

    fn load_asset_handle(&mut self, path: &PathBuf, asset_index: AssetIndex) -> Arc<AssetHandle<D>> {
        match &self.storages[asset_index.index] {
            Entry::None => {
                let loaded_sender = self.asset_loaded_sender.clone();
                tokio::spawn(self.loader.load_asset(PathBuf::from(path), asset_index, loaded_sender));
                let (handle, info) = self.create_asset_handle(path, asset_index);
                self.infos.push(info);
                handle
            },
            Entry::Loading { version } | Entry::Some { version , ..} => {
                if *version < asset_index.version {
                    let loaded_sender = self.asset_loaded_sender.clone();
                     tokio::spawn(self.loader.load_asset(PathBuf::from(path), asset_index, loaded_sender));
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
    fn create_asset_handle(&mut self, path: &PathBuf, asset_index: AssetIndex) -> (Arc<AssetHandle<D>>, AssetInfo<D>) {
        let sender = self.asset_drop_sender.clone();
        let handle = AssetHandle::new(asset_index, sender);
        let handle = Arc::new(handle);
        let info = AssetInfo::new(Arc::downgrade(&handle), path.clone());
        (handle, info)
    }

    #[inline(always)]
    fn get_asset_handle(&mut self, asset_index: AssetIndex) -> Arc<AssetHandle<D>> {
        let info = &mut self.infos[asset_index.index];
        if let Some(handle) = info.weak.upgrade() {
            handle
        } else {
            info.relive_drops = info.relive_drops + 1;
            let sender = self.asset_drop_sender.clone();
            let handle = AssetHandle::<D>::new(asset_index, sender);
            let handle = Arc::new(handle);
            info.weak = Arc::downgrade(&handle);
            handle
        }
    }

    pub fn get(&self, handle: &AssetHandle<D>) -> Option<&V> {
        let entry = &self.storages[handle.index.index];
        match entry {
            Entry::None | Entry::Loading { .. } => None,
            Entry::Some { value, version } => {
                if handle.index.version == *version {
                    Some(value)
                } else {
                    None
                }
            }
        }
    }

    pub fn get_mut(&mut self, handle: &AssetHandle<D>) -> Option<&mut V> {
        let entry = &mut self.storages[handle.index.index];
        match entry {
            Entry::None | Entry::Loading { .. } => None,
            Entry::Some { value, version } => {
                if handle.index.version == *version {
                    Some(value)
                } else {
                    None
                }
            }
        }
    }
}