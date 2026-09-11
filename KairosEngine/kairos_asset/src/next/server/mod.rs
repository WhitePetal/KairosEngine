//! The asset server: the single entry point for loading assets, obtaining
//! handles, and querying load state.
//!
//! This mirrors `bevy_asset`'s `server/mod.rs`. The pieces that are out of scope
//! for the P0 core are deliberately absent: untyped/`load_folder` loads,
//! `add_async`/guards, and the `wait_for_asset*` family. What is here is the
//! pipeline the A-tier API needs: registration, `load`/`load_builder`/`add`,
//! `reload`, and the load-state and handle accessors.
//!
//! Loading itself is asynchronous. A `load` records the handle and spawns a task
//! on the [`IoTaskPool`]; that task reads the source, runs the loader, and
//! pushes an [`InternalAssetEvent`] back to the server. The value is inserted
//! into its [`Assets<A>`](crate::next::Assets) store by
//! [`handle_internal_asset_events`], which runs on the main thread (where the
//! world lives). This is why a load is not visible in the store until that
//! system runs.
//!
//! Deliberately, an asset's load state lives in [`AssetInfos`] rather than as a
//! world component: the store owns the values, the server owns what is known
//! about loads. Removing the bookkeeping of an already-loaded asset once its
//! last handle drops is the store-side `Assets::track_assets` integration and
//! lands with the engine install; for now a load whose handles are all gone is
//! discarded when its result is processed.

mod info;
mod loaders;

pub use info::{DependencyLoadState, LoadState, RecursiveDependencyLoadState};

use core::any::{TypeId, type_name};
use std::{
    fmt,
    path::Path,
    panic::AssertUnwindSafe,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use crossbeam_channel::{Receiver, Sender};
use futures_lite::FutureExt;
use kairos_ecs::change_detection::Mut;
use kairos_ecs::resource::Resource;
use kairos_ecs::world::World;
use kairos_tasks::{IoTaskPool, TaskPool};

use crate::next::asset::Asset;
use crate::next::assets::Assets;
use crate::next::event::{AssetEvent, AssetLoadFailedEvent};
use crate::next::handle::{Handle, UntypedHandle};
use crate::next::id::{AssetId, UntypedAssetId};
use crate::next::io::{
    AssetReaderError, AssetSource, AssetSourceBuilders, AssetSourceId, AssetSources,
    ErasedAssetReader, MissingAssetSourceError, MissingProcessedAssetReaderError, Reader,
    UnapprovedPathMode,
};
use crate::next::loader::{ErasedAssetLoader, LoadContext, LoadedAsset};
use crate::next::meta::{
    AssetActionMinimal, AssetMetaCheck, AssetMetaDyn, AssetMetaMinimal, DeserializeMetaError,
    MetaTransform, Settings, loader_settings_meta_transform,
};
use crate::next::path::AssetPath;

use info::{AssetInfos, HandleLoadingMode};
use loaders::AssetLoaders;

/// The asset system's server: the single entry point for loading assets and
/// obtaining handles.
///
/// It is a [`Clone`]able world resource that shares its state behind an
/// [`Arc`], so systems can hold it by value and load tasks can keep it alive
/// across threads.
#[derive(Resource, Clone)]
pub struct AssetServer {
    pub(crate) data: Arc<AssetServerData>,
}

/// The shared state behind every [`AssetServer`] clone.
pub(crate) struct AssetServerData {
    /// What the server knows about each asset, keyed by id.
    pub(crate) infos: RwLock<AssetInfos>,
    /// The registered loaders, behind their own lock so a load task can resolve
    /// one without holding the infos lock.
    loaders: Arc<RwLock<AssetLoaders>>,
    /// The asset sources paths resolve against.
    sources: Arc<AssetSources>,
    /// Whether the server reads unprocessed or processed assets.
    mode: AssetServerMode,
    /// Whether and where the server consults `.meta` sidecars.
    meta_check: AssetMetaCheck,
    /// How paths that escape their source root are treated.
    unapproved_path_mode: UnapprovedPathMode,
    /// Load results travelling from load tasks back to the main thread.
    internal_event_sender: Sender<InternalAssetEvent>,
    internal_event_receiver: Receiver<InternalAssetEvent>,
}

/// The "asset mode" the server is currently in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetServerMode {
    /// This server loads unprocessed assets.
    Unprocessed,
    /// This server loads processed assets.
    Processed,
}

