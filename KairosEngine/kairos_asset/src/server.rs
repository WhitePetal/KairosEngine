//! The asset server: the single entry point for loading assets, obtaining
//! handles, and querying load state.
//!
//! This mirrors `bevy_asset`'s `server/mod.rs`. The pieces that are out of scope
//! for the P0 core are deliberately absent: the `wait_for_asset*` family. What is
//! here is the pipeline the A-tier API needs: registration,
//! `load`/`load_builder`/`add`/`add_async`, `reload`, untyped loads, folder
//! loads, and the load-state and handle accessors.
//!
//! Loading itself is asynchronous. A `load` records the handle and spawns a task
//! on the [`IoTaskPool`]; that task reads the source, runs the loader, and
//! pushes an [`InternalAssetEvent`] back to the server. The value is inserted
//! into its [`Assets<A>`](crate::Assets) store by
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
use core::future::Future;
use std::{
    fmt,
    panic::AssertUnwindSafe,
    path::Path,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use atomicow::CowArc;
use crossbeam_channel::{Receiver, Sender};
use futures_lite::{FutureExt, StreamExt};
use kairos_ecs::change_detection::Mut;
use kairos_ecs::resource::Resource;
use kairos_ecs::world::World;
use kairos_tasks::{IoTaskPool, TaskPool};
use thiserror::Error;

use crate::asset::{Asset, VisitAssetDependencies};
use crate::assets::{Assets, LoadedUntypedAsset};
use crate::event::{AssetEvent, AssetLoadFailedEvent};
use crate::folder::LoadedFolder;
use crate::handle::{Handle, UntypedHandle};
use crate::id::{AssetId, UntypedAssetId};
use crate::io::{
    AssetReaderError, AssetSource, AssetSourceBuilders, AssetSourceId, AssetSources,
    ErasedAssetReader, MissingAssetSourceError, MissingProcessedAssetReaderError, Reader,
    UnapprovedPathMode,
};
use crate::loader::{ErasedAssetLoader, LoadContext, LoadedAsset};
use crate::meta::{
    AssetActionMinimal, AssetMetaCheck, AssetMetaDyn, AssetMetaMinimal, DeserializeMetaError,
    MetaTransform, Settings, loader_settings_meta_transform,
};
use crate::path::AssetPath;

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
        let sources = Arc::new(builders.build_sources(false, false));
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
        Self::new_with_loaders(
            sources,
            Arc::new(RwLock::new(AssetLoaders::default())),
            mode,
            meta_check,
            watching_for_changes,
            unapproved_path_mode,
        )
    }

    /// Creates a server that shares `shared`'s loader registry.
    ///
    /// Layout ② builds the app's server from the processor's [`AssetSources`] and
    /// its loaders: the processor resolves loaders to read source assets, and the
    /// app's server must resolve the exact same set, so a loader registered on
    /// one is visible to the other.
    pub(crate) fn new_sharing_loaders_with(
        shared: &AssetServer,
        sources: Arc<AssetSources>,
        mode: AssetServerMode,
        meta_check: AssetMetaCheck,
        watching_for_changes: bool,
        unapproved_path_mode: UnapprovedPathMode,
    ) -> Self {
        Self::new_with_loaders(
            sources,
            shared.data.loaders.clone(),
            mode,
            meta_check,
            watching_for_changes,
            unapproved_path_mode,
        )
    }

    /// Builds a server from its parts, sharing the given loader registry.
    fn new_with_loaders(
        sources: Arc<AssetSources>,
        loaders: Arc<RwLock<AssetLoaders>>,
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
                loaders,
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
        self.data
            .loaders
            .read()
            .expect("asset loaders lock poisoned")
    }

    fn write_loaders(&self) -> RwLockWriteGuard<'_, AssetLoaders> {
        self.data
            .loaders
            .write()
            .expect("asset loaders lock poisoned")
    }

    /// The [`AssetSource`] named by `source`.
    pub fn get_source<'a>(
        &self,
        source: impl Into<AssetSourceId<'a>>,
    ) -> Result<&AssetSource, MissingAssetSourceError> {
        self.data.sources.get(source)
    }

    /// Returns true if the [`AssetServer`] watches for changes.
    pub fn watching_for_changes(&self) -> bool {
        self.read_infos().watching_for_changes
    }

    /// Registers a new [`AssetLoader`](crate::AssetLoader). Loaders must
    /// be registered before they can read assets.
    pub fn register_loader<L: crate::AssetLoader>(&self, loader: L) {
        self.write_loaders().push(loader);
    }

    /// Pre-registers a loader that will later be added.
    ///
    /// A load whose path resolves to the placeholder blocks until a loader named
    /// [`loader_name::<L>()`](crate::meta::loader_name) is registered with
    /// [`AssetServer::register_loader`].
    pub fn preregister_loader<L: crate::AssetLoader>(&self, extensions: &[&str]) {
        self.write_loaders().reserve::<L>(extensions);
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

    /// Returns the registered [`ErasedAssetLoader`] associated with
    /// `extension`, if one exists.
    pub async fn get_asset_loader_with_extension(
        &self,
        extension: &str,
    ) -> Result<Arc<dyn ErasedAssetLoader>, MissingAssetLoaderForExtensionError> {
        let error = || MissingAssetLoaderForExtensionError {
            extensions: vec![extension.to_string()],
        };
        let loader = self
            .read_loaders()
            .get_by_extension(extension)
            .ok_or_else(error)?;
        loader.get().await.map_err(|_| error())
    }

    /// Returns the registered [`ErasedAssetLoader`] with `type_name`, if one
    /// exists.
    pub async fn get_asset_loader_with_type_name(
        &self,
        type_name: &str,
    ) -> Result<Arc<dyn ErasedAssetLoader>, MissingAssetLoaderForTypeNameError> {
        let error = || MissingAssetLoaderForTypeNameError {
            type_name: type_name.to_string(),
        };
        let loader = self.read_loaders().get_by_name(type_name).ok_or_else(error)?;
        loader.get().await.map_err(|_| error())
    }

    /// Retrieves the default [`ErasedAssetLoader`] for the given path, if one
    /// can be found.
    ///
    /// Used by the processor to build a default `.meta` for an asset that has
    /// none.
    pub async fn get_path_asset_loader<'a>(
        &self,
        path: impl Into<AssetPath<'a>>,
    ) -> Result<Arc<dyn ErasedAssetLoader>, MissingAssetLoaderForExtensionError> {
        let path = path.into();

        let error = || {
            let Some(full_extension) = path.get_full_extension() else {
                return MissingAssetLoaderForExtensionError {
                    extensions: Vec::new(),
                };
            };
            let mut extensions = vec![full_extension.to_string()];
            extensions.extend(
                AssetPath::iter_secondary_extensions(full_extension).map(ToString::to_string),
            );
            MissingAssetLoaderForExtensionError { extensions }
        };

        let loader = self.read_loaders().get_by_path(&path).ok_or_else(error)?;
        loader.get().await.map_err(|_| error())
    }

    /// Retrieves the default [`ErasedAssetLoader`] for the given asset
    /// [`TypeId`], if one can be found.
    pub async fn get_asset_loader_with_asset_type_id(
        &self,
        type_id: TypeId,
    ) -> Result<Arc<dyn ErasedAssetLoader>, MissingAssetLoaderForTypeIdError> {
        let error = || MissingAssetLoaderForTypeIdError { type_id };
        let loader = self.read_loaders().get_by_type(type_id).ok_or_else(error)?;
        loader.get().await.map_err(|_| error())
    }

    /// Retrieves the default [`ErasedAssetLoader`] for the given [`Asset`]
    /// type, if one can be found.
    pub async fn get_asset_loader_with_asset_type<A: Asset>(
        &self,
    ) -> Result<Arc<dyn ErasedAssetLoader>, MissingAssetLoaderForTypeIdError> {
        self.get_asset_loader_with_asset_type_id(TypeId::of::<A>())
            .await
    }

    /// Resolves the loader for `path`, preferring the asset type when known.
    ///
    /// The lookup is async so a pre-registered (pending) loader can be awaited
    /// until the real one is registered.
    async fn find_loader(
        &self,
        asset_type_id: Option<TypeId>,
        asset_path: &AssetPath<'_>,
    ) -> Result<Arc<dyn ErasedAssetLoader>, AssetLoadError> {
        let error = || AssetLoadError::MissingAssetLoader {
            asset_type_id,
            asset_path: asset_path.to_string(),
        };
        // Scope the read guard so it is not held across the await: holding it
        // would make the future non-`Send` and could deadlock the registry.
        let loader = { self.read_loaders().find(asset_type_id, asset_path) };
        loader.ok_or_else(error)?.get().await.map_err(|_| error())
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
    ///
    /// `guard` is held until the load finishes (successfully or not), so its
    /// [`Drop`] can signal completion to the caller. `override_unapproved`
    /// permits a path that escapes its source to load even when the server is in
    /// [`UnapprovedPathMode::Deny`]; [`UnapprovedPathMode::Forbid`] still rejects
    /// it.
    pub(crate) fn load_with_meta_transform<'a, G: Send + Sync + 'static>(
        &self,
        path: impl Into<AssetPath<'a>>,
        type_id: TypeId,
        type_name: Option<&str>,
        meta_transform: Option<MetaTransform>,
        guard: G,
        override_unapproved: bool,
    ) -> UntypedHandle {
        let path = path.into().into_owned();
        if path.path() == Path::new("") {
            return UntypedHandle::default_for_type(type_id);
        }
        if path.is_unapproved()
            && !unapproved_allowed(self.data.unapproved_path_mode, override_unapproved)
        {
            return UntypedHandle::default_for_type(type_id);
        }

        let mut infos = self.write_infos();
        let (handle, should_load) = infos.get_or_create_path_handle_erased(
            path.clone(),
            type_id,
            type_name,
            HandleLoadingMode::Request,
        );
        if should_load {
            self.spawn_load_task(handle.clone(), path, infos, meta_transform, guard);
        }
        handle
    }

    /// Spawns the task that performs one load.
    ///
    /// The task is stored in [`AssetInfos::pending_tasks`] so dropping the
    /// handle does not cancel it; it is cancelled only when the store's drop
    /// processing removes the asset, or when the task finishes. `guard` is moved
    /// into the task and dropped once the load settles.
    fn spawn_load_task<G: Send + Sync + 'static>(
        &self,
        handle: UntypedHandle,
        path: AssetPath<'static>,
        mut infos: RwLockWriteGuard<'_, AssetInfos>,
        meta_transform: Option<MetaTransform>,
        guard: G,
    ) {
        let owned_handle = handle.clone();
        let server = self.clone();
        let task = io_task_pool().spawn(async move {
            let _ = server
                .load_internal(Some(owned_handle), path, meta_transform)
                .await;
            drop(guard);
        });
        infos.pending_tasks.insert(handle.id(), task);
    }

    /// Performs one asset load and reports the result over the event channel.
    ///
    /// When `input_handle` is [`Some`], the caller already reserved a handle for
    /// the load; when it is [`None`], the loader is resolved from the path alone
    /// and a handle is created for the loader's asset type. Either way a failure
    /// is reported over the event channel against the id the load resolved, and
    /// returned to the caller.
    ///
    /// The owned `input_handle` is dropped once the load is underway, so if
    /// every other strong handle disappears before it completes,
    /// [`AssetInfos::process_asset_load`] discards the result.
    async fn load_internal(
        &self,
        input_handle: Option<UntypedHandle>,
        path: AssetPath<'static>,
        meta_transform: Option<MetaTransform>,
    ) -> Result<Option<UntypedHandle>, AssetLoadError> {
        let input_handle_type_id = input_handle.as_ref().map(UntypedHandle::type_id);
        let (mut meta, loader, mut reader) = match self
            .get_meta_loader_and_reader(&path, input_handle_type_id)
            .await
        {
            Ok(found) => found,
            Err(error) => {
                return self.fail_load(input_handle.as_ref().map(UntypedHandle::id), &path, error);
            }
        };

        if let Some(meta_transform) = meta_transform {
            meta_transform(&mut *meta);
        }

        // The id the load reports against, plus the handle to return when the
        // caller did not supply one (the untyped case).
        let (asset_id, fetched_handle) = if let Some(input_handle) = input_handle {
            let asset_id = input_handle.id();
            // A handle of the wrong type for this path is a configuration error; a
            // labeled handle is exempt because a label may name a different type.
            if path.label().is_none() && asset_id.type_id() != loader.asset_type_id() {
                let error = AssetLoadError::RequestedHandleTypeMismatch {
                    path: path.clone(),
                    requested: asset_id.type_id(),
                    actual_asset_name: loader.asset_type_name(),
                    loader_name: loader.type_name(),
                };
                return self.fail_load(Some(asset_id), &path, error);
            }
            // Drop our own handle reference now that the load is underway.
            drop(input_handle);
            (Some(asset_id), None)
        } else if path.label().is_none() {
            // The loader's asset type is the only thing that can name the handle.
            let (handle, should_load) = self.write_infos().get_or_create_path_handle_erased(
                path.clone(),
                loader.asset_type_id(),
                Some(loader.asset_type_name()),
                HandleLoadingMode::Request,
            );
            if !should_load {
                return Ok(Some(handle));
            }
            (Some(handle.id()), Some(handle))
        } else {
            // A labeled path's sub-asset type is unknown until the load resolves
            // it, so there is no handle to create (or reuse) up front.
            (None, None)
        };

        // A labeled load also needs the base asset's handle kept alive until the
        // load finishes, or `process_asset_load` would discard the result.
        let (base_asset_id, _base_handle, base_path) = if path.label().is_some() {
            let base_path = path.without_label().into_owned();
            let base_handle = self
                .write_infos()
                .get_or_create_path_handle_erased(
                    base_path.clone(),
                    loader.asset_type_id(),
                    Some(loader.asset_type_name()),
                    HandleLoadingMode::Force,
                )
                .0;
            (base_handle.id(), Some(base_handle), base_path)
        } else {
            (
                asset_id.expect("a non-labeled path always resolves a handle"),
                None,
                path.clone(),
            )
        };

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
            Err(error) => return self.fail_load(asset_id, &path, error),
        };

        let final_handle = if let Some(label) = path.label_cow() {
            match loaded_asset.label_to_asset_index.get(&label) {
                Some(index) => {
                    let labeled = &loaded_asset.labeled_assets[*index];
                    // If we know the requested type then check it matches the
                    // labeled asset; an untyped load has no type to check.
                    if let Some(asset_id) = asset_id
                        && labeled.handle.type_id() != asset_id.type_id()
                    {
                        let error = AssetLoadError::RequestedHandleTypeMismatch {
                            path: path.clone(),
                            requested: asset_id.type_id(),
                            actual_asset_name: labeled.asset.asset_type_name(),
                            loader_name: loader.type_name(),
                        };
                        return self.fail_load(Some(asset_id), &path, error);
                    }
                    Some(labeled.handle.clone())
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
                    return self.fail_load(asset_id, &path, error);
                }
            }
        } else {
            fetched_handle
        };

        self.send_asset_event(InternalAssetEvent::Loaded {
            id: base_asset_id,
            loaded_asset,
        });
        Ok(final_handle)
    }

    /// Reports a failure for `asset_id` (when the load resolved one) over the
    /// event channel, then returns the error to the caller.
    ///
    /// The path is taken by reference because the load's reader borrows it for
    /// the duration of [`AssetServer::load_internal`].
    fn fail_load(
        &self,
        asset_id: Option<UntypedAssetId>,
        path: &AssetPath<'static>,
        error: AssetLoadError,
    ) -> Result<Option<UntypedHandle>, AssetLoadError> {
        if let Some(asset_id) = asset_id {
            self.send_asset_event(InternalAssetEvent::Failed {
                id: asset_id,
                path: path.clone(),
                error: Arc::new(error.clone()),
            });
        }
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
                    let loader = self.get_asset_loader_with_type_name(&loader_name).await?;
                    let meta = loader.deserialize_meta(&meta_bytes).map_err(|error| {
                        AssetLoadError::DeserializeMeta {
                            path: asset_path.clone_owned(),
                            error: Box::new(error),
                        }
                    })?;
                    (meta, loader)
                }
                Err(AssetReaderError::NotFound(_)) => {
                    let loader = self.find_loader(asset_type_id, asset_path).await?;
                    let meta = loader.default_meta();
                    (meta, loader)
                }
                Err(error) => return Err(AssetLoadError::AssetReaderError(error)),
            }
        } else {
            let loader = self.find_loader(asset_type_id, asset_path).await?;
            let meta = loader.default_meta();
            (meta, loader)
        };

        let reader = asset_reader
            .read(asset_path.path())
            .await
            .map_err(AssetLoadError::AssetReaderError)?;
        Ok((meta, loader, reader))
    }

    /// Runs one [`AssetLoader`](crate::AssetLoader) over its reader.
    pub(crate) async fn load_with_settings_loader_and_reader(
        &self,
        asset_path: &AssetPath<'_>,
        settings: &dyn Settings,
        loader: &dyn ErasedAssetLoader,
        reader: &mut dyn Reader,
        load_dependencies: bool,
        populate_hashes: bool,
    ) -> Result<crate::loader::ErasedLoadedAsset, AssetLoadError> {
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
                    let _ = server.load_internal(Some(handle), path.clone(), None).await;
                }
            })
            .detach();
    }

    /// Loads an asset without knowing its type up front.
    ///
    /// The returned handle names a [`LoadedUntypedAsset`] addressed under a
    /// synthesized source (`--untyped`, or `<source>--untyped` for a named one)
    /// so it cannot collide with a typed load of the same path. Resolving the
    /// real asset type needs the loader, so the actual load runs as a task; when
    /// it settles, the resolved handle is placed in the store under this
    /// wrapper.
    ///
    /// `meta_transform` is accepted for parity with the typed path but not yet
    /// applied: kairos does not store it on the handle for hot-reload reuse.
    pub(crate) fn load_unknown_type_with_meta_transform<'a, G: Send + Sync + 'static>(
        &self,
        path: impl Into<AssetPath<'a>>,
        _meta_transform: Option<MetaTransform>,
        guard: G,
        override_unapproved: bool,
    ) -> Handle<LoadedUntypedAsset> {
        let path = path.into().into_owned();
        if path.path() == Path::new("") {
            return Handle::default();
        }
        if path.is_unapproved()
            && !unapproved_allowed(self.data.unapproved_path_mode, override_unapproved)
        {
            return Handle::default();
        }

        let untyped_source = AssetSourceId::Name(match path.source() {
            AssetSourceId::Default => CowArc::Static(UNTYPED_SOURCE_SUFFIX),
            AssetSourceId::Name(source) => {
                CowArc::Owned(format!("{source}--{UNTYPED_SOURCE_SUFFIX}").into())
            }
        });

        let mut infos = self.write_infos();
        let (handle, should_load) = infos.get_or_create_path_handle_erased(
            path.clone().with_source(untyped_source),
            TypeId::of::<LoadedUntypedAsset>(),
            Some(type_name::<LoadedUntypedAsset>()),
            HandleLoadingMode::Request,
        );
        if !should_load {
            return handle.typed_debug_checked();
        }

        let untyped_asset_id = handle.id();
        let server = self.clone();
        let task = io_task_pool().spawn(async move {
            let path_clone = path.clone();
            match server.load_internal(None, path, None).await {
                Ok(Some(resolved_handle)) => {
                    server.send_asset_event(InternalAssetEvent::Loaded {
                        id: untyped_asset_id,
                        loaded_asset: LoadedAsset::new_with_dependencies(LoadedUntypedAsset {
                            handle: resolved_handle,
                        })
                        .into(),
                    });
                }
                Ok(None) => unreachable!("an untyped load always resolves a handle"),
                Err(error) => {
                    server.send_asset_event(InternalAssetEvent::Failed {
                        id: untyped_asset_id,
                        path: path_clone,
                        error: Arc::new(error),
                    });
                }
            }
            drop(guard);
        });
        infos.pending_tasks.insert(untyped_asset_id, task);

        handle.typed_debug_checked()
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
        let erased: crate::loader::ErasedLoadedAsset = asset.into().into();
        self.load_asset_untyped(None, erased).typed_debug_checked()
    }

    /// The type-erased counterpart of [`AssetServer::load_asset`].
    pub(crate) fn load_asset_untyped(
        &self,
        path: Option<AssetPath<'static>>,
        asset: impl Into<crate::loader::ErasedLoadedAsset>,
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

    /// Asynchronously adds an asset to the server, returning a handle to it.
    ///
    /// The handle is available immediately, while the asset is still
    /// [`LoadState::Loading`]; its value appears in the associated [`Assets`]
    /// store once `future` resolves and [`handle_internal_asset_events`] runs.
    ///
    /// A future that resolves to an error puts the asset into
    /// [`LoadState::Failed`] with an [`AssetLoadError::AddAsyncError`].
    #[must_use = "not using the returned strong handle may result in the unexpected release of the asset"]
    pub fn add_async<A: Asset, E: core::error::Error + Send + Sync + 'static>(
        &self,
        future: impl Future<Output = Result<A, E>> + Send + 'static,
    ) -> Handle<A> {
        let mut infos = self.write_infos();
        let handle = infos.create_loading_handle_untyped(TypeId::of::<A>(), type_name::<A>());
        let id = handle.id();

        let sender = self.data.internal_event_sender.clone();
        let task = io_task_pool().spawn(async move {
            match future.await {
                Ok(asset) => {
                    let loaded_asset: crate::loader::ErasedLoadedAsset =
                        LoadedAsset::new_with_dependencies(asset).into();
                    let _ = sender.send(InternalAssetEvent::Loaded { id, loaded_asset });
                }
                Err(error) => {
                    let error = AddAsyncError {
                        error: Arc::new(error),
                    };
                    tracing::error!("{error}");
                    let _ = sender.send(InternalAssetEvent::Failed {
                        id,
                        path: AssetPath::default(),
                        error: Arc::new(AssetLoadError::AddAsyncError(error)),
                    });
                }
            }
        });
        infos.pending_tasks.insert(id, task);

        handle.typed_debug_checked()
    }

    /// Loads every asset under `path` recursively.
    ///
    /// The returned handle names a [`LoadedFolder`] whose value lists the
    /// handles to all discovered assets. Loading the same folder again returns
    /// the same handle; its
    /// [`RecursiveDependencyLoadState`](crate::RecursiveDependencyLoadState)
    /// reports whether every asset it discovered has finished loading too.
    ///
    /// A file that no registered loader can read is skipped rather than failing
    /// the whole folder. An empty directory loads an empty folder; a directory
    /// that does not exist fails the load.
    #[must_use = "not using the returned strong handle may result in the unexpected release of the assets"]
    pub fn load_folder<'a>(&self, path: impl Into<AssetPath<'a>>) -> Handle<LoadedFolder> {
        let path = path.into().into_owned();
        let (handle, should_load) = self.write_infos().get_or_create_path_handle_erased(
            path.clone(),
            TypeId::of::<LoadedFolder>(),
            Some(type_name::<LoadedFolder>()),
            HandleLoadingMode::Request,
        );
        if should_load {
            self.load_folder_internal(handle.id(), path);
        }
        handle.typed_debug_checked()
    }

    /// Spawns the task that walks a folder and collects its assets' handles.
    ///
    /// Kept separate from [`AssetServer::load_folder`] so the hot-reload track
    /// can re-run a folder walk for a handle that already exists.
    pub(crate) fn load_folder_internal(&self, id: UntypedAssetId, path: AssetPath<'static>) {
        /// Walks `path` in `reader`, pushing one handle per discovered asset.
        async fn load_folder<'a>(
            source: AssetSourceId<'static>,
            path: &'a Path,
            reader: &'a dyn ErasedAssetReader,
            server: &'a AssetServer,
            handles: &'a mut Vec<UntypedHandle>,
        ) -> Result<(), AssetLoadError> {
            if reader.is_directory(path).await? {
                let mut path_stream = reader.read_directory(path).await?;
                while let Some(child_path) = path_stream.next().await {
                    if reader.is_directory(&child_path).await? {
                        Box::pin(load_folder(
                            source.clone(),
                            &child_path,
                            reader,
                            server,
                            handles,
                        ))
                        .await?;
                    } else {
                        // Build the path directly instead of parsing a string,
                        // so a `#` in a filename is not read as a label.
                        let asset_path =
                            AssetPath::from_path_buf(child_path).with_source(source.clone());
                        match server.load_builder().load_untyped_async(asset_path).await {
                            Ok(handle) => handles.push(handle),
                            // A file no loader recognizes is not an asset of this
                            // server; skip it instead of failing the folder.
                            Err(
                                AssetLoadError::MissingAssetLoader { .. }
                                | AssetLoadError::MissingAssetLoaderForTypeName(_)
                                | AssetLoadError::MissingAssetLoaderForExtension(_),
                            ) => {}
                            Err(error) => return Err(error),
                        }
                    }
                }
            }
            Ok(())
        }

        let server = self.clone();
        io_task_pool()
            .spawn(async move {
                let result = async {
                    let source = server
                        .get_source(path.source())
                        .map_err(AssetLoadError::MissingAssetSourceError)?;
                    let reader: &dyn ErasedAssetReader = match server.data.mode {
                        AssetServerMode::Unprocessed => source.reader(),
                        AssetServerMode::Processed => source.processed_reader()?,
                    };
                    let mut handles = Vec::new();
                    load_folder(source.id(), path.path(), reader, &server, &mut handles).await?;
                    Ok(handles)
                }
                .await;

                match result {
                    Ok(handles) => server.send_asset_event(InternalAssetEvent::Loaded {
                        id,
                        loaded_asset: LoadedAsset::new_with_dependencies(LoadedFolder { handles })
                            .into(),
                    }),
                    Err(error) => server.send_asset_event(InternalAssetEvent::Failed {
                        id,
                        path,
                        error: Arc::new(error),
                    }),
                }
            })
            .detach();
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

    /// Whether every asset `value` depends on — directly or transitively — has
    /// finished loading.
    ///
    /// Lets callers ask whether all handles held in a resource or component are
    /// ready without naming each one.
    pub fn are_dependencies_loaded(&self, value: &impl VisitAssetDependencies) -> bool {
        let infos = self.read_infos();
        let mut loaded = true;
        value.visit_dependencies(&mut |asset_id| {
            // UUID assets are not tracked by the server, so they count as loaded.
            if matches!(asset_id, UntypedAssetId::Uuid { .. }) {
                return;
            }
            let Some(info) = infos.get(asset_id) else {
                // An id the server no longer knows cannot be proven loaded.
                loaded = false;
                return;
            };
            if !info.rec_dep_load_state.is_loaded() {
                loaded = false;
            }
        });
        loaded
    }

    /// Whether every direct dependency of `value` has finished loading.
    ///
    /// Recursive dependencies are not considered; see
    /// [`are_dependencies_loaded`](Self::are_dependencies_loaded) for those.
    pub fn are_direct_dependencies_loaded(&self, value: &impl VisitAssetDependencies) -> bool {
        let infos = self.read_infos();
        let mut loaded = true;
        value.visit_dependencies(&mut |asset_id| {
            // UUID assets are not tracked by the server, so they count as loaded.
            if matches!(asset_id, UntypedAssetId::Uuid { .. }) {
                return;
            }
            let Some(info) = infos.get(asset_id) else {
                // An id the server no longer knows cannot be proven loaded.
                loaded = false;
                return;
            };
            if !info.dep_load_state.is_loaded() {
                loaded = false;
            }
        });
        loaded
    }

    /// An active handle for `path`, if the asset has started loading or is alive.
    pub fn get_handle<'a, A: Asset>(&self, path: impl Into<AssetPath<'a>>) -> Option<Handle<A>> {
        self.get_path_and_type_id_handle(&path.into(), TypeId::of::<A>())
            .map(UntypedHandle::typed_debug_checked)
    }

    /// An active untyped handle for `path`, if any asset at that path has
    /// started loading or is still alive.
    ///
    /// Returns the first handle when several asset types are registered for one
    /// path; see [`get_handles_untyped`](Self::get_handles_untyped) for all of
    /// them.
    pub fn get_handle_untyped<'a>(&self, path: impl Into<AssetPath<'a>>) -> Option<UntypedHandle> {
        self.read_infos()
            .get_handles_untyped(&path.into())
            .into_iter()
            .next()
    }

    /// An active handle for `id`, if the server manages that asset.
    pub fn get_id_handle<A: Asset>(&self, id: AssetId<A>) -> Option<Handle<A>> {
        self.get_id_handle_untyped(id.untyped())
            .map(UntypedHandle::typed_debug_checked)
    }

    /// The type-erased counterpart of [`AssetServer::get_id_handle`].
    pub fn get_id_handle_untyped(&self, id: UntypedAssetId) -> Option<UntypedHandle> {
        self.read_infos().get_index_handle(id)
    }

    /// Whether `id` names an asset managed by this server.
    pub fn is_managed(&self, id: impl Into<UntypedAssetId>) -> bool {
        self.read_infos().contains_key(id)
    }

    /// An active untyped asset id for `path`, if any asset at that path has
    /// started loading or is still alive.
    ///
    /// Returns the first id when several assets are registered for one path; see
    /// [`get_path_ids`](Self::get_path_ids) for all of them.
    pub fn get_path_id<'a>(&self, path: impl Into<AssetPath<'a>>) -> Option<UntypedAssetId> {
        self.read_infos()
            .get_path_ids(&path.into())
            .into_iter()
            .next()
    }

    /// Every active untyped asset id for `path`, across every asset type.
    pub fn get_path_ids<'a>(&self, path: impl Into<AssetPath<'a>>) -> Vec<UntypedAssetId> {
        self.read_infos().get_path_ids(&path.into())
    }

    /// Every active untyped handle for `path`, across every asset type.
    pub fn get_handles_untyped<'a>(&self, path: impl Into<AssetPath<'a>>) -> Vec<UntypedHandle> {
        self.read_infos().get_handles_untyped(&path.into())
    }

    /// An active handle for `path` and `type_id`.
    pub fn get_path_and_type_id_handle(
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
///
/// A load can combine several modifiers, for example:
///
/// ```ignore
/// asset_server
///     .load_builder()
///     .with_settings(settings)
///     .override_unapproved()
///     .load("my.path")
/// ```
#[must_use = "the load doesn't start until LoadBuilder has been consumed"]
pub struct LoadBuilder<'a> {
    asset_server: &'a AssetServer,
    meta_transform: Option<MetaTransform>,
    /// Whether an unapproved path may load even when the server is in
    /// [`UnapprovedPathMode::Deny`].
    override_unapproved: bool,
    /// A guard held until the load settles.
    guard: Option<Box<dyn Send + Sync + 'static>>,
}

