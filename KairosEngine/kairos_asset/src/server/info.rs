//! The per-asset information the server tracks: path-addressed handles, the
//! handle providers that allocate them, each asset's load state, and the
//! dependency bookkeeping that lets a load wait for the assets it needs.
//!
//! An asset's load state lives here rather than as a world component: the store
//! ([`Assets<A>`](crate::Assets)) owns the values, the server owns what is
//! known about loads. This mirrors `bevy_asset`'s `server/info.rs`, with the
//! asset identity represented by [`UntypedAssetId`] instead of a dedicated
//! erased-index type.

use core::any::TypeId;
use std::{
    collections::{HashMap, HashSet, hash_map::Entry},
    sync::{Arc, Weak},
};

use crossbeam_channel::Sender;
use kairos_ecs::world::World;
use kairos_tasks::Task;

use crate::asset::Asset;
use crate::handle::{AssetHandleProvider, Handle, StrongHandle, UntypedHandle};
use crate::id::UntypedAssetId;
use crate::index::AssetIndexAllocator;
use crate::loader::ErasedLoadedAsset;
use crate::path::AssetPath;

use super::{AssetLoadError, InternalAssetEvent};

/// What the server knows about one asset.
#[derive(Debug)]
pub(crate) struct AssetInfo {
    /// A non-owning reference to the asset's current strong handle, used to tell
    /// whether any live handle still wants the asset.
    pub(crate) weak_handle: Weak<StrongHandle>,
    /// The path the asset was requested from, if it was loaded by path rather
    /// than added directly.
    pub(crate) path: Option<AssetPath<'static>>,
    /// How far the asset's own load has got.
    pub(crate) load_state: LoadState,
    /// How far its direct dependencies' loads have got.
    pub(crate) dep_load_state: DependencyLoadState,
    /// How far its recursive dependencies' loads have got.
    pub(crate) rec_dep_load_state: RecursiveDependencyLoadState,
    /// Direct dependencies whose own load is not finished yet.
    loading_dependencies: HashSet<UntypedAssetId>,
    /// Recursive dependencies whose load is not finished yet.
    loading_rec_dependencies: HashSet<UntypedAssetId>,
    /// Recursive dependencies that failed to load.
    failed_rec_dependencies: HashSet<UntypedAssetId>,
    /// Assets waiting for this asset's own load to finish.
    dependents_waiting_on_load: HashSet<UntypedAssetId>,
    /// Assets waiting for this asset's recursive dependencies to finish.
    dependents_waiting_on_recursive_dep_load: HashSet<UntypedAssetId>,
}

impl AssetInfo {
    fn new(weak_handle: Weak<StrongHandle>, path: Option<AssetPath<'static>>) -> Self {
        Self {
            weak_handle,
            path,
            load_state: LoadState::NotLoaded,
            dep_load_state: DependencyLoadState::NotLoaded,
            rec_dep_load_state: RecursiveDependencyLoadState::NotLoaded,
            loading_dependencies: HashSet::default(),
            loading_rec_dependencies: HashSet::default(),
            failed_rec_dependencies: HashSet::default(),
            dependents_waiting_on_load: HashSet::default(),
            dependents_waiting_on_recursive_dep_load: HashSet::default(),
        }
    }

    /// Moves the asset into [`LoadState::Loading`] across all three load states.
    fn mark_loading(&mut self) {
        self.load_state = LoadState::Loading;
        self.dep_load_state = DependencyLoadState::Loading;
        self.rec_dep_load_state = RecursiveDependencyLoadState::Loading;
    }
}

