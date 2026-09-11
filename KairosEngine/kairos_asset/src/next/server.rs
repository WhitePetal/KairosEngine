//! The asset server and the per-asset information it tracks.
//!
//! This is the loader face's state backbone: the [`AssetServer`] hands out
//! path-addressed [`Handle`]s (via [`AssetInfos`]) and records the
//! [`LoadState`] of every asset it knows about. The loader itself —
//! [`AssetLoader`](crate::next::loader::AssetLoader) and
//! [`LoadContext`](crate::next::loader::LoadContext) — reads the handle
//! registry and writes load state through here.
//!
//! Only the parts the loader face needs have landed so far: handle allocation
//! by path, and the three load states. The loading pipeline (task spawning over
//! `IoTaskPool`, meta/loader selection, `AssetServer`'s public API) is the next
//! ticket and extends this module. Deliberately, an asset's load state lives in
//! [`AssetInfos`] rather than as a world component: the store ([`Assets<A>`])
//! owns the values, the server owns what is known about loads.
//!
//! [`Assets<A>`]: crate::next::Assets

use core::any::{TypeId, type_name};
use std::{
    collections::{HashMap, hash_map::Entry},
    fmt,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use kairos_ecs::error::KairosError;
use kairos_ecs::resource::Resource;

use crate::next::asset::Asset;
use crate::next::handle::{AssetHandleProvider, Handle, UntypedHandle};
use crate::next::id::UntypedAssetId;
use crate::next::index::AssetIndexAllocator;
use crate::next::io::{
    AssetReaderError, MissingAssetSourceError, MissingProcessedAssetReaderError,
};
use crate::next::meta::DeserializeMetaError;
use crate::next::path::AssetPath;

/// The asset system's server: the single entry point for obtaining handles and
/// (later) loading assets.
///
/// It is a [`Clone`]able world resource that shares its state behind an
/// [`Arc`], so systems can hold it by value and load tasks can keep it alive
/// across threads. All mutation goes through the [`AssetInfos`] lock.
#[derive(Resource, Clone)]
pub struct AssetServer {
    data: Arc<AssetServerData>,
}

/// The shared state behind every [`AssetServer`] clone.
pub(crate) struct AssetServerData {
    /// What the server knows about each asset, keyed by id.
    pub(crate) infos: RwLock<AssetInfos>,
}

impl AssetServer {
    /// Creates a server with no handles, providers, or load state yet.
    pub fn new() -> Self {
        Self {
            data: Arc::new(AssetServerData {
                infos: RwLock::new(AssetInfos::default()),
            }),
        }
    }

    /// Locks the shared asset information for reading.
    pub(crate) fn read_infos(&self) -> RwLockReadGuard<'_, AssetInfos> {
        self.data.infos.read().expect("asset infos lock poisoned")
    }

    /// Locks the shared asset information for writing.
    pub(crate) fn write_infos(&self) -> RwLockWriteGuard<'_, AssetInfos> {
        self.data.infos.write().expect("asset infos lock poisoned")
    }

    /// Returns the strong handle for `path` and `A`, reserving one — and
    /// recording a fresh [`AssetInfo`] — if no live handle for that pair exists
    /// yet.
    pub(crate) fn get_or_create_path_handle<A: Asset>(
        &self,
        path: AssetPath<'static>,
    ) -> Handle<A> {
        self.write_infos().get_or_create_path_handle(path).0
    }

    /// The strong handles currently registered for `path`, across every asset
    /// type that has been loaded from it.
    pub(crate) fn get_handles_untyped(&self, path: &AssetPath<'_>) -> Vec<UntypedHandle> {
        self.read_infos().get_handles_untyped(path)
    }
}

impl Default for AssetServer {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for AssetServer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AssetServer").finish_non_exhaustive()
    }
}

/// What the server knows about one asset.
#[derive(Debug, Clone)]
pub(crate) struct AssetInfo {
    /// The path the asset was requested from, if it was loaded by path rather
    /// than added directly.
    pub(crate) path: Option<AssetPath<'static>>,
    /// How far the asset's own load has got.
    pub(crate) load_state: LoadState,
    /// How far its direct dependencies' loads have got.
    pub(crate) dep_load_state: DependencyLoadState,
    /// How far its recursive dependencies' loads have got.
    pub(crate) rec_dep_load_state: RecursiveDependencyLoadState,
}

impl AssetInfo {
    fn new(path: Option<AssetPath<'static>>) -> Self {
        Self {
            path,
            load_state: LoadState::NotLoaded,
            dep_load_state: DependencyLoadState::NotLoaded,
            rec_dep_load_state: RecursiveDependencyLoadState::NotLoaded,
        }
    }
}