impl AssetServer {
    /// Creates a server over the default source: the process working directory,
    /// reading unprocessed assets and consulting `.meta` sidecars (ADR 0004).
    pub fn new() -> Self {
        let mut builders = AssetSourceBuilders::default();
        builders.init_default_source("", None);
        let sources = Arc::new(builders.build_sources());
        Self::new_with_meta_check(
            sources,
            AssetServerMode::Unprocessed,
            AssetMetaCheck::Always,
            false,
            UnapprovedPathMode::Forbid,
        )
    }

    /// Creates a server with explicit sources and meta handling.
    pub(crate) fn new_with_meta_check(
        sources: Arc<AssetSources>,
        mode: AssetServerMode,
        meta_check: AssetMetaCheck,
        watching_for_changes: bool,
        unapproved_path_mode: UnapprovedPathMode,
    ) -> Self {
        let (internal_event_sender, internal_event_receiver) = crossbeam_channel::unbounded();
        let mut infos = AssetInfos::default();
        infos.watching_for_changes = watching_for_changes;
        Self {
            data: Arc::new(AssetServerData {
                infos: RwLock::new(infos),
                loaders: Arc::new(RwLock::new(AssetLoaders::default())),
                sources,
                mode,
                meta_check,
                unapproved_path_mode,
                internal_event_sender,
                internal_event_receiver,
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

    fn read_loaders(&self) -> RwLockReadGuard<'_, AssetLoaders> {
        self.data.loaders.read().expect("asset loaders lock poisoned")
    }

    fn write_loaders(&self) -> RwLockWriteGuard<'_, AssetLoaders> {
        self.data.loaders.write().expect("asset loaders lock poisoned")
    }

    /// The [`AssetSource`] named by `source`.
    pub(crate) fn get_source<'a>(
        &self,
        source: impl Into<AssetSourceId<'a>>,
    ) -> Result<&AssetSource, MissingAssetSourceError> {
        self.data.sources.get(source)
    }

    /// Returns true if the [`AssetServer`] watches for changes.
    pub fn watching_for_changes(&self) -> bool {
        self.read_infos().watching_for_changes
    }

    /// Registers a new [`AssetLoader`](crate::next::AssetLoader). Loaders must
    /// be registered before they can read assets.
    pub fn register_loader<L: crate::next::AssetLoader>(&self, loader: L) {
        self.write_loaders().push(loader);
    }

    /// Registers a new [`Asset`] type.
    ///
    /// Asset types must be registered before assets of that type can be loaded,
    /// and `assets` must be the store the loaded values are inserted into: the
    /// server takes its handle provider so both allocate slots from the same
    /// allocator.
    pub fn register_asset<A: Asset>(&self, assets: &Assets<A>) {
        self.write_infos()
            .register_handle_provider_erased(assets.get_handle_provider());

        fn loaded_sender<A: Asset>(world: &mut World, id: UntypedAssetId) {
            let id = id.typed_debug_checked::<A>();
            world.write_message(AssetEvent::LoadedWithDependencies { id });
        }
        fn failed_sender<A: Asset>(
            world: &mut World,
            id: UntypedAssetId,
            _path: AssetPath<'static>,
            _error: Arc<AssetLoadError>,
        ) {
            // The failed event carries only the id for now: the path and error
            // are available once `AssetLoadFailedEvent` grows them.
            let id = id.typed_debug_checked::<A>();
            world.write_message(AssetLoadFailedEvent::new(id));
        }

        let mut infos = self.write_infos();
        infos
            .dependency_loaded_event_sender
            .insert(TypeId::of::<A>(), loaded_sender::<A>);
        infos
            .dependency_failed_event_sender
            .insert(TypeId::of::<A>(), failed_sender::<A>);
    }

    /// Returns the registered [`ErasedAssetLoader`] with `type_name`, if any.
    fn get_asset_loader_with_type_name(
        &self,
        type_name: &str,
    ) -> Result<Arc<dyn ErasedAssetLoader>, AssetLoadError> {
        self.read_loaders().get_by_name(type_name).ok_or_else(|| {
            AssetLoadError::MissingAssetLoaderForTypeName {
                type_name: type_name.to_owned(),
            }
        })
    }

    /// Resolves the loader for `path`, preferring the asset type when known.
    fn find_loader(
        &self,
        asset_type_id: Option<TypeId>,
        asset_path: &AssetPath<'_>,
    ) -> Result<Arc<dyn ErasedAssetLoader>, AssetLoadError> {
        self.read_loaders()
            .find(asset_type_id, asset_path)
            .ok_or_else(|| AssetLoadError::MissingAssetLoader {
                asset_type_id,
                asset_path: asset_path.to_string(),
            })
    }

    /// Begins loading an [`Asset`] of type `A` stored at `path`.
    ///
    /// The returned "strong" handle keeps the asset alive while it lives. Once
    /// the asset is in [`LoadState::Loaded`] it is inserted into the associated
    /// [`Assets`] resource by [`handle_internal_asset_events`].
    #[must_use = "not using the returned strong handle may result in the unexpected release of the asset"]
    pub fn load<'a, A: Asset>(&self, path: impl Into<AssetPath<'a>>) -> Handle<A> {
        self.load_builder().load(path.into())
    }

