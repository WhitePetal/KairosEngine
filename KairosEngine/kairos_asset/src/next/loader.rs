//! Loaders, load contexts, and the values they produce.
//!
//! An [`AssetLoader`] turns an asset's bytes into an asset value. While it runs
//! it is handed a [`LoadContext`], which:
//!
//! - hands back [`Handle`]s for the other assets the loader depends on
//!   (declaring a dependency), and
//! - collects the labeled sub-assets the loader produces alongside the root
//!   value.
//!
//! [`LoadContext::finish`] closes the context, folding in any dependency the
//! value embeds but the loader did not explicitly declare. The result is a
//! [`LoadedAsset<A>`] — the root value plus its dependencies and labeled
//! assets — which is then erased into an [`ErasedLoadedAsset`] so loads of
//! different asset types share one pipeline.
//!
//! Unlike `bevy_asset`, neither [`AssetLoader`] nor the values it produces
//! carry `TypePath` reflection; a loader names itself with
//! [`std::any::type_name`]. A loader's error type only has to convert into
//! [`KairosError`].
//!
//! The immediate (direct) loading surface — [`LoadContext::load_builder`] and
//! the `load_value*` family that synchronously returns a loaded value — is
//! deliberately not here; it is a separate deferred item.

use core::{
    any::{Any, TypeId},
    convert::Infallible,
    fmt,
};
use std::{
    collections::{HashMap, HashSet, hash_map::Entry},
    path::Path,
};

use atomicow::CowArc;
use kairos_ecs::error::KairosError;
use kairos_ecs::world::World;
use kairos_tasks::ConditionalSendFuture;
use serde::{Deserialize, Serialize};

use crate::next::asset::Asset;
use crate::next::assets::Assets;
use crate::next::handle::{Handle, UntypedHandle};
use crate::next::id::UntypedAssetId;
use crate::next::index::AssetIndex;
use crate::next::io::{BoxedFuture, Reader};
use crate::next::meta::{
    AssetAction, AssetHash, AssetMeta, AssetMetaDyn, DeserializeMetaError, Settings, loader_name,
};
use crate::next::path::AssetPath;
use crate::next::server::AssetServer;

/// Loads an [`Asset`] from a byte [`Reader`].
///
/// A loader declares which asset type it produces ([`AssetLoader::Asset`]) and
/// how it is configured ([`AssetLoader::Settings`]). Its [`AssetLoader::Error`]
/// only has to convert into [`KairosError`], so loaders are free to use their
/// own error types (or [`Infallible`]) without pulling in an error framework.
pub trait AssetLoader: Send + Sync + 'static {
    /// The top-level [`Asset`] this loader produces.
    type Asset: Asset;

    /// The settings this loader is configured with.
    type Settings: Settings + Default + Serialize + for<'a> Deserialize<'a>;

    /// The error this loader can produce.
    type Error: Into<KairosError>;

    /// Asynchronously loads [`AssetLoader::Asset`] — and any labeled sub-assets
    /// — from the bytes provided by `reader`.
    fn load(
        &self,
        reader: &mut dyn Reader,
        settings: &Self::Settings,
        load_context: &mut LoadContext,
    ) -> impl ConditionalSendFuture<Output = Result<Self::Asset, Self::Error>>;

    /// The file extensions this loader supports, without the leading dot.
    ///
    /// Callers are free to load the loader for a file whose extension is not
    /// listed here.
    fn extensions(&self) -> &[&str] {
        &[]
    }
}