impl<'a> LoadBuilder<'a> {
    fn new(asset_server: &'a AssetServer) -> Self {
        Self {
            asset_server,
            meta_transform: None,
            override_unapproved: false,
            guard: None,
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

    /// Allows this load to read a path that escapes its source even when the
    /// server is in [`UnapprovedPathMode::Deny`].
    ///
    /// [`UnapprovedPathMode::Forbid`] is unaffected: it rejects unapproved paths
    /// whatever the builder asks for.
    pub fn override_unapproved(mut self) -> Self {
        self.override_unapproved = true;
        self
    }

    /// Holds `guard` until the load finishes (successfully or not), then drops
    /// it.
    ///
    /// The guard's [`Drop`] can therefore signal completion to the caller. Only
    /// the last guard is kept; adding a second drops the first before the load
    /// begins.
    pub fn with_guard(mut self, guard: impl Send + Sync + 'static) -> Self {
        self.guard = Some(Box::new(guard));
        self
    }

    /// Starts the load and returns the asset's handle.
    #[must_use = "not using the returned strong handle may result in the unexpected release of the asset"]
    pub fn load<'b, A: Asset>(self, asset_path: impl Into<AssetPath<'b>>) -> Handle<A> {
        self.load_typed_internal(TypeId::of::<A>(), Some(type_name::<A>()), asset_path.into())
            .typed_debug_checked()
    }

    /// Same as [`load`](Self::load), but the asset type is given at runtime as a
    /// [`TypeId`] rather than statically.
    #[must_use = "not using the returned strong handle may result in the unexpected release of the asset"]
    pub fn load_erased<'b>(
        self,
        type_id: TypeId,
        asset_path: impl Into<AssetPath<'b>>,
    ) -> UntypedHandle {
        self.load_typed_internal(type_id, None, asset_path.into())
    }