/// The server's per-asset bookkeeping: path-addressed handles, the handle
/// providers that allocate them, and the load state of every known asset.
///
/// This is an internal type; callers reach it through
/// [`AssetServer`](super::AssetServer) or its public state accessors.
#[derive(Default)]
pub(crate) struct AssetInfos {
    /// Which id each `(path, asset type)` pair resolves to, so repeated
    /// requests for the same address reuse one handle.
    path_to_index: HashMap<AssetPath<'static>, HashMap<TypeId, UntypedAssetId>>,
    /// Every known asset's information, keyed by id.
    infos: HashMap<UntypedAssetId, AssetInfo>,
    /// The handle provider for each asset type. A type must have a provider
    /// before any handle for it can be allocated.
    pub(crate) handle_providers: HashMap<TypeId, AssetHandleProvider>,
    /// Whether to track data needed for hot-reloading. Set once at startup.
    pub(crate) watching_for_changes: bool,
    /// Writes a typed [`AssetEvent::LoadedWithDependencies`] for an asset of
    /// each registered type.
    pub(crate) dependency_loaded_event_sender:
        HashMap<TypeId, fn(&mut World, UntypedAssetId)>,
    /// Writes a typed [`AssetLoadFailedEvent`] for an asset of each registered
    /// type.
    pub(crate) dependency_failed_event_sender:
        HashMap<TypeId, fn(&mut World, UntypedAssetId, AssetPath<'static>, Arc<AssetLoadError>)>,
    /// The in-flight load tasks, kept alive so they are not cancelled.
    pub(crate) pending_tasks: HashMap<UntypedAssetId, Task<()>>,
}

impl core::fmt::Debug for AssetInfos {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AssetInfos")
            .field("path_to_index", &self.path_to_index)
            .field("infos", &self.infos)
            .finish_non_exhaustive()
    }
}

impl AssetInfos {
    /// Registers a fresh handle provider for `A`, so handles for that type can
    /// be allocated. Does nothing if one is already registered.
    #[allow(dead_code)] // used by tests and the `Assets`-less processor path
    pub(crate) fn register_handle_provider<A: Asset>(&mut self) {
        self.handle_providers
            .entry(TypeId::of::<A>())
            .or_insert_with(|| {
                AssetHandleProvider::new(TypeId::of::<A>(), Arc::new(AssetIndexAllocator::default()))
            });
    }

    /// Registers `provider`, replacing any earlier one for its asset type.
    ///
    /// [`AssetServer::register_asset`](super::AssetServer::register_asset) calls
    /// this with the provider owned by the asset's [`Assets<A>`] store, so the
    /// server allocates slots from the same allocator the store inserts into.
    ///
    /// [`Assets<A>`]: crate::Assets
    pub(crate) fn register_handle_provider_erased(&mut self, provider: AssetHandleProvider) {
        let type_id = provider.type_id();
        self.handle_providers.insert(type_id, provider);
    }

    /// Returns the handle for `path` and `A`, creating one — and recording a
    /// fresh [`AssetInfo`] — if no live handle for that pair exists yet.
    ///
    /// This never starts a load; see
    /// [`AssetServer::get_or_create_path_handle`](super::AssetServer::get_or_create_path_handle).
    pub(crate) fn get_or_create_path_handle<A: Asset>(
        &mut self,
        path: AssetPath<'static>,
    ) -> (Handle<A>, bool) {
        let (handle, created) = self.get_or_create_path_handle_erased(
            path,
            TypeId::of::<A>(),
            Some(core::any::type_name::<A>()),
            HandleLoadingMode::NotLoading,
        );
        (handle.typed_debug_checked(), created)
    }

    /// The type-erased counterpart of [`AssetInfos::get_or_create_path_handle`].
    ///
    /// The returned `bool` is `true` when the load should be kicked off: either
    /// the handle was freshly created in a loading mode, or an existing handle
    /// was moved back into [`LoadState::Loading`].
    ///
    /// # Panics
    ///
    /// Panics if no handle provider has been registered for `type_id`.
    pub(crate) fn get_or_create_path_handle_erased(
        &mut self,
        path: AssetPath<'static>,
        type_id: TypeId,
        type_name: Option<&str>,
        loading_mode: HandleLoadingMode,
    ) -> (UntypedHandle, bool) {
        let handles = self.path_to_index.entry(path.clone()).or_default();

        match handles.entry(type_id) {
            Entry::Occupied(entry) => {
                let id = *entry.get();
                let provider = self
                    .handle_providers
                    .get(&type_id)
                    .cloned()
                    .unwrap_or_else(|| missing_provider(type_id, type_name));
                // `path_to_index` and `infos` are kept in lockstep.
                let info = self
                    .infos
                    .get_mut(&id)
                    .expect("a path-registered id always has an AssetInfo");

                let should_load = match loading_mode {
                    HandleLoadingMode::Force => true,
                    HandleLoadingMode::Request
                        if matches!(info.load_state, LoadState::NotLoaded | LoadState::Failed(_)) =>
                    {
                        true
                    }
                    HandleLoadingMode::Request | HandleLoadingMode::NotLoading => false,
                };
                if should_load {
                    info.mark_loading();
                }

                if let Some(strong) = info.weak_handle.upgrade() {
                    (UntypedHandle::Strong(strong), should_load)
                } else {
                    // Every live handle was dropped while the info lingered;
                    // re-open the slot for the caller asking to load it again.
                    let UntypedAssetId::Index { index, .. } = id else {
                        unreachable!("path-registered ids are always strong")
                    };
                    let handle = provider.get_handle(index);
                    info.weak_handle = Arc::downgrade(&handle);
                    (UntypedHandle::Strong(handle), should_load)
                }
            }
            Entry::Vacant(entry) => {
                let should_load = match loading_mode {
                    HandleLoadingMode::NotLoading => false,
                    HandleLoadingMode::Request | HandleLoadingMode::Force => true,
                };
                let handle = Self::create_handle_internal(
                    &mut self.infos,
                    &self.handle_providers,
                    type_id,
                    Some(path),
                    should_load,
                    type_name,
                );
                entry.insert(handle.id());
                (handle, should_load)
            }
        }
    }