/// Provides type-erased access to an [`AssetLoader`].
pub trait ErasedAssetLoader: Send + Sync + 'static {
    /// Asynchronously loads the asset (and its labeled sub-assets) from the
    /// bytes provided by `reader`.
    fn load<'a>(
        &'a self,
        reader: &'a mut dyn Reader,
        settings: &'a dyn Settings,
        load_context: LoadContext<'a>,
    ) -> BoxedFuture<'a, Result<ErasedLoadedAsset, KairosError>>;

    /// The file extensions this loader supports, without the leading dot.
    fn extensions(&self) -> &[&str];

    /// Deserializes the meta sidecar bytes into the loader's settings type.
    fn deserialize_meta(&self, meta: &[u8]) -> Result<Box<dyn AssetMetaDyn>, DeserializeMetaError>;

    /// The meta to use when an asset has no sidecar: name this loader and use
    /// its default settings.
    fn default_meta(&self) -> Box<dyn AssetMetaDyn>;

    /// The loader's fully-qualified name, matching the name written in a `.meta`
    /// sidecar.
    fn type_name(&self) -> &'static str;

    /// The loader's runtime [`TypeId`].
    fn type_id(&self) -> TypeId;

    /// The type name of the top-level [`Asset`] this loader produces.
    fn asset_type_name(&self) -> &'static str;

    /// The [`TypeId`] of the top-level [`Asset`] this loader produces.
    fn asset_type_id(&self) -> TypeId;
}

impl<L: AssetLoader> ErasedAssetLoader for L {
    fn load<'a>(
        &'a self,
        reader: &'a mut dyn Reader,
        settings: &'a dyn Settings,
        mut load_context: LoadContext<'a>,
    ) -> BoxedFuture<'a, Result<ErasedLoadedAsset, KairosError>> {
        Box::pin(async move {
            let settings = settings
                .downcast_ref::<L::Settings>()
                .expect("AssetLoader settings should match the loader type");
            let asset = <L as AssetLoader>::load(self, reader, settings, &mut load_context)
                .await
                .map_err(Into::into)?;
            Ok(load_context.finish(asset).into())
        })
    }

    fn extensions(&self) -> &[&str] {
        <L as AssetLoader>::extensions(self)
    }

    fn deserialize_meta(&self, meta: &[u8]) -> Result<Box<dyn AssetMetaDyn>, DeserializeMetaError> {
        let meta = AssetMeta::<L::Settings, ()>::deserialize(meta)?;
        Ok(Box::new(meta))
    }

    fn default_meta(&self) -> Box<dyn AssetMetaDyn> {
        Box::new(AssetMeta::<L::Settings, ()>::new(AssetAction::Load {
            loader: loader_name::<L>().to_string(),
            settings: L::Settings::default(),
        }))
    }

    fn type_name(&self) -> &'static str {
        loader_name::<L>()
    }

    fn type_id(&self) -> TypeId {
        TypeId::of::<L>()
    }

    fn asset_type_name(&self) -> &'static str {
        core::any::type_name::<L::Asset>()
    }

    fn asset_type_id(&self) -> TypeId {
        TypeId::of::<L::Asset>()
    }
}

/// A secondary asset a loader produced alongside its root asset.
pub(crate) struct LabeledAsset {
    /// The labeled asset's value and its own dependencies.
    pub(crate) asset: ErasedLoadedAsset,
    /// The handle the labeled asset is addressed by.
    ///
    /// Holding the strong handle keeps the labeled asset's slot alive until the
    /// load is inserted into its store.
    #[allow(dead_code)]
    pub(crate) handle: UntypedHandle,
}

/// The successful result of an [`AssetLoader::load`] call.
///
/// It carries the root value, the dependencies to load before it is ready
/// (both those the loader declared and those found embedded in the value), and
/// any labeled sub-assets.
pub struct LoadedAsset<A: Asset> {
    pub(crate) value: A,
    pub(crate) dependencies: HashSet<UntypedAssetId>,
    pub(crate) loader_dependencies: HashMap<AssetPath<'static>, AssetHash>,
    pub(crate) labeled_assets: Vec<LabeledAsset>,
    pub(crate) label_to_asset_index: HashMap<CowArc<'static, str>, usize>,
    pub(crate) asset_id_to_asset_index: HashMap<UntypedAssetId, usize>,
}