    /// Loads an asset without knowing its type, returning a handle to a
    /// [`LoadedUntypedAsset`].
    ///
    /// Once that wrapper has loaded, [`LoadedUntypedAsset::handle`] names the
    /// asset at the requested path. The indirection is unavoidable because the
    /// asset type is only known after the loader has been resolved.
    #[must_use = "not using the returned strong handle may result in the unexpected release of the asset"]
    pub fn load_untyped<'b>(
        self,
        asset_path: impl Into<AssetPath<'b>>,
    ) -> Handle<LoadedUntypedAsset> {
        let Self {
            asset_server,
            meta_transform,
            override_unapproved,
            guard,
        } = self;
        let asset_path = asset_path.into();
        asset_server.load_unknown_type_with_meta_transform(
            asset_path,
            meta_transform,
            guard,
            override_unapproved,
        )
    }

    /// Asynchronously loads an asset without knowing its type, returning the
    /// resolved untyped handle once the load finishes.
    ///
    /// # Errors
    ///
    /// Returns [`AssetLoadError::EmptyPath`] for a path with no file component,
    /// [`AssetLoadError::UnapprovedPath`] for a path the server will not load,
    /// or whatever error the load failed with.
    #[must_use = "not using the returned strong handle may result in the unexpected release of the asset"]
    pub async fn load_untyped_async<'b>(
        self,
        asset_path: impl Into<AssetPath<'b>>,
    ) -> Result<UntypedHandle, AssetLoadError> {
        // The guard (if any) is held across the await and dropped when this
        // function returns, so it covers the whole load.
        let Self {
            asset_server,
            guard: _guard,
            override_unapproved,
            ..
        } = self;
        let path: AssetPath = asset_path.into();
        if path.path() == Path::new("") {
            return Err(AssetLoadError::EmptyPath(path.into_owned()));
        }
        if path.is_unapproved()
            && !unapproved_allowed(asset_server.data.unapproved_path_mode, override_unapproved)
        {
            return Err(AssetLoadError::UnapprovedPath {
                path: path.into_owned(),
            });
        }

        match asset_server
            .load_internal(None, path.into_owned(), None)
            .await
        {
            Ok(Some(handle)) => Ok(handle),
            Ok(None) => unreachable!("an untyped load always resolves a handle"),
            Err(error) => Err(error),
        }
    }

    /// Starts a (deferred) load for an asset with the given `type_id`.
    fn load_typed_internal(
        self,
        type_id: TypeId,
        type_name: Option<&str>,
        asset_path: AssetPath<'_>,
    ) -> UntypedHandle {
        let Self {
            asset_server,
            meta_transform,
            override_unapproved,
            guard,
        } = self;
        asset_server.load_with_meta_transform(
            asset_path,
            type_id,
            type_name,
            meta_transform,
            guard,
            override_unapproved,
        )
    }
}