/// The server's per-asset bookkeeping: path-addressed handles, the handle
/// providers that allocate them, and the load state of every known asset.
///
/// This is an internal type; callers reach it through [`AssetServer`] or, once
/// the loading pipeline lands, the server's public state accessors.
#[derive(Default)]
pub(crate) struct AssetInfos {
    /// Which id each `(path, asset type)` pair resolves to, so repeated
    /// requests for the same address reuse one handle.
    path_to_index: HashMap<AssetPath<'static>, HashMap<TypeId, UntypedAssetId>>,
    /// Every known asset's information, keyed by id.
    infos: HashMap<UntypedAssetId, AssetInfo>,
    /// The handle provider for each asset type. A type must have a provider
    /// before any handle for it can be allocated.
    handle_providers: HashMap<TypeId, AssetHandleProvider>,
}

impl fmt::Debug for AssetInfos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AssetInfos")
            .field("path_to_index", &self.path_to_index)
            .field("infos", &self.infos)
            .finish_non_exhaustive()
    }
}

impl AssetInfos {
    /// Registers a handle provider for `A`, so handles for that type can be
    /// allocated. Does nothing if one is already registered.
    #[allow(dead_code)] // consumed by the loading-pipeline ticket
    pub(crate) fn register_handle_provider<A: Asset>(&mut self) {
        self.handle_providers
            .entry(TypeId::of::<A>())
            .or_insert_with(|| {
                AssetHandleProvider::new(TypeId::of::<A>(), Arc::new(AssetIndexAllocator::default()))
            });
    }

    /// Returns the handle for `path` and `A`, creating one if necessary.
    ///
    /// The returned `bool` is `true` when a new handle was reserved and
    /// recorded, and `false` when an existing one was reused.
    pub(crate) fn get_or_create_path_handle<A: Asset>(
        &mut self,
        path: AssetPath<'static>,
    ) -> (Handle<A>, bool) {
        let (handle, created) =
            self.get_or_create_path_handle_erased(path, TypeId::of::<A>(), Some(type_name::<A>()));
        (handle.typed_debug_checked(), created)
    }

    /// The type-erased counterpart of [`AssetInfos::get_or_create_path_handle`].
    ///
    /// # Panics
    ///
    /// Panics if no handle provider has been registered for `type_id`.
    pub(crate) fn get_or_create_path_handle_erased(
        &mut self,
        path: AssetPath<'static>,
        type_id: TypeId,
        type_name: Option<&str>,
    ) -> (UntypedHandle, bool) {
        if let Some(id) = self.path_to_index.get(&path).and_then(|by_type| by_type.get(&type_id))
            && self.infos.contains_key(id)
        {
            let UntypedAssetId::Index { index, .. } = id else {
                unreachable!("path-registered ids are always strong")
            };
            let provider = self.handle_provider(type_id, type_name);
            let handle = UntypedHandle::Strong(provider.get_handle(*index));
            return (handle, false);
        }

        let handle = self.handle_provider(type_id, type_name).reserve_handle();
        let id = handle.id();
        self.path_to_index
            .entry(path.clone())
            .or_default()
            .insert(type_id, id);
        match self.infos.entry(id) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().path = Some(path);
            }
            Entry::Vacant(entry) => {
                entry.insert(AssetInfo::new(Some(path)));
            }
        }
        (handle, true)
    }

    /// The strong handles currently registered for `path`, across every type.
    pub(crate) fn get_handles_untyped(&self, path: &AssetPath<'_>) -> Vec<UntypedHandle> {
        let Some(by_type) = self.path_to_index.get(&path.clone_owned()) else {
            return Vec::new();
        };
        by_type
            .iter()
            .filter_map(|(type_id, id)| {
                let UntypedAssetId::Index { index, .. } = id else {
                    return None;
                };
                let provider = self.handle_providers.get(type_id)?;
                Some(UntypedHandle::Strong(provider.get_handle(*index)))
            })
            .collect()
    }

    fn handle_provider(&self, type_id: TypeId, type_name: Option<&str>) -> &AssetHandleProvider {
        self.handle_providers.get(&type_id).unwrap_or_else(|| {
            panic!(
                "no handle provider registered for the asset type '{}'; register the asset type \
                 before loading it",
                type_name.unwrap_or("<unknown>"),
            )
        })
    }

    /// The information recorded for `id`, if any.
    #[allow(dead_code)] // consumed by the loading-pipeline ticket
    pub(crate) fn get(&self, id: impl Into<UntypedAssetId>) -> Option<&AssetInfo> {
        self.infos.get(&id.into())
    }

    /// Every known asset's information, keyed by id.
    #[allow(dead_code)] // consumed by the loading-pipeline ticket
    pub(crate) fn iter(&self) -> impl Iterator<Item = (UntypedAssetId, &AssetInfo)> {
        self.infos.iter().map(|(id, info)| (*id, info))
    }

    /// The load state of `id`, or [`None`] if the asset is unknown.
    #[allow(dead_code)] // consumed by the loading-pipeline ticket
    pub(crate) fn load_state(&self, id: impl Into<UntypedAssetId>) -> Option<LoadState> {
        self.get(id).map(|info| info.load_state.clone())
    }

    /// The dependency load state of `id`, or [`None`] if the asset is unknown.
    #[allow(dead_code)] // consumed by the loading-pipeline ticket
    pub(crate) fn dependency_load_state(
        &self,
        id: impl Into<UntypedAssetId>,
    ) -> Option<DependencyLoadState> {
        self.get(id).map(|info| info.dep_load_state.clone())
    }

    /// The recursive dependency load state of `id`, or [`None`] if the asset is
    /// unknown.
    #[allow(dead_code)] // consumed by the loading-pipeline ticket
    pub(crate) fn recursive_dependency_load_state(
        &self,
        id: impl Into<UntypedAssetId>,
    ) -> Option<RecursiveDependencyLoadState> {
        self.get(id).map(|info| info.rec_dep_load_state.clone())
    }

    /// Whether `id` has finished loading.
    #[allow(dead_code)] // consumed by the loading-pipeline ticket
    pub(crate) fn is_loaded(&self, id: impl Into<UntypedAssetId>) -> bool {
        matches!(self.load_state(id), Some(LoadState::Loaded))
    }

    /// Whether `id`, its direct dependencies, and its recursive dependencies
    /// have all finished loading.
    #[allow(dead_code)] // consumed by the loading-pipeline ticket
    pub(crate) fn is_loaded_with_dependencies(&self, id: impl Into<UntypedAssetId>) -> bool {
        match self.get(id) {
            Some(info) => {
                info.load_state.is_loaded()
                    && info.dep_load_state.is_loaded()
                    && info.rec_dep_load_state.is_loaded()
            }
            None => false,
        }
    }

    /// Overrides the load state recorded for `id`.
    ///
    /// Used by tests to put an asset into a state the loading pipeline will
    /// produce once it lands.
    #[allow(dead_code)] // consumed by the loading-pipeline ticket and tests
    pub(crate) fn set_load_state(&mut self, id: impl Into<UntypedAssetId>, state: LoadState) {
        if let Some(info) = self.infos.get_mut(&id.into()) {
            info.load_state = state;
        }
    }
}

