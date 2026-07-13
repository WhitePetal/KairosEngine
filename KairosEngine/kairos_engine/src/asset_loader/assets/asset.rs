mod audio;
mod material;
mod mesh;
mod shader;
mod texture;
mod toml;

use std::{
    any::Any,
    collections::HashMap,
    fmt::Debug,
    hash::Hash,
    path::PathBuf,
    sync::{Arc, Weak},
};
use tokio::sync::mpsc::{self};

pub use audio::AudioAssetHandle;
pub use audio::AudioAssetsSystem;
pub use material::MaterialAssetsSystem;
pub use mesh::MeshAssetsSystem;
pub use shader::ShaderAssetsSystem;
pub use texture::TextureAssetsSystem;
pub use toml::TomlTableAssetsSystem;

use crate::asset_loader::assets::DependencyLoadRequestEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AssetIndex {
    index: usize,
    version: u32,
}
impl AssetIndex {
    #[inline(always)]
    pub fn index(&self) -> usize {
        self.index
    }

    #[inline(always)]
    pub fn version(&self) -> u32 {
        self.version
    }
}
#[derive(Debug)]
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

pub trait DropEvent: Send {
    fn new(index: AssetIndex) -> Self;

    fn get_index(&self) -> AssetIndex;
}
pub trait LoadedEvent<T>: Send {
    fn get_index(&self) -> AssetIndex;

    fn get_asset(self) -> T;
}

#[derive(Debug)]
pub struct AssetHandle<T>
where
    T: AssetsSystem,
{
    index: AssetIndex,
    drop_sender: Option<mpsc::Sender<T::DropEvent>>,
}
impl<T> AssetHandle<T>
where
    T: AssetsSystem,
{
    pub fn new(index: AssetIndex, sender: mpsc::Sender<T::DropEvent>) -> Self {
        Self {
            index,
            drop_sender: Some(sender),
        }
    }

    pub fn id(&self) -> AssetIndex {
        self.index
    }
}
impl<T> Drop for AssetHandle<T>
where
    T: AssetsSystem,
{
    fn drop(&mut self) {
        if let Some(sender) = self.drop_sender.take() {
            let index = self.index;
            let event = T::DropEvent::new(index);
            match sender.try_send(event) {
                Ok(()) => {}
                Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                    // Receiver already dropped (e.g., during shutdown).
                    // No need to send — the asset will be cleaned up when
                    // Assets<System> is dropped.
                }
                Err(tokio::sync::mpsc::error::TrySendError::Full(event)) => {
                    // Channel buffer is full (rare with adequate buffer size).
                    // Fall back to spawning a task to send later.
                    tokio::spawn(async move {
                        let _ = sender.send(event).await;
                    });
                }
            }
        }
    }
}
impl<T> PartialEq for AssetHandle<T>
where
    T: AssetsSystem,
{
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}
impl<T> Eq for AssetHandle<T> where T: AssetsSystem {}
impl<T> Hash for AssetHandle<T>
where
    T: AssetsSystem,
{
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.index.hash(state);
    }
}

#[derive(Debug)]
struct AssetInfo<T>
where
    T: AssetsSystem,
{
    weak: Weak<AssetHandle<T>>,
    relive_drops: usize,
    path: PathBuf,
}
impl<T> AssetInfo<T>
where
    T: AssetsSystem,
{
    pub fn new(weak: Weak<AssetHandle<T>>, path: PathBuf) -> Self {
        Self {
            weak,
            relive_drops: 0,
            path,
        }
    }
}

#[derive(Debug)]
pub enum Entry<T> {
    None,
    Loading { version: u32 },
    Some { value: T, version: u32 },
}

#[derive(Debug)]
struct CounterHeader(usize);
impl CounterHeader {
    fn next(&mut self) -> AssetIndex {
        let index = AssetIndex {
            index: self.0,
            version: 0,
        };
        self.0 = self.0 + 1;
        index
    }
}

