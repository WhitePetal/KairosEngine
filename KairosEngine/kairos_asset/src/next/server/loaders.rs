//! The loader registry: which [`AssetLoader`]s the server knows about, indexed
//! by name, asset type, and file extension.
//!
//! This is a deliberately small port of `bevy_asset`'s `AssetLoaders`: kairos
//! has no `TypePath` (a loader is named by [`loader_name`]), and the
//! pre-registration API (`preregister_loader`, used by bevy to block loads
//! until a loader is added) is out of scope, so there is no `Pending` loader
//! state and every registered loader is immediately ready.
//!
//! [`loader_name`]: crate::next::meta::loader_name

use core::any::TypeId;
use std::{collections::HashMap, sync::Arc};

use crate::next::loader::{AssetLoader, ErasedAssetLoader};
use crate::next::path::AssetPath;

/// Every loader registered with the server.
#[derive(Default)]
pub(crate) struct AssetLoaders {
    /// The loaders themselves, indexed by registration order.
    loaders: Vec<Arc<dyn ErasedAssetLoader>>,
    /// Which loader indices produce each asset type.
    type_id_to_loaders: HashMap<TypeId, Vec<usize>>,
    /// Which loader indices declare each file extension.
    extension_to_loaders: HashMap<Box<str>, Vec<usize>>,
    /// Which loader index answers to each loader name.
    name_to_loader: HashMap<&'static str, usize>,
}

impl AssetLoaders {
    /// Registers `loader`, indexing it by name, asset type, and extensions.
    ///
    /// Later registrations win when a lookup is ambiguous, mirroring bevy's
    /// "last one registered" resolution.
    pub(crate) fn push<L: AssetLoader>(&mut self, loader: L) {
        let loader: Arc<dyn ErasedAssetLoader> = Arc::new(loader);
        let index = self.loaders.len();

        self.name_to_loader.insert(loader.type_name(), index);
        for extension in loader.extensions() {
            self.extension_to_loaders
                .entry((*extension).into())
                .or_default()
                .push(index);
        }
        self.type_id_to_loaders
            .entry(loader.asset_type_id())
            .or_default()
            .push(index);

        self.loaders.push(loader);
    }

    /// The loader stored at `index`.
    fn get_by_index(&self, index: usize) -> Option<Arc<dyn ErasedAssetLoader>> {
        self.loaders.get(index).cloned()
    }

    /// The loader registered under its [`type_name`](ErasedAssetLoader::type_name).
    pub(crate) fn get_by_name(&self, name: &str) -> Option<Arc<dyn ErasedAssetLoader>> {
        let index = *self.name_to_loader.get(name)?;
        self.get_by_index(index)
    }

    /// Resolves the loader that should read `asset_path`.
    ///
    /// The lookup prefers the asset type (when the path has no label, since a
    /// labeled asset may have a different type), then the file's full and
    /// final extensions, and finally falls back to the type's last loader.
    pub(crate) fn find(
        &self,
        asset_type_id: Option<TypeId>,
        asset_path: &AssetPath<'_>,
    ) -> Option<Arc<dyn ErasedAssetLoader>> {
        // A label names a labeled asset, whose type may differ from the
        // loader's asset type, so only the extension is trustworthy there.
        let candidates = if asset_path.label().is_none() {
            asset_type_id.and_then(|type_id| self.type_id_to_loaders.get(&type_id))
        } else {
            None
        };

        if let Some(candidates) = candidates
            && candidates.len() == 1
        {
            return self.get_by_index(candidates[0]);
        }

        let try_extension = |extension: &str| -> Option<usize> {
            let indices = self.extension_to_loaders.get(extension)?;
            match candidates {
                Some(candidates) => indices
                    .iter()
                    .rev()
                    .find(|index| candidates.contains(index))
                    .copied(),
                None => indices.last().copied(),
            }
        };

        if let Some(full_extension) = asset_path.get_full_extension() {
            if let Some(index) = try_extension(full_extension) {
                return self.get_by_index(index);
            }
            // `config.ron` also matches a loader registered for `ron`.
            if let Some(extension) = asset_path.get_extension()
                && extension != full_extension
                && let Some(index) = try_extension(extension)
            {
                return self.get_by_index(index);
            }
        }

        candidates?.last().copied().and_then(|index| self.get_by_index(index))
    }
}