/// The load state of an asset.
#[derive(Default, Clone, Debug)]
pub enum LoadState {
    /// The asset has not started loading yet.
    #[default]
    NotLoaded,
    /// The asset is in the process of loading.
    Loading,
    /// The asset has been loaded and can be inserted into its store.
    Loaded,
    /// The asset failed to load.
    ///
    /// The underlying [`AssetLoadError`] is shared by [`Arc`] clones with the
    /// related [`DependencyLoadState`]s and [`RecursiveDependencyLoadState`]s
    /// in the asset's dependency tree.
    Failed(Arc<AssetLoadError>),
}

impl LoadState {
    /// Whether this is [`LoadState::Loading`].
    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Loading)
    }

    /// Whether this is [`LoadState::Loaded`].
    pub fn is_loaded(&self) -> bool {
        matches!(self, Self::Loaded)
    }

    /// Whether this is [`LoadState::Failed`].
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed(_))
    }
}

/// The load state of an asset's direct dependencies.
#[derive(Default, Clone, Debug)]
pub enum DependencyLoadState {
    /// The asset's dependencies have not started loading yet.
    #[default]
    NotLoaded,
    /// The asset's dependencies are still loading.
    Loading,
    /// All of the asset's dependencies have loaded.
    Loaded,
    /// One or more of the asset's dependencies failed to load.
    Failed(Arc<AssetLoadError>),
}

impl DependencyLoadState {
    /// Whether this is [`DependencyLoadState::Loading`].
    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Loading)
    }

    /// Whether this is [`DependencyLoadState::Loaded`].
    pub fn is_loaded(&self) -> bool {
        matches!(self, Self::Loaded)
    }

    /// Whether this is [`DependencyLoadState::Failed`].
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed(_))
    }
}

/// The load state of an asset's recursive dependencies.
#[derive(Default, Clone, Debug)]
pub enum RecursiveDependencyLoadState {
    /// The asset's dependency tree has not started loading yet.
    #[default]
    NotLoaded,
    /// The asset's dependency tree is still loading.
    Loading,
    /// Every asset in the asset's dependency tree has loaded.
    Loaded,
    /// One or more assets in the asset's dependency tree failed to load.
    Failed(Arc<AssetLoadError>),
}