/// Processes the load results that load tasks have sent back to the server.
///
/// This is the main-thread half of the pipeline: it inserts loaded values into
/// their [`Assets<A>`](crate::Assets) stores and advances load state. It
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
pub(crate) fn io_task_pool() -> &'static IoTaskPool {
    IoTaskPool::get_or_init(TaskPool::default)
}

/// Appended to an asset source name for an untyped load, so the
/// [`LoadedUntypedAsset`] wrapper cannot collide with a typed load of the same
/// path (bevy parity).
const UNTYPED_SOURCE_SUFFIX: &str = "--untyped";

/// Whether the server may load an unapproved path: [`Allow`] always may, and
/// [`Deny`] may only when the load explicitly overrides it. [`Forbid`] never may.
///
/// [`Allow`]: UnapprovedPathMode::Allow
/// [`Deny`]: UnapprovedPathMode::Deny
/// [`Forbid`]: UnapprovedPathMode::Forbid
fn unapproved_allowed(mode: UnapprovedPathMode, override_unapproved: bool) -> bool {
    matches!(
        (mode, override_unapproved),
        (UnapprovedPathMode::Allow, _) | (UnapprovedPathMode::Deny, true)
    )
}

/// The results a load task sends back to the main thread.
pub(crate) enum InternalAssetEvent {
    /// A load succeeded; the value should be inserted into its store.
    Loaded {
        /// The id of the asset that finished loading.
        id: UntypedAssetId,
        /// The loaded value, its dependencies, and its labeled assets.
        loaded_asset: crate::loader::ErasedLoadedAsset,
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
///
/// `Clone` lets the same failure be both reported over the event channel and
/// returned to the caller (mirroring `bevy_asset`).
#[non_exhaustive]
#[derive(Error, Debug, Clone)]
pub enum AssetLoadError {
    /// A load was requested for a path with no file component.
    #[error("Attempted to load an asset with an empty path \"{0}\"")]
    EmptyPath(AssetPath<'static>),
    /// A load was requested for an unapproved path while the server forbade it.
    #[error("Asset path {path} is unapproved. See UnapprovedPathMode for details.")]
    UnapprovedPath {
        /// The path that was requested.
        path: AssetPath<'static>,
    },
    /// A handle of the wrong asset type was requested for a path.
    #[error(
        "Requested handle of type {requested:?} for asset '{path}' does not match actual asset type '{actual_asset_name}', which used loader '{loader_name}'"
    )]
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
    #[error(
        "Could not find an asset loader matching: Asset Type: {asset_type_id:?}; Path: {asset_path:?};"
    )]
    MissingAssetLoader {
        /// The asset type the caller asked for, if it was known.
        asset_type_id: Option<TypeId>,
        /// The path that was being loaded.
        asset_path: String,
    },
    /// No loader is registered under the requested name.
    #[error(transparent)]
    MissingAssetLoaderForTypeName(#[from] MissingAssetLoaderForTypeNameError),
    /// No loader is registered for the requested extension.
    #[error(transparent)]
    MissingAssetLoaderForExtension(#[from] MissingAssetLoaderForExtensionError),
    /// No loader is registered for the requested asset type id.
    #[error(transparent)]
    MissingAssetLoaderForTypeIdError(#[from] MissingAssetLoaderForTypeIdError),
    /// Reading the asset's bytes failed.
    #[error("{0}")]
    AssetReaderError(AssetReaderError),
    /// The asset's source does not exist.
    #[error("{0}")]
    MissingAssetSourceError(MissingAssetSourceError),
    /// The asset's source has no processed reader.
    #[error("{0}")]
    MissingProcessedAssetReaderError(MissingProcessedAssetReaderError),
    /// Reading the asset's meta sidecar failed.
    #[error("Encountered an error while reading asset metadata bytes")]
    AssetMetaReadError,
    /// The asset's meta sidecar could not be deserialized.
    #[error("Failed to deserialize meta for asset {path}: {error}")]
    DeserializeMeta {
        /// The path whose meta failed to parse.
        path: AssetPath<'static>,
        /// The underlying deserialization error.
        error: Box<DeserializeMetaError>,
    },
    /// The asset is configured to be processed and cannot be loaded directly.
    #[error("Asset '{path}' is configured to be processed. It cannot be loaded directly.")]
    CannotLoadProcessedAsset {
        /// The path of the asset.
        path: AssetPath<'static>,
    },
    /// The asset is configured to be ignored and cannot be loaded.
    #[error("Asset '{path}' is configured to be ignored. It cannot be loaded.")]
    CannotLoadIgnoredAsset {
        /// The path of the asset.
        path: AssetPath<'static>,
    },
    /// The loader panicked.
    #[error("Failed to load asset '{path}', asset loader '{loader_name}' panicked")]
    AssetLoaderPanic {
        /// The path of the asset that was loading.
        path: AssetPath<'static>,
        /// The name of the loader that panicked.
        loader_name: &'static str,
    },
    /// The loader itself returned an error.
    #[error("{0}")]
    AssetLoaderError(AssetLoaderError),
    /// Resolving an asset added by [`AssetServer::add_async`] failed.
    #[error(transparent)]
    AddAsyncError(#[from] AddAsyncError),
    /// The requested label does not exist on the loaded asset.
    #[error(
        "The file at '{base_path}' does not contain the labeled asset '{label}'; it contains the following {} assets: {}",
        all_labels.len(),
        all_labels.iter().map(|label| format!("'{label}'")).collect::<Vec<_>>().join(", ")
    )]
    MissingLabel {
        /// The path of the asset that was loaded.
        base_path: AssetPath<'static>,
        /// The label that was requested.
        label: String,
        /// The labels the asset did expose.
        all_labels: Vec<String>,
    },
}

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

/// An error that occurs when an
/// [`AssetLoader`](crate::AssetLoader) is not registered for a given extension.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("no `AssetLoader` found{}", format_missing_asset_ext(extensions))]
pub struct MissingAssetLoaderForExtensionError {
    extensions: Vec<String>,
}

/// Formats the extensions a loader was looked up by for the error message.
fn format_missing_asset_ext(extensions: &[String]) -> String {
    if !extensions.is_empty() {
        format!(
            " for the following extension{}: {}",
            if extensions.len() > 1 { "s" } else { "" },
            extensions.join(", ")
        )
    } else {
        " for file with no extension".to_string()
    }
}

/// An error that occurs when an
/// [`AssetLoader`](crate::AssetLoader) is not registered under a given loader
/// name.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("no `AssetLoader` found with the name '{type_name}'")]
pub struct MissingAssetLoaderForTypeNameError {
    /// The loader name that was not found.
    pub type_name: String,
}

/// An error that occurs when an
/// [`AssetLoader`](crate::AssetLoader) is not registered for a given asset
/// [`TypeId`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("no `AssetLoader` found with the ID '{type_id:?}'")]
pub struct MissingAssetLoaderForTypeIdError {
    /// The asset type id that was not found.
    pub type_id: TypeId,
}

/// The failure of one [`AssetLoader`](crate::AssetLoader).
///
/// The loader's error type only has to convert into [`KairosError`], so it is
/// erased into one here; `Arc` lets clones share it across the dependent states
/// that reference the same failure.
#[derive(Error, Debug, Clone)]
#[error(
    "Failed to load asset '{}' with asset loader '{}': {}",
    path,
    loader_name,
    error
)]
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

/// An error that occurred while resolving an asset added by
/// [`AssetServer::add_async`].
///
/// The future's error type is erased into a trait object here so it can travel
/// through [`AssetLoadError`] without naming the caller's error type.
#[derive(Error, Debug, Clone)]
#[error("An error occurred while resolving an asset added by `add_async`: {error}")]
pub struct AddAsyncError {
    error: Arc<dyn core::error::Error + Send + Sync + 'static>,
}