impl<A: Asset> LoadedAsset<A> {
    /// Creates a loaded asset from `value`, discovering its dependencies by
    /// visiting the handles it embeds.
    pub fn new_with_dependencies(value: A) -> Self {
        let mut dependencies = HashSet::default();
        value.visit_dependencies(&mut |id| {
            if matches!(id, UntypedAssetId::Index { .. }) {
                dependencies.insert(id);
            }
        });
        Self {
            value,
            dependencies,
            loader_dependencies: HashMap::default(),
            labeled_assets: Default::default(),
            label_to_asset_index: Default::default(),
            asset_id_to_asset_index: Default::default(),
        }
    }

    /// Takes the root value out of the loaded asset.
    pub fn take(self) -> A {
        self.value
    }

    /// The root value.
    pub fn get(&self) -> &A {
        &self.value
    }

    /// The [`ErasedLoadedAsset`] for `label`, if one was produced.
    pub fn get_labeled(&self, label: impl AsRef<str>) -> Option<&ErasedLoadedAsset> {
        let index = label_index(&self.label_to_asset_index, label.as_ref())?;
        Some(&self.labeled_assets[index].asset)
    }

    /// The labeled asset addressed by `id`, if one exists.
    pub fn get_labeled_by_id(&self, id: impl Into<UntypedAssetId>) -> Option<&ErasedLoadedAsset> {
        let index = *self.asset_id_to_asset_index.get(&id.into())?;
        Some(&self.labeled_assets[index].asset)
    }

    /// Every label of a labeled asset in this load.
    pub fn iter_labels(&self) -> impl Iterator<Item = &str> {
        self.label_to_asset_index.keys().map(|label| label.as_ref())
    }
}

impl<A: Asset> From<A> for LoadedAsset<A> {
    fn from(asset: A) -> Self {
        LoadedAsset::new_with_dependencies(asset)
    }
}

/// A type-erased counterpart to [`LoadedAsset`], used where the loaded type is
/// not statically known.
pub struct ErasedLoadedAsset {
    pub(crate) value: Box<dyn AssetContainer>,
    pub(crate) dependencies: HashSet<UntypedAssetId>,
    pub(crate) loader_dependencies: HashMap<AssetPath<'static>, AssetHash>,
    pub(crate) labeled_assets: Vec<LabeledAsset>,
    pub(crate) label_to_asset_index: HashMap<CowArc<'static, str>, usize>,
    pub(crate) asset_id_to_asset_index: HashMap<UntypedAssetId, usize>,
}

impl<A: Asset> From<LoadedAsset<A>> for ErasedLoadedAsset {
    fn from(asset: LoadedAsset<A>) -> Self {
        Self {
            value: Box::new(asset.value),
            dependencies: asset.dependencies,
            loader_dependencies: asset.loader_dependencies,
            labeled_assets: asset.labeled_assets,
            label_to_asset_index: asset.label_to_asset_index,
            asset_id_to_asset_index: asset.asset_id_to_asset_index,
        }
    }
}

impl ErasedLoadedAsset {
    /// Takes the value out if it is of type `A`, otherwise returns [`None`] and
    /// drops it.
    pub fn take<A: Asset>(self) -> Option<A> {
        self.value.into_any().downcast::<A>().ok().map(|value| *value)
    }

    /// A reference to the value if it is of type `A`.
    pub fn get<A: Asset>(&self) -> Option<&A> {
        self.value.as_any().downcast_ref::<A>()
    }

    /// The [`TypeId`] of the stored value's asset type.
    pub fn asset_type_id(&self) -> TypeId {
        self.value.as_any().type_id()
    }