    /// Creates a strong handle for an asset with no path (one [`add`]ed
    /// directly), and records it as loading.
    ///
    /// [`add`]: super::AssetServer::add
    pub(crate) fn create_loading_handle_untyped(
        &mut self,
        type_id: TypeId,
        type_name: &'static str,
    ) -> UntypedHandle {
        Self::create_handle_internal(
            &mut self.infos,
            &self.handle_providers,
            type_id,
            None,
            true,
            Some(type_name),
        )
    }

    /// Reserves a strong handle and records its [`AssetInfo`].
    fn create_handle_internal(
        infos: &mut HashMap<UntypedAssetId, AssetInfo>,
        handle_providers: &HashMap<TypeId, AssetHandleProvider>,
        type_id: TypeId,
        path: Option<AssetPath<'static>>,
        loading: bool,
        type_name: Option<&str>,
    ) -> UntypedHandle {
        let provider = handle_providers
            .get(&type_id)
            .unwrap_or_else(|| missing_provider(type_id, type_name));
        let handle = provider.reserve_handle();
        let weak_handle = match &handle {
            UntypedHandle::Strong(strong) => Arc::downgrade(strong),
            UntypedHandle::Uuid { .. } => unreachable!("reserve_handle returns a strong handle"),
        };

        let mut info = AssetInfo::new(weak_handle, path);
        if loading {
            info.mark_loading();
        }
        infos.insert(handle.id(), info);
        handle
    }

    /// The strong handles currently registered for `path`, across every type.
    pub(crate) fn get_handles_untyped(&self, path: &AssetPath<'_>) -> Vec<UntypedHandle> {
        let Some(by_type) = self.path_to_index.get(&path.clone_owned()) else {
            return Vec::new();
        };
        by_type.values().filter_map(|id| self.get_index_handle(*id)).collect()
    }

    /// The handle for `path` and `type_id`, if one is registered and alive.
    pub(crate) fn get_path_and_type_id_handle(
        &self,
        path: &AssetPath<'_>,
        type_id: TypeId,
    ) -> Option<UntypedHandle> {
        let id = *self.path_to_index.get(&path.clone_owned())?.get(&type_id)?;
        self.get_index_handle(id)
    }

    /// Upgrades `id`'s weak handle into a strong one, if the asset is alive.
    pub(crate) fn get_index_handle(&self, id: UntypedAssetId) -> Option<UntypedHandle> {
        let info = self.infos.get(&id)?;
        Some(UntypedHandle::Strong(info.weak_handle.upgrade()?))
    }

    /// The information recorded for `id`, if any.
    pub(crate) fn get(&self, id: impl Into<UntypedAssetId>) -> Option<&AssetInfo> {
        self.infos.get(&id.into())
    }

    /// The information recorded for `id`, mutably.
    pub(crate) fn get_mut(&mut self, id: impl Into<UntypedAssetId>) -> Option<&mut AssetInfo> {
        self.infos.get_mut(&id.into())
    }

    /// Whether `id` names an asset managed by this server.
    pub(crate) fn contains_key(&self, id: impl Into<UntypedAssetId>) -> bool {
        self.infos.contains_key(&id.into())
    }