    /// Returns a [`LoadBuilder`] that can be used to start more complex loads.
    #[must_use = "the load doesn't start until LoadBuilder has been consumed"]
    pub fn load_builder(&self) -> LoadBuilder<'_> {
        LoadBuilder::new(self)
    }

    /// Starts (or joins) the load for `path` and returns its handle.
    pub(crate) fn load_with_meta_transform<'a>(
        &self,
        path: impl Into<AssetPath<'a>>,
        type_id: TypeId,
        type_name: Option<&str>,
        meta_transform: Option<MetaTransform>,
    ) -> UntypedHandle {
        let path = path.into().into_owned();
        if path.path() == Path::new("") {
            return UntypedHandle::default_for_type(type_id);
        }
        if path.is_unapproved() {
            match self.data.unapproved_path_mode {
                UnapprovedPathMode::Allow => {}
                UnapprovedPathMode::Deny | UnapprovedPathMode::Forbid => {
                    return UntypedHandle::default_for_type(type_id);
                }
            }
        }

        let mut infos = self.write_infos();
        let (handle, should_load) = infos.get_or_create_path_handle_erased(
            path.clone(),
            type_id,
            type_name,
            HandleLoadingMode::Request,
        );
        if should_load {
            self.spawn_load_task(handle.clone(), path, infos, meta_transform);
        }
        handle
    }

    /// Spawns the task that performs one load.
    ///
    /// The task is stored in [`AssetInfos::pending_tasks`] so dropping the
    /// handle does not cancel it; it is cancelled only when the store's drop
    /// processing removes the asset, or when the task finishes.
    fn spawn_load_task(
        &self,
        handle: UntypedHandle,
        path: AssetPath<'static>,
        mut infos: RwLockWriteGuard<'_, AssetInfos>,
        meta_transform: Option<MetaTransform>,
    ) {
        let owned_handle = handle.clone();
        let server = self.clone();
        let task = io_task_pool().spawn(async move {
            let _ = server
                .load_internal(owned_handle, path, meta_transform)
                .await;
        });
        infos.pending_tasks.insert(handle.id(), task);
    }

    /// Performs one asset load and reports the result over the event channel.
    ///
    /// The owned `input_handle` is dropped once the load is underway, so if
    /// every other strong handle disappears before it completes,
    /// [`AssetInfos::process_asset_load`] discards the result.
    async fn load_internal(
        &self,
        input_handle: UntypedHandle,
        path: AssetPath<'static>,
        meta_transform: Option<MetaTransform>,
    ) -> Result<(), Arc<AssetLoadError>> {
        let asset_id = input_handle.id();
        let (mut meta, loader, mut reader) = match self
            .get_meta_loader_and_reader(&path, Some(input_handle.type_id()))
            .await
        {
            Ok(found) => found,
            Err(error) => return self.fail_load(asset_id, path.clone(), error),
        };

        if let Some(meta_transform) = meta_transform {
            meta_transform(&mut *meta);
        }

        // A handle of the wrong type for this path is a configuration error; a
        // labeled handle is exempt because a label may name a different type.
        if path.label().is_none() && asset_id.type_id() != loader.asset_type_id() {
            let error = AssetLoadError::RequestedHandleTypeMismatch {
                path: path.clone(),
                requested: asset_id.type_id(),
                actual_asset_name: loader.asset_type_name(),
                loader_name: loader.type_name(),
            };
            return self.fail_load(asset_id, path.clone(), error);
        }
        // A labeled load also needs the base asset's handle kept alive until the
        // load finishes, or `process_asset_load` would discard the result.
        let base_path = if path.label().is_some() {
            path.without_label().into_owned()
        } else {
            path.clone()
        };
        let base_handle = if path.label().is_some() {
            Some(
                self.write_infos()
                    .get_or_create_path_handle_erased(
                        base_path.clone(),
                        loader.asset_type_id(),
                        Some(loader.asset_type_name()),
                        HandleLoadingMode::Force,
                    )
                    .0,
            )
        } else {
            None
        };
        let base_id = base_handle
            .as_ref()
            .map(UntypedHandle::id)
            .unwrap_or(asset_id);

        // Drop our own handle reference now that the load is underway.
        drop(input_handle);

        let loaded_asset = match self
            .load_with_settings_loader_and_reader(
                &base_path,
                meta.loader_settings().expect("the meta action is Load"),
                &*loader,
                &mut *reader,
                true,
                false,
            )
            .await
        {
            Ok(loaded_asset) => loaded_asset,
            Err(error) => return self.fail_load(asset_id, path.clone(), error),
        };

        if let Some(label) = path.label_cow() {
            match loaded_asset.label_to_asset_index.get(&label) {
                Some(index) => {
                    let labeled = &loaded_asset.labeled_assets[*index];
                    if labeled.handle.type_id() != asset_id.type_id() {
                        let error = AssetLoadError::RequestedHandleTypeMismatch {
                            path: path.clone(),
                            requested: asset_id.type_id(),
                            actual_asset_name: labeled.asset.asset_type_name(),
                            loader_name: loader.type_name(),
                        };
                        return self.fail_load(asset_id, path.clone(), error);
                    }
                }
                None => {
                    let mut all_labels: Vec<String> = loaded_asset
                        .label_to_asset_index
                        .keys()
                        .map(|label| label.to_string())
                        .collect();
                    all_labels.sort_unstable();
                    let error = AssetLoadError::MissingLabel {
                        base_path: path.without_label().into_owned(),
                        label: label.to_string(),
                        all_labels,
                    };
                    return self.fail_load(asset_id, path.clone(), error);
                }
            }
        }

        self.send_asset_event(InternalAssetEvent::Loaded {
            id: base_id,
            loaded_asset,
        });
        Ok(())
    }

    /// Sends a failure event for `asset_id` and returns the wrapped error.
    fn fail_load(
        &self,
        asset_id: UntypedAssetId,
        path: AssetPath<'static>,
        error: AssetLoadError,
    ) -> Result<(), Arc<AssetLoadError>> {
        let error = Arc::new(error);
        self.send_asset_event(InternalAssetEvent::Failed {
            id: asset_id,
            path,
            error: error.clone(),
        });
        Err(error)
    }

    /// Resolves the meta, loader, and reader trio for one load.
    pub(crate) async fn get_meta_loader_and_reader<'a>(
        &'a self,
        asset_path: &'a AssetPath<'_>,
        asset_type_id: Option<TypeId>,
    ) -> Result<
        (
            Box<dyn AssetMetaDyn>,
            Arc<dyn ErasedAssetLoader>,
            Box<dyn Reader + 'a>,
        ),
        AssetLoadError,
    > {
        let source = self
            .get_source(asset_path.source())
            .map_err(AssetLoadError::MissingAssetSourceError)?;
        let asset_reader: &dyn ErasedAssetReader = match self.data.mode {
            AssetServerMode::Unprocessed => source.reader(),
            AssetServerMode::Processed => source
                .processed_reader()
                .map_err(AssetLoadError::MissingProcessedAssetReaderError)?,
        };
        let read_meta = match &self.data.meta_check {
            AssetMetaCheck::Always => true,
            AssetMetaCheck::Paths(paths) => paths.contains(asset_path),
            AssetMetaCheck::Never => false,
        };

        let (meta, loader) = if read_meta {
            match asset_reader.read_meta(asset_path.path()).await {
                Ok(mut meta_reader) => {
                    let mut meta_bytes = Vec::new();
                    meta_reader
                        .read_to_end(&mut meta_bytes)
                        .await
                        .map_err(|error| AssetLoadError::AssetReaderError(error.into()))?;
                    let minimal = AssetMetaMinimal::deserialize(&meta_bytes).map_err(|error| {
                        AssetLoadError::DeserializeMeta {
                            path: asset_path.clone_owned(),
                            error: Box::new(error),
                        }
                    })?;
                    let loader_name = match minimal.asset {
                        AssetActionMinimal::Load { loader } => loader,
                        AssetActionMinimal::Process { .. } => {
                            return Err(AssetLoadError::CannotLoadProcessedAsset {
                                path: asset_path.clone_owned(),
                            });
                        }
                        AssetActionMinimal::Ignore => {
                            return Err(AssetLoadError::CannotLoadIgnoredAsset {
                                path: asset_path.clone_owned(),
                            });
                        }
                    };
                    let loader = self.get_asset_loader_with_type_name(&loader_name)?;
                    let meta = loader.deserialize_meta(&meta_bytes).map_err(|error| {
                        AssetLoadError::DeserializeMeta {
                            path: asset_path.clone_owned(),
                            error: Box::new(error),
                        }
                    })?;
                    (meta, loader)
                }
                Err(AssetReaderError::NotFound(_)) => {
                    let loader = self.find_loader(asset_type_id, asset_path)?;
                    let meta = loader.default_meta();
                    (meta, loader)
                }
                Err(error) => return Err(AssetLoadError::AssetReaderError(error)),
            }
        } else {
            let loader = self.find_loader(asset_type_id, asset_path)?;
            let meta = loader.default_meta();
            (meta, loader)
        };

        let reader = asset_reader
            .read(asset_path.path())
            .await
            .map_err(AssetLoadError::AssetReaderError)?;
        Ok((meta, loader, reader))
    }

    /// Runs one [`AssetLoader`](crate::next::AssetLoader) over its reader.
    pub(crate) async fn load_with_settings_loader_and_reader(
        &self,
        asset_path: &AssetPath<'_>,
        settings: &dyn Settings,
        loader: &dyn ErasedAssetLoader,
        reader: &mut dyn Reader,
        load_dependencies: bool,
        populate_hashes: bool,
    ) -> Result<crate::next::loader::ErasedLoadedAsset, AssetLoadError> {
        let asset_path = asset_path.clone_owned();
        let load_context =
            LoadContext::new(self, asset_path.clone(), load_dependencies, populate_hashes);
        let load = AssertUnwindSafe(loader.load(reader, settings, load_context)).catch_unwind();
        load.await
            .map_err(|_| AssetLoadError::AssetLoaderPanic {
                path: asset_path.clone_owned(),
                loader_name: loader.type_name(),
            })?
            .map_err(|error| {
                AssetLoadError::AssetLoaderError(AssetLoaderError::new(
                    asset_path.clone_owned(),
                    loader.type_name(),
                    error,
                ))
            })
    }

    /// Kicks off a reload of the assets at `path`, if any are alive.
    pub fn reload<'a>(&self, path: impl Into<AssetPath<'a>>) {
        let path = path.into().into_owned();
        if !self.read_infos().should_reload(&path) {
            return;
        }
        let server = self.clone();
        io_task_pool()
            .spawn(async move {
                let handles = server.read_infos().get_handles_untyped(&path);
                for handle in handles {
                    let _ = server.load_internal(handle, path.clone(), None).await;
                }
            })
            .detach();
    }

    /// Queues a new asset to be tracked by the server and returns a handle to
    /// it.
    ///
    /// After the asset has been fully loaded by the server, it shows up in the
    /// relevant [`Assets`] storage.
    #[must_use = "not using the returned strong handle may result in the unexpected release of the asset"]
    pub fn add<A: Asset>(&self, asset: A) -> Handle<A> {
        self.load_asset(LoadedAsset::new_with_dependencies(asset))
    }

    /// Queues an already-[`LoadedAsset`] and returns its handle.
    pub(crate) fn load_asset<A: Asset>(&self, asset: impl Into<LoadedAsset<A>>) -> Handle<A> {
        let erased: crate::next::loader::ErasedLoadedAsset = asset.into().into();
        self.load_asset_untyped(None, erased).typed_debug_checked()
    }

    /// The type-erased counterpart of [`AssetServer::load_asset`].
    pub(crate) fn load_asset_untyped(
        &self,
        path: Option<AssetPath<'static>>,
        asset: impl Into<crate::next::loader::ErasedLoadedAsset>,
    ) -> UntypedHandle {
        let loaded_asset = asset.into();
        let handle = if let Some(path) = path {
            self.write_infos()
                .get_or_create_path_handle_erased(
                    path,
                    loaded_asset.asset_type_id(),
                    Some(loaded_asset.asset_type_name()),
                    HandleLoadingMode::NotLoading,
                )
                .0
        } else {
            self.write_infos().create_loading_handle_untyped(
                loaded_asset.asset_type_id(),
                loaded_asset.asset_type_name(),
            )
        };
        self.send_asset_event(InternalAssetEvent::Loaded {
            id: handle.id(),
            loaded_asset,
        });
        handle
    }

    /// The load states of `id`: its own, its direct dependencies', and its
    /// recursive dependencies'.
    pub fn get_load_states(
        &self,
        id: impl Into<UntypedAssetId>,
    ) -> Option<(LoadState, DependencyLoadState, RecursiveDependencyLoadState)> {
        self.read_infos().get(id).map(|info| {
            (
                info.load_state.clone(),
                info.dep_load_state.clone(),
                info.rec_dep_load_state.clone(),
            )
        })
    }

    /// The main [`LoadState`] of `id`, or [`None`] if the asset is unknown.
    pub fn get_load_state(&self, id: impl Into<UntypedAssetId>) -> Option<LoadState> {
        self.read_infos().load_state(id)
    }

    /// The [`DependencyLoadState`] of `id`'s direct dependencies.
    pub fn get_dependency_load_state(
        &self,
        id: impl Into<UntypedAssetId>,
    ) -> Option<DependencyLoadState> {
        self.read_infos().dependency_load_state(id)
    }

    /// The [`RecursiveDependencyLoadState`] of `id`'s dependency tree.
    pub fn get_recursive_dependency_load_state(
        &self,
        id: impl Into<UntypedAssetId>,
    ) -> Option<RecursiveDependencyLoadState> {
        self.read_infos().recursive_dependency_load_state(id)
    }

    /// The main [`LoadState`] of `id`, defaulting to [`LoadState::NotLoaded`]
    /// when the asset is unknown.
    pub fn load_state(&self, id: impl Into<UntypedAssetId>) -> LoadState {
        self.get_load_state(id).unwrap_or(LoadState::NotLoaded)
    }

    /// The [`DependencyLoadState`] of `id`, defaulting to
    /// [`DependencyLoadState::NotLoaded`] when unknown.
    pub fn dependency_load_state(&self, id: impl Into<UntypedAssetId>) -> DependencyLoadState {
        self.get_dependency_load_state(id)
            .unwrap_or(DependencyLoadState::NotLoaded)
    }

    /// The [`RecursiveDependencyLoadState`] of `id`, defaulting to
    /// [`RecursiveDependencyLoadState::NotLoaded`] when unknown.
    pub fn recursive_dependency_load_state(
        &self,
        id: impl Into<UntypedAssetId>,
    ) -> RecursiveDependencyLoadState {
        self.get_recursive_dependency_load_state(id)
            .unwrap_or(RecursiveDependencyLoadState::NotLoaded)
    }

    /// Whether the asset has finished loading.
    pub fn is_loaded(&self, id: impl Into<UntypedAssetId>) -> bool {
        matches!(self.load_state(id), LoadState::Loaded)
    }

    /// Whether the asset and its direct dependencies have finished loading.
    pub fn is_loaded_with_direct_dependencies(&self, id: impl Into<UntypedAssetId>) -> bool {
        matches!(
            self.get_load_states(id),
            Some((LoadState::Loaded, DependencyLoadState::Loaded, _))
        )
    }

    /// Whether the asset and its whole dependency tree have finished loading.
    pub fn is_loaded_with_dependencies(&self, id: impl Into<UntypedAssetId>) -> bool {
        matches!(
            self.get_load_states(id),
            Some((
                LoadState::Loaded,
                DependencyLoadState::Loaded,
                RecursiveDependencyLoadState::Loaded
            ))
        )
    }

    /// An active handle for `path`, if the asset has started loading or is alive.
    pub fn get_handle<'a, A: Asset>(&self, path: impl Into<AssetPath<'a>>) -> Option<Handle<A>> {
        self.get_path_and_type_id_handle(&path.into(), TypeId::of::<A>())
            .map(UntypedHandle::typed_debug_checked)
    }

    /// An active handle for `id`, if the server manages that asset.
    pub fn get_id_handle<A: Asset>(&self, id: AssetId<A>) -> Option<Handle<A>> {
        self.get_id_handle_untyped(id.untyped())
            .map(UntypedHandle::typed_debug_checked)
    }

    /// The type-erased counterpart of [`AssetServer::get_id_handle`].
    pub(crate) fn get_id_handle_untyped(&self, id: UntypedAssetId) -> Option<UntypedHandle> {
        self.read_infos().get_index_handle(id)
    }

    /// Whether `id` names an asset managed by this server.
    pub fn is_managed(&self, id: impl Into<UntypedAssetId>) -> bool {
        self.read_infos().contains_key(id)
    }

    /// Every active untyped handle for `path`.
    pub(crate) fn get_handles_untyped<'a>(&self, path: impl Into<AssetPath<'a>>) -> Vec<UntypedHandle> {
        self.read_infos().get_handles_untyped(&path.into())
    }

    /// An active handle for `path` and `type_id`.
    fn get_path_and_type_id_handle(
        &self,
        path: &AssetPath<'_>,
        type_id: TypeId,
    ) -> Option<UntypedHandle> {
        self.read_infos().get_path_and_type_id_handle(path, type_id)
    }

    /// The path `id` was loaded from, if it has one.
    pub fn get_path(&self, id: impl Into<UntypedAssetId>) -> Option<AssetPath<'static>> {
        self.read_infos().get(id)?.path.clone()
    }

    /// The [`AssetServerMode`] this server is currently in.
    pub fn mode(&self) -> AssetServerMode {
        self.data.mode
    }

    /// Retrieve a handle for `path`, creating one (and its [`AssetInfo`]) if it
    /// does not exist, without starting a load.
    pub(crate) fn get_or_create_path_handle<A: Asset>(
        &self,
        path: AssetPath<'static>,
    ) -> Handle<A> {
        self.write_infos().get_or_create_path_handle(path).0
    }

    /// Sends a load result to the main thread.
    fn send_asset_event(&self, event: InternalAssetEvent) {
        let _ = self.data.internal_event_sender.send(event);
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

/// A builder for a single load, created by [`AssetServer::load_builder`].
#[must_use = "the load doesn't start until LoadBuilder has been consumed"]
pub struct LoadBuilder<'a> {
    asset_server: &'a AssetServer,
    meta_transform: Option<MetaTransform>,
}