    /// The type name of the stored value's asset type.
    pub fn asset_type_name(&self) -> &'static str {
        self.value.asset_type_name()
    }

    /// The [`ErasedLoadedAsset`] for `label`, if one was produced.
    pub fn get_labeled(&self, label: impl AsRef<str>) -> Option<&ErasedLoadedAsset> {
        let index = label_index(&self.label_to_asset_index, label.as_ref())?;
        Some(&self.labeled_assets[index].asset)
    }

    /// The labeled asset addressed by `id`, if one exists.
    pub fn get_labeled_by_id(&self, id: impl Into<UntypedAssetId>) -> Option<&ErasedLoadedAsset> {
        let index = *self.asset_id_to_asset_index.get(&id.into())?;
        Some(&self.labeled_assets[index].asset)
    }

    /// Every label of a labeled asset in this load.
    pub fn iter_labels(&self) -> impl Iterator<Item = &str> {
        self.label_to_asset_index.keys().map(|label| label.as_ref())
    }

    /// Casts this loaded asset to `A`, returning `self` unchanged if the stored
    /// type does not match.
    pub fn downcast<A: Asset>(self) -> Result<LoadedAsset<A>, ErasedLoadedAsset> {
        if self.value.as_any().type_id() != TypeId::of::<A>() {
            return Err(self);
        }
        let value = self
            .value
            .into_any()
            .downcast::<A>()
            .expect("the type was just checked");
        Ok(LoadedAsset {
            value: *value,
            dependencies: self.dependencies,
            loader_dependencies: self.loader_dependencies,
            labeled_assets: self.labeled_assets,
            label_to_asset_index: self.label_to_asset_index,
            asset_id_to_asset_index: self.asset_id_to_asset_index,
        })
    }
}

impl fmt::Debug for ErasedLoadedAsset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ErasedLoadedAsset")
            .field("asset_type_name", &self.asset_type_name())
            .field("dependencies", &self.dependencies.len())
            .field("labeled_assets", &self.labeled_assets.len())
            .finish()
    }
}

/// A type-erased container for an [`Asset`] value.
///
/// It is what makes [`ErasedLoadedAsset`] possible: the boxed value can be
/// inserted into its per-type store, downcast back to a concrete
/// [`LoadedAsset`], or queried for its type, all without the loader knowing the
/// concrete type.
pub trait AssetContainer: Any + Send + Sync + 'static {
    /// Inserts the value into its store for `index` in `world`.
    fn insert(self: Box<Self>, index: AssetIndex, world: &mut World);

    /// The type name of the contained asset.
    fn asset_type_name(&self) -> &'static str;

    /// This container as a [`dyn Any`], for downcasting.
    fn as_any(&self) -> &dyn Any;

    /// Consumes this container into a [`Box<dyn Any>`], for downcasting.
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

impl<A: Asset> AssetContainer for A {
    fn insert(self: Box<Self>, index: AssetIndex, world: &mut World) {
        // The caller only inserts values whose slot is still alive.
        world
            .resource_mut::<Assets<A>>()
            .insert(index, *self)
            .expect("the AssetIndex is still valid");
    }

    fn asset_type_name(&self) -> &'static str {
        core::any::type_name::<A>()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

/// The context a [`AssetLoader`] is given while it loads.
///
/// It declares dependencies (by handing out handles), collects labeled
/// sub-assets, and — at [`LoadContext::finish`] — closes the load into a
/// [`LoadedAsset`].
pub struct LoadContext<'a> {
    pub(crate) asset_server: &'a AssetServer,
    /// Whether dependencies should be loaded when the loader declares them.
    pub(crate) should_load_dependencies: bool,
    /// Whether to record content hashes while loading (used by the processing
    /// pipeline, which is not here yet).
    #[allow(dead_code)]
    populate_hashes: bool,
    /// The path of the asset being loaded; labeled sub-assets hang off it.
    asset_path: AssetPath<'static>,
    /// The ids this load depends on.
    dependencies: HashSet<UntypedAssetId>,
    /// Dependencies whose *values* were read while loading (used by the
    /// processing pipeline, which is not here yet).
    #[allow(dead_code)]
    loader_dependencies: HashMap<AssetPath<'static>, AssetHash>,
    /// The labeled sub-assets produced so far.
    labeled_assets: Vec<LabeledAsset>,
    /// Maps a label to its index in [`LoadContext::labeled_assets`].
    label_to_asset_index: HashMap<CowArc<'static, str>, usize>,
    /// Maps a labeled asset's id to its index in
    /// [`LoadContext::labeled_assets`].
    asset_id_to_asset_index: HashMap<UntypedAssetId, usize>,
}