    /// The load state of `id`, or [`None`] if the asset is unknown.
    pub(crate) fn load_state(&self, id: impl Into<UntypedAssetId>) -> Option<LoadState> {
        self.get(id).map(|info| info.load_state.clone())
    }

    /// The dependency load state of `id`, or [`None`] if the asset is unknown.
    pub(crate) fn dependency_load_state(
        &self,
        id: impl Into<UntypedAssetId>,
    ) -> Option<DependencyLoadState> {
        self.get(id).map(|info| info.dep_load_state.clone())
    }

    /// The recursive dependency load state of `id`, or [`None`] if the asset is
    /// unknown.
    pub(crate) fn recursive_dependency_load_state(
        &self,
        id: impl Into<UntypedAssetId>,
    ) -> Option<RecursiveDependencyLoadState> {
        self.get(id).map(|info| info.rec_dep_load_state.clone())
    }

    /// Whether `id` has finished loading.
    #[allow(dead_code)] // the server exposes the same check over its public API
    pub(crate) fn is_loaded(&self, id: impl Into<UntypedAssetId>) -> bool {
        matches!(self.load_state(id), Some(LoadState::Loaded))
    }

    /// Whether `id`, its direct dependencies, and its recursive dependencies
    /// have all finished loading.
    #[allow(dead_code)] // the server exposes the same check over its public API
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
    /// Used by tests to put an asset into a state the loading pipeline produces.
    #[allow(dead_code)] // used by tests
    pub(crate) fn set_load_state(&mut self, id: impl Into<UntypedAssetId>, state: LoadState) {
        if let Some(info) = self.infos.get_mut(&id.into()) {
            info.load_state = state;
        }
    }

    /// Whether any live strong handle still names `path`.
    pub(crate) fn is_path_alive(&self, path: &AssetPath<'_>) -> bool {
        let Some(by_type) = self.path_to_index.get(&path.clone_owned()) else {
            return false;
        };
        by_type.values().any(|id| {
            self.infos
                .get(id)
                .is_some_and(|info| info.weak_handle.strong_count() > 0)
        })
    }

    /// Whether the asset at `path` should be reloaded: it is either still alive
    /// or still loading.
    pub(crate) fn should_reload(&self, path: &AssetPath<'_>) -> bool {
        self.is_path_alive(path)
    }

    /// Removes an asset's bookkeeping, keeping `path_to_index` in lockstep.
    fn remove_info(&mut self, id: UntypedAssetId) {
        if let Some(info) = self.infos.remove(&id)
            && let Some(path) = info.path
        {
            let path_is_empty = match self.path_to_index.get_mut(&path) {
                Some(by_type) => {
                    by_type.remove(&id.type_id());
                    by_type.is_empty()
                }
                None => false,
            };
            if path_is_empty {
                self.path_to_index.remove(&path);
            }
        }
        self.pending_tasks.remove(&id);
    }

