//! The loader registry: which [`AssetLoader`]s the server knows about, indexed
//! by name, asset type, and file extension.
//!
//! This is a deliberately small port of `bevy_asset`'s `AssetLoaders`: kairos
//! has no `TypePath` (a loader is named by [`loader_name`]). A loader slot can
//! be *pre-registered* through [`AssetLoaders::reserve`], which installs a
//! [`MaybeAssetLoader::Pending`] placeholder whose lookups block until the real
//! loader is [`pushed`](AssetLoaders::push).
//!
//! [`loader_name`]: crate::meta::loader_name

use core::any::TypeId;
use std::sync::Arc;

use kairos_collections::FixedHashMap as HashMap;
use thiserror::Error;

use crate::loader::{AssetLoader, ErasedAssetLoader};
use crate::meta::loader_name;
use crate::path::AssetPath;

use super::io_task_pool;

/// Every loader registered with the server.
#[derive(Default)]
pub(crate) struct AssetLoaders {
    /// The loader slots, indexed by registration order. A slot is either a real
    /// loader or a pending placeholder awaiting [`AssetLoaders::push`].
    loaders: Vec<MaybeAssetLoader>,
    /// Which loader indices produce each asset type.
    type_id_to_loaders: HashMap<TypeId, Vec<usize>>,
    /// Which loader indices declare each file extension.
    extension_to_loaders: HashMap<Box<str>, Vec<usize>>,
    /// Which loader index answers to each loader name.
    name_to_loader: HashMap<&'static str, usize>,
    /// Which loader index was reserved by [`AssetLoaders::reserve`] for a name
    /// that has not been registered yet.
    name_to_preregistered_loader: HashMap<&'static str, usize>,
}

impl AssetLoaders {
    /// Registers `loader`, indexing it by name, asset type, and extensions.
    ///
    /// Later registrations win when a lookup is ambiguous, mirroring bevy's
    /// "last one registered" resolution. A loader with a name that was
    /// [`reserve`d](AssetLoaders::reserve) fills its placeholder slot in place
    /// and wakes any lookup waiting on it.
    pub(crate) fn push<L: AssetLoader>(&mut self, loader: L) {
        let type_name = loader_name::<L>();
        let loader: Arc<dyn ErasedAssetLoader> = Arc::new(loader);

        let (index, is_new) = if let Some(index) = self.name_to_preregistered_loader.remove(type_name)
        {
            (index, false)
        } else {
            (self.loaders.len(), true)
        };

        if is_new {
            self.name_to_loader.insert(type_name, index);
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

            self.loaders.push(MaybeAssetLoader::Ready(loader));
        } else {
            let placeholder = core::mem::replace(
                self.loaders
                    .get_mut(index)
                    .expect("a preregistered loader occupies its reserved slot"),
                MaybeAssetLoader::Ready(loader.clone()),
            );
            match placeholder {
                MaybeAssetLoader::Ready(_) => {
                    unreachable!("a preregistered slot is not ready until it is filled")
                }
                MaybeAssetLoader::Pending { sender, .. } => {
                    // Wake anyone awaiting the placeholder; the broadcast must
                    // run off the current thread or it could block on `await`.
                    io_task_pool()
                        .spawn(async move {
                            let _ = sender.broadcast(loader).await;
                        })
                        .detach();
                }
            }
        }
    }

    /// Pre-registers a loader slot for `extensions` and `L`'s asset type.
    ///
    /// Lookups that resolve to the placeholder block until a loader named
    /// [`loader_name::<L>()`](crate::meta::loader_name) is registered.
    pub(crate) fn reserve<L: AssetLoader>(&mut self, extensions: &[&str]) {
        let type_name = loader_name::<L>();
        let index = self.loaders.len();

        self.name_to_preregistered_loader
            .insert(type_name, index);
        self.name_to_loader.insert(type_name, index);

        for extension in extensions {
            self.extension_to_loaders
                .entry((*extension).into())
                .or_default()
                .push(index);
        }
        self.type_id_to_loaders
            .entry(TypeId::of::<L::Asset>())
            .or_default()
            .push(index);

        let (mut sender, receiver) = async_broadcast::broadcast(1);
        sender.set_overflow(true);
        self.loaders
            .push(MaybeAssetLoader::Pending { sender, receiver });
    }