impl<'a> LoadBuilder<'a> {
    fn new(asset_server: &'a AssetServer) -> Self {
        Self {
            asset_server,
            meta_transform: None,
        }
    }

    /// Overrides the loader's settings for this load.
    ///
    /// The settings type must match the loader's `Settings`; a mismatch is
    /// silently ignored (see [`loader_settings_meta_transform`]).
    pub fn with_settings<S: Settings>(
        mut self,
        settings: impl Fn(&mut S) + Send + Sync + 'static,
    ) -> Self {
        self.meta_transform = Some(loader_settings_meta_transform(settings));
        self
    }

    /// Starts the load and returns the asset's handle.
    #[must_use = "not using the returned strong handle may result in the unexpected release of the asset"]
    pub fn load<'b, A: Asset>(self, asset_path: impl Into<AssetPath<'b>>) -> Handle<A> {
        self.asset_server
            .load_with_meta_transform(
                asset_path.into(),
                TypeId::of::<A>(),
                Some(type_name::<A>()),
                self.meta_transform,
            )
            .typed_debug_checked()
    }
}

/// Processes the load results that load tasks have sent back to the server.
///
/// This is the main-thread half of the pipeline: it inserts loaded values into
/// their [`Assets<A>`](crate::next::Assets) stores and advances load state. It
/// is a plain function over [`World`] so callers can schedule it in whatever
/// stage they use for asset work.
pub fn handle_internal_asset_events(world: &mut World) {
    world.resource_scope(|world, server: Mut<AssetServer>| {
        for event in server.data.internal_event_receiver.try_iter() {
            match event {
                InternalAssetEvent::Loaded { id, loaded_asset } => {
                    server.write_infos().process_asset_load(
                        id,
                        loaded_asset,
                        world,
                        &server.data.internal_event_sender,
                    );
                }
                InternalAssetEvent::LoadedWithDependencies { id } => {
                    let sender = server
                        .read_infos()
                        .dependency_loaded_event_sender
                        .get(&id.type_id())
                        .copied();
                    if let Some(sender) = sender {
                        sender(world, id);
                    }
                }
                InternalAssetEvent::Failed { id, path, error } => {
                    server.write_infos().process_asset_fail(id, error.clone());
                    let sender = server
                        .read_infos()
                        .dependency_failed_event_sender
                        .get(&id.type_id())
                        .copied();
                    if let Some(sender) = sender {
                        sender(world, id, path, error);
                    }
                }
            }
        }
        server
            .write_infos()
            .pending_tasks
            .retain(|_, task| !task.is_finished());
    });
}