    /// Records that an asset finished loading, inserting its value into its
    /// store and updating dependency state across the tree.
    pub(crate) fn process_asset_load(
        &mut self,
        loaded_id: UntypedAssetId,
        loaded_asset: ErasedLoadedAsset,
        world: &mut World,
        sender: &Sender<InternalAssetEvent>,
    ) {
        let ErasedLoadedAsset {
            value,
            dependencies,
            labeled_assets,
            ..
        } = loaded_asset;

        // Labeled assets are processed first so they are not skipped when the
        // root's handle has already been dropped.
        for labeled in labeled_assets {
            let id = labeled.handle.id();
            self.process_asset_load(id, labeled.asset, world, sender);
        }

        let UntypedAssetId::Index { index, .. } = loaded_id else {
            // UUID assets are never managed by the loading pipeline.
            return;
        };
        if !self.infos.contains_key(&loaded_id) {
            // The load was cancelled (its handle was dropped and processed).
            return;
        }
        let alive = self
            .infos
            .get(&loaded_id)
            .is_some_and(|info| info.weak_handle.strong_count() > 0);
        if !alive {
            // Every strong handle was dropped before the load completed: the
            // value is unwanted, so drop it and forget the asset.
            self.remove_info(loaded_id);
            return;
        }

        value.insert(index, world);

        let mut loading_deps = dependencies;
        let mut failed_deps: HashSet<UntypedAssetId> = HashSet::default();
        let mut dep_error: Option<Arc<AssetLoadError>> = None;
        let mut loading_rec_deps = loading_deps.clone();
        let mut failed_rec_deps = HashSet::default();
        let mut rec_dep_error: Option<Arc<AssetLoadError>> = None;

        loading_deps.retain(|dep_id| {
            let Some(dep_info) = self.get_mut(*dep_id) else {
                // An unknown dependency can never finish loading; wait forever
                // rather than silently reporting the asset ready.
                return true;
            };

            match dep_info.rec_dep_load_state {
                RecursiveDependencyLoadState::Loading | RecursiveDependencyLoadState::NotLoaded => {
                    dep_info
                        .dependents_waiting_on_recursive_dep_load
                        .insert(loaded_id);
                }
                RecursiveDependencyLoadState::Loaded => {
                    loading_rec_deps.remove(dep_id);
                }
                RecursiveDependencyLoadState::Failed(ref error) => {
                    if rec_dep_error.is_none() {
                        rec_dep_error = Some(error.clone());
                    }
                    failed_rec_deps.insert(*dep_id);
                    loading_rec_deps.remove(dep_id);
                }
            }

            match dep_info.load_state {
                LoadState::NotLoaded | LoadState::Loading => {
                    dep_info.dependents_waiting_on_load.insert(loaded_id);
                    true
                }
                LoadState::Loaded => false,
                LoadState::Failed(ref error) => {
                    if dep_error.is_none() {
                        dep_error = Some(error.clone());
                    }
                    failed_deps.insert(*dep_id);
                    false
                }
            }
        });

        let dep_load_state = match (loading_deps.len(), failed_deps.len()) {
            (0, 0) => DependencyLoadState::Loaded,
            (_, 0) => DependencyLoadState::Loading,
            (_, _) => DependencyLoadState::Failed(
                dep_error.expect("a failed dependency always carries an error"),
            ),
        };

        let rec_dep_load_state = match (loading_rec_deps.len(), failed_rec_deps.len()) {
            (0, 0) => {
                // Everything this asset needs is ready: announce it.
                let _ = sender.send(InternalAssetEvent::LoadedWithDependencies { id: loaded_id });
                RecursiveDependencyLoadState::Loaded
            }
            (_, 0) => RecursiveDependencyLoadState::Loading,
            (_, _) => RecursiveDependencyLoadState::Failed(
                rec_dep_error.expect("a failed recursive dependency always carries an error"),
            ),
        };

        let (dependents_waiting_on_load, dependents_waiting_on_rec_load) = {
            let info = self
                .infos
                .get_mut(&loaded_id)
                .expect("the asset's info was just checked");
            info.loading_dependencies = loading_deps;
            info.loading_rec_dependencies = loading_rec_deps;
            info.failed_rec_dependencies = failed_rec_deps;
            info.load_state = LoadState::Loaded;
            info.dep_load_state = dep_load_state;
            info.rec_dep_load_state = rec_dep_load_state.clone();

            let rec_waiting = if rec_dep_load_state.is_loaded() || rec_dep_load_state.is_failed() {
                Some(std::mem::take(
                    &mut info.dependents_waiting_on_recursive_dep_load,
                ))
            } else {
                None
            };
            (
                std::mem::take(&mut info.dependents_waiting_on_load),
                rec_waiting,
            )
        };

        for waiting_id in dependents_waiting_on_load {
            if let Some(info) = self.get_mut(waiting_id) {
                info.loading_dependencies.remove(&loaded_id);
                if info.loading_dependencies.is_empty() && !info.dep_load_state.is_failed() {
                    info.dep_load_state = DependencyLoadState::Loaded;
                }
            }
        }

        if let Some(dependents_waiting_on_rec_load) = dependents_waiting_on_rec_load {
            match rec_dep_load_state {
                RecursiveDependencyLoadState::Loaded => {
                    for waiting_id in dependents_waiting_on_rec_load {
                        Self::propagate_loaded_state(self, loaded_id, waiting_id, sender);
                    }
                }
                RecursiveDependencyLoadState::Failed(ref error) => {
                    for waiting_id in dependents_waiting_on_rec_load {
                        Self::propagate_failed_state(self, loaded_id, waiting_id, error);
                    }
                }
                RecursiveDependencyLoadState::Loading | RecursiveDependencyLoadState::NotLoaded => {
                    unreachable!("only a settled recursive state is propagated")
                }
            }
        }
    }