pub trait AssetsHandler: Any + Debug {
    fn handle_receves(&mut self);

    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub trait AssetsSystem: AssetsHandler + Default {
    type AssetType;
    type LoadedEvent: LoadedEvent<Self::AssetType>;
    type DropEvent: DropEvent;
    type Loader: AssetLoader<Self::LoadedEvent, Self::AssetType>;

    fn get_assets(&self) -> &Assets<Self>
    where
        Self: Sized;
    fn get_assets_mut(&mut self) -> &mut Assets<Self>
    where
        Self: Sized;
}

pub trait AssetLoader<T, A> {
    fn load_asset(
        &self,
        path: PathBuf,
        asset_index: AssetIndex,
        loaded_sender: mpsc::Sender<T>,
        // on_completed: Option<impl FnOnce(&mut A) -> () + Send + Sync + 'static>,
        denpendency_request_sender: mpsc::Sender<DependencyLoadRequestEvent>,
    );
}

#[derive(Debug)]
pub struct Assets<System>
where
    System: AssetsSystem,
{
    storages: Vec<Entry<System::AssetType>>,
    infos: Vec<AssetInfo<System>>,
    recyled_indexs: Vec<RecyledAssetIndex>,
    path_to_index: HashMap<PathBuf, AssetIndex>,
    asset_loaded_sender: mpsc::Sender<System::LoadedEvent>,
    asset_loaded_recever: mpsc::Receiver<System::LoadedEvent>,
    asset_drop_sender: mpsc::Sender<System::DropEvent>,
    asset_drop_recever: mpsc::Receiver<System::DropEvent>,
    head: CounterHeader,
    loader: System::Loader,
}

impl<System> Assets<System>
where
    System: AssetsSystem,
{
    pub fn new(
        loader: System::Loader,
        capacity: usize,
        loaded_channel_buffer_size: usize,
        drop_channel_buffer_size: usize,
    ) -> Self {
        let (asset_loaded_sender, asset_loaded_recever) =
            tokio::sync::mpsc::channel(loaded_channel_buffer_size);
        let (asset_drop_sender, asset_drop_recever) =
            tokio::sync::mpsc::channel(drop_channel_buffer_size);
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
            loader,
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

    pub fn load(
        &mut self,
        path: PathBuf,
        // on_completed: Option<impl FnOnce(&mut System::AssetType) -> () + Send + Sync + 'static>,
        denpendency_request_sender: mpsc::Sender<DependencyLoadRequestEvent>,
    ) -> Arc<AssetHandle<System>> {
        let asset_index = self.load_asset_index(&path);

        let asset_handle = self.load_asset_handle(
            &path,
            asset_index,
            // on_completed,
            denpendency_request_sender,
        );

        asset_handle
    }

    fn load_asset_index(&mut self, path: &PathBuf) -> AssetIndex {
        if let Some(index) = self.path_to_index.get(path) {
            *index
        } else {
            let index = self.alloc_slot();
            self.path_to_index.insert(path.clone(), index);
            index
        }
    }

    fn load_asset_handle(
        &mut self,
        path: &PathBuf,
        asset_index: AssetIndex,
        // on_completed: Option<impl FnOnce(&mut System::AssetType) -> () + Send + Sync + 'static>,
        denpendency_request_sender: mpsc::Sender<DependencyLoadRequestEvent>,
    ) -> Arc<AssetHandle<System>> {
        match &self.storages[asset_index.index] {
            Entry::None => {
                let loaded_sender = self.asset_loaded_sender.clone();
                self.storages[asset_index.index] = Entry::Loading {
                    version: asset_index.version,
                };
                self.loader.load_asset(
                    PathBuf::from(path),
                    asset_index,
                    loaded_sender,
                    // on_completed,
                    denpendency_request_sender,
                );
                let (handle, info) = self.create_asset_handle(path, asset_index);
                self.infos.push(info);
                handle
            }
            Entry::Loading { version } | Entry::Some { version, .. } => {
                if *version < asset_index.version {
                    let loaded_sender = self.asset_loaded_sender.clone();
                    self.storages[asset_index.index] = Entry::Loading {
                        version: asset_index.version,
                    };
                    self.loader.load_asset(
                        PathBuf::from(path),
                        asset_index,
                        loaded_sender,
                        // on_completed,
                        denpendency_request_sender,
                    );
                    let (handle, info) = self.create_asset_handle(path, asset_index);
                    self.infos[asset_index.index] = info;
                    handle
                } else {
                    self.get_asset_handle(asset_index)
                }
            }
        }
    }

    #[inline(always)]
    fn create_asset_handle(
        &mut self,
        path: &PathBuf,
        asset_index: AssetIndex,
    ) -> (Arc<AssetHandle<System>>, AssetInfo<System>) {
        let sender = self.asset_drop_sender.clone();
        let handle = AssetHandle::new(asset_index, sender);
        let handle = Arc::new(handle);
        let info = AssetInfo::new(Arc::downgrade(&handle), path.clone());
        (handle, info)
    }

    #[inline(always)]
    fn get_asset_handle(&mut self, asset_index: AssetIndex) -> Arc<AssetHandle<System>> {
        let info = &mut self.infos[asset_index.index];
        if let Some(handle) = info.weak.upgrade() {
            handle
        } else {
            info.relive_drops = info.relive_drops + 1;
            let sender = self.asset_drop_sender.clone();
            let handle = AssetHandle::<System>::new(asset_index, sender);
            let handle = Arc::new(handle);
            info.weak = Arc::downgrade(&handle);
            handle
        }
    }

    pub fn get(&self, handle: &AssetHandle<System>) -> Option<&System::AssetType> {
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

    pub fn get_mut(&mut self, handle: &AssetHandle<System>) -> Option<&mut System::AssetType> {
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

    /// Allocate a new slot (from recycled or fresh) and return its AssetIndex.
    fn alloc_slot(&mut self) -> AssetIndex {
        if let Some(recycled) = self.recyled_indexs.pop() {
            let index: AssetIndex = recycled.into();
            self.storages[index.index] = Entry::None;
            index
        } else {
            self.storages.push(Entry::None);
            self.head.next()
        }
    }

    /// Insert a runtime-created asset directly, bypassing the async file loader.
    /// Returns an `Arc<AssetHandle>` that participates in the normal lifecycle.
    ///
    /// If `key` is provided, the same key always returns the same handle (dedup).
    pub fn insert(&mut self, asset: System::AssetType, path: PathBuf) -> Arc<AssetHandle<System>> {
        // 1. If a key is provided and already exists, return existing handle.
        if let Some(&existing_index) = self.path_to_index.get(&path) {
            return self.get_asset_handle(existing_index);
        }

        // 2. Allocate a slot.
        let asset_index = self.alloc_slot();

        // 3. Store immediately as Some (no async loading).
        self.storages[asset_index.index] = Entry::Some {
            value: asset,
            version: asset_index.version,
        };

        // 4. Register key if provided.
        self.path_to_index.insert(path.clone(), asset_index);

        // 5. Create the handle and info (no path).
        let (handle, info) = self.create_asset_handle(&path, asset_index);
        self.infos.push(info);
        handle
    }
}