    /// The loader slot stored at `index`.
    fn get_by_index(&self, index: usize) -> Option<MaybeAssetLoader> {
        self.loaders.get(index).cloned()
    }

    /// The loader registered under its [`type_name`](ErasedAssetLoader::type_name).
    pub(crate) fn get_by_name(&self, name: &str) -> Option<MaybeAssetLoader> {
        let index = *self.name_to_loader.get(name)?;
        self.get_by_index(index)
    }

    /// The most recently registered loader that produces `type_id`.
    pub(crate) fn get_by_type(&self, type_id: TypeId) -> Option<MaybeAssetLoader> {
        let index = self.type_id_to_loaders.get(&type_id)?.last().copied()?;
        self.get_by_index(index)
    }

    /// The most recently registered loader that declares `extension`.
    pub(crate) fn get_by_extension(&self, extension: &str) -> Option<MaybeAssetLoader> {
        let index = self.extension_to_loaders.get(extension)?.last().copied()?;
        self.get_by_index(index)
    }

    /// The most recently registered loader that declares the path's full or a
    /// secondary extension.
    pub(crate) fn get_by_path(&self, path: &AssetPath<'_>) -> Option<MaybeAssetLoader> {
        let extension = path.get_full_extension()?;

        core::iter::once(extension)
            .chain(AssetPath::iter_secondary_extensions(extension))
            .filter_map(|extension| self.extension_to_loaders.get(extension)?.last().copied())
            .find_map(|index| self.get_by_index(index))
    }

    /// Resolves the loader that should read `asset_path`.
    ///
    /// The lookup prefers the asset type (when the path has no label, since a
    /// labeled asset may have a different type), then the file's full and
    /// secondary extensions, and finally falls back to the type's last loader.
    pub(crate) fn find(
        &self,
        asset_type_id: Option<TypeId>,
        asset_path: &AssetPath<'_>,
    ) -> Option<MaybeAssetLoader> {
        // A label names a labeled asset, whose type may differ from the
        // loader's asset type, so only the extension is trustworthy there.
        let candidates = if asset_path.label().is_none() {
            asset_type_id.and_then(|type_id| self.type_id_to_loaders.get(&type_id))
        } else {
            None
        };

        if let Some(candidates) = candidates {
            if candidates.is_empty() {
                return None;
            } else if candidates.len() == 1 {
                return self.get_by_index(candidates[0]);
            }
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
            for extension in AssetPath::iter_secondary_extensions(full_extension) {
                if let Some(index) = try_extension(extension) {
                    return self.get_by_index(index);
                }
            }
        }

        candidates?
            .last()
            .copied()
            .and_then(|index| self.get_by_index(index))
    }
}

/// A loader slot: either ready, or a pre-registered placeholder that yields the
/// loader once it is registered.
#[derive(Clone)]
pub(crate) enum MaybeAssetLoader {
    /// The loader is registered and can be handed out immediately.
    Ready(Arc<dyn ErasedAssetLoader>),
    /// A placeholder installed by [`AssetLoaders::reserve`]; `get` waits on the
    /// broadcast receiver until [`AssetLoaders::push`] fills the slot.
    Pending {
        /// Broadcasts the real loader when it is registered.
        sender: async_broadcast::Sender<Arc<dyn ErasedAssetLoader>>,
        /// Receives the real loader once it is registered.
        receiver: async_broadcast::Receiver<Arc<dyn ErasedAssetLoader>>,
    },
}

impl MaybeAssetLoader {
    /// Resolves to the loader, waiting out a pending placeholder if necessary.
    pub(crate) async fn get(self) -> Result<Arc<dyn ErasedAssetLoader>, GetLoaderError> {
        match self {
            MaybeAssetLoader::Ready(loader) => Ok(loader),
            MaybeAssetLoader::Pending { mut receiver, .. } => Ok(receiver.recv().await?),
        }
    }
}

