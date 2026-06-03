mod asset;
use std::{any::TypeId, collections::HashMap, path::PathBuf, sync::Arc};

use crate::asset_loader::assets::asset::{AssetsHandler, AssetsSystem};

pub use asset::AssetHandle;
pub use asset::TextureAssetsSystem;
pub use asset::ShaderAssetsSystem;
pub use asset::MaterialAssetsSystem;
pub use asset::MeshAssetsSystem;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

type DependencyLoadRequestEvent = Box<dyn DependencyLoadSetBack>;

pub trait DependencyLoadSetBack: Send + Sync {
    fn set_back(self: Box<Self>, assets_server: &mut AssetsServer);
}
pub struct DependencyLoadRequest<T> where T: AssetsSystem {
    dependency_path: PathBuf,
    setback_sender: oneshot::Sender<Arc<AssetHandle<T>>>,
}
impl<T> DependencyLoadSetBack for DependencyLoadRequest<T> where T: AssetsSystem {
    fn set_back(self: Box<Self>, assets_server: &mut AssetsServer) {
        let handle = assets_server.load::<T>(self.dependency_path);
        let _ = self.setback_sender.send(handle);
    }
}



pub struct AssetsServer {
    handlers: HashMap<TypeId, Box<dyn AssetsHandler>>,
    dependency_request_recever: mpsc::Receiver<DependencyLoadRequestEvent>,
    dependency_request_sender: mpsc::Sender<DependencyLoadRequestEvent>,
}

impl AssetsServer {
    pub fn new() -> Self {
        let (dependency_request_sender, dependency_request_recever) = mpsc::channel::<DependencyLoadRequestEvent>(32);
        Self {
            handlers: HashMap::new(),
            dependency_request_recever,
            dependency_request_sender,
        }
    }

    pub fn push<T>(&mut self, system: T) where T: AssetsSystem
    {
        let type_id = TypeId::of::<T>();
        self.handlers.insert(type_id, Box::new(system));
    }

    pub fn load<T>(&mut self, path: PathBuf) -> Arc<AssetHandle<T>>
    where
        T: AssetsSystem + 'static,
    {
        let sender = self.dependency_request_sender.clone();

        let handler = self.get_handler_mut::<T>();
        let Some(handler) = handler else {
            unreachable!()
        };

        let assets = handler.get_assets_mut();
        assets.load(path, sender)
    }

    pub fn get<T>(&self, handle: &AssetHandle<T>) -> Option<&T::AssetType>
    where
        T: AssetsSystem + 'static,
    {
        let handler = self.get_handler::<T>();
        let Some(handler) = handler else {
            return None;
        };

        let assets = handler.get_assets();
        assets.get(handle)
    }

    pub fn get_mut<T>(&mut self, handle: &AssetHandle<T>) -> Option<&mut T::AssetType>
    where
        T: AssetsSystem + 'static,
    {
        let handler = self.get_handler_mut::<T>();
        let Some(handler) = handler else {
            return None;
        };

        let assets = handler.get_assets_mut();
        assets.get_mut(handle)
    }

    pub fn handle(&mut self) {
        for handler in self.handlers.values_mut() {
            handler.handle_receves();
        }

        while let Ok(recv) = self.dependency_request_recever.try_recv() {
            recv.set_back(self);
        }
    }

    fn get_handler<T>(&self) -> Option<&T>
    where
        T: AssetsHandler + 'static,
    {
        self.handlers
            .get(&TypeId::of::<T>())
            .and_then(|handler| handler.as_any().downcast_ref())
    }

    fn get_handler_mut<T>(&mut self) -> Option<&mut T>
    where
        T: AssetsHandler + 'static,
    {
        self.handlers
            .get_mut(&TypeId::of::<T>())
            .and_then(|handler| handler.as_any_mut().downcast_mut())
    }
}