/// The lazy default [`IoTaskPool`].
///
/// The pool is initialized on first use so loading works without an explicit
/// startup step (ADR 0001).
fn io_task_pool() -> &'static IoTaskPool {
    IoTaskPool::get_or_init(TaskPool::default)
}

/// The results a load task sends back to the main thread.
pub(crate) enum InternalAssetEvent {
    /// A load succeeded; the value should be inserted into its store.
    Loaded {
        /// The id of the asset that finished loading.
        id: UntypedAssetId,
        /// The loaded value, its dependencies, and its labeled assets.
        loaded_asset: crate::next::loader::ErasedLoadedAsset,
    },
    /// An asset and everything it depends on finished loading.
    LoadedWithDependencies {
        /// The id of the fully loaded asset.
        id: UntypedAssetId,
    },
    /// A load failed.
    Failed {
        /// The id of the asset that failed.
        id: UntypedAssetId,
        /// The path that was being loaded.
        path: AssetPath<'static>,
        /// The failure.
        error: Arc<AssetLoadError>,
    },
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
    /// No loader matched the requested asset type and path.
    MissingAssetLoader {
        /// The asset type the caller asked for, if it was known.
        asset_type_id: Option<TypeId>,
        /// The path that was being loaded.
        asset_path: String,
    },
    /// No loader is registered under the requested name.
    MissingAssetLoaderForTypeName {
        /// The loader name that was not found.
        type_name: String,
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
    /// The asset is configured to be processed and cannot be loaded directly.
    CannotLoadProcessedAsset {
        /// The path of the asset.
        path: AssetPath<'static>,
    },
    /// The asset is configured to be ignored and cannot be loaded.
    CannotLoadIgnoredAsset {
        /// The path of the asset.
        path: AssetPath<'static>,
    },
    /// The loader panicked.
    AssetLoaderPanic {
        /// The path of the asset that was loading.
        path: AssetPath<'static>,
        /// The name of the loader that panicked.
        loader_name: &'static str,
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
            Self::MissingAssetLoader {
                asset_type_id,
                asset_path,
            } => write!(
                f,
                "Could not find an asset loader matching: Asset Type: {asset_type_id:?}; Path: \
                 {asset_path:?};"
            ),
            Self::MissingAssetLoaderForTypeName { type_name } => {
                write!(f, "no `AssetLoader` found with the name '{type_name}'")
            }
            Self::AssetReaderError(error) => write!(f, "{error}"),
            Self::MissingAssetSourceError(error) => write!(f, "{error}"),
            Self::MissingProcessedAssetReaderError(error) => write!(f, "{error}"),
            Self::AssetMetaReadError => {
                write!(f, "Encountered an error while reading asset metadata bytes")
            }
            Self::DeserializeMeta { path, error } => {
                write!(f, "Failed to deserialize meta for asset {path}: {error}")
            }
            Self::CannotLoadProcessedAsset { path } => write!(
                f,
                "Asset '{path}' is configured to be processed. It cannot be loaded directly."
            ),
            Self::CannotLoadIgnoredAsset { path } => {
                write!(f, "Asset '{path}' is configured to be ignored. It cannot be loaded.")
            }
            Self::AssetLoaderPanic { path, loader_name } => write!(
                f,
                "Failed to load asset '{path}', asset loader '{loader_name}' panicked"
            ),
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

impl From<AssetReaderError> for AssetLoadError {
    fn from(error: AssetReaderError) -> Self {
        Self::AssetReaderError(error)
    }
}

impl From<MissingAssetSourceError> for AssetLoadError {
    fn from(error: MissingAssetSourceError) -> Self {
        Self::MissingAssetSourceError(error)
    }
}

impl From<MissingProcessedAssetReaderError> for AssetLoadError {
    fn from(error: MissingProcessedAssetReaderError) -> Self {
        Self::MissingProcessedAssetReaderError(error)
    }
}

/// The failure of one [`AssetLoader`](crate::next::AssetLoader).
///
/// The loader's error type only has to convert into [`KairosError`], so it is
/// erased into one here; `Arc` lets clones share it across the dependent states
/// that reference the same failure.
#[derive(Debug)]
pub struct AssetLoaderError {
    path: AssetPath<'static>,
    loader_name: &'static str,
    error: Arc<kairos_ecs::error::KairosError>,
}

impl AssetLoaderError {
    /// Wraps a loader's error for an asset at `path`.
    pub(crate) fn new(
        path: AssetPath<'static>,
        loader_name: &'static str,
        error: impl Into<kairos_ecs::error::KairosError>,
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
    pub fn error(&self) -> &kairos_ecs::error::KairosError {
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