/// An error from [`MaybeAssetLoader::get`].
#[derive(Error, Debug)]
pub(crate) enum GetLoaderError {
    /// The loader's placeholder was dropped before a real loader was registered.
    #[error(transparent)]
    CouldNotResolve(#[from] async_broadcast::RecvError),
}

#[cfg(test)]
mod tests {
    use super::*;

    use futures_lite::future::block_on;
    use kairos_tasks::ConditionalSendFuture;

    use crate::asset::{Asset, VisitAssetDependencies};
    use crate::io::Reader;
    use crate::loader::LoadContext;

    #[derive(Debug)]
    struct AssetA;
    impl Asset for AssetA {}
    impl VisitAssetDependencies for AssetA {}

    #[derive(Debug)]
    struct AssetB;
    impl Asset for AssetB {}
    impl VisitAssetDependencies for AssetB {}

    struct LoaderA;
    impl AssetLoader for LoaderA {
        type Asset = AssetA;
        type Settings = ();
        type Error = std::io::Error;

        fn load(
            &self,
            _reader: &mut dyn Reader,
            _settings: &(),
            _load_context: &mut LoadContext,
        ) -> impl ConditionalSendFuture<Output = Result<AssetA, std::io::Error>> {
            async { Ok(AssetA) }
        }

        fn extensions(&self) -> &[&str] {
            &["a"]
        }
    }

    struct LoaderB;
    impl AssetLoader for LoaderB {
        type Asset = AssetB;
        type Settings = ();
        type Error = std::io::Error;

        fn load(
            &self,
            _reader: &mut dyn Reader,
            _settings: &(),
            _load_context: &mut LoadContext,
        ) -> impl ConditionalSendFuture<Output = Result<AssetB, std::io::Error>> {
            async { Ok(AssetB) }
        }

        fn extensions(&self) -> &[&str] {
            &["b"]
        }
    }

    #[test]
    fn registered_loaders_resolve_by_every_index() {
        let mut loaders = AssetLoaders::default();
        loaders.push(LoaderA);
        loaders.push(LoaderB);

        let path = AssetPath::from("thing.a");

        assert!(
            block_on(
                loaders
                    .get_by_name(loader_name::<LoaderA>())
                    .unwrap()
                    .get()
            )
            .is_ok()
        );

        let a = block_on(loaders.get_by_type(TypeId::of::<AssetA>()).unwrap().get()).unwrap();
        assert_eq!(a.extensions(), &["a"]);
        let b = block_on(loaders.get_by_type(TypeId::of::<AssetB>()).unwrap().get()).unwrap();
        assert_eq!(b.extensions(), &["b"]);

        assert!(block_on(loaders.get_by_extension("a").unwrap().get()).is_ok());
        assert!(block_on(loaders.get_by_path(&path).unwrap().get()).is_ok());
        assert!(
            block_on(
                loaders
                    .find(Some(TypeId::of::<AssetA>()), &path)
                    .unwrap()
                    .get()
            )
            .is_ok()
        );
    }

    #[test]
    fn preregistered_loader_resolves_once_the_slot_is_filled() {
        let mut loaders = AssetLoaders::default();
        loaders.reserve::<LoaderA>(&["a"]);

        // Before registration the reserved slot is pending, not missing: a
        // lookup finds it and its `get` blocks on the broadcast.
        let pending = loaders
            .get_by_extension("a")
            .expect("the reserved slot is indexed");
        assert!(matches!(&pending, MaybeAssetLoader::Pending { .. }));

        loaders.push(LoaderA);

        let loader = block_on(pending.get()).expect("the pending loader resolves after registration");
        assert_eq!(loader.extensions(), &["a"]);
    }

    #[test]
    fn unknown_extension_and_type_have_no_slot() {
        let loaders = AssetLoaders::default();

        assert!(loaders.get_by_extension("nope").is_none());
        assert!(loaders.get_by_type(TypeId::of::<AssetA>()).is_none());
        assert!(loaders.get_by_name("nope").is_none());
        assert!(loaders.get_by_path(&AssetPath::from("thing.nope")).is_none());
        assert!(loaders.find(None, &AssetPath::from("thing.nope")).is_none());
    }
}
