mod asset;
use std::{any::TypeId, collections::HashMap, path::PathBuf, sync::Arc};

use crate::asset_loader::assets::asset::{AssetsHandler, AssetsSystem};

pub use asset::AssetHandle;
pub use asset::TextureAssetsSystem;

pub struct AssetsServer {
    handlers: HashMap<TypeId, Box<dyn AssetsHandler>>,
}

impl AssetsServer {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    pub fn push<T>(&mut self, asset_handler: T)
    where
        T: AssetsHandler + 'static,
    {
        let type_id = TypeId::of::<T>();
        self.handlers.insert(type_id, Box::new(asset_handler));
    }

    pub fn load<T>(&mut self, path: PathBuf) -> Arc<AssetHandle<T::DropEvent>>
    where
        T: AssetsSystem + 'static,
    {
        let handler = self.get_handler_mut::<T>();
        let Some(handler) = handler else {
            unreachable!()
        };

        let assets = handler.get_assets_mut();
        assets.load(path)
    }

    pub fn get<T>(&self, handle: &AssetHandle<T::DropEvent>) -> Option<&T::AssetType>
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

    pub fn get_mut<T>(&mut self, handle: &AssetHandle<T::DropEvent>) -> Option<&mut T::AssetType>
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