    /// Walks `loaded_id`'s completion up the tree to `waiting_id`.
    fn propagate_loaded_state(
        infos: &mut AssetInfos,
        loaded_id: UntypedAssetId,
        waiting_id: UntypedAssetId,
        sender: &Sender<InternalAssetEvent>,
    ) {
        let dependents_waiting_on_rec_load = if let Some(info) = infos.get_mut(waiting_id) {
            info.loading_rec_dependencies.remove(&loaded_id);
            if info.loading_rec_dependencies.is_empty() && info.failed_rec_dependencies.is_empty() {
                info.rec_dep_load_state = RecursiveDependencyLoadState::Loaded;
                if info.load_state.is_loaded() {
                    let _ = sender.send(InternalAssetEvent::LoadedWithDependencies { id: waiting_id });
                }
                Some(std::mem::take(
                    &mut info.dependents_waiting_on_recursive_dep_load,
                ))
            } else {
                None
            }
        } else {
            None
        };

        if let Some(dependents) = dependents_waiting_on_rec_load {
            for dep_id in dependents {
                Self::propagate_loaded_state(infos, waiting_id, dep_id, sender);
            }
        }
    }

    /// Walks a failure up the tree from `failed_id` to `waiting_id`.
    fn propagate_failed_state(
        infos: &mut AssetInfos,
        failed_id: UntypedAssetId,
        waiting_id: UntypedAssetId,
        error: &Arc<AssetLoadError>,
    ) {
        let dependents_waiting_on_rec_load = if let Some(info) = infos.get_mut(waiting_id) {
            info.loading_rec_dependencies.remove(&failed_id);
            info.failed_rec_dependencies.insert(failed_id);
            info.rec_dep_load_state = RecursiveDependencyLoadState::Failed(error.clone());
            Some(std::mem::take(
                &mut info.dependents_waiting_on_recursive_dep_load,
            ))
        } else {
            None
        };

        if let Some(dependents) = dependents_waiting_on_rec_load {
            for dep_id in dependents {
                Self::propagate_failed_state(infos, waiting_id, dep_id, error);
            }
        }
    }

    /// Records that an asset failed to load, propagating the failure to its
    /// dependents.
    pub(crate) fn process_asset_fail(
        &mut self,
        failed_id: UntypedAssetId,
        error: Arc<AssetLoadError>,
    ) {
        if !self.infos.contains_key(&failed_id) {
            // The handle was dropped, so there is nobody left to inform.
            return;
        }

        let (dependents_waiting_on_load, dependents_waiting_on_rec_load) = {
            let Some(info) = self.infos.get_mut(&failed_id) else {
                return;
            };
            info.load_state = LoadState::Failed(error.clone());
            info.dep_load_state = DependencyLoadState::Failed(error.clone());
            info.rec_dep_load_state = RecursiveDependencyLoadState::Failed(error.clone());
            (
                std::mem::take(&mut info.dependents_waiting_on_load),
                std::mem::take(&mut info.dependents_waiting_on_recursive_dep_load),
            )
        };

        for waiting_id in dependents_waiting_on_load {
            if let Some(info) = self.get_mut(waiting_id) {
                info.loading_dependencies.remove(&failed_id);
                // Keep the first error a dependent saw.
                if !info.dep_load_state.is_failed() {
                    info.dep_load_state = DependencyLoadState::Failed(error.clone());
                }
            }
        }

        for waiting_id in dependents_waiting_on_rec_load {
            Self::propagate_failed_state(self, failed_id, waiting_id, &error);
        }
    }
}

/// Panics with the same guidance bevy gives when a handle is requested for an
/// asset type that was never registered.
fn missing_provider(type_id: TypeId, type_name: Option<&str>) -> ! {
    match type_name {
        Some(name) => panic!(
            "no handle provider registered for the asset type '{name}'; register the asset type \
             before loading it"
        ),
        None => panic!(
            "no handle provider registered for the asset type '{type_id:?}'; register the asset \
             type before loading it"
        ),
    }
}

/// How a handle should be initialized when it is requested.
#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) enum HandleLoadingMode {
    /// The handle is for an asset that is not to be loaded yet.
    NotLoading,
    /// Start a load unless one is already running or finished. A failed
    /// previous load counts as unfinished, so requesting it retries.
    Request,
    /// Start a load even if one already finished.
    Force,
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