impl RecursiveDependencyLoadState {
    /// Whether this is [`RecursiveDependencyLoadState::Loading`].
    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Loading)
    }

    /// Whether this is [`RecursiveDependencyLoadState::Loaded`].
    pub fn is_loaded(&self) -> bool {
        matches!(self, Self::Loaded)
    }

    /// Whether this is [`RecursiveDependencyLoadState::Failed`].
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed(_))
    }
}

/// An error that occurred while loading an asset.
#[non_exhaustive]
#[derive(Debug)]
pub enum AssetLoadError {
    /// A load was requested for a path with no file component.
    EmptyPath(AssetPath<'static>),
    /// A handle of the wrong asset type was requested for a path.
    RequestedHandleTypeMismatch {
        /// The path that was requested.
        path: AssetPath<'static>,
        /// The [`TypeId`] the caller asked for.
        requested: TypeId,
        /// The name of the asset type the loader actually produced.
        actual_asset_name: &'static str,
        /// The name of the loader that produced it.
        loader_name: &'static str,
    },
    /// Reading the asset's bytes failed.
    AssetReaderError(AssetReaderError),
    /// The asset's source does not exist.
    MissingAssetSourceError(MissingAssetSourceError),
    /// The asset's source has no processed reader.
    MissingProcessedAssetReaderError(MissingProcessedAssetReaderError),
    /// Reading the asset's meta sidecar failed.
    AssetMetaReadError,
    /// The asset's meta sidecar could not be deserialized.
    DeserializeMeta {
        /// The path whose meta failed to parse.
        path: AssetPath<'static>,
        /// The underlying deserialization error.
        error: Box<DeserializeMetaError>,
    },
    /// The loader itself returned an error.
    AssetLoaderError(AssetLoaderError),
    /// The requested label does not exist on the loaded asset.
    MissingLabel {
        /// The path of the asset that was loaded.
        base_path: AssetPath<'static>,
        /// The label that was requested.
        label: String,
        /// The labels the asset did expose.
        all_labels: Vec<String>,
    },
}

impl fmt::Display for AssetLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPath(path) => {
                write!(f, "Attempted to load an asset with an empty path \"{path}\"")
            }
            Self::RequestedHandleTypeMismatch {
                path,
                requested,
                actual_asset_name,
                loader_name,
            } => write!(
                f,
                "Requested handle of type {requested:?} for asset '{path}' does not match actual \
                 asset type '{actual_asset_name}', which used loader '{loader_name}'"
            ),
            Self::AssetReaderError(error) => write!(f, "{error}"),
            Self::MissingAssetSourceError(error) => write!(f, "{error}"),
            Self::MissingProcessedAssetReaderError(error) => write!(f, "{error}"),
            Self::AssetMetaReadError => write!(f, "Encountered an error while reading asset metadata bytes"),
            Self::DeserializeMeta { path, error } => {
                write!(f, "Failed to deserialize meta for asset {path}: {error}")
            }
            Self::AssetLoaderError(error) => write!(f, "{error}"),
            Self::MissingLabel {
                base_path,
                label,
                all_labels,
            } => write!(
                f,
                "The file at '{base_path}' does not contain the labeled asset '{label}'; it \
                 contains the following {} assets: {}",
                all_labels.len(),
                all_labels
                    .iter()
                    .map(|label| format!("'{label}'"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

impl std::error::Error for AssetLoadError {}

/// The failure of one [`AssetLoader`](crate::next::loader::AssetLoader).
///
/// The loader's error type only has to convert into [`KairosError`], so it is
/// erased into one here; `Arc` lets clones share it across the dependent states
/// that reference the same failure.
#[derive(Debug)]
pub struct AssetLoaderError {
    path: AssetPath<'static>,
    loader_name: &'static str,
    error: Arc<KairosError>,
}

impl AssetLoaderError {
    /// Wraps a loader's error for an asset at `path`.
    #[allow(dead_code)] // the loading pipeline is the first caller
    pub(crate) fn new(
        path: AssetPath<'static>,
        loader_name: &'static str,
        error: impl Into<KairosError>,
    ) -> Self {
        Self {
            path,
            loader_name,
            error: Arc::new(error.into()),
        }
    }

    /// The path of the asset that failed to load.
    pub fn path(&self) -> &AssetPath<'static> {
        &self.path
    }

    /// The name of the loader that failed.
    pub fn loader_name(&self) -> &'static str {
        self.loader_name
    }

    /// The loader's own error.
    pub fn error(&self) -> &KairosError {
        &self.error
    }
}

impl fmt::Display for AssetLoaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Failed to load asset '{}' with asset loader '{}': {}",
            self.path, self.loader_name, self.error
        )
    }
}

impl std::error::Error for AssetLoaderError {}