impl<'a> LoadContext<'a> {
    /// Creates a context for loading `asset_path` through `asset_server`.
    pub(crate) fn new(
        asset_server: &'a AssetServer,
        asset_path: AssetPath<'static>,
        should_load_dependencies: bool,
        populate_hashes: bool,
    ) -> Self {
        Self {
            asset_server,
            asset_path,
            should_load_dependencies,
            populate_hashes,
            dependencies: HashSet::default(),
            loader_dependencies: HashMap::default(),
            labeled_assets: Default::default(),
            label_to_asset_index: Default::default(),
            asset_id_to_asset_index: Default::default(),
        }
    }

    /// Begins a labeled sub-asset load.
    ///
    /// The returned context is finished and then added back to this one with
    /// [`LoadContext::add_loaded_labeled_asset`]. Prefer
    /// [`LoadContext::labeled_asset_scope`] unless the sub-asset is loaded in
    /// parallel.
    pub fn begin_labeled_asset(&self) -> LoadContext<'_> {
        LoadContext::new(
            self.asset_server,
            self.asset_path.clone(),
            self.should_load_dependencies,
            self.populate_hashes,
        )
    }

    /// Loads the labeled sub-asset `label` with `load`, registering the result
    /// on this context and returning its handle.
    pub fn labeled_asset_scope<A: Asset, E>(
        &mut self,
        label: impl Into<CowArc<'static, str>>,
        load: impl FnOnce(&mut LoadContext) -> Result<A, E>,
    ) -> Result<Handle<A>, E> {
        let mut context = self.begin_labeled_asset();
        let asset = load(&mut context)?;
        let loaded_asset = context.finish(asset);
        Ok(self.add_loaded_labeled_asset(label, loaded_asset))
    }

    /// Registers `asset` as the labeled sub-asset `label`, returning its
    /// handle.
    ///
    /// The asset's embedded dependencies are discovered; dependencies declared
    /// through a [`LoadContext`] are not, so use
    /// [`LoadContext::labeled_asset_scope`] when the loaded value depends on
    /// other assets.
    pub fn add_labeled_asset<A: Asset>(
        &mut self,
        label: impl Into<CowArc<'static, str>>,
        asset: A,
    ) -> Handle<A> {
        let Ok(handle) = self.labeled_asset_scope(label, |_| Ok::<_, Infallible>(asset));
        handle
    }

    /// Registers an already-[finished](LoadContext::finish) [`LoadedAsset`] as
    /// the labeled sub-asset `label`, returning its handle.
    pub fn add_loaded_labeled_asset<A: Asset>(
        &mut self,
        label: impl Into<CowArc<'static, str>>,
        loaded_asset: LoadedAsset<A>,
    ) -> Handle<A> {
        let label = label.into();
        let loaded_asset: ErasedLoadedAsset = loaded_asset.into();
        let labeled_path = self.asset_path.clone().with_label(label.clone());
        let handle = self.asset_server.get_or_create_path_handle::<A>(labeled_path);
        let asset = LabeledAsset {
            asset: loaded_asset,
            handle: handle.clone().untyped(),
        };
        match self.label_to_asset_index.entry(label) {
            Entry::Occupied(entry) => {
                let index = *entry.get();
                self.labeled_assets[index] = asset;
            }
            Entry::Vacant(entry) => {
                entry.insert(self.labeled_assets.len());
                self.asset_id_to_asset_index
                    .insert(handle.id().untyped(), self.labeled_assets.len());
                self.labeled_assets.push(asset);
            }
        }
        handle
    }

    /// Whether a handle has been registered for the labeled sub-asset `label`.
    ///
    /// This is true once the label has been reserved through
    /// [`LoadContext::get_label_handle`] or produced through
    /// [`LoadContext::add_labeled_asset`], even if the sub-asset's value has not
    /// been finished yet.
    pub fn has_labeled_asset(&self, label: impl Into<CowArc<'static, str>>) -> bool {
        let path = self.asset_path.clone().with_label(label.into());
        !self.asset_server.get_handles_untyped(&path).is_empty()
    }

    /// "Finishes" this context into a [`LoadedAsset`].
    ///
    /// Dependencies the value embeds but the loader did not declare are added
    /// here, by visiting the value's handles.
    pub fn finish<A: Asset>(mut self, value: A) -> LoadedAsset<A> {
        value.visit_dependencies(&mut |id| {
            if matches!(id, UntypedAssetId::Index { .. }) {
                self.dependencies.insert(id);
            }
        });
        LoadedAsset {
            value,
            dependencies: self.dependencies,
            loader_dependencies: self.loader_dependencies,
            labeled_assets: self.labeled_assets,
            label_to_asset_index: self.label_to_asset_index,
            asset_id_to_asset_index: self.asset_id_to_asset_index,
        }
    }

    /// The path of the asset being loaded.
    pub fn path(&self) -> &AssetPath<'static> {
        &self.asset_path
    }

    /// Returns the handle for the labeled sub-asset `label` and records it as a
    /// dependency of this asset.
    ///
    /// The label can be requested before or after the sub-asset is added; the
    /// same label always resolves to the same handle.
    pub fn get_label_handle<A: Asset>(
        &mut self,
        label: impl Into<CowArc<'static, str>>,
    ) -> Handle<A> {
        let path = self.asset_path.clone().with_label(label.into());
        let handle = self.asset_server.get_or_create_path_handle::<A>(path);
        self.dependencies.insert(handle.id().untyped());
        handle
    }

    /// The labeled asset `label`, if it has already been registered.
    pub fn get_labeled(&self, label: impl AsRef<str>) -> Option<&ErasedLoadedAsset> {
        let index = label_index(&self.label_to_asset_index, label.as_ref())?;
        Some(&self.labeled_assets[index].asset)
    }

    /// The labeled asset addressed by `id`, if it exists.
    pub fn get_labeled_by_id(&self, id: impl Into<UntypedAssetId>) -> Option<&ErasedLoadedAsset> {
        let index = *self.asset_id_to_asset_index.get(&id.into())?;
        Some(&self.labeled_assets[index].asset)
    }

    /// Returns the handle for the asset at `path` and records it as a
    /// dependency of this asset.
    ///
    /// When this context loads dependencies, the asset at `path` is queued for
    /// loading; otherwise only the dependency is recorded and the handle is
    /// reserved.
    pub fn load<'b, A: Asset>(&mut self, path: impl Into<AssetPath<'b>>) -> Handle<A> {
        let path = path.into().into_owned();
        if path.path() == Path::new("") {
            // Reported as a load failure once the pipeline lands.
            return Handle::default();
        }
        let handle = if self.should_load_dependencies {
            self.asset_server.load::<A>(path)
        } else {
            self.asset_server.get_or_create_path_handle::<A>(path)
        };
        self.dependencies.insert(handle.id().untyped());
        handle
    }
}

/// Looks up a label's index, comparing by `str` so the map's `CowArc` key does
/// not force an allocation at the call site.
fn label_index(map: &HashMap<CowArc<'static, str>, usize>, label: &str) -> Option<usize> {
    map.get(label).copied()
}
